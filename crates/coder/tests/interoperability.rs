//! End-to-end proof that the landed program, extension, Wasm, and execution
//! contracts compose. This is not a second runtime.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use coder::Agent;
use coder::generate::{Door, StubGenerate};
use coder::trace::Recorder;
use coder::turn;
use nostr::contracts::{RefusalCode, digest_bytes};
use nostr::domain::{RelaySigner, Tag};
use nostr::execution::{self, Admission, Service, Window};
use nostr::ext::{self, Install, InstallStep};
use nostr::nip44;
use nostr::prg::{self, Selection};
use nostr::run::{self, Boundary, Dispatcher};
use plugin::{Limits, Profile, Snapshot, invoke};
use secp256k1::SecretKey;
use serde_json::json;

const PUB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SCHEMA: &str = "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0";
const NOW: u64 = 1_700_000_000;
const DEADLINE: u64 = 1_700_003_600;
const RETAIN: u64 = 1_700_010_000;

struct Keys {
    signer: RelaySigner,
    secret: SecretKey,
    pubkey: String,
}

fn keys(byte: u8) -> Keys {
    let raw = [byte; 32];
    let secret = SecretKey::from_byte_array(raw).unwrap();
    let hex: String = raw.iter().map(|item| format!("{item:02x}")).collect();
    let signer = RelaySigner::from_secret_hex(&hex).unwrap();
    let pubkey = signer.pubkey().to_owned();
    Keys {
        signer,
        secret,
        pubkey,
    }
}

fn schema() -> serde_json::Value {
    json!({"digest": SCHEMA, "size": 17, "media_type": "application/schema+json"})
}

fn program() -> serde_json::Value {
    let step = |name: &str, kind: &str, after: serde_json::Value| {
        json!({
            "name": name,
            "kind": kind,
            "target": {
                "id": format!("{PUB}:openagents/{name}"),
                "artifact": {"digest": SCHEMA, "size": 17, "media_type": "application/wasm"}
            },
            "after": after,
            "input": {"from": "input", "pointer": ""},
            "output": schema(),
            "bounds": {},
            "on_error": "stop"
        })
    };
    json!({
        "v": 1,
        "requires": [],
        "id": format!("{PUB}:openagents/demo"),
        "summary": "a typed workflow",
        "input": schema(),
        "output": schema(),
        "steps": [
            step("native", "query", json!([])),
            step("guest", "invoke", json!(["native"]))
        ],
        "result": {"from": "step:guest", "pointer": "/value"},
        "bounds": {}
    })
}

fn artifact(bytes: &[u8]) -> serde_json::Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len() as u64,
        "media_type": "application/json"
    })
}

fn execute_body() -> serde_json::Value {
    json!({
        "v": execution::SCHEMA,
        "requires": [],
        "type": "execute",
        "request": "req-1",
        "attempt": 1,
        "run": "run-1",
        "target": {
            "id": format!("{PUB}:pkg/op"),
            "artifact": artifact(b"target")
        },
        "lock": artifact(b"lock-bytes"),
        "input": {"task": "count"},
        "context": artifact(b"context"),
        "requirements": artifact(b"requirements"),
        "bounds": {"output_bytes": 1024},
        "deadline": DEADLINE,
        "retain_until": RETAIN
    })
}

