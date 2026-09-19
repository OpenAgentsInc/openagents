//! Revocation, proved against a door that is running.
//!
//! The four steps of #9390, in order, each against a real `lev-serve` router
//! bound to a real port and asked over HTTP by a real client:
//!
//! 1. Publish the manifest with a `policySnapshot` and a freshness window.
//! 2. Mark the release revoked.
//! 3. Show a running door stop serving that family.
//! 4. Show a door that never reaches the service again stop within the
//!    window regardless.
//!
//! The fourth is the one that distinguishes a revocation mechanism from a
//! check, and it is the reason the door reads a [`Clock`] it can be given: the
//! guarantee is about a day passing, and a test that waited a day would not
//! be run.
//!
//! **No device.** Every door here is built over [`Pool::none`], so nothing
//! reaches Apple's runtime and nothing here measures it. That is deliberate
//! twice over. It keeps the suite runnable on any machine, and it proves the
//! stronger property: a revoked release refuses for the reason it was revoked
//! rather than for whatever the device happened to say, because the policy is
//! asked before the runtime is. A request that the policy allows through
//! reaches the empty pool and comes back `model_unavailable`, and that
//! refusal is how these tests read "the policy let this one past".

#![cfg(feature = "serve")]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use gym::calibrate::{EstimatorConfig, Map, Metrics, Observation, RECORD_SCHEMA, Record};
use gym::row::DoorIdentity;

use lev::adapter::{Metadata, Package, write_package};
use lev::bridge::Pool;
use lev::manifest::{
    Artifact, Base, EvalRef, Interface, MANIFEST_SCHEMA, Manifest, digest_of,
};
use lev::policy::{Clock, DEFAULT_WINDOW_SECONDS, Revocation, Snapshot, SnapshotRef};
use lev::serve::Door;

/// A moment the fixed clock starts at. Any moment; the arithmetic is what
/// matters.
const NOW: i64 = 1_789_000_000;
const BASE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";
const OS_BUILD: &str = "25E246";
const RELEASE: &str = "lev-adapted@1";
const WINDOW: i64 = DEFAULT_WINDOW_SECONDS as i64;

/// A released door, its service, and the clock both read.
struct Lane {
    dir: tempfile::TempDir,
    clock: Clock,
    base_url: String,
}

impl Lane {
    /// Publishes `snapshot` to the canonical service.
    ///
    /// This is the publishing half. It writes where the door fetches from,
    /// and never where the door loads from: a door picks a published
    /// snapshot up by fetching it, which is what [`Lane::fetch`] does.
    fn publish(&self, snapshot: &Snapshot) {
        std::fs::write(
            self.dir.path().join("service.json"),
            snapshot.to_json().expect("the snapshot encodes"),
        )
        .expect("the service publishes");
    }

    /// Fetches the published snapshot into the door's cache.
    fn fetch(&self) -> Result<String, lev::policy::Trouble> {
        policy(self.dir.path()).fetch()
    }

    /// Asks one Choice, naming a family or naming none.
    async fn ask(&self, family: Option<&str>) -> (u16, Value) {
        let mut body = json!({
            "state": "the printer will not connect",
            "questions": {
                "q": {
                    "type": "choice",
                    "criteria": {"billing": null, "technical": null, "account": null},
                },
            },
        });
        if let Some(family) = family {
            body["extensions"] = json!({"family": family});
        }
        let response = reqwest::Client::new()
            .post(format!("{}/v1/systemone", self.base_url))
            .json(&body)
            .send()
            .await
            .expect("the door answers");
        let status = response.status().as_u16();
        (status, response.json().await.expect("the answer is JSON"))
    }

    /// Reads the door's model card.
    async fn card(&self) -> Value {
        let card: Value = reqwest::get(format!("{}/v1/models", self.base_url))
            .await
            .expect("the door answers")
            .json()
            .await
            .expect("the card is JSON");
        card["models"][0].clone()
    }
}

/// The refusal code in a door's answer, or `"none"` when it answered.
fn code(body: &Value) -> String {
    body["error"]["code"].as_str().unwrap_or("none").to_string()
}

fn message(body: &Value) -> String {
    body["error"]["message"].as_str().unwrap_or_default().to_string()
}

