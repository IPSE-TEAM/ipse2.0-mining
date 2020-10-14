use crate::com::api::{FetchError, MiningInfoResponse};
use crate::com::client::{Client, ProxyDetails, SubmissionParameters};
use crate::future::prio_retry::PrioRetry;
use futures::future::Future;
use futures::stream::Stream;
use futures::sync::mpsc;
use std::collections::HashMap;
use std::time::Duration;
use std::u64;
use tokio;
use tokio::runtime::TaskExecutor;
use url::Url;

#[derive(Clone)]
pub struct RequestHandler {
    client: Client,
    tx_submit_data: mpsc::UnboundedSender<SubmissionParameters>,
}

impl RequestHandler {
    pub fn new(
        base_uri: Url,
        secret_phrases: HashMap<u64, String>,
        timeout: u64,
        total_size_gb: usize,
        send_proxy_details: bool,
        additional_headers: HashMap<String, String>,
        executor: TaskExecutor,
    ) -> RequestHandler {

        let client = Client::new(
            base_uri,
            secret_phrases,
            total_size_gb,
        );

        let (tx_submit_data, rx_submit_nonce_data) = mpsc::unbounded();
        RequestHandler::handle_submissions(
            client.clone(),
            rx_submit_nonce_data,
            tx_submit_data.clone(),
            executor,
        );

        RequestHandler {
            client,
            tx_submit_data,
        }
    }

    fn handle_submissions(
        client: Client,
        rx: mpsc::UnboundedReceiver<SubmissionParameters>,
        tx_submit_data: mpsc::UnboundedSender<SubmissionParameters>,
        executor: TaskExecutor,
    ) {
        let stream = PrioRetry::new(rx, Duration::from_secs(3))
            .and_then(move |submission_params| {
                client
                    .clone()
                    .submit_nonce(&submission_params)
                    .then(move |res| {
                        match res {
                            Ok(res) => {
                                if res.verify_result{
                                    println!("verify succeed!!!");
                                } else {
                                    warn!("verify failed: accountId = {}, height = {}, nonce = {}, deadline = {}",
                                          &submission_params.account_id,
                                          &submission_params.height,
                                          &submission_params.nonce,
                                          &submission_params.deadline,
                                    )
                                }
                            }
                            Err(err) => {
                                error!("submit nonce error: {:?}", err)
                            }
                        };
                        Ok(())
                    })
            })
            .for_each(|_| Ok(()))
            .map_err(|e| error!("can't handle submission params: {:?}", e));
        executor.spawn(stream);
    }

    pub fn get_mining_info(&self) -> impl Future<Item = MiningInfoResponse, Error = FetchError> {
        self.client.get_mining_info()
    }

    pub fn submit_nonce(
        &self,
        account_id: u64,
        nonce: u64,
        height: u64,
        block: u64,
        deadline_unadjusted: u64,
        deadline: u64,
        gen_sig: [u8; 32],
    ) {
        let res = self.tx_submit_data.unbounded_send(SubmissionParameters {
            account_id,
            nonce,
            height,
            block,
            deadline_unadjusted,
            deadline,
            gen_sig,
        });
        if let Err(e) = res {
            error!("can't send submission params: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    static BASE_URL: &str = "http://94.130.178.37:31000";

    #[test]
    fn test_submit_nonce() {
        let rt = tokio::runtime::Runtime::new().expect("can't create runtime");

        let request_handler = RequestHandler::new(
            BASE_URL.parse().unwrap(),
            HashMap::new(),
            3,
            12,
            true,
            HashMap::new(),
            rt.executor(),
        );

        request_handler.submit_nonce(1337, 12, 111, 0, 7123, 1193, [0; 32]);

        rt.shutdown_on_idle();
    }
}
