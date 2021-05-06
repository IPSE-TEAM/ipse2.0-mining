use bytes::Buf;
use reqwest::r#async::Chunk;
use serde::de::{self, DeserializeOwned};
use std::fmt;
use codec::{
    Encode,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitNonceRequest<'a> {
    pub request_type: &'a str,
    pub account_id: u64,
    pub nonce: u64,
    pub secret_phrase: Option<&'a String>,
    pub blockheight: u64,
    pub deadline: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMiningInfoRequest<'a> {
    pub request_type: &'a str,
}

#[derive(Encode, Deserialize)]
pub struct SubmitNonceResponse {
    pub verify_result: bool,
}

#[derive(Encode)]
pub struct MiningArgs {
    pub account_id: u64,
    pub height: u64,
    pub sig: [u8; 32],
    pub nonce: u64,
    pub deadline: u64,
}

#[derive(Debug)]
pub struct MiningInfoResponse {
    pub generation_signature: [u8; 32],

    pub base_target: u64,

    pub height: u64,

    pub target_deadline: u64,

    pub duration_from_last_mining: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoolErrorWrapper {
    error: PoolError,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug)]
pub enum FetchError {
    Http(reqwest::Error),
    Pool(PoolError),
    Substrate(substrate_subxt::Error),
    Pcodec(codec::Error),
}

impl From<substrate_subxt::Error> for FetchError {
    fn from(err: substrate_subxt::Error) -> FetchError {
        FetchError::Substrate(err)
    }
}

impl From<codec::Error> for FetchError {
    fn from(err: codec::Error) -> FetchError {
        FetchError::Pcodec(err)
    }
}

impl From<reqwest::Error> for FetchError {
    fn from(err: reqwest::Error) -> FetchError {
        FetchError::Http(err)
    }
}

impl From<PoolError> for FetchError {
    fn from(err: PoolError) -> FetchError {
        FetchError::Pool(err)
    }
}