/// The policy for the release written into `dir`.
fn policy(dir: &Path) -> lev::policy::Policy {
    Manifest::load(dir.join("manifest.json")).expect("the manifest loads").policy()
}

/// Writes a package, a record, and a manifest, then starts a door over them.
///
/// Everything a released door needs and nothing it does not: one artifact,
/// one admitted calibration record for `routing`, and a `policySnapshot`
/// naming the service beside them.
async fn lane() -> Lane {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let clock = Clock::fixed(NOW);

    let package = dir.path().join("lev.fmadapter");
    let metadata = Metadata {
        adapter_identifier: "fmadapter-lev-9799725".to_string(),
        base_model_signature: BASE.to_string(),
        lora_rank: 32,
        author: None,
        description: None,
        license: None,
        draft_token_count: None,
        creator_defined: BTreeMap::new(),
    };
    write_package(&package, &metadata, &[(1, vec![7; 64])]).expect("the package writes");
    let package = Package::open(&package).expect("the package opens");

    let calibration = dir.path().join("calibration");
    std::fs::create_dir_all(&calibration).expect("a calibration directory");
    let record = record("routing");
    let record_path = calibration.join("routing.json");
    std::fs::write(
        &record_path,
        serde_json::to_string_pretty(&record).expect("the record encodes"),
    )
    .expect("the record writes");

    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_string(),
        name: "lev-adapted".to_string(),
        version: 1,
        description: "a released decision model, under a policy".to_string(),
        created: "2026-09-19".to_string(),
        artifact: Some(Artifact::of_package(&package).expect("the package describes")),
        base: Base {
            signature: BASE.to_string(),
            min_os_build: OS_BUILD.to_string(),
            runtime: "Apple FoundationModels".to_string(),
        },
        policy_snapshot: SnapshotRef {
            source: "service.json".to_string(),
            cache: dir.path().join("cache/current.json").display().to_string(),
            freshness_window_seconds: DEFAULT_WINDOW_SECONDS,
        },
        interface: Interface::of_contract(vec!["routing".to_string()]),
        estimator: EstimatorConfig::new("l2", 8, 0),
        eval_ref: vec![EvalRef::of_record(
            &record,
            "calibration/routing.json",
            digest_of(&record_path).expect("the record hashes"),
        )],
        source: dir.path().to_path_buf(),
    };
    std::fs::write(
        dir.path().join("manifest.json"),
        manifest.to_json().expect("the manifest encodes"),
    )
    .expect("the manifest writes");

    let lane = Lane { dir, clock: clock.clone(), base_url: String::new() };
    lane.publish(&Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS));
    lane.fetch().expect("the first snapshot fetches");

    // Loaded from the file rather than used from memory, because the
    // `policySnapshot` block has to survive a round trip through the document
    // for any of this to be a property of the release.
    let manifest = Manifest::load(lane.dir.path().join("manifest.json")).expect("it loads");
    manifest.check_eval_refs().expect("the record is the one the release names");
    let door = Door::new(Pool::none(), manifest.name.clone(), 8)
        .with_manifest(manifest)
        .with_os_build(OS_BUILD)
        .with_calibration(lane.dir.path().join("calibration"))
        .with_clock(clock.clone());
    assert_eq!(door.calibration().families(), vec!["routing"], "the release serves its map");

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let door = Arc::new(door);
    tokio::spawn(async move {
        let _ = axum::serve(listener, door.router()).await;
    });
    Lane { base_url: format!("http://127.0.0.1:{port}"), ..lane }
}

/// A record fitted against the door this lane starts.
fn record(family: &str) -> Record {
    Record {
        schema: RECORD_SCHEMA.to_string(),
        family: family.to_string(),
        estimator_config: EstimatorConfig::new("l2", 8, 0),
        language: "en".to_string(),
        suite: "support-v2-three-way".to_string(),
        suite_digest: "54fbf4137c".to_string(),
        partition_id: "calibration".to_string(),
        os_build: OS_BUILD.to_string(),
        door: "lev-adapted".to_string(),
        door_identity: DoorIdentity::published("lev-adapted", BASE, RELEASE),
        gate_id: Some("probability-v1".to_string()),
        gate_digest: Some("gate:abc".to_string()),
        locked_reads: Vec::new(),
        fitted: "2026-09-19".to_string(),
        map: Map::fit(&[Observation::new(1.0, true), Observation::new(1.0, false)], 2),
        raw_metrics: Metrics::default(),
        calibrated_metrics: Metrics::default(),
        admitted: true,
        verdict: "admitted: a test".to_string(),
    }
}

