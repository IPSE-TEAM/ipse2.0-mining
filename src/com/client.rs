use crate::com::api::*;
use futures::Future;
use futures::future;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use url::form_urlencoded::byte_serialize;
use url::Url;
use log::info;
use std::convert::TryInto;

pub use substrate_subxt::{
    system::System,
    ExtrinsicSuccess,
    Call,
    Error as SubError,
    Client as SubClient,
    //DefaultNodeRuntime as Runtime,
    ClientBuilder,
};
use sp_core::{storage::StorageKey, twox_128};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::{Header};
use sub_runtime::poc::{Difficulty, MiningInfo};
use crate::com::runtimes::{PocRuntime, Timestamp};

type Runtime = PocRuntime;
type AccountId = <Runtime as System>::AccountId;
type Moment = <Runtime as Timestamp>::Moment;

pub const MAX_MINING_TIME: u64 = 9000;

pub const POC_MODULE: &str = "PoC";
pub const TS_MODULE: &str = "Timestamp";

pub const MINING: &str = "mining";

/// A client for communicating with Pool/Proxy/Wallet.
#[derive(Clone)]
pub struct Client {
    inner: SubClient<Runtime>,
    account_id_to_secret_phrase: Arc<HashMap<u64, String>>,
    base_uri: Url,
    total_size_gb: usize,
}

/// Parameters ussed for nonce submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionParameters {
    pub account_id: u64,
    pub nonce: u64,
    pub height: u64,
    pub block: u64,
    pub deadline_unadjusted: u64,
    pub deadline: u64,
    pub gen_sig: [u8; 32],
}

/// Usefull for deciding which submission parameters are the newest and best.
/// We always cache the currently best submission parameters and on fail
/// resend them with an exponential backoff. In the meantime if we get better
/// parameters the old ones need to be replaced.
impl Ord for SubmissionParameters {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.block < other.block {
            Ordering::Less
        } else if self.block > other.block {
            Ordering::Greater
        } else if self.gen_sig == other.gen_sig {
            // on the same chain, best deadline wins
            if self.deadline <= other.deadline {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else {
            // switched to a new chain
            Ordering::Less
        }
    }
}

impl PartialOrd for SubmissionParameters {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Whether to send additional data for Proxies.
#[derive(Clone, PartialEq, Debug)]
pub enum ProxyDetails {
    /// Send additional data like capacity, miner name, ...
    Enabled,
    /// Don't send any additional data:
    Disabled,
}

impl Client {

    /// Create a new client communicating with Pool/Proxy/Wallet.
    pub fn new(
        base_uri: Url,
        mut secret_phrases: HashMap<u64, String>,
        total_size_gb: usize,
    ) -> Self {
        for secret_phrase in secret_phrases.values_mut() {
            *secret_phrase = byte_serialize(secret_phrase.as_bytes()).collect();
        }

        let url = base_uri.as_str();
        let client = async_std::task::block_on(async move {
            ClientBuilder::<Runtime>::new()
                .set_url(url)
                .build().await.unwrap()
        });

        Self {
            inner: client,
            account_id_to_secret_phrase: Arc::new(secret_phrases),
            base_uri,
            total_size_gb,
        }
    }

    /// Get current mining info.
    pub fn get_mining_info(&self) -> impl Future<Item = MiningInfoResponse, Error = FetchError> {
        async_std::task::block_on(async move {
            // use block_hash as gen_sig
            let block_hash = self.inner.block_hash(None).await.unwrap().unwrap();
            let block_hash = block_hash.as_fixed_bytes();
            let height = self.get_current_height().await;

            let base_target = if let Some(di) = self.get_last_difficulty().await {
                info!("THERE WAS a !!!!base_target = {}", di.base_target);
                di.base_target
            } else {
                info!("!!!!!!!use default base-target!!!!");
                488671834567_u64
            };

            let deadline = if let Some(dl) = self.get_last_mining_info().await {
                info!("THERE WAS a !!!!best_dl = {}", dl.best_dl);
                dl.best_dl
            } else {
                info!("!!!!!!use default deadline!!!!");
                std::u64::MAX
            };

            let now_ts = self.get_now_ts().await;
            let last_mining_ts = self.get_last_mining_ts().await;

            let duration_from_last_mining = now_ts - last_mining_ts;
            info!("NOW-ts = {}, last_mining_ts = {}, duration_from_last_mining = {}", now_ts, last_mining_ts, duration_from_last_mining);

            info!("GET CURRENT Mining Info: base_target = {}, height = {}, sig = {:?}, target_deadline = {}",
                  base_target, height, *block_hash, deadline);
            future::ok(MiningInfoResponse{
                base_target,
                height,
                generation_signature: *block_hash,
                target_deadline: deadline,
                duration_from_last_mining,
            })
        })

    }

