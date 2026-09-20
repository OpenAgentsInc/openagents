//! The admission floor, proved against a fake runtime.
//!
//! [`Gate::run`] is the one function that says whether a door may serve, and
//! every step it runs is exercised here without Apple's runtime: a shell
//! script speaks the helper's line protocol, the way `supervision.rs` fakes
//! the helper's faults. The fake is told what base signature to report and
//! whether it leaks: a leaking helper remembers a token it saw in one session
//! and names it in the next, which is exactly what step 3 exists to catch.
//!
//! What this proves and what it does not. Steps 1, 2, and 4, the decision
//! logic behind them, and the probe's arithmetic are checked here in full.
//! Whether Apple's runtime isolates sessions is a fact about a Mac, measured
//! by `isolation.rs` with the helper built; this file shows that a runtime
//! that did not isolate would be refused.

#![cfg(feature = "serve")]

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use gym::calibrate::{EstimatorConfig, Map, Metrics, Observation, RECORD_SCHEMA, Record};
use gym::row::DoorIdentity;

use lev::adapter::{Metadata, Package, write_package};
use lev::admission::{Denied, Gate, Isolation, Step};
use lev::bridge::Pool;
use lev::manifest::{Artifact, Base, EvalRef, Interface, MANIFEST_SCHEMA, Manifest, digest_of};
use lev::policy::{Clock, DEFAULT_WINDOW_SECONDS, Snapshot, SnapshotRef};
use lev::serve::Door;

const NOW: i64 = 1_789_000_000;
const BASE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";
const OS_BUILD: &str = "25E246";
const RELEASE: &str = "lev-adapted@1";
/// The token the probe plants. It is the probe's own, and the fake only has
/// to recognise it.
const TOKEN: &str = "FALCON7";

/// The fake helper, with `@SIGNATURE@`, `@MODE@`, and `@STATE@` filled in
/// per instance so the tests can run in parallel.
///
/// A decide line carries the session's instructions, the state as its
/// prompt, and the options. The fake answers the first option unless the
/// state names the token and the options offer it, in which case it answers
/// the token: a runtime that reads its own session and nothing else. In
/// `leaky` mode it also notices when a session tells it the token without
/// asking about it, and for the next eight calls names the token whenever
/// it is offered, whatever those calls' own state says. The pool these
/// tests start has one lane, so the sibling arm's draws all reach the helper
/// the planted question did.
const FAKE: &str = r#"#!/bin/sh
id_of() { printf '%s' "$1" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p'; }
field() { printf '%s' "$2" | sed -n 's/.*"'"$1"'":"\([^"]*\)".*/\1/p'; }
first_option() { printf '%s' "$1" | sed -n 's/.*"options":\["\([^"]*\)".*/\1/p'; }
offers_token() { printf '%s' "$1" | sed -n 's/.*"options":\(\[[^]]*\]\).*/\1/p' | grep -q '"@TOKEN@"'; }
told_token() { printf '%s' "$(field instructions "$1") $(field prompt "$1")" | grep -q '@TOKEN@'; }
while IFS= read -r line; do
    id=$(id_of "$line")
    case "$line" in
        *'"op":"availability"'*)
            printf '{"id":"%s","ok":true,"availability":{"status":"available"}}\n' "$id"
            continue ;;
        *'"op":"adapter_compat"'*)
            printf '{"id":"%s","ok":true,"compatibleAdapters":["fmadapter-lev-@SIGNATURE@"]}\n' "$id"
            continue ;;
    esac
    choice=$(first_option "$line")
    if field prompt "$line" | grep -q '@TOKEN@' && offers_token "$line"; then
        choice=@TOKEN@
    elif [ "@MODE@" = leaky ]; then
        if told_token "$line" && ! offers_token "$line"; then
            echo 8 > "@STATE@"
        elif [ -s "@STATE@" ] && offers_token "$line"; then
            left=$(cat "@STATE@")
            if [ "$left" -gt 0 ]; then
                choice=@TOKEN@
                echo $((left - 1)) > "@STATE@"
            fi
        fi
    fi
    printf '{"id":"%s","ok":true,"choice":"%s","latencyMs":1}\n' "$id" "$choice"
done
"#;