/// Step 1. The release names a service, a cache, and a window, and a door
/// that has fetched a current snapshot serves.
#[tokio::test]
async fn a_released_door_under_a_current_snapshot_serves() {
    let lane = lane().await;

    let card = lane.card().await;
    assert_eq!(card["policy"]["state"], "current");
    assert_eq!(card["policy"]["freshness_window_seconds"], DEFAULT_WINDOW_SECONDS);
    assert_eq!(card["serving"], json!(["routing"]));
    assert!(
        card["policy"]["snapshot_sha256"].as_str().is_some_and(|digest| digest.len() == 64),
        "the card names the snapshot it is serving under: {card}"
    );

    // The policy let this past, and the empty pool is what stopped it. That
    // is the whole assertion: `model_unavailable` here means the question
    // reached the runtime, which a revoked or stale door's question does not.
    let (status, body) = lane.ask(Some("routing")).await;
    assert_eq!(code(&body), "model_unavailable", "{body}");
    assert_eq!(status, 503);
}

/// Steps 2 and 3. A revocation published against a door that is already
/// running stops it, with no restart and no second look at the manifest.
#[tokio::test]
async fn a_running_door_stops_serving_a_revoked_family() {
    let lane = lane().await;
    let (_, before) = lane.ask(Some("routing")).await;
    assert_eq!(code(&before), "model_unavailable", "the door starts out serving");

    // The service withdraws one family of the release. Nothing restarts;
    // nothing about the door's own files changes.
    lane.publish(
        &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(
            Revocation::of(RELEASE, "the routing map was refitted and lost its gate", NOW)
                .for_families(vec!["routing".to_string()]),
        ),
    );
    lane.fetch().expect("the revocation fetches");

    let (status, body) = lane.ask(Some("routing")).await;
    assert_eq!(status, 410, "{body}");
    assert_eq!(code(&body), "revoked", "{body}");
    assert!(message(&body).contains("refitted"), "the refusal carries the reason: {body}");

    // A family the revocation did not name still reaches the runtime, which
    // is what "that family" in the issue means: a family-scoped revocation
    // takes one thing away rather than stopping the door.
    let (_, other) = lane.ask(Some("severity")).await;
    assert_eq!(code(&other), "model_unavailable", "{other}");

    let card = lane.card().await;
    assert_eq!(card["policy"]["state"], "revoked");
    assert_eq!(card["serving"], json!([]), "the card stops listing the family");
    assert_eq!(card["policy"]["revoked"][0]["families"], "routing");
    assert_eq!(
        card["policy"]["detail"],
        Value::Null,
        "the release itself still serves, and the card says so: {card}"
    );
}

/// Step 2 and 3 again, aimed the way the treadmill arrives.
///
/// An operating system update replaces the base and nobody lists the releases
/// fitted against the old signature, because the update did not either. One
/// entry naming `base.signature` stops all of them.
#[tokio::test]
async fn revoking_a_base_signature_stops_the_whole_release() {
    let lane = lane().await;
    lane.publish(&Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(Revocation::of_base(
        "9799725",
        "25E246 replaced the base model, and every map fitted against it is invalid",
        NOW,
    )));
    lane.fetch().expect("the revocation fetches");

    for family in [Some("routing"), Some("severity"), None] {
        let (status, body) = lane.ask(family).await;
        assert_eq!(status, 410, "{family:?} survived a base revocation: {body}");
        assert_eq!(code(&body), "revoked", "{body}");
    }
    assert!(message(&lane.ask(None).await.1).contains("replaced the base"));
}

