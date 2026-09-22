//! The decision-service discovery contract end to end: a `kind:30180`
//! service manifest published to the relay, resolved under an
//! operator pin, then an authorized decision request whose result
//! carries the identity the manifest advertised — plus the refusal
//! cases the discovery trust model defines: stale and replaced
//! advertisements, unknown versions, identity mismatch, doors a
//! tenant's access must not leak into a public document, and absent
//! capabilities.
//!
//! Every test stands up its own relay, gateway, backend, and
//! directories; nothing shares state but the shapes being checked.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use nostr::cap::{self, DISCOVERY_KIND};
use nostr::contracts::RefusalCode;
use nostr::decision::{self, Pending};
use nostr::domain::{Event, Tag};
use serde_json::{Value, json};
use tokio_tungstenite::connect_async;

use common::{
    CALLER_BYTE, WORKER_BYTE, answer, artifact, authenticated_socket, backend, body, deploy,
    hex_secret, manifest, read_json, relay, request_event, send, signer, subscribed, unix_now,
    xonly,
};
use gateway::advertise::{self, AdvertiseConfig};
use gateway::relay_worker::{Principal, Worker, WorkerConfig};

const SERVICE_BYTE: u8 = 0x53;
const SLUG: &str = "decision-edge";
const SCHEMA_REF_DIGEST: &str =
    "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0";

fn schema_ref() -> Value {
    json!({"digest": SCHEMA_REF_DIGEST, "size": 17, "media_type": "application/schema+json"})
}

/// The advertise config for a rig: the relay lane plus an HTTP lane
/// the test names but never calls.
fn advertise_config(
    rig: &Rig,
    signer_secret: String,
    created_at_safety: Option<u64>,
) -> AdvertiseConfig {
    AdvertiseConfig {
        relay: rig.relay_url.clone(),
        service_secret: Some(signer_secret),
        slug: SLUG.to_string(),
        package: "openagents".to_string(),
        summary: "The decision service at the edge.".to_string(),
        endpoint: Some(rig.deployment.address.clone()),
        worker: Some(rig.worker_pub.clone()),
        worker_relays: vec![rig.relay_url.clone()],
        registry: rig.deployment.registry.clone(),
        limits: BTreeMap::from([
            ("max_questions".to_string(), 256),
            ("request_window_seconds".to_string(), 600),
        ]),
        input: schema_ref(),
        output: schema_ref(),
        expiration_seconds: created_at_safety,
    }
}

/// The pin a client operator provisions: this publisher, this slug.
fn trust(publisher: &str, max_age_seconds: u64) -> cap::ServiceTrust<'_> {
    cap::ServiceTrust {
        publisher,
        slug: SLUG,
        max_age_seconds,
    }
}

/// REQ the manifest address and collect every stored event until EOSE.
async fn discover(url: &str, publisher: &str) -> Vec<Event> {
    let mut socket = authenticated_socket(url, CALLER_BYTE).await;
    send(
        &mut socket,
        json!(["REQ", "cap", {"kinds": [DISCOVERY_KIND], "authors": [publisher], "#d": [SLUG]}]),
    )
    .await;
    let mut events = Vec::new();
    loop {
        let message = read_json(&mut socket).await;
        match message[0].as_str() {
            Some("EVENT") => events.push(serde_json::from_value(message[2].clone()).unwrap()),
            Some("EOSE") => return events,
            _ => {}
        }
    }
}

