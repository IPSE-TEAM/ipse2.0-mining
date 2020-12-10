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
use sp_core::{sr25519::Pair};
use sp_core::Pair as PairT;
// use hex_literal::hex;

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
    //DefaultNodeRuntime as Runtime,
    ClientBuilder,
};
use sp_core::{storage::StorageKey, twox_128};
use sp_keyring::AccountKeyring;
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
pub const MiningDuration: u64 = 1;
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

            info!("请求链上信息！");
            let block_hash = self.inner.block_hash(None).await.unwrap().unwrap();

            let block_hash = block_hash.as_fixed_bytes();

            let height = self.get_current_height().await;

            let base_target = if let Some(di) = self.get_last_difficulty().await {
                di.base_target
            } else {
                std::u64::MAX
            };

           let deadline = if let Some(dl) = self.get_last_mining_info().await {
               dl.best_dl
           } else {
               std::u64::MAX
           };

            let duration_from_last_mining = 12000;

            info!("获取数据成功!, base_target = {:?}, height = {:?}", base_target, height);

            future::ok(MiningInfoResponse{
                base_target,
                height,
                generation_signature: *block_hash,
                // 这个target_deadline几乎没有任何意义
               target_deadline: deadline,
//                 target_deadline: u64::max_value(),
                // duration_from_last_mining也没有任何意义
                duration_from_last_mining,
            })
        })


    }

    /// Submit nonce to Substrate.
    pub fn submit_nonce(
        &self,
        submission_data: &SubmissionParameters,
    ) -> impl Future<Item = SubmitNonceResponse, Error = FetchError> {

        info!(" --------------扫盘完成， 正在检查提交。-------------------");

        let check_dl_result =
        async_std::task::block_on(async move {

            // 当前真正的高度
            let current_block = self.get_current_height().await;

            info!("请求数据的区块是：{:?}, 现在的区块是: {:?}, 提交的deadline是: {:?}", submission_data.height, current_block, submission_data.deadline);

            // 必须在同一周期 并且提交的时间比处理的时间迟
            if !(current_block/MiningDuration == submission_data.height/MiningDuration && current_block >= submission_data.height)
            {
                info!("禁止提交! 请求数据的区块离当前区块间隔较大（已经过期)");
                return Err(())
            }

            if let Some(info) = self.get_last_mining_info().await {

                let last_mining_block = info.block;
                if info.best_dl <= submission_data.deadline && current_block / MiningDuration == info.block / MiningDuration {
                    info!("禁止提交! 本挖矿周期已经有比较好的deadline = {} ", info.best_dl);
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

//        if submission_data.deadline > MAX_MINING_TIME {
//            info!("deadline too large: deadline = {:?}, MAX = {:?}", submission_data.deadline, MAX_MINING_TIME);
//            return future::ok(SubmitNonceResponse{verify_result: false})
//        }

        let xt_result =
        async_std::task::block_on(async move {

            let phrase = self.str_convert_to_phrase(self.account_id_to_secret_phrase.get(&submission_data.account_id).expect("获取助记词错误").as_str().to_string());

            let pair = Pair::from_phrase(&phrase, None).expect("签名错误");

            let mut signer = PairSigner::new(pair.0.clone());

            // 提交的高度 + deadline 作为nonce值， 避免重复
            signer.set_nonce((submission_data.height + submission_data.deadline) as u32);

            info!("助记词签名成功， public_key = {:?}, account_id = {:?}, 正在提交挖矿请求.........", pair.0.public(), signer.clone().account_id());

            let xt_result = self.inner.
                mining(
                    &signer,
                    submission_data.account_id,
                    submission_data.height,
                    submission_data.gen_sig,
                    submission_data.nonce,
                    submission_data.deadline

                ).await;

            Ok(xt_result)


        });

        match xt_result {
            Ok(success) => {
                if success.is_ok() {
                    info!("***************************************** 挖矿请求提交成功, 区块是: {:?}, hash是： {:?} **************************************", submission_data.height, success.unwrap());
                }
                else {
                    info!("挖矿请求提交错误! 错误信息是: {:?}", success);
                }

                return future::ok(SubmitNonceResponse{verify_result: true});

            },

            Err(err) => {
                info!("挖矿请求提交错误! 错误信息是: {:?}", err);
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

    /// Get last mining timestamp from Substrate.
    async fn get_last_mining_ts(&self) -> u64 {

        let ts_opt: Option<u64> = self.inner.last_mining_ts(None).await.unwrap();
        ts_opt.unwrap()
    }

    /// GET now timestamp from Substrate.
    async fn get_now_ts(&self) -> u64 {

        let ts_opt: Option<u64> = self.inner.now(None).await.unwrap();
        ts_opt.unwrap()
    }

    /// Get current block height from Substrate.
    async fn get_current_height(&self) -> u64 {
        let header = self.inner.header::<<Runtime as System>::Hash>(None).await.unwrap().unwrap();
        let block_num = *header.number() as u64 + 1u64;
        info!("当前区块的高度是: {:?}", block_num + 1);
        block_num
    }


}


