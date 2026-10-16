//! Bounded read-only Bitcoin Core RPC, for signet and isolated regtest.
use super::unhex;
use crate::follow::ChainSource;
use bitcoin::{Block, BlockHash};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{io::Read, str::FromStr, time::Duration};

pub const RPC_LIMIT: u64 = 16 * 1024 * 1024;
pub fn client() -> Result<Client, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "TLS configuration failed")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Client::builder()
        .use_preconfigured_tls(tls)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "HTTP client configuration failed".into())
}
/// Public endpoints use TLS; local Core and test mirrors may use loopback HTTP.
pub fn endpoint(text: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(text).map_err(|_| "Choose an HTTP or HTTPS endpoint.")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if (!local && url.scheme() != "https")
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Use HTTPS, or loopback HTTP, without credentials or query parameters in the URL."
                .into(),
        );
    }
    Ok(url)
}
pub fn read_response(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err(format!(
            "HTTP returned status {}.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "HTTP response could not be read.")?;
    if bytes.len() as u64 > limit {
        return Err("HTTP response exceeds the size limit.".into());
    }
    Ok(bytes)
}
pub struct RpcChain {
    client: Client,
    url: reqwest::Url,
    user: Option<String>,
    password: Option<String>,
}
impl RpcChain {
    pub fn new(url: &str, network: &str) -> Result<Self, String> {
        if !matches!(network, "signet" | "regtest") {
            return Err("Choose signet or regtest.".into());
        }
        let user = std::env::var("LAYERTREE_RPC_USER").ok();
        let password = std::env::var("LAYERTREE_RPC_PASSWORD").ok();
        if password.is_some() && user.is_none() {
            return Err("Set LAYERTREE_RPC_USER with the RPC password.".into());
        }
        let mut this = Self {
            client: client()?,
            url: endpoint(url)?,
            user,
            password,
        };
        if this
            .call("getblockchaininfo", json!([]))?
            .get("chain")
            .and_then(Value::as_str)
            != Some(network)
        {
            return Err("Bitcoin Core is on a different network.".into());
        }
        Ok(this)
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let mut request = self
            .client
            .post(self.url.clone())
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}));
        if let Some(user) = &self.user {
            request = request.basic_auth(user, self.password.as_ref());
        }
        let response = request
            .send()
            .map_err(|_| format!("Bitcoin Core did not answer {method}."))?;
        let bytes = read_response(response, RPC_LIMIT)?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Bitcoin Core returned invalid JSON.")?;
        if !value.get("error").is_none_or(Value::is_null) {
            return Err(format!("Bitcoin Core refused {method}."));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "Bitcoin Core returned no result.".into())
    }
}
impl ChainSource for RpcChain {
    fn tip(&mut self) -> Result<u32, String> {
        self.call("getblockcount", json!([]))?
            .as_u64()
            .and_then(|n| n.try_into().ok())
            .ok_or_else(|| "Invalid block height from Core.".into())
    }
    fn block_hash(&mut self, height: u32) -> Result<BlockHash, String> {
        let hash = self.call("getblockhash", json!([height]))?;
        BlockHash::from_str(hash.as_str().ok_or("Invalid block hash from Core.")?)
            .map_err(|_| "Invalid block hash from Core.".into())
    }
    fn header(&mut self, hash: &BlockHash) -> Result<bitcoin::block::Header, String> {
        let value = self.call("getblockheader", json!([hash.to_string(), false]))?;
        let bytes = unhex(value.as_str().ok_or("Invalid block header from Core.")?, 80)?;
        let header: bitcoin::block::Header = bitcoin::consensus::deserialize(&bytes)
            .map_err(|_| "Could not decode the header from Core.")?;
        if header.block_hash() != *hash {
            return Err("Core returned another block header.".into());
        }
        Ok(header)
    }
    fn filter(&mut self, hash: &BlockHash) -> Result<Vec<u8>, String> {
        let result = self.call("getblockfilter", json!([hash.to_string(), "basic"]))?;
        unhex(
            result
                .get("filter")
                .and_then(Value::as_str)
                .ok_or("Core has no basic filter; enable blockfilterindex.")?,
            RPC_LIMIT as usize,
        )
    }
    fn block(&mut self, hash: &BlockHash) -> Result<Block, String> {
        let value = self.call("getblock", json!([hash.to_string(), 0]))?;
        let bytes = unhex(
            value.as_str().ok_or("Invalid block from Core.")?,
            RPC_LIMIT as usize,
        )?;
        let block: Block = bitcoin::consensus::deserialize(&bytes)
            .map_err(|_| "Could not decode the block from Core.")?;
        if block.block_hash() != *hash || !block.check_merkle_root() {
            return Err("Core returned a block with a different hash or Merkle root.".into());
        }
        Ok(block)
    }
}
