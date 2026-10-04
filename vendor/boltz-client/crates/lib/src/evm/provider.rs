use std::collections::HashMap;

use serde::Deserialize;

use platform_utils::http::HttpClient;

use crate::error::BoltzError;

/// Maximum retries for rate-limited (429) RPC requests.
const MAX_RPC_RETRIES: u32 = 5;

/// Base delay in milliseconds for exponential backoff (doubles each retry).
const RPC_RETRY_BASE_MS: u64 = 1000;

/// Thin JSON-RPC wrapper over `platform_utils::HttpClient` for EVM read operations.
pub struct EvmProvider {
    rpc_url: String,
    http_client: Box<dyn HttpClient>,
}

/// Minimal block header from `eth_getBlockByNumber`. Only the Arbitrum-specific
/// `l1BlockNumber` field is decoded — the L1 (Ethereum) height that Solidity
/// `block.number` reflects on Arbitrum.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockHeader {
    l1_block_number: String,
}

/// A single log entry from `eth_getLogs`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub transaction_hash: String,
}

/// Minimal transaction receipt from `eth_getTransactionReceipt`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxReceipt {
    pub transaction_hash: String,
    /// `"0x1"` = success, `"0x0"` = reverted. `None` if absent — Arbitrum
    /// (post-Byzantium) always includes it, but defaulting keeps a missing
    /// field from being a parse error, and the success/revert decision is never
    /// inferred from an absent status (see `is_success` / `is_reverted`).
    #[serde(default)]
    pub status: Option<String>,
    pub block_hash: String,
    pub block_number: String,
    pub gas_used: String,
    #[serde(default)]
    pub logs: Vec<LogEntry>,
}

impl TxReceipt {
    /// Returns true only if the transaction explicitly succeeded (status 0x1).
    pub fn is_success(&self) -> bool {
        self.status.as_deref() == Some("0x1")
    }

    /// Returns true only if the transaction explicitly reverted (status 0x0).
    /// An absent/unknown status is neither success nor revert — callers should
    /// keep waiting rather than finalize.
    pub fn is_reverted(&self) -> bool {
        self.status.as_deref() == Some("0x0")
    }
}

impl EvmProvider {
    pub fn new(rpc_url: String, http_client: Box<dyn HttpClient>) -> Self {
        Self {
            rpc_url,
            http_client,
        }
    }

    /// Execute a read-only contract call (`eth_call`).
    pub async fn eth_call(&self, to: &str, data: &[u8]) -> Result<Vec<u8>, BoltzError> {
        let result: String = self
            .rpc_request(
                "eth_call",
                serde_json::json!([
                    { "to": to, "data": format!("0x{}", hex::encode(data)) },
                    "latest"
                ]),
            )
            .await?;

        let clean = result.strip_prefix("0x").unwrap_or(&result);
        hex::decode(clean).map_err(|e| BoltzError::Evm {
            reason: format!("Failed to decode eth_call result: {e}"),
            tx_hash: None,
        })
    }

    /// Get a transaction receipt by hash.
    pub async fn eth_get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<TxReceipt>, BoltzError> {
        let result: Option<TxReceipt> = self
            .rpc_request("eth_getTransactionReceipt", serde_json::json!([tx_hash]))
            .await?;
        Ok(result)
    }

    /// Get the chain ID.
    pub async fn eth_chain_id(&self) -> Result<u64, BoltzError> {
        let result: String = self
            .rpc_request("eth_chainId", serde_json::json!([]))
            .await?;
        parse_hex_u64(&result)
    }

