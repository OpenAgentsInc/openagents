pub mod types;
pub mod ws;

use std::collections::HashMap;

use platform_utils::http::HttpClient;

use crate::config::BoltzConfig;
use crate::error::BoltzError;

use self::types::{
    ContractsResponse, CreateReverseSwapRequest, CreateReverseSwapResponse, EncodeRequest,
    EncodeResponse, QuoteResponse, ReversePairsResponse, SwapStatusResponse,
    SwapTransactionResponse,
};

/// HTTP client for the Boltz REST API.
pub struct BoltzApiClient {
    config: BoltzConfig,
    http_client: Box<dyn HttpClient>,
}

impl BoltzApiClient {
    pub fn new(config: &BoltzConfig, http_client: Box<dyn HttpClient>) -> Self {
        Self {
            config: config.clone(),
            http_client,
        }
    }

    // ─── Reverse Swap ────────────────────────────────────────────────────

    /// `GET /v2/swap/reverse` — fetch pair info (fees, limits, pairHash).
    pub async fn get_reverse_swap_pairs(&self) -> Result<ReversePairsResponse, BoltzError> {
        self.get_request("v2/swap/reverse").await
    }

    /// `POST /v2/swap/reverse` — create a reverse swap.
    pub async fn create_reverse_swap(
        &self,
        req: &CreateReverseSwapRequest,
    ) -> Result<CreateReverseSwapResponse, BoltzError> {
        self.post_request("v2/swap/reverse", req).await
    }

    /// `GET /v2/swap/{id}` — get current swap status.
    pub async fn get_swap_status(&self, id: &str) -> Result<SwapStatusResponse, BoltzError> {
        self.get_request(&format!("v2/swap/{id}")).await
    }

    /// `GET /v2/swap/reverse/{id}/transaction` — get lockup transaction details.
    pub async fn get_swap_transaction(
        &self,
        id: &str,
    ) -> Result<SwapTransactionResponse, BoltzError> {
        self.get_request(&format!("v2/swap/reverse/{id}/transaction"))
            .await
    }

    // ─── DEX Quotes ──────────────────────────────────────────────────────

    /// `GET /v2/quote/{chain}/in` — quote by input amount.
    pub async fn get_quote_in(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: u128,
    ) -> Result<Vec<QuoteResponse>, BoltzError> {
        let endpoint = format!(
            "v2/quote/{chain}/in?tokenIn={token_in}&tokenOut={token_out}&amountIn={amount_in}"
        );
        self.get_request(&endpoint).await
    }

    /// `GET /v2/quote/{chain}/out` — quote by output amount.
    pub async fn get_quote_out(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_out: u128,
    ) -> Result<Vec<QuoteResponse>, BoltzError> {
        let endpoint = format!(
            "v2/quote/{chain}/out?tokenIn={token_in}&tokenOut={token_out}&amountOut={amount_out}"
        );
        self.get_request(&endpoint).await
    }

    /// `POST /v2/quote/{chain}/encode` — encode a quote into calldata.
    pub async fn encode_quote(
        &self,
        chain: &str,
        req: &EncodeRequest,
    ) -> Result<EncodeResponse, BoltzError> {
        self.post_request(&format!("v2/quote/{chain}/encode"), req)
            .await
    }

    // ─── Discovery ───────────────────────────────────────────────────────

    /// `GET /v2/chain/contracts` — fetch contract addresses.
    pub async fn get_contracts(&self) -> Result<ContractsResponse, BoltzError> {
        self.get_request("v2/chain/contracts").await
    }

    // ─── Internal Helpers ────────────────────────────────────────────────

    /// Headers sent on every request: `Content-Type` plus the `referral`
    /// header (used by Boltz for attribution).
    fn default_headers(&self) -> HashMap<String, String> {
        HashMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            ("referral".to_string(), self.config.referral_id.clone()),
        ])
    }

    async fn get_request<D>(&self, endpoint: &str) -> Result<D, BoltzError>
    where
        D: serde::de::DeserializeOwned,
    {
        self.get_request_with_headers(endpoint, self.default_headers())
            .await
    }

    async fn get_request_with_headers<D>(
        &self,
        endpoint: &str,
        headers: HashMap<String, String>,
    ) -> Result<D, BoltzError>
    where
        D: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{endpoint}", self.config.api_url);
        let response = self.http_client.get(url, Some(headers)).await?;

        if !response.is_success() {
            return Err(BoltzError::Api {
                reason: response.body,
                code: Some(response.status),
            });
        }

        response.json::<D>().map_err(|e| BoltzError::Api {
            reason: format!("Failed to parse response: {e}"),
            code: None,
        })
    }

    async fn post_request<S, D>(&self, endpoint: &str, body: &S) -> Result<D, BoltzError>
    where
        S: serde::Serialize,
        D: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{endpoint}", self.config.api_url);
        let body_json = serde_json::to_string(body).map_err(|e| BoltzError::Api {
            reason: format!("Failed to serialize request: {e}"),
            code: None,
        })?;

        let response = self
            .http_client
            .post(url, Some(self.default_headers()), Some(body_json))
            .await?;

        if !response.is_success() {
            return Err(BoltzError::Api {
                reason: response.body,
                code: Some(response.status),
            });
        }

        response.json::<D>().map_err(|e| BoltzError::Api {
            reason: format!("Failed to parse response: {e}"),
            code: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;

    #[macros::test_all]
    fn test_api_client_construction() {
        let config = BoltzConfig::mainnet("test_referral".to_string());
        let client = BoltzApiClient::new(
            &config,
            Box::new(platform_utils::DefaultHttpClient::new(None).expect("http client")),
        );
        assert_eq!(client.config.api_url, "https://api.boltz.exchange");
    }

    #[macros::test_all]
    fn test_default_headers() {
        let config = BoltzConfig::mainnet("test_referral".to_string());
        let client = BoltzApiClient::new(
            &config,
            Box::new(platform_utils::DefaultHttpClient::new(None).expect("http client")),
        );
        let headers = client.default_headers();
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(headers.get("referral").unwrap(), "test_referral");
    }
}
