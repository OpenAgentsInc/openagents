//! Bootstrap public evidence, verify its channel binding, then pin TLS.
use crate::{Binding, TokenPolicy, verify_token};
use bitcoin::{
    hashes::{Hash, sha256},
    key::Keypair,
    secp256k1::{Secp256k1, rand::RngCore},
};
use layertree_tee_lab::{
    rpc::{self, Parameters, ServiceConfig, Snapshot},
    rules::TreeParams,
};
use serde_json::Value;
use std::{io::Read, net::SocketAddr, time::Duration};

pub struct Channel {
    client: reqwest::blocking::Client,
    origin: String,
    pub params: TreeParams,
    pub evidence: Value,
    pub certificate_der: Vec<u8>,
    pub challenge: [u8; 32],
}
fn json(response: reqwest::blocking::Response) -> Result<Value, String> {
    let success = response.status().is_success();
    let mut body = Vec::new();
    response
        .take((rpc::MAX_BODY + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| "cannot read service response")?;
    if body.len() > rpc::MAX_BODY {
        return Err("service response is too large".into());
    }
    let value: Value = serde_json::from_slice(&body).map_err(|_| "invalid service response")?;
    if !success {
        return Err(value["error"]
            .as_str()
            .unwrap_or("service refused the request")
            .into());
    }
    Ok(value)
}
fn builder(address: SocketAddr) -> reqwest::blocking::ClientBuilder {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .resolve(rpc::HOSTNAME, address)
}
impl Channel {
    /// `policy.image_digest` must come from the caller's verified release.
    /// Only the public evidence GET uses the initial untrusted connection.
    pub fn connect(
        address: SocketAddr,
        config: &ServiceConfig,
        policy: &TokenPolicy<'_>,
        clock: impl Fn() -> u64,
    ) -> Result<Self, String> {
        let expected = config.enclave()?;
        let origin = format!("https://{}:{}", rpc::HOSTNAME, address.port());
        let mut challenge = [0; 32];
        bitcoin::secp256k1::rand::thread_rng().fill_bytes(&mut challenge);
        let bootstrap = builder(address)
            .danger_accept_invalid_certs(true)
            .tls_info(true)
            .build()
            .map_err(|_| "cannot create evidence client")?;
        let response = bootstrap
            .get(format!(
                "{origin}/attestation?challenge={}",
                rpc::hex(&challenge)
            ))
            .send()
            .map_err(|_| "cannot get guest evidence")?;
        let certificate_der = response
            .extensions()
            .get::<reqwest::tls::TlsInfo>()
            .and_then(|info| info.peer_certificate())
            .ok_or("missing TLS certificate")?
            .to_vec();
        let evidence = json(response)?;
        if evidence["schema"] != "openagents.layertree.live-attestation.v1" {
            return Err("unknown guest evidence format".into());
        }
        let parameters: Parameters = serde_json::from_value(evidence["params"].clone())
            .map_err(|_| "invalid guest parameters")?;
        let params = parameters.params()?;
        if params.operator_key != expected.operator_key
            || params.tree_id != expected.tree_id
            || params.depth != expected.depth
            || params.dust_min != expected.dust_min
            || params.unroll_delay != expected.unroll_delay
            || params.migration_m.is_some()
        {
            return Err("guest parameters differ from the intended tree".into());
        }
        let certificate_sha256 = sha256::Hash::hash(&certificate_der).to_byte_array();
        verify_token(
            evidence["token"].as_str().ok_or("missing guest token")?,
            &params,
            policy,
            &Binding::Live {
                challenge: &challenge,
                certificate_sha256: &certificate_sha256,
            },
            clock(),
        )
        .map_err(|e| format!("guest evidence refused: {}", e.code()))?;
        let cert = reqwest::Certificate::from_der(&certificate_der)
            .map_err(|_| "invalid guest certificate")?;
        let client = builder(address)
            .tls_built_in_root_certs(false)
            .add_root_certificate(cert)
            .build()
            .map_err(|_| "cannot pin guest certificate")?;
        Ok(Self {
            client,
            origin,
            params,
            evidence,
            certificate_der,
            challenge,
        })
    }
    pub fn state(&self) -> Result<Snapshot, String> {
        serde_json::from_value(json(
            self.client
                .get(format!("{}/state", self.origin))
                .send()
                .map_err(|_| "cannot read guest state")?,
        )?)
        .map_err(|_| "invalid guest state".into())
    }
    pub fn events(&self) -> Result<Value, String> {
        json(
            self.client
                .get(format!("{}/events", self.origin))
                .send()
                .map_err(|_| "cannot read guest events")?,
        )
    }
    pub fn operator(
        &self,
        path: &str,
        payload: Value,
        key: &Keypair,
        timestamp: u64,
    ) -> Result<Value, String> {
        if !matches!(path, "/next" | "/genesis" | "/prepare" | "/finish") {
            return Err("unknown operator route".into());
        }
        let mut nonce = [0; 32];
        bitcoin::secp256k1::rand::thread_rng().fill_bytes(&mut nonce);
        let nonce = rpc::hex(&nonce);
        let sig = Secp256k1::new().sign_schnorr(
            &bitcoin::secp256k1::Message::from_digest(rpc::authorization(
                path, timestamp, &nonce, &payload,
            )),
            key,
        );
        self.post(
            path,
            &serde_json::to_value(rpc::Authorized {
                timestamp,
                nonce,
                signature: sig.to_string(),
                payload,
            })
            .expect("authorized request"),
        )
    }
    pub fn exit_request(
        &self,
        pos: u64,
        signature: &bitcoin::secp256k1::schnorr::Signature,
    ) -> Result<Value, String> {
        self.post(
            "/exit-request",
            &serde_json::json!({"pos":pos,"signature":signature.to_string()}),
        )
    }
    pub fn halt(
        &self,
        signature: &bitcoin::secp256k1::schnorr::Signature,
    ) -> Result<Value, String> {
        self.post(
            "/halt",
            &serde_json::json!({"signature":signature.to_string()}),
        )
    }
    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        json(
            self.client
                .post(format!("{}{path}", self.origin))
                .json(body)
                .send()
                .map_err(|_| "guest request failed")?,
        )
    }
}