#[test]
fn programs_extensions_wasm_and_execution_compose_without_a_second_runtime() {
    let definition = prg::parse_definition(&program()).unwrap();
    assert_eq!(definition.steps.len(), 2);
    let tags = prg::discovery_tags(&definition);
    prg::check_discovery_tags(&tags, &definition).unwrap();
    assert!(prg::admits(
        &Selection::Program(definition.id.clone()),
        true
    ));
    assert!(!prg::admits(&Selection::None, true));

    let pinned = "sha256:pinned";
    let head = "sha256:newer";
    let mut store = BTreeMap::new();
    store.insert(pinned.to_owned(), b"lock-v1".to_vec());
    store.insert(head.to_owned(), b"lock-v2".to_vec());
    let mut edges = BTreeMap::new();
    edges.insert(pinned.to_owned(), Vec::new());
    let locked = prg::pin_closure(pinned, &edges, &store).unwrap();
    assert_eq!(locked[pinned], b"lock-v1");
    assert_eq!(
        prg::locked_definition(pinned, head, &store).unwrap(),
        b"lock-v1"
    );
    store.remove(pinned);
    assert_eq!(
        prg::locked_definition(pinned, head, &store)
            .unwrap_err()
            .code,
        RefusalCode::Stale
    );
    assert_eq!(ext::preserve_active_pin(pinned, head), pinned);
    assert!(ext::grants_after_migration().is_empty());
    assert_eq!(
        ext::expand_archive("application/zip").unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    let installed = ext::transition(
        &Install::Absent,
        InstallStep::Stage {
            lock: pinned.to_owned(),
            verified: true,
        },
    )
    .unwrap();
    assert!(matches!(installed, Install::Staged { .. }));

    let wasm = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugin/fixtures/pure.wasm"
    ))
    .unwrap();
    let guest = invoke(plugin::Call {
        wasm: &wasm,
        profile: Profile::Pure,
        invocation: "inv-1",
        operation: "echo",
        input: &json!({"topic": "notes"}),
        snapshot: &Snapshot::default(),
        handles: &BTreeMap::new(),
        limits: Limits {
            fuel: 50_000_000,
            ..Limits::default()
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        required: true,
    })
    .unwrap();
    assert_eq!(guest.value["topic"], json!("notes"));
    assert_eq!(guest.verification, "not_run");
    assert_eq!(
        prg::allow_retry(true).unwrap_err().code,
        RefusalCode::CannotEnforce
    );

    let caller = keys(0x11);
    let worker = keys(0x22);
    let peer = worker.pubkey.parse().unwrap();
    let event = execution::Seal {
        signer: &caller.signer,
        conversation: nip44::conversation_key(&caller.secret, &peer),
        nonce: [7; 32],
        created_at: NOW,
    }
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), worker.pubkey.clone()]),
            Tag::new(vec!["expiration".into(), DEADLINE.to_string()]),
        ],
        &execute_body(),
    )
    .unwrap();
    let opened =
        execution::open_request(&event, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT)
            .unwrap();
    let mut service = Service::new(worker.pubkey.clone(), 1, RETAIN);
    assert!(!service.promises_exactly_once());
    assert!(!service.relay_ok_is_acceptance());
    assert!(!service.socket_close_is_cancel());
    assert!(!service.expiration_stops_subprocess());
    let Admission::Reserved { claim, .. } =
        service.prepare(&opened, NOW, &"ab".repeat(32)).unwrap()
    else {
        panic!("the first execute reserves");
    };
    assert!(!claim.acknowledged);
    let durability = run::crash(Boundary::Effect, "reservation-1");
    assert!(durability.effect_started);
    assert_eq!(durability.outcome.as_deref(), Some("unknown"));
    assert!(!run::accepted("unknown", "not_run", "pending"));
    assert!(run::advertise(&durability).is_err());
    let replay = run::replay_offline(&[]).unwrap();
    assert!(!replay.executed);
    assert!(!run::empty_retrieval_complete(0, true));
    assert_eq!(
        run::handoff(false, false, 1, &[]).unwrap_err().code,
        RefusalCode::NotAdmitted
    );
    let fenced = Dispatcher {
        id: "worker-a".into(),
        reachable: true,
        acked_generation: Some(1),
    };
    assert!(run::handoff(true, false, 1, &[fenced]).unwrap().is_empty());
    assert!(run::accept_generation(2, 1).is_err());

    let other = keys(0x33);
    let left = Mutex::new(Service::new(worker.pubkey.clone(), 1, RETAIN));
    let right = Mutex::new(Service::new(other.pubkey.clone(), 1, RETAIN));
    let mut bad = program();
    bad["steps"][0]["kind"] = json!("teleport");
    assert_eq!(
        prg::parse_definition(&bad).unwrap_err().code,
        RefusalCode::UnsupportedFeature
    );
    assert_eq!(
        run::within_horizon(RETAIN + 1, RETAIN).unwrap_err().code,
        RefusalCode::ContentUnavailable
    );
    assert!(run::within_horizon(NOW, RETAIN).is_ok());
    assert!(!run::searchable(nostr::run::RECORD_KIND));
    let hidden = caller.signer.sign(
        NOW,
        nostr::run::RECORD_KIND,
        vec![
            Tag::new(vec!["p".into(), worker.pubkey.clone()]),
            Tag::new(vec!["h".into(), "ab".repeat(32)]),
            Tag::new(vec!["t".into(), nostr::run::MARKER.into()]),
        ],
        "ciphertext".into(),
    );
    let mut stranger = HashSet::new();
    stranger.insert(other.pubkey.clone());
    assert!(!run::record_visible(&hidden, &stranger));
    let mut recipient = HashSet::new();
    recipient.insert(worker.pubkey.clone());
    assert!(run::record_visible(&hidden, &recipient));

    thread::scope(|scope| {
        scope.spawn(|| {
            let ledger = left.lock().unwrap();
            assert_eq!(ledger.worker, worker.pubkey);
            assert_eq!(ledger.active, 0);
        });
        scope.spawn(|| {
            let ledger = right.lock().unwrap();
            assert_eq!(ledger.worker, other.pubkey);
            assert_eq!(ledger.capacity, 1);
        });
    });
}

/// The turn the terminal and `coder --print` both run, recorded once.
#[tokio::test]
async fn the_terminal_and_headless_turn_leave_one_trace() {
    let logs = tempfile::tempdir().unwrap();
    let recorder = Recorder::open(logs.path(), "stub", "stub", "repo").unwrap();
    let mut agent = Agent::new(None, Door::Stub(StubGenerate::saying("counted the crates")))
        .with_trace(Some(recorder));
    let finished = turn::run(&mut agent, "count the crates".to_string(), &mut |_| {})
        .await
        .expect("the shared turn finishes");
    assert_eq!(finished.reply, "counted the crates");
    drop(agent);
    let path = std::fs::read_dir(logs.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let steps = atif::log::read(&path).unwrap().steps;
    let messages = steps
        .iter()
        .map(|step| step.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("count the crates")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("counted the crates")),
        "{messages:?}"
    );
}
