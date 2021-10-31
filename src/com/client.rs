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
use sp_core::{sr25519::{Pair, Public}};
use sp_core::Pair as PairT;
use pallet_indices::address::Address;

use substrate_subxt::system::AccountStoreExt;
use crate::com::poc_staking::DiskOfStoreExt;
use crate::com::poc_staking::RegisterCallExt;
use sp_core::crypto::{AccountId32, Ss58Codec};

use codec::{
    Decode,
    Encode,
};

use std::{
    collections::BTreeMap,
    fmt::Debug,
    marker::PhantomData,
};

pub use substrate_subxt::{
    system::System,
    ExtrinsicSuccess,
    PairSigner,
    Call,
    Error as SubError,
    Client as SubClient,
    ClientBuilder,
};
use sp_core::{storage::StorageKey, twox_128};
use sp_keyring::AccountKeyring;
use substrate_subxt::balances::TransferCallExt;
use sp_runtime::traits::{Header};
use sub_runtime::poc::{Difficulty, MiningInfo};
use crate::com::runtimes::{PocRuntime};
use  crate::com::timestamp::Timestamp;
use crate::com::timestamp::NowStoreExt;
use crate::com::runtimes::LastMiningTsStoreExt;
use crate::com::runtimes::TargetInfoStoreExt;
use crate::com::runtimes::DlInfoStoreExt;
use substrate_subxt::system::BlockNumberStoreExt;
use crate::com::runtimes::MiningCallExt;
use crate::com::runtimes::MiningEventExt;
use substrate_subxt::Signer;

type Runtime = PocRuntime;
type AccountId = <Runtime as System>::AccountId;

pub const MAX_MINING_TIME: u64 = 12000;

pub const POC_MODULE: &str = "PoC";
pub const TS_MODULE: &str = "Timestamp";
pub const MiningExpire: u64 = 2;
pub const MINING: &str = "mining";



/// A client for communicating with Pool/Proxy/Wallet.
#[derive(Clone)]
pub struct Client {
    pub inner: SubClient<Runtime>,

