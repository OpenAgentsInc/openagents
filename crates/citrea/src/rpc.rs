use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{
    CitreaError, BlockTag, RpcCallRequest,
    types::{Address, Bytes32},
    util::{format_hex_prefixed, parse_hex_u64},
};

static RPC_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a, T> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: T,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcResponse<T> {
    jsonrpc: Option<String>,
    id: Option<u64>,
    result: Option<T>,
    error: Option<RpcErrorObject>,
}

#[derive(Debug, Deserialize)]
struct RpcErrorObject {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct RpcClient {
    url: String,
    client: reqwest::Client,
}

impl RpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn request<P, R>(&self, method: &str, params: P) -> Result<R, CitreaError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = RPC_ID.fetch_add(1, Ordering::Relaxed);
        let payload = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(CitreaError::Http(format!(
                "HTTP {} from {}",
                status,
                self.url
            )));
        }

        let body: JsonRpcResponse<R> = response.json().await?;
        if let Some(error) = body.error {
            return Err(CitreaError::Rpc {
                code: error.code,
                message: error.message,
                data: error.data,
            });
        }

        body.result.ok_or(CitreaError::MissingResult)
    }

    pub async fn chain_id(&self) -> Result<u64, CitreaError> {
        let result: String = self.request("eth_chainId", Vec::<serde_json::Value>::new()).await?;
        parse_hex_u64(&result)
    }

    pub async fn block_number(&self) -> Result<u64, CitreaError> {
        let result: String = self
            .request("eth_blockNumber", Vec::<serde_json::Value>::new())
            .await?;
        parse_hex_u64(&result)
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> Result<String, CitreaError> {
        let params = vec![
            serde_json::Value::String(format_hex_prefixed(address)),
            block.to_param(),
        ];
        self.request("eth_getBalance", params).await
    }

    pub async fn get_transaction_count(
        &self,
        address: &Address,
        block: BlockTag,
    ) -> Result<u64, CitreaError> {
        let params = vec![
            serde_json::Value::String(format_hex_prefixed(address)),
            block.to_param(),
        ];
        let result: String = self.request("eth_getTransactionCount", params).await?;
        parse_hex_u64(&result)
    }

    pub async fn call(&self, request: RpcCallRequest, block: BlockTag) -> Result<String, CitreaError> {
        let params = vec![serde_json::to_value(request)?, block.to_param()];
        self.request("eth_call", params).await
    }

    pub async fn send_raw_transaction(&self, raw_tx: &str) -> Result<String, CitreaError> {
        let params = vec![raw_tx];
        self.request("eth_sendRawTransaction", params).await
    }

    pub async fn send_raw_deposit_transaction(
        &self,
        raw_deposit: &str,
    ) -> Result<serde_json::Value, CitreaError> {
        let params = vec![raw_deposit];
        self.request("citrea_sendRawDepositTransaction", params).await
    }

    pub async fn transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<serde_json::Value, CitreaError> {
        let params = vec![tx_hash];
        self.request("eth_getTransactionReceipt", params).await
    }

    pub async fn txpool_content(&self) -> Result<serde_json::Value, CitreaError> {
        self.request("txpool_content", Vec::<serde_json::Value>::new())
            .await
    }
}

pub fn erc20_balance_of_data(owner: &Address) -> String {
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&[0x70, 0xa0, 0x82, 0x31]);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(owner);
    format_hex_prefixed(&data)
}

pub fn erc20_transfer_data(to: &Address, amount: &Bytes32) -> String {
    let mut data = Vec::with_capacity(4 + 32 + 32);
    data.extend_from_slice(&[0xa9, 0x05, 0x9c, 0xbb]);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(to);
    data.extend_from_slice(amount);
    format_hex_prefixed(&data)
}