/// Publish a decision request and collect the terminal result.
async fn run_job(
    socket: &mut common::Socket,
    caller: u8,
    worker_pub: &str,
    body: &decision::RequestBody,
    event: &Event,
    secs: u64,
) -> Option<decision::ResultPayload> {
    send(
        socket,
        json!(["REQ", "answers", {"kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND], "#e": [event.id]}]),
    )
    .await;
    send(socket, json!(["EVENT", event])).await;
    let pending = Pending {
        attempt_id: &event.id,
        worker: worker_pub,
        customer: &xonly(caller).to_string(),
        request: &body.request,
        attempt: body.attempt,
        request_digest: body.digest(),
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let frame = match tokio::time::timeout(remaining, read_json(socket)).await {
            Ok(frame) => frame,
            Err(_) => return None,
        };
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(answer_event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        match decision::bind_answer(&answer_event, &pending, &common::secret(caller)) {
            Ok(decision::Answer::Result(result)) => return Some(result),
            Ok(decision::Answer::Status(status)) if status.status == decision::Status::Error => {
                return None;
            }
            _ => continue,
        }
    }
}

/// One whole lane under test: relay, backend, gateway, worker.
struct Rig {
    relay_url: String,
    worker_pub: String,
    deployment: common::Deployment,
    _jobs_dir: tempfile::TempDir,
}

async fn rig() -> Rig {
    let (relay_url, conns) = relay().await;
    let (endpoint, _forwards) =
        backend(&artifact('a'), axum::http::StatusCode::OK, answer(), 0).await;
    let deployment = deploy(manifest(&artifact('a'), None), &endpoint).await;
    let mut principals = BTreeMap::new();
    principals.insert(
        xonly(CALLER_BYTE).to_string(),
        Principal {
            key: deployment.tokens["acme"].clone(),
            tenant: None,
            workspace: None,
        },
    );
    let jobs_dir = tempfile::tempdir().unwrap();
    let worker = Worker::open(WorkerConfig {
        relay: relay_url.clone(),
        worker_secret: Some(hex_secret(WORKER_BYTE)),
        upstream: deployment.address.clone(),
        principals,
        anonymous: true,
        jobs: 4,
        upstream_timeout_secs: 30,
        jobs_dir: jobs_dir.path().to_path_buf(),
        request_window: None,
    })
    .unwrap();
    let worker_pub = worker.pubkey().to_string();
    let serving = Arc::clone(&worker);
    let url = relay_url.clone();
    tokio::spawn(async move {
        let (socket, _) = connect_async(&url).await.unwrap();
        let _ = serving.serve(socket).await;
    });
    subscribed(&conns).await;
    Rig {
        relay_url,
        worker_pub,
        deployment,
        _jobs_dir: jobs_dir,
    }
}

/// The contract's proof: discover the manifest from the relay, take
/// its `nostr-cj` lane, run an authorized call, and the result's
/// sealed receipt names the model and artifact the manifest
/// advertised — nothing more, nothing less.
#[tokio::test]
async fn discovery_drives_an_authorized_call_with_matching_identity() {
    let rig = rig().await;
    let service = signer(SERVICE_BYTE);
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    advertise::run(config).await.unwrap();

    // Discovery: the relay serves the stored manifest; the pin
    // resolves it.
    let events = discover(&rig.relay_url, service.pubkey()).await;
    let manifest = cap::resolve_service(&events, &trust(service.pubkey(), 600), unix_now())
        .expect("the manifest resolves");
    let lane = manifest
        .contract
        .relay_lane()
        .expect("a nostr-cj lane is advertised");
    assert_eq!(lane.worker.as_deref(), Some(rig.worker_pub.as_str()));
    assert_eq!(lane.request_kind, Some(decision::REQUEST_KIND));
    let door = manifest
        .contract
        .door("shared-kev")
        .expect("the shared door is advertised");
    assert_eq!(door.model, "kev-0.6b");
    assert_eq!(door.artifact_signature, artifact('a'));

    // The discovered lane answers; the result's identity matches the
    // discovered binding.
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    let body = body("cap-request-1", 1, "shared-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let result = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10)
        .await
        .expect("the discovered lane answers");
    assert_eq!(result.outcome, decision::Outcome::Answered);
    let served = &result.receipt["served"];
    assert_eq!(served["model"].as_str(), Some(door.model.as_str()));
    assert_eq!(
        served["artifact_signature"].as_str(),
        Some(door.artifact_signature.as_str())
    );
    // The receipt is the relay lane's shape, bound to this delivery.
    assert_eq!(result.receipt["transport"].as_str(), Some("relay"));
    assert_eq!(
        result.receipt["attempt_id"].as_str(),
        Some(event.id.as_str())
    );
}

/// A stale advertisement is not current fact: resolve refuses it
/// rather than fall back.
#[tokio::test]
async fn stale_manifest_refused() {
    let rig = rig().await;
    let service = signer(SERVICE_BYTE);
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    let event = advertise::event(&config, &service, unix_now() - 900).unwrap();
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    send(&mut socket, json!(["EVENT", event])).await;
    let events = discover(&rig.relay_url, service.pubkey()).await;
    assert_eq!(
        cap::resolve_service(&events, &trust(service.pubkey(), 600), unix_now())
            .unwrap_err()
            .code,
        RefusalCode::Stale
    );
}

/// A newer manifest replaces an older one; a stale newest still
/// refuses rather than reviving the older.
#[tokio::test]
async fn replaced_manifest_resolves_to_newest() {
    let rig = rig().await;
    let service = signer(SERVICE_BYTE);
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    let old = advertise::event(&config, &service, unix_now() - 30).unwrap();
    let new = advertise::event(&config, &service, unix_now() - 10).unwrap();
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    send(&mut socket, json!(["EVENT", old])).await;
    send(&mut socket, json!(["EVENT", new.clone()])).await;
    let events = discover(&rig.relay_url, service.pubkey()).await;
    // The relay's own replacement serves only the newest.
    assert_eq!(events.len(), 1);
    let manifest =
        cap::resolve_service(&events, &trust(service.pubkey(), 600), unix_now()).unwrap();
    assert_eq!(manifest.event_id, new.id);
}

/// A manifest whose schema version is not implemented refuses.
#[tokio::test]
async fn unknown_version_refused() {
    let rig = rig().await;
    let service = signer(SERVICE_BYTE);
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    let mut definition = advertise::definition(&config, service.pubkey()).unwrap();
    definition["v"] = json!(99);
    let contract = advertise::service_contract(&config).unwrap();
    let mut tags = vec![Tag::new(vec!["d".into(), SLUG.into()])];
    tags.extend(cap::service_tags(&contract));
    let event = service.sign(
        unix_now(),
        DISCOVERY_KIND,
        tags,
        json!({"definition": definition}).to_string(),
    );
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    send(&mut socket, json!(["EVENT", event])).await;
    let events = discover(&rig.relay_url, service.pubkey()).await;
    assert_eq!(
        cap::resolve_service(&events, &trust(service.pubkey(), 600), unix_now())
            .unwrap_err()
            .code,
        RefusalCode::UnsupportedVersion
    );
}

/// A manifest that claims an artifact the serving path does not serve
/// is detected at use: the result's identity is the check, and a
/// mismatch is a fault, not an answer to relabel.
#[tokio::test]
async fn identity_mismatch_detected_at_use() {
    let rig = rig().await;
    let service = signer(SERVICE_BYTE);
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    // A manifest whose door claims artifact 'b' while the serving
    // path serves 'a'.
    let mut definition = advertise::definition(&config, service.pubkey()).unwrap();
    definition["binding_contract"]["service"]["doors"][0]["artifact_signature"] =
        json!(artifact('b'));
    let event = service.sign(
        unix_now(),
        DISCOVERY_KIND,
        vec![
            Tag::new(vec!["d".into(), SLUG.into()]),
            Tag::new(vec!["t".into(), cap::CAP_MARKER.into()]),
            Tag::new(vec!["t".into(), cap::Profile::Service.tag().into()]),
            Tag::new(vec!["t".into(), "oa:transport:http".into()]),
            Tag::new(vec!["t".into(), "oa:transport:nostr-cj".into()]),
        ],
        json!({"definition": definition}).to_string(),
    );
    let mut socket = authenticated_socket(&rig.relay_url, CALLER_BYTE).await;
    send(&mut socket, json!(["EVENT", event])).await;
    let events = discover(&rig.relay_url, service.pubkey()).await;
    let manifest = cap::resolve_service(&events, &trust(service.pubkey(), 600), unix_now())
        .expect("a well-formed lie still resolves");
    let door = manifest.contract.door("shared-kev").unwrap();
    assert_eq!(door.artifact_signature, artifact('b'));

    let body = body("cap-request-2", 1, "shared-kev");
    let event = request_event(CALLER_BYTE, &rig.worker_pub, &body, unix_now());
    let result = run_job(&mut socket, CALLER_BYTE, &rig.worker_pub, &body, &event, 10)
        .await
        .expect("the lane answers");
    // The discovered claim and the served identity disagree — the
    // caller's check is what catches it.
    assert_ne!(
        result.receipt["served"]["artifact_signature"].as_str(),
        Some(door.artifact_signature.as_str())
    );
    assert_eq!(
        result.receipt["served"]["artifact_signature"].as_str(),
        Some(artifact('a').as_str())
    );
}

/// The public manifest names the shared set only: a tenant's own door
/// — and nothing about the tenant — is discovery's business.
#[tokio::test]
async fn tenant_doors_stay_out_of_public_discovery() {
    let rig = rig().await;
    let config = advertise_config(&rig, hex_secret(SERVICE_BYTE), None);
    let contract = advertise::service_contract(&config).unwrap();
    assert!(contract.door("shared-kev").is_some());
    assert!(contract.door("acme-kev").is_none());
    let definition = advertise::definition(&config, signer(SERVICE_BYTE).pubkey()).unwrap();
    let text = definition.to_string();
    assert!(!text.contains("acme"));
}

/// Nothing advertised under the pin: resolution fails closed.
#[tokio::test]
async fn absent_capability_refused() {
    let rig = rig().await;
    let events = discover(&rig.relay_url, signer(SERVICE_BYTE).pubkey()).await;
    assert!(events.is_empty());
    assert_eq!(
        cap::resolve_service(
            &events,
            &trust(signer(SERVICE_BYTE).pubkey(), 600),
            unix_now()
        )
        .unwrap_err()
        .code,
        RefusalCode::Unavailable
    );
}

/// A signed event under the wrong key is not the pinned publisher's
/// manifest — a signature alone is not the trust.
#[tokio::test]
async fn a_signed_event_from_another_publisher_is_not_the_manifest() {
    let rig = rig().await;
    let impostor = signer(0x99);
    let config = advertise_config(&rig, hex_secret(0x99), None);
    advertise::run(config).await.unwrap();
    let events = discover(&rig.relay_url, impostor.pubkey()).await;
    assert_eq!(events.len(), 1);
    assert_eq!(
        cap::resolve_service(
            &events,
            &trust(signer(SERVICE_BYTE).pubkey(), 600),
            unix_now()
        )
        .unwrap_err()
        .code,
        RefusalCode::Unavailable
    );
}