    /// Query logs matching the given filter via `eth_getLogs`.
    /// Each element in `topics` is either `Some(hex_topic)` for an exact match
    /// or `None` for a wildcard.
    pub async fn eth_get_logs(
        &self,
        address: &str,
        topics: &[Option<&str>],
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<LogEntry>, BoltzError> {
        let topics_json: Vec<serde_json::Value> = topics
            .iter()
            .map(|t| match t {
                Some(hex) => serde_json::Value::String(hex.to_string()),
                None => serde_json::Value::Null,
            })
            .collect();

        let params = serde_json::json!([{
            "address": address,
            "topics": topics_json,
            "fromBlock": format_hex_u64(from_block),
            "toBlock": format_hex_u64(to_block),
        }]);

        self.rpc_request("eth_getLogs", params).await
    }

    /// Get the latest block number.
    ///
    /// On Arbitrum this is the **L2** (Arbitrum) block number — it can advance
    /// several times per L1 block. Do NOT compare it against values denominated
    /// in L1 block height (e.g. a swap `timeout_block_height`); use
    /// [`Self::eth_l1_block_number`] for that.
    pub async fn eth_block_number(&self) -> Result<u64, BoltzError> {
        let result: String = self
            .rpc_request("eth_blockNumber", serde_json::json!([]))
            .await?;
        parse_hex_u64(&result)
    }

    /// Get the current **L1** (Ethereum) block number as seen on Arbitrum.
    ///
    /// Reads the Arbitrum-specific `l1BlockNumber` field from the latest block
    /// header. This is the value Solidity `block.number` reflects on Arbitrum,
    /// so it is the correct reference for anything denominated in L1 block
    /// height — notably the `ERC20Swap` lockup `timeout_block_height`, which
    /// Boltz denominates in L1 blocks. The standard `eth_blockNumber`
    /// ([`Self::eth_block_number`]) returns the L2 number and must not be used
    /// for that comparison.
    pub async fn eth_l1_block_number(&self) -> Result<u64, BoltzError> {
        let header: BlockHeader = self
            .rpc_request("eth_getBlockByNumber", serde_json::json!(["latest", false]))
            .await?;
        parse_hex_u64(&header.l1_block_number)
    }

    /// Internal: send a JSON-RPC request and parse the result.
    /// Retries with exponential backoff on HTTP 429 (rate limit) responses.
    async fn rpc_request<T: for<'a> Deserialize<'a>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, BoltzError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let body = serde_json::to_string(&request).map_err(|e| BoltzError::Evm {
            reason: format!("Failed to serialize JSON-RPC request: {e}"),
            tx_hash: None,
        })?;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let mut last_err = None;
        for attempt in 0..MAX_RPC_RETRIES {
            let response = self
                .http_client
                .post(
                    self.rpc_url.clone(),
                    Some(headers.clone()),
                    Some(body.clone()),
                )
                .await?;

            if response.status == 429 {
                let delay = RPC_RETRY_BASE_MS
                    .saturating_mul(2u64.saturating_pow(attempt))
                    .min(30_000);
                tracing::warn!(
                    method,
                    attempt,
                    delay_ms = delay,
                    "RPC rate limited (429), retrying"
                );
                sleep_ms(delay).await;
                last_err = Some(BoltzError::Evm {
                    reason: format!("RPC HTTP error 429: {}", response.body),
                    tx_hash: None,
                });
                continue;
            }

            if !response.is_success() {
                return Err(BoltzError::Evm {
                    reason: format!("RPC HTTP error {}: {}", response.status, response.body),
                    tx_hash: None,
                });
            }

            let rpc_response: serde_json::Value =
                serde_json::from_str(&response.body).map_err(|e| BoltzError::Evm {
                    reason: format!(
                        "Failed to parse JSON-RPC response: {e} (body: {})",
                        response.body
                    ),
                    tx_hash: None,
                })?;

            // JSON-RPC level 429 (rate limit in the JSON body)
            if let Some(err) = rpc_response.get("error") {
                let code = err
                    .get("code")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                if code == 429 {
                    let delay = RPC_RETRY_BASE_MS
                        .saturating_mul(2u64.saturating_pow(attempt))
                        .min(30_000);
                    tracing::warn!(
                        method,
                        attempt,
                        delay_ms = delay,
                        "RPC rate limited (JSON-RPC 429), retrying"
                    );
                    sleep_ms(delay).await;
                    let message = err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Too Many Requests");
                    last_err = Some(BoltzError::Evm {
                        reason: format!("JSON-RPC error 429: {message}"),
                        tx_hash: None,
                    });
                    continue;
                }
            }

            return Self::parse_rpc_response(&rpc_response);
        }

        Err(last_err.unwrap_or_else(|| BoltzError::Evm {
            reason: format!("RPC request failed after {MAX_RPC_RETRIES} retries"),
            tx_hash: None,
        }))
    }

