use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FaucetConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            url: std::env::var("FAUCET_URL")
                .unwrap_or_else(|_| "https://api.lightspark.com/graphql/spark/rc".to_string()),
            username: std::env::var("FAUCET_USERNAME").ok(),
            password: std::env::var("FAUCET_PASSWORD").ok(),
        }
    }
}

pub struct RegtestFaucet {
    client: Client,
    config: FaucetConfig,
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    #[serde(rename = "operationName")]
    operation_name: String,
    variables: FaucetVariables,
    query: String,
}

#[derive(Debug, Serialize)]
struct FaucetVariables {
    amount_sats: u64,
    address: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<ResponseData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct ResponseData {
    request_regtest_funds: RequestRegtestFunds,
}

#[derive(Debug, Deserialize)]
struct RequestRegtestFunds {
    transaction_hash: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

impl RegtestFaucet {
    pub fn new() -> Result<Self> {
        Self::with_config(FaucetConfig::default())
    }

    pub fn with_config(config: FaucetConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to create HTTP client")?;
        Ok(Self { client, config })
    }

    pub async fn fund_address(&self, address: &str, amount_sats: u64) -> Result<String> {
        let request_body = GraphQLRequest {
            operation_name: "RequestRegtestFunds".to_string(),
            variables: FaucetVariables {
                amount_sats,
                address: address.to_string(),
            },
            query: "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash}}"
                .to_string(),
        };

        let mut req = self.client.post(&self.config.url).json(&request_body);
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            req = req.basic_auth(username, Some(password));
        }
        req = req.header("Content-Type", "application/json");

        let response = req.send().await.context("failed to send faucet request")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("faucet request failed with status {}: {}", status, body);
        }

        let response_text = response.text().await?;
        let graphql_response: GraphQLResponse =
            serde_json::from_str(&response_text).context(response_text)?;

        if let Some(errors) = graphql_response.errors {
            let error_messages: Vec<String> = errors.into_iter().map(|e| e.message).collect();
            bail!("faucet returned errors: {}", error_messages.join(", "));
        }

        let txid = graphql_response
            .data
            .ok_or_else(|| anyhow::anyhow!("faucet response missing data"))?
            .request_regtest_funds
            .transaction_hash;

        Ok(txid)
    }
}