    pair: Pair,

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
        // mut secret_phrases: HashMap<u64, String>,
        total_size_gb: usize,
        mut pair: Pair,
    ) -> Self {

        let url = base_uri.as_str();
        let client = async_std::task::block_on(async move {
            ClientBuilder::<Runtime>::new()
                .set_url(url)
                .build().await.unwrap()
        });

        Self {
            inner: client,
            base_uri,
            total_size_gb,
            pair,
        }
    }



    /// Get current mining info.
    pub fn get_mining_info(&self) -> impl Future<Item = MiningInfoResponse, Error = FetchError> {
        async_std::task::block_on(async move {

            let block_hash = self.inner.block_hash(None).await.unwrap().unwrap();

            let block_hash = block_hash.as_fixed_bytes();

            let height = self.get_current_height().await;

            let base_target = if let Some(di) = self.get_last_difficulty().await {
                di.base_target
            } else {
                std::u64::MAX
            };


            let duration_from_last_mining = 12000;

            future::ok(MiningInfoResponse{
                base_target,
                height,
                generation_signature: *block_hash,
                target_deadline: u64::max_value(), // 无意义
                duration_from_last_mining, // 无意义
            })
        })


    }



    /// Submit nonce to Substrate.
    pub fn submit_nonce(
        &self,
        submission_data: &SubmissionParameters,
    ) -> impl Future<Item = SubmitNonceResponse, Error = FetchError> {

        let check_dl_result =
        async_std::task::block_on(async move {

            if self.is_stop(self.pair.clone()).await.is_err() {
                info!("%%%%%%%%%%%%%%%%%%%%%%%%%%%% the miner is not register, or has stoped mining on the chain  %%%%%%%%%%%%%%%%%%%%%%%%%%%%");
                return Err(());
            };

            let current_block = self.get_current_height().await;

            info!("block of get info is ：{:?}, now block is: {:?}, submit deadline is: {:?}", submission_data.height, current_block, submission_data.deadline);

            // 必须在同一周期 并且提交的时间比处理的时间迟
            if current_block / MiningExpire != submission_data.height / MiningExpire
            {
                info!("expire! can not submit.");
                return Err(())
            }

            if let Some(info) = self.get_last_mining_info().await {

                let last_mining_block = info.block;

                if info.best_dl <= submission_data.deadline && current_block / MiningExpire  == info.block / MiningExpire {

                    info!("not best deadline = {}, can not submit.", info.best_dl);
                    Err(())
                }

                else
                {
                    Ok(())
                }

            } else {

                Ok(())
            }
        });

        if check_dl_result.is_err() {
            return future::ok(SubmitNonceResponse{verify_result: false})
        }

        let xt_result =
        async_std::task::block_on(async move {

            let mut signer = PairSigner::new(self.pair.clone());

            info!("Signature success， public_key = {:?}, account_id = {:?}, submit.........", self.pair.public().clone(), signer.clone().account_id());

            let xt_result = self.inner.
                mining(
                    &signer,
                    submission_data.account_id,
                    submission_data.height,
                    submission_data.gen_sig,
                    submission_data.nonce,
                    submission_data.deadline

                ).await;

             // 如果返回错误 那么试着去另外提交一次（nonce值加1）
            if xt_result.is_err() {
                info!("submit err, modify the nonce, and send again.");

                let nonce = self.inner.account(signer.clone().account_id(),None).await.unwrap().nonce;

                signer.set_nonce(nonce + 1);

                let last_result = self.inner.
                    mining(
                    &signer,
                    submission_data.account_id,
                    submission_data.height,
                    submission_data.gen_sig,
                    submission_data.nonce,
                    submission_data.deadline

                ).await;

                if last_result.is_err() {

                    info!("failed again.");
                }
                else {

                    info!("Success the second time! tx hash is {:?}", last_result.ok());
                }

             }

            Ok(xt_result)


        });

        match xt_result {
            Ok(success) => {
                if success.is_ok() {
                    info!("***************************************** submit success, block is : {:?}, deadline is : {:?}, tx hash is： {:?} **************************************", submission_data.height, submission_data.deadline, success.unwrap());
                }
                else {
                    info!("submit failed, and info is : {:?}", success);
                }

                return future::ok(SubmitNonceResponse{verify_result: true});

            },

            Err(err) => {
                info!("submit failed, and info is: {:?}", err);
                return future::err(err);
            },
        }

    }



    /// Get the last mining info from Substrate.
    async fn get_last_mining_info(&self) -> Option<MiningInfo<AccountId>> {

        let dl_opt: Option<Vec<MiningInfo<AccountId>>> = self.inner.dl_info(None).await.unwrap();
        if let Some(dls) = dl_opt {
            if let Some(dl) = dls.last(){
                Some(dl.clone())
            } else { None }
        } else { None }
    }



    fn str_convert_to_phrase(&self, st: String) -> String{
        let mut string = st.to_string();
        let mut new_string = String::new();

        loop {
            let offset = string.find("+").unwrap_or(string.len());
            let pre_string: String= string.drain(..offset).collect();
            new_string.push_str(pre_string.as_str());
            new_string.push_str(" ");
            if string.is_empty() {
                break;
            }
            else {
                string.remove(0);
            }

        }

        new_string
    }



    /// Get the last difficulty from Substrate.
    async fn get_last_difficulty(&self) -> Option<Difficulty> {

        let targets_opt: Option<Vec<Difficulty>> = self.inner.target_info(None).await.unwrap();
        if let Some(targets) = targets_opt {
            let target = targets.last().unwrap();
            Some(target.clone())
        } else {
            None
        }
    }



    /// Get current block height from Substrate.
    async fn get_current_height(&self) -> u64 {

        let block_num = self.inner.block_number(None).await.unwrap() + 1;

        block_num.into()


    }

    async fn is_stop(&self, pair: Pair) -> std::result::Result<(bool), &'static str> {

        let public = pair.clone().public();
        let disk_info_opt = self.inner.disk_of(public.into(), None).await.unwrap();
        if let Some(disk_info) = disk_info_opt {
            if disk_info.is_stop == true {
                return Err("the miner has stoped mining, please resart on the chain.");
            }
        }

        else {
            return Err("the miner is not register.");
        }

        return Ok(false);
    }



    pub fn register(&self, pair: Pair,  payee_pair: Pair, plot_size: u64, numeric_id:u128, miner_proportion: u32, dest: AccountId32) -> std::result::Result<(), &'static str>{

        let alice_signer: PairSigner<PocRuntime, Pair> = PairSigner::new(AccountKeyring::Alice.pair());

        let result = async_std::task::block_on(async move {
            let public = pair.clone().public();

            let payee_public = payee_pair.clone().public();

            let disk_info = self.inner.disk_of(public.into(), None).await.unwrap();

            match disk_info {
                Some(x) => {

//                 }

                info!("you are rergister, and now start mining."); },

                None => {

                    info!("you are not register, and help you register now. wait......");

                    let signer: PairSigner<PocRuntime, Pair> = PairSigner::new(self.pair.clone());

                    info!("registere account is ：{:?}, plot id is : {:?}, plot size is: {:?} GiB, proportion of the miner is: {:?} %, rewad dest is: {:?}", signer.clone().account_id(), numeric_id, plot_size, miner_proportion, dest);

                    let result = self.inner.register(&signer, plot_size , numeric_id, miner_proportion, Some(dest)).await;

                    info!("register info:{:?}", result);
                },
            };

            return Ok(());
        });

        result

    }



}