/// A fake helper on disk, and the release it is asked to admit.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    /// Writes a helper that reports `signature` and behaves as `mode`, and a
    /// release pinned to [`BASE`] with one admitted record for `routing`.
    fn new(mode: &str, signature: &str) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let state = dir.path().join("leaked");
        let script = FAKE
            .replace("@TOKEN@", TOKEN)
            .replace("@SIGNATURE@", signature)
            .replace("@MODE@", mode)
            .replace("@STATE@", &state.display().to_string());
        let helper = dir.path().join("fake-helper");
        std::fs::write(&helper, script).expect("the fake helper writes");
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        // SAFETY: the tests here only ever set this variable, to this value.
        unsafe { std::env::set_var("LEV_BRIDGE_ALLOW_UNSIGNED", "1") };

        let fixture = Self { dir };
        fixture.write_release();
        fixture
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn pool(&self) -> Pool {
        Pool::start(&self.path().join("fake-helper"), 1, Duration::from_secs(5))
            .expect("the fake helper starts")
    }

    fn manifest(&self) -> Manifest {
        Manifest::load(self.path().join("manifest.json")).expect("the manifest loads")
    }

    fn gate(&self) -> Gate {
        Gate::new(self.manifest())
            .with_os_build(OS_BUILD)
            .with_calibration(self.path().join("calibration"))
            .with_probe_seeds(8)
    }

    fn write_release(&self) {
        let package = self.path().join("lev.fmadapter");
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

        let calibration = self.path().join("calibration");
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
            description: "a released decision model, under the floor".to_string(),
            created: "2026-09-20".to_string(),
            artifact: Some(Artifact::of_package(&package).expect("the package describes")),
            base: Base {
                signature: BASE.to_string(),
                min_os_build: OS_BUILD.to_string(),
                runtime: "Apple FoundationModels".to_string(),
            },
            policy_snapshot: SnapshotRef {
                source: "service.json".to_string(),
                cache: self.path().join("cache/current.json").display().to_string(),
                freshness_window_seconds: DEFAULT_WINDOW_SECONDS,
            },
            // The release serves two families and fitted a record for one.
            interface: Interface::of_contract(vec!["routing".to_string(), "sentiment".to_string()]),
            estimator: EstimatorConfig::new("l2", 8, 0),
            eval_ref: vec![EvalRef::of_record(
                &record,
                "calibration/routing.json",
                digest_of(&record_path).expect("the record hashes"),
            )],
            source: self.path().to_path_buf(),
        };
        std::fs::write(
            self.path().join("manifest.json"),
            manifest.to_json().expect("the manifest encodes"),
        )
        .expect("the manifest writes");
        std::fs::write(
            self.path().join("service.json"),
            Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS)
                .to_json()
                .expect("the snapshot encodes"),
        )
        .expect("the service publishes");
        manifest.policy().fetch().expect("the snapshot fetches");
    }
}

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
        fitted: "2026-09-20".to_string(),
        map: Map::fit(
            &[Observation::new(1.0, true), Observation::new(1.0, false)],
            2,
        ),
        raw_metrics: Metrics::default(),
        calibrated_metrics: Metrics::default(),
        admitted: true,
        verdict: "admitted: a test".to_string(),
    }
}

fn denied(fixture: &Fixture) -> Denied {
    fixture
        .gate()
        .run(&fixture.pool())
        .err()
        .expect("the floor refuses")
}

/// Steps 1 to 3 pass against a runtime that reports the pinned signature and
/// keeps its sessions apart; step 4 finds the one record the release fitted.
#[test]
fn a_release_that_clears_the_floor_serves_its_fitted_families() {
    let fixture = Fixture::new("isolated", &BASE[..7]);
    let floor = fixture
        .gate()
        .run(&fixture.pool())
        .expect("the release clears the floor");
    let Isolation {
        sibling,
        absent,
        state,
        ..
    } = *floor.isolation();
    assert!(
        (sibling - absent).abs() <= Isolation::MARGIN,
        "sibling {sibling} against absent {absent}"
    );
    assert!(state >= Isolation::FLOOR, "state {state}");
    assert_eq!(floor.calibration().families(), vec!["routing"]);
    assert!(floor.fitted("routing").is_some());
    assert!(floor.fitted("sentiment").is_none(), "no record was fitted");
    assert!(floor.fitted("").is_none(), "no family names no record");
}

/// Step 1. A package whose bytes are not the ones the manifest names does not
/// start, and the refusal says which step said so.
#[test]
fn a_changed_artifact_fails_step_one() {
    let fixture = Fixture::new("isolated", &BASE[..7]);
    let package = fixture.path().join("lev.fmadapter");
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
    write_package(&package, &metadata, &[(1, vec![8; 64])]).expect("the package rewrites");

    let denied = denied(&fixture);
    assert_eq!(denied.step(), Step::Digest);
    assert!(
        denied
            .to_string()
            .starts_with("admission step 1 (digest) failed"),
        "{denied}"
    );
}

/// Step 1 also covers the records: an `evalRef` whose bytes changed is the
/// same failure.
#[test]
fn a_changed_record_fails_step_one() {
    let fixture = Fixture::new("isolated", &BASE[..7]);
    let path = fixture.path().join("calibration/routing.json");
    let mut record: Record =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the record reads"))
            .expect("the record decodes");
    record.verdict = "admitted: edited after the manifest was written".to_string();
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&record).expect("the record encodes"),
    )
    .expect("the record rewrites");

    let denied = denied(&fixture);
    assert_eq!(denied.step(), Step::Digest);
}

/// Step 2. The runtime reports a signature that is not the pinned one.
#[test]
fn a_different_base_signature_fails_step_two() {
    let fixture = Fixture::new("isolated", "0badbad");
    let denied = denied(&fixture);
    assert_eq!(denied.step(), Step::BaseSignature);
    assert!(
        denied
            .to_string()
            .starts_with("admission step 2 (base signature) failed"),
        "{denied}"
    );
    assert!(denied.to_string().contains("0badbad"), "{denied}");
}

