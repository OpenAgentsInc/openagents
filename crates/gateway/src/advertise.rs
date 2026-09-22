//! Publish the decision service's NIP-CAP capability manifest.
//!
//! A `kind:30180` service manifest is the public discovery document the
//! contract in `nips/openagents/NIP-CAP.md` ("Decision services")
//! defines: the lanes the serving path answers on, the doors an
//! unauthenticated caller may name, the limits it enforces, and the
//! schema versions it speaks. The publisher's signature is an identity
//! claim — a client resolves it under an operator-provisioned pin and
//! checks the discovered identity against every result, so a signed
//! event alone is never authorization.
//!
//! What the manifest carries is derived, never invented: doors come
//! from the tenancy registry's shared set — tenant bindings, quotas,
//! and credentials cannot reach this document — and the advertised
//! lanes come from the operator's own configuration. Advertising a lane
//! the serving path does not run is a misconfiguration the check at
//! use catches.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::cap::{self, DISCOVERY_KIND};
use nostr::decision;
use nostr::domain::{Event, RelaySigner, Tag};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

use crate::relay_worker::Trouble;
use crate::serve::unix_now;

/// The socket write a stalled relay gets before the publish fails.
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);
/// The relay answer the publish waits for before failing.
const OK_TIMEOUT: Duration = Duration::from_secs(15);

/// One `decision-advertise.json`: what the public manifest claims and
/// where it is published.
#[derive(Debug, Deserialize)]
pub struct AdvertiseConfig {
    /// The relay that stores and serves the manifest.
    pub relay: String,
    /// The publisher's secret key, 64 lowercase hex. Absent means the
    /// `DECISION_ADVERTISE_SECRET` environment variable supplies it.
    /// The signer is the identity a client's discovery pin names.
    #[serde(default)]
    pub service_secret: Option<String>,
    /// The `d` slug — the manifest's addressable name and the
    /// definition's component.
    pub slug: String,
    /// The qualified id's package, such as `openagents`.
    #[serde(default = "default_package")]
    pub package: String,
    /// Inert selector text.
    pub summary: String,
    /// The HTTP lane's public base URL, when the service answers one.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// The NIP-CJ lane's worker pubkey, when the service answers one.
    #[serde(default)]
    pub worker: Option<String>,
    /// The relays the worker lane names. Absent means `relay` alone.
    #[serde(default)]
    pub worker_relays: Vec<String>,
    /// The tenancy registry directory; the manifest's `doors` are its
    /// shared set — tenant bindings never reach a public document.
    pub registry: PathBuf,
    /// The lane-level ceilings the service enforces.
    #[serde(default)]
    pub limits: BTreeMap<String, u64>,
    /// The request envelope's schema reference.
    pub input: Value,
    /// The response envelope's schema reference.
    pub output: Value,
    /// Seconds from publish the advertisement stays fresh; absent means
    /// no NIP-40 `expiration` tag.
    #[serde(default)]
    pub expiration_seconds: Option<u64>,
}

fn default_package() -> String {
    "openagents".into()
}

/// The `binding_contract` JSON the manifest carries, built from the
/// configuration's lanes and the registry's shared door set. Tenant
/// bindings never reach it.
fn contract_value(config: &AdvertiseConfig) -> Result<Value, Trouble> {
    let mut lanes = Vec::new();
    if let Some(endpoint) = &config.endpoint {
        lanes.push(json!({
            "transport": "http",
            "endpoint": endpoint,
            "call": "/v1/systemone",
            "models": "/v1/models",
        }));
    }
    if let Some(worker) = &config.worker {
        let relays = if config.worker_relays.is_empty() {
            vec![config.relay.clone()]
        } else {
            config.worker_relays.clone()
        };
        lanes.push(json!({
            "transport": "nostr-cj",
            "worker": worker,
            "relays": relays,
            "request_kind": decision::REQUEST_KIND,
            "result_kind": decision::RESULT_KIND,
            "feedback_kind": decision::FEEDBACK_KIND,
        }));
    }
    if lanes.is_empty() {
        return Err(Trouble::Config(
            "the manifest needs a lane: set `endpoint` or `worker`".into(),
        ));
    }
    let registry = tenancy::Registry::open(&config.registry).map_err(|error| {
        Trouble::Config(format!(
            "the registry at {} does not open: {error}",
            config.registry.display()
        ))
    })?;
    let doors: Vec<Value> = registry
        .manifest()
        .shared
        .iter()
        .map(|(name, binding)| {
            json!({
                "name": name,
                "model": binding.artifact.model,
                "artifact_signature": binding.artifact.artifact_signature,
            })
        })
        .collect();
    Ok(json!({
        "interface": decision::SCHEMA,
        "service": {
            "lanes": lanes,
            "doors": doors,
            "limits": config.limits,
            "versions": {
                "request": decision::SCHEMA,
                "receipt": decision::RECEIPT_SCHEMA,
            },
        },
    }))
}

/// The manifest's `service` contract, validated.
pub fn service_contract(config: &AdvertiseConfig) -> Result<cap::ServiceContract, Trouble> {
    cap::parse_service_contract(contract_value(config)?.as_object().expect("an object"))
        .map_err(|error| Trouble::Config(format!("the service contract does not build: {error}")))
}