    fn parse_rpc_response<T: for<'a> Deserialize<'a>>(
        rpc_response: &serde_json::Value,
    ) -> Result<T, BoltzError> {
        if let Some(err) = rpc_response.get("error") {
            let code = err
                .get("code")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(BoltzError::Evm {
                reason: format!("JSON-RPC error {code}: {message}"),
                tx_hash: None,
            });
        }

        let result = rpc_response.get("result").ok_or_else(|| BoltzError::Evm {
            reason: "JSON-RPC response has no result".to_string(),
            tx_hash: None,
        })?;

        serde_json::from_value(result.clone()).map_err(|e| BoltzError::Evm {
            reason: format!("Failed to deserialize JSON-RPC result: {e}"),
            tx_hash: None,
        })
    }
}

fn format_hex_u64(n: u64) -> String {
    format!("0x{n:x}")
}

fn parse_hex_u64(s: &str) -> Result<u64, BoltzError> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(clean, 16).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to parse hex u64 '{s}': {e}"),
        tx_hash: None,
    })
}

async fn sleep_ms(ms: u64) {
    platform_utils::tokio::time::sleep(platform_utils::time::Duration::from_millis(ms)).await;
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use platform_utils::http::{HttpError, HttpResponse};
    use std::sync::{Arc, Mutex};

    /// Mock HTTP client that returns canned responses.
    struct MockHttpClient {
        responses: Arc<Mutex<Vec<HttpResponse>>>,
    }

    impl MockHttpClient {
        fn new(responses: Vec<HttpResponse>) -> Self {
            // Reverse so we can pop from the back
            let mut r = responses;
            r.reverse();
            Self {
                responses: Arc::new(Mutex::new(r)),
            }
        }
    }

    #[macros::async_trait]
    impl HttpClient for MockHttpClient {
        async fn get(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
        ) -> Result<HttpResponse, HttpError> {
            unimplemented!()
        }

        async fn post(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
            _body: Option<String>,
        ) -> Result<HttpResponse, HttpError> {
            let mut responses = self.responses.lock().unwrap();
            Ok(responses.pop().expect("no more mock responses"))
        }

        async fn delete(
            &self,
            _url: String,
            _headers: Option<HashMap<String, String>>,
            _body: Option<String>,
        ) -> Result<HttpResponse, HttpError> {
            unimplemented!()
        }
    }

    fn rpc_success(result: &serde_json::Value) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": result
            })
            .to_string(),
        }
    }

    fn rpc_error(code: i64, message: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            body: serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": { "code": code, "message": message }
            })
            .to_string(),
        }
    }

    #[macros::async_test_all]
    async fn test_eth_chain_id() {
        let client = MockHttpClient::new(vec![rpc_success(&serde_json::json!("0xa4b1"))]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let chain_id = provider.eth_chain_id().await.unwrap();
        assert_eq!(chain_id, 42161); // 0xa4b1 = Arbitrum
    }

    #[macros::async_test_all]
    async fn test_eth_block_number() {
        let client = MockHttpClient::new(vec![rpc_success(&serde_json::json!("0x1234"))]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let block = provider.eth_block_number().await.unwrap();
        assert_eq!(block, 0x1234);
    }

    #[macros::async_test_all]
    async fn test_eth_l1_block_number() {
        // `eth_getBlockByNumber` returns a header object; we decode only the
        // Arbitrum-specific `l1BlockNumber` field (and ignore the rest).
        let header = serde_json::json!({
            "number": "0x1c180a0c",        // L2 number — must NOT be returned
            "l1BlockNumber": "0x181b3b4",  // 25_278_900
            "hash": "0xabc",
        });
        let client = MockHttpClient::new(vec![rpc_success(&header)]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let l1 = provider.eth_l1_block_number().await.unwrap();
        assert_eq!(l1, 0x0181_b3b4);
    }

    #[macros::async_test_all]
    async fn test_eth_l1_block_number_missing_field() {
        // A header without `l1BlockNumber` (e.g. a non-Arbitrum RPC) must error,
        // not silently succeed — the timeout guard fails closed on the result.
        let header = serde_json::json!({ "number": "0x10", "hash": "0xabc" });
        let client = MockHttpClient::new(vec![rpc_success(&header)]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        assert!(provider.eth_l1_block_number().await.is_err());
    }

    #[macros::async_test_all]
    async fn test_eth_call() {
        // Return ABI-encoded uint256(6) — 32 bytes, value 6
        let hex_result = format!("0x{}", "00".repeat(31) + "06");
        let client = MockHttpClient::new(vec![rpc_success(&serde_json::json!(hex_result))]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let data = hex::decode("54fd4d50").unwrap(); // version() selector
        let result = provider
            .eth_call("0x6398B76DF91C5eBe9f488e3656658E79284dDc0F", &data)
            .await
            .unwrap();

        assert_eq!(result.len(), 32);
        assert_eq!(result[31], 6);
    }

    #[macros::async_test_all]
    async fn test_eth_get_transaction_receipt_found() {
        let receipt_json = serde_json::json!({
            "transactionHash": "0xabc123",
            "status": "0x1",
            "blockHash": "0xblock",
            "blockNumber": "0x100",
            "gasUsed": "0x5208"
        });
        let client = MockHttpClient::new(vec![rpc_success(&receipt_json)]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let receipt = provider
            .eth_get_transaction_receipt("0xabc123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.transaction_hash, "0xabc123");
        assert!(receipt.is_success());
    }

    #[macros::async_test_all]
    async fn test_eth_get_transaction_receipt_not_found() {
        let client = MockHttpClient::new(vec![rpc_success(&serde_json::json!(null))]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let receipt = provider
            .eth_get_transaction_receipt("0xnonexistent")
            .await
            .unwrap();
        assert!(receipt.is_none());
    }

    #[macros::async_test_all]
    async fn test_rpc_error_response() {
        let client = MockHttpClient::new(vec![rpc_error(-32601, "Method not found")]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let result = provider.eth_chain_id().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Method not found"));
    }

    #[macros::async_test_all]
    async fn test_http_error_response() {
        let client = MockHttpClient::new(vec![HttpResponse {
            status: 500,
            body: "Internal Server Error".to_string(),
        }]);
        let provider = EvmProvider::new("http://localhost:8545".to_string(), Box::new(client));

        let result = provider.eth_chain_id().await;
        assert!(result.is_err());
    }

    #[macros::test_all]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0xa4b1").unwrap(), 42161);
        assert_eq!(parse_hex_u64("0x0").unwrap(), 0);
        assert_eq!(parse_hex_u64("0x1").unwrap(), 1);
        assert_eq!(parse_hex_u64("ff").unwrap(), 255);
    }

    #[macros::test_all]
    fn test_tx_receipt_is_success() {
        let receipt = TxReceipt {
            transaction_hash: "0x".to_string(),
            status: Some("0x1".to_string()),
            block_hash: "0x".to_string(),
            block_number: "0x1".to_string(),
            gas_used: "0x0".to_string(),
            logs: vec![],
        };
        assert!(receipt.is_success());
        assert!(!receipt.is_reverted());

        let reverted = TxReceipt {
            status: Some("0x0".to_string()),
            ..receipt.clone()
        };
        assert!(!reverted.is_success());
        assert!(reverted.is_reverted());

        // Absent status is neither success nor revert (keep waiting).
        let unknown = TxReceipt {
            status: None,
            ..receipt
        };
        assert!(!unknown.is_success());
        assert!(!unknown.is_reverted());
    }
}