    /// Submit nonce to Substrate.
    pub fn submit_nonce(
        &self,
        submission_data: &SubmissionParameters,
    ) -> impl Future<Item = SubmitNonceResponse, Error = FetchError> {
        println!(" --------------start submit nonce to Substrate-------------------");
        let check_dl_result =
        async_std::task::block_on(async move {
            info!("check current best deadline!!!");
            let height = self.get_current_height().await;
            if height/3 - submission_data.height/3 > 1 {
                info!("verification of this round is expired, Now on-chain height = {}", height);
                return Err(())
            }

            if let Some(info) = self.get_last_mining_info().await {
                info!("on-chain best deadline = {} ,  deadline to submit = {}", info.best_dl, submission_data.deadline);
                if info.best_dl <= submission_data.deadline
                    && (info.block - 1)/3 == (submission_data.height - 1)/3 {
                    info!(" There was already a better deadline on chain, the best deadline on-chain is {} ", info.best_dl);
                    Err(())
                } else {
                    info!("find a better deadline = {}", submission_data.deadline );
                    Ok(())
                }
            } else {
                info!("find no last-mining-info");
                Ok(())
            }
        });

        if check_dl_result.is_err() {
            return future::ok(SubmitNonceResponse{verify_result: false})
        }

        // if submission_data.deadline > MAX_MINING_TIME {
        //     return future::ok(SubmitNonceResponse{verify_result: false})
        // }

        let xt_result =
        async_std::task::block_on(async move {
            info!("starting submit_nonce to substrate!!!");
            let signer = AccountKeyring::Alice.pair();
            let xt = self.inner.xt(signer, None).await?;
            let xt_result = xt
                .watch()
                .submit(Self::mining(
                    submission_data.account_id,
                    submission_data.height,
                    submission_data.gen_sig,
                    submission_data.nonce,
                    submission_data.deadline
                )).await?;
            Ok(xt_result)
        });

        match xt_result {
            Ok(success) => {
                match success
                    .find_event::<(AccountId, bool)>(
                        POC_MODULE, "VerifyDeadline",
                    ) {
                    Some(Ok((_id, verify_result))) => {
                        info!("verify result: {}", verify_result);
                        return future::ok(SubmitNonceResponse{verify_result})
                    }
                    Some(Err(err)) => return future::err(err.into()),
                    None => return future::err(FetchError::Substrate(SubError::Other("Failed to find PoC::VerifyDeadline".to_string()))),
                }
            }
            Err(err) => future::err(err),
        }

    }

    /// Get the last mining info from Substrate.
    async fn get_last_mining_info(&self) -> Option<MiningInfo<AccountId>> {
        let mut storage_key = twox_128(POC_MODULE.as_ref()).to_vec();
        storage_key.extend(twox_128(b"DlInfo").to_vec());
        let dl_key = StorageKey(storage_key);
        let dl_opt: Option<Vec<MiningInfo<AccountId>>> = self.inner.fetch(dl_key, None).await.unwrap();
        if let Some(dls) = dl_opt {
            if let Some(dl) = dls.last(){
                Some(dl.clone())
            } else { None }
        } else { None }
    }

    /// Get the last difficulty from Substrate.
    async fn get_last_difficulty(&self) -> Option<Difficulty> {
        let mut storage_key = twox_128(POC_MODULE.as_ref()).to_vec();
        storage_key.extend(twox_128(b"TargetInfo").to_vec());
        let targets_key = StorageKey(storage_key);
        let targets_opt: Option<Vec<Difficulty>> = self.inner.fetch(targets_key, None).await.unwrap();
        if let Some(targets) = targets_opt {
            let target = targets.last().unwrap();
            Some(target.clone())
        } else {
            None
        }
    }

    /// Get last mining timestamp from Substrate.
    async fn get_last_mining_ts(&self) -> u64 {
        let mut storage_key = twox_128(POC_MODULE.as_ref()).to_vec();
        storage_key.extend(twox_128(b"LastMiningTs").to_vec());
        let ts_key = StorageKey(storage_key);
        let ts_opt: Option<u64> = self.inner.fetch(ts_key, None).await.unwrap();
        ts_opt.unwrap()
    }

    /// GET now timestamp from Substrate.
    async fn get_now_ts(&self) -> u64 {
        let mut storage_key = twox_128(TS_MODULE.as_ref()).to_vec();
        storage_key.extend(twox_128(b"Now").to_vec());
        let ts_key = StorageKey(storage_key);

        let ts_opt: Option<u64> = self.inner.fetch(ts_key, None).await.unwrap();
        ts_opt.unwrap()
    }

    fn mining(account_id: u64, height: u64, sig: [u8; 32], nonce: u64, deadline: u64) -> Call<MiningArgs>{
        Call::new(POC_MODULE, MINING, MiningArgs{
            account_id,
            height,
            sig,
            nonce,
            deadline,
        })
    }

    /// Get current block height from Substrate.
    async fn get_current_height(&self) -> u64 {
        let header = self.inner.header::<<Runtime as System>::Hash>(None).await.unwrap().unwrap();
        let block_num = *header.number();
        block_num as u64
    }
}