/// The portable definition the manifest event carries — validated
/// before it is handed back.
pub fn definition(config: &AdvertiseConfig, publisher: &str) -> Result<Value, Trouble> {
    let definition = json!({
        "v": 1,
        "requires": [],
        "id": format!("{publisher}:{}/{}", config.package, config.slug),
        "profile": "service",
        "summary": config.summary,
        "input": config.input,
        "output": config.output,
        "effects": {
            "reads": [],
            "writes": [],
            "network": ["relay"],
            "process": false,
            "delegates": false,
            "spend": true
        },
        "minimum": {},
        "support": {
            "bounds": {"concurrency": "enforced"},
            "cancellation": "cooperative",
            "idempotency": "request_attempt",
            "evidence": [decision::RECEIPT_SCHEMA]
        },
        "binding_contract": contract_value(config)?,
    });
    cap::parse_definition(&definition)
        .map_err(|error| Trouble::Config(format!("the definition does not build: {error}")))?;
    Ok(definition)
}

/// The signed `kind:30180` event — `d` tag, the service tag set, and an
/// optional NIP-40 expiration.
pub fn event(config: &AdvertiseConfig, signer: &RelaySigner, now: u64) -> Result<Event, Trouble> {
    let definition = definition(config, signer.pubkey())?;
    let contract = cap::parse_service_contract(
        definition["binding_contract"]
            .as_object()
            .expect("an object"),
    )
    .map_err(|error| Trouble::Config(format!("the service contract does not build: {error}")))?;
    let mut tags = vec![Tag::new(vec!["d".into(), config.slug.clone()])];
    tags.extend(cap::service_tags(&contract));
    if let Some(seconds) = config.expiration_seconds {
        tags.push(Tag::new(vec![
            "expiration".into(),
            (now + seconds).to_string(),
        ]));
    }
    Ok(signer.sign(
        now,
        DISCOVERY_KIND,
        tags,
        json!({"definition": definition}).to_string(),
    ))
}

/// The publisher's signing identity.
pub fn signer(config: &AdvertiseConfig) -> Result<RelaySigner, Trouble> {
    let secret = config
        .service_secret
        .clone()
        .or_else(|| std::env::var("DECISION_ADVERTISE_SECRET").ok())
        .ok_or_else(|| {
            Trouble::Config(
                "no publisher secret: set `service_secret` or DECISION_ADVERTISE_SECRET".into(),
            )
        })?;
    RelaySigner::from_secret_hex(&secret)
        .map_err(|error| Trouble::Config(format!("the publisher secret does not parse: {error}")))
}

/// Connect to the relay, authenticate, publish the manifest, and wait
/// for the relay's `OK` verdict.
pub async fn run(config: AdvertiseConfig) -> Result<(), Trouble> {
    let signer = signer(&config)?;
    let event = event(&config, &signer, unix_now())?;
    let (mut socket, _) = tokio_tungstenite::connect_async(&config.relay)
        .await
        .map_err(|error| Trouble::Relay(format!("connect: {error}")))?;
    let challenge = read_json(&mut socket).await?;
    if challenge[0].as_str() != Some("AUTH") {
        return Err(Trouble::Relay(format!(
            "expected an AUTH challenge, got {challenge}"
        )));
    }
    let auth = signer.sign(
        unix_now(),
        22_242,
        vec![
            Tag::new(vec!["relay".into(), config.relay.clone()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap_or_default().into(),
            ]),
        ],
        String::new(),
    );
    send_json(&mut socket, &json!(["AUTH", auth])).await?;
    let ok = read_json(&mut socket).await?;
    if ok[1].as_str() != Some(auth.id.as_str()) || ok[2] != true {
        return Err(Trouble::Relay(format!("the relay refused AUTH: {ok}")));
    }
    send_json(&mut socket, &json!(["EVENT", event])).await?;
    let ok = read_json(&mut socket).await?;
    if ok[0].as_str() != Some("OK") || ok[1].as_str() != Some(event.id.as_str()) {
        return Err(Trouble::Relay(format!(
            "expected an OK for {}, got {ok}",
            event.id
        )));
    }
    if ok[2] != true {
        return Err(Trouble::Relay(format!(
            "the relay refused the manifest: {ok}"
        )));
    }
    eprintln!(
        "decision-advertise: published {} as {}",
        event.id, config.slug
    );
    Ok(())
}

async fn read_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Value, Trouble> {
    let message = tokio::time::timeout(OK_TIMEOUT, socket.next())
        .await
        .map_err(|_| Trouble::Relay("the relay did not answer in time".into()))?;
    match message {
        Some(Ok(tungstenite::Message::Text(text))) => serde_json::from_str::<Value>(&text)
            .map_err(|error| Trouble::Relay(format!("a relay message does not parse: {error}"))),
        Some(Ok(_)) => Err(Trouble::Relay("a non-text relay message".into())),
        Some(Err(error)) => Err(Trouble::Relay(format!("socket: {error}"))),
        None => Err(Trouble::Relay("the socket closed".into())),
    }
}

async fn send_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    value: &Value,
) -> Result<(), Trouble> {
    tokio::time::timeout(
        WRITE_TIMEOUT,
        socket.send(tungstenite::Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| Trouble::Relay("a socket write stalled".into()))?
    .map_err(|error| Trouble::Relay(format!("socket write: {error}")))
}