/// Step 4, which is the whole point.
///
/// The door never hears from the service again. Nothing is published, nothing
/// is fetched, and the door's own files are untouched. It stops anyway, at
/// the window, because a client that cannot confirm its policy does not go on
/// serving a managed release. The maximum enforcement delay is the window.
#[tokio::test]
async fn a_door_that_never_reaches_the_service_again_stops_within_the_window() {
    let lane = lane().await;
    let (_, before) = lane.ask(Some("routing")).await;
    assert_eq!(code(&before), "model_unavailable", "the door starts out serving");

    // The service goes away entirely — not unreachable, gone. A fetch from
    // here on fails, and failing changes nothing about when the door stops.
    std::fs::remove_file(lane.dir.path().join("service.json")).expect("the service goes away");
    assert!(lane.fetch().is_err(), "the service is gone");

    lane.clock.advance(WINDOW);
    let (_, inside) = lane.ask(Some("routing")).await;
    assert_eq!(code(&inside), "model_unavailable", "the last second inside the window serves");

    lane.clock.advance(1);
    let (status, body) = lane.ask(Some("routing")).await;
    assert_eq!(status, 503, "{body}");
    assert_eq!(code(&body), "policy_stale", "{body}");
    assert!(message(&body).contains("86400"), "the refusal names the window: {body}");

    // Every family, and a request naming none. A stale snapshot stops the
    // managed release outright rather than family by family: the door cannot
    // say that any part of it is still allowed to serve.
    for family in [Some("routing"), Some("severity"), None] {
        let (_, body) = lane.ask(family).await;
        assert_eq!(code(&body), "policy_stale", "{family:?} outlived the window: {body}");
    }

    let card = lane.card().await;
    assert_eq!(card["policy"]["state"], "stale");
    assert_eq!(card["serving"], json!([]));
    assert_eq!(
        card["calibrated_families"],
        json!(["routing"]),
        "the map is still fitted and still matches; it is the policy that stopped it"
    );
    assert!(card["policy"]["expires_in_seconds"].as_i64().is_some_and(|left| left < 0));

    // And the guarantee stated the way the issue asks for it: a revocation
    // published the instant after this door's snapshot was issued would have
    // been enforced by now, having never reached the door.
    assert!(lane.clock.now() - NOW <= WINDOW + 1);
}

/// The complement, which matters as much: deleting the cache must not turn a
/// managed release into an unmanaged local one.
#[tokio::test]
async fn deleting_the_cache_does_not_unmanage_a_running_door() {
    let lane = lane().await;
    let (_, before) = lane.ask(Some("routing")).await;
    assert_eq!(code(&before), "model_unavailable", "the door starts out serving");

    std::fs::remove_file(lane.dir.path().join("cache/current.json")).expect("the cache deletes");

    let (status, body) = lane.ask(Some("routing")).await;
    assert_eq!(status, 503, "{body}");
    assert_eq!(code(&body), "policy_stale", "{body}");
    assert!(message(&body).contains("deleting the cache"), "{body}");

    let card = lane.card().await;
    assert_eq!(card["policy"]["state"], "absent");
    assert_eq!(card["manifest"]["release"], RELEASE, "the release is still what it was");
    assert_eq!(card["serving"], json!([]));

    // And it comes back by fetching, rather than by the door forgetting it
    // was managed.
    lane.fetch().expect("the snapshot fetches again");
    let (_, after) = lane.ask(Some("routing")).await;
    assert_eq!(code(&after), "model_unavailable", "{after}");
}

/// A door with no manifest is not a managed release, and says so.
///
/// This is the boundary the mechanism draws. A manifest is what grants a
/// family, so it is also what puts a door under a policy; a door started
/// without one admits nothing, serves no probability, and has nothing for a
/// revocation to take away.
#[tokio::test]
async fn an_unreleased_door_carries_no_policy_and_admits_nothing() {
    let door = Arc::new(Door::new(Pool::none(), "lev-base", 8));
    assert!(door.policy().is_none());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.expect("a port");
    let port = listener.local_addr().expect("an address").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, door.router()).await;
    });

    let card: Value = reqwest::get(format!("http://127.0.0.1:{port}/v1/models"))
        .await
        .expect("the door answers")
        .json()
        .await
        .expect("the card is JSON");
    assert_eq!(card["models"][0]["policy"], Value::Null);
    assert_eq!(card["models"][0]["manifest"], Value::Null);
    assert_eq!(card["models"][0]["serving"], json!([]));
}
