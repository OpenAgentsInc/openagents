//! The NIP-46 custody adapter. The custodian holds only its client transport
//! key; the agent key stays inside the remote signer and is never exported.
use crate::Result;
use nostr::domain::{Event, RemoteRequest, RemoteSigner, open_response, seal_request};
use secp256k1::{SecretKey, XOnlyPublicKey};
use std::str::FromStr;

/// One request/response exchange with a remote signer.
pub trait SignerTransport {
    /// Deliver a kind `24133` request and return the signer's response. An
    /// error after delivery is an unknown outcome, not a refusal.
    fn exchange(&mut self, request: &Event, now: u64) -> std::result::Result<Event, Exchange>;
}

/// Why an exchange produced no response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exchange {
    /// Nothing was delivered.
    Unreachable,
    /// The request may have been served; the reply was lost.
    Lost,
}

/// A local NIP-46 signer service with outage injection, for fixtures and
/// same-machine signers.
pub struct InProcessSigner {
    signer: RemoteSigner,
    pub unreachable: bool,
    pub lose_reply: bool,
    pub served: usize,
}

impl InProcessSigner {
    pub fn new(remote: SecretKey, agent: SecretKey, bunker_secret: Option<String>) -> Self {
        Self {
            signer: RemoteSigner::new(remote, agent, bunker_secret, Vec::new()),
            unreachable: false,
            lose_reply: false,
            served: 0,
        }
    }
    pub fn remote_pubkey(&self) -> String {
        self.signer.remote_pubkey()
    }
    pub fn agent_pubkey(&self) -> String {
        self.signer.user_pubkey()
    }
}

impl SignerTransport for InProcessSigner {
    fn exchange(&mut self, request: &Event, now: u64) -> std::result::Result<Event, Exchange> {
        if self.unreachable {
            return Err(Exchange::Unreachable);
        }
        let response = self
            .signer
            .handle(request, now, secp256k1::rand::random())
            .map_err(|_| Exchange::Unreachable)?;
        self.served += 1;
        if self.lose_reply {
            return Err(Exchange::Lost);
        }
        Ok(response)
    }
}

/// The client side of one NIP-46 connection.
pub struct Nip46 {
    client: SecretKey,
    remote: XOnlyPublicKey,
    agent: XOnlyPublicKey,
    transport: Box<dyn SignerTransport>,
    connected: bool,
    secret: Option<String>,
}

/// What one remote call produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Result(String),
    Refused(String),
    Unknown,
}

impl Nip46 {
    pub fn new(
        client: SecretKey,
        remote: &str,
        agent: &str,
        transport: Box<dyn SignerTransport>,
        bunker_secret: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            client,
            remote: XOnlyPublicKey::from_str(remote).map_err(|_| "malformed: signer key")?,
            agent: XOnlyPublicKey::from_str(agent).map_err(|_| "malformed: agent key")?,
            transport,
            connected: false,
            secret: bunker_secret,
        })
    }
    pub fn remote(&self) -> String {
        self.remote.to_string()
    }
    pub fn expected_agent(&self) -> String {
        self.agent.to_string()
    }
    pub fn transport_pubkey(&self) -> String {
        self.client
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string()
    }
    fn call(&mut self, method: &str, params: Vec<String>, now: u64) -> Call {
        let request = RemoteRequest {
            id: workspace_host::artifact::token(),
            method: method.into(),
            params,
        };
        let Ok(event) = seal_request(
            &self.client,
            &self.remote,
            &request,
            now,
            secp256k1::rand::random(),
        ) else {
            return Call::Refused("malformed".into());
        };
        match self.transport.exchange(&event, now) {
            Ok(response) if response.pubkey == self.remote.to_string() => {
                match open_response(&response, &self.client) {
                    Ok(r) if r.id == request.id => match (r.result, r.error) {
                        (Some(result), None) => Call::Result(result),
                        (_, Some(error)) => Call::Refused(error),
                        _ => Call::Unknown,
                    },
                    _ => Call::Unknown,
                }
            }
            Ok(_) => Call::Unknown,
            Err(Exchange::Unreachable) => Call::Refused("unavailable".into()),
            Err(Exchange::Lost) => Call::Unknown,
        }
    }
    fn ensure_connected(&mut self, now: u64) -> Call {
        if self.connected {
            return Call::Result("ack".into());
        }
        let mut params = vec![self.remote.to_string()];
        if let Some(secret) = &self.secret {
            params.push(secret.clone());
        }
        match self.call("connect", params, now) {
            Call::Result(ack) if ack == "ack" || self.secret.as_deref() == Some(ack.as_str()) => {}
            Call::Result(_) => return Call::Unknown,
            other => return other,
        }
        match self.call("get_public_key", vec![], now) {
            Call::Result(key) if key == self.agent.to_string() => {
                self.connected = true;
                Call::Result("ack".into())
            }
            Call::Result(_) => Call::Refused("identity_mismatch".into()),
            other => other,
        }
    }
    /// Sign one exact unsigned event.
    pub fn sign_event(&mut self, unsigned: &str, now: u64) -> Call {
        match self.ensure_connected(now) {
            Call::Result(_) => self.call("sign_event", vec![unsigned.into()], now),
            other => other,
        }
    }
    pub fn nip44(&mut self, method: &str, peer: &str, text: &str, now: u64) -> Call {
        match self.ensure_connected(now) {
            Call::Result(_) => self.call(method, vec![peer.into(), text.into()], now),
            other => other,
        }
    }
}