/// Step 3. A runtime that carries one session's text into the next names the
/// planted token in the sibling arm and not in the control, and does not
/// start.
#[test]
fn a_leaking_runtime_fails_step_three() {
    let fixture = Fixture::new("leaky", &BASE[..7]);
    let denied = denied(&fixture);
    assert_eq!(denied.step(), Step::Isolation);
    let Denied::Isolation { report, reason } = &denied else {
        panic!("expected an isolation denial, got {denied}");
    };
    assert!(
        report.sibling > report.absent + Isolation::MARGIN,
        "the leak shows in the sibling arm: {report}"
    );
    assert!(reason.contains("sibling"), "{reason}");
    assert!(
        denied
            .to_string()
            .starts_with("admission step 3 (isolation) failed"),
        "{denied}"
    );
}

/// Step 4, and the one rule behind it, against a door built from the floor
/// and asked over HTTP.
#[tokio::test]
async fn an_unfitted_family_serves_without_probabilities_and_refuses_when_asked() {
    let fixture = Fixture::new("isolated", &BASE[..7]);
    let floor = tokio::task::spawn_blocking({
        let gate = fixture.gate();
        let pool = fixture.pool();
        move || gate.run(&pool).expect("the release clears the floor")
    })
    .await
    .expect("the gate ran");
    let door = Door::new(fixture.pool(), "lev-adapted", 8)
        .with_floor(floor)
        .with_os_build(OS_BUILD)
        .with_clock(Clock::fixed(NOW));

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("a port");
    let port = listener.local_addr().expect("an address").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, Arc::new(door).router()).await;
    });
    let base_url = format!("http://127.0.0.1:{port}");

    // The card says what the floor measured.
    let card: Value = reqwest::get(format!("{base_url}/v1/models"))
        .await
        .expect("the door answers")
        .json()
        .await
        .expect("the card is JSON");
    let card = &card["models"][0];
    assert_eq!(card["serving"], json!(["routing"]), "{card}");
    assert!(card["isolation"]["state"].is_number(), "{card}");

    // The fitted family serves a probability.
    let (status, body) = ask(&base_url, json!({"family": "routing"})).await;
    assert_eq!(status, 200, "{body}");
    let answer = &body["answers"]["q"];
    assert_eq!(answer["type"], "choice");
    assert!(answer["probabilities"].is_object(), "{body}");
    assert!(answer["confidence"].is_number(), "{body}");

    // The unfitted one serves the typed answer and nothing behind it.
    let (status, body) = ask(&base_url, json!({"family": "sentiment"})).await;
    assert_eq!(status, 200, "{body}");
    let answer = body["answers"]["q"].as_object().expect("an answer");
    assert_eq!(answer["type"], "choice");
    assert_eq!(
        answer["choice"], "billing",
        "the fake picks the first option"
    );
    assert!(!answer.contains_key("probabilities"), "{body}");
    assert!(!answer.contains_key("confidence"), "{body}");
    assert_eq!(body["extensions"]["calibration"]["state"], "uncalibrated");
    assert_eq!(body["extensions"]["calibration"]["family"], "sentiment");

    // And refuses through the same rule when the caller insists.
    let (status, body) = ask(
        &base_url,
        json!({"family": "sentiment", "require_calibration": true}),
    )
    .await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"]["code"], "uncalibrated", "{body}");

    // A request that names no family asks for no map: the seeded frequency
    // is served, which is what a measurement run reads.
    let (status, body) = ask(&base_url, json!({})).await;
    assert_eq!(status, 200, "{body}");
    assert!(body["answers"]["q"]["probabilities"].is_object(), "{body}");
}

async fn ask(base_url: &str, extensions: Value) -> (u16, Value) {
    let body = json!({
        "state": "the printer will not connect",
        "questions": {
            "q": {
                "type": "choice",
                "criteria": {"billing": null, "technical": null, "account": null},
            },
        },
        "extensions": extensions,
    });
    let response = reqwest::Client::new()
        .post(format!("{base_url}/v1/systemone"))
        .json(&body)
        .send()
        .await
        .expect("the door answers");
    let status = response.status().as_u16();
    (status, response.json().await.expect("the answer is JSON"))
}

/// The probe's arithmetic, apart from any runtime.
#[test]
fn the_isolation_report_names_what_failed() {
    let passing = Isolation {
        seeds: 8,
        sibling: 0.25,
        absent: 0.25,
        state: 1.0,
    };
    assert_eq!(passing.fault(), None);
    let leaking = Isolation {
        sibling: 1.0,
        ..passing
    };
    assert!(
        leaking
            .fault()
            .is_some_and(|reason| reason.contains("sibling"))
    );
    let deaf = Isolation {
        state: 0.25,
        ..passing
    };
    assert!(deaf.fault().is_some_and(|reason| reason.contains("state")));
}
