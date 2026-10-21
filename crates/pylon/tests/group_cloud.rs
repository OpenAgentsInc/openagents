#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `pylon cloud up` for a V4.1 group (#12402), all on this computer.
//!
//! The real spend guard (`scripts/dev/cloud-budget.py`) runs with a private
//! ledger directory, home, and host name, and a fake gcloud that never
//! creates a machine. Each stage is W10's synthetic stage
//! (`fixtures/stage_worker.py`), run here instead of over ssh. The relay is
//! the in-process fixture. Nothing touches a production pylon, the owner's
//! home, or a cloud project.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nostr::pylon::{Beacon, GroupRole, Readiness, Status, parse_beacon};
use pylon::cloud::{self, Guard, Up};
use pylon::engine::{Sampling, Turn};
use pylon::group::Want;
use pylon::identity::Identity;
use pylon::router::{self, Decide, Deciding, Outcome, Question};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A private guard, fake cloud, stage directory, and pylon home.
struct Rig {
    dir: tempfile::TempDir,
    up: Up,
}

impl Rig {
    fn new(placement: &str, relay: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = |name: &str| dir.path().join(name);
        for name in ["home", "guard-home", "budget", "stages", "gcloud"] {
            std::fs::create_dir_all(path(name)).unwrap();
        }
        let script = path("stage.py");
        std::fs::write(&script, include_bytes!("fixtures/stage_worker.py")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut guard = Guard::new(&repo());
        guard.budget = "pylon-test".into();
        guard.env = [
            ("HOME", path("guard-home")),
            ("OA_CLOUD_BUDGET_DIR", path("budget")),
            ("OA_GCLOUD", fixture("fake_gcloud.py")),
            ("FAKE_GCLOUD_ROOT", path("gcloud")),
            ("OA_AGENT_RUN", fixture("fake_agent_run.py")),
            ("CLOUDSDK_CONFIG", path("gcloud")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.display().to_string()))
        .chain([("OA_CLOUD_BUDGET_HOST".into(), "pylon-test-host".into())])
        .collect();
        let mut up = Up::new(placement, &repo(), &path("home"));
        up.relay = relay.into();
        up.guard = guard;
        up.worker = Some("gs://pylon-test/stage-worker".into());
        up.local_stages = Some((
            script,
            vec!["--root".into(), path("stages").display().to_string()],
        ));
        up.stage_weights = Some("sha256:fixture".into());
        up.stage_runtime = "fixture-runtime/1".into();
        up.hours = 1.0;
        up.max_output_tokens = 6;
        up.sampling = Sampling::new(0.6, 0.95, 7).unwrap();
        up.every = Duration::from_millis(100);
        up.budget_every = Duration::from_millis(200);
        up.ready_timeout = Duration::from_secs(20);
        up.step_timeout = Duration::from_millis(1500);
        up.startup_timeout = Duration::from_secs(5);
        up.max_restarts = 2;
        Self { dir, up }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn home(&self) -> PathBuf {
        self.path("home")
    }

    /// What the fake gcloud was asked, as `verb name` lines.
    fn gcloud(&self, verb: &str) -> Vec<String> {
        std::fs::read_to_string(self.path("gcloud/calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Vec<String>>(l).ok())
            .filter(|args| args.get(2).map(String::as_str) == Some(verb))
            .map(|args| args[3].clone())
            .collect()
    }

    fn ledger(&self, event: &str) -> Vec<Value> {
        self.up
            .guard
            .records()
            .into_iter()
            .filter(|r| r["event"] == event)
            .collect()
    }

    fn stage_events(&self, kind: &str) -> Vec<Value> {
        std::fs::read_to_string(self.path("stages/events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|e| e["event"] == kind)
            .collect()
    }
}

/// The relay's latest beacon at each address.
async fn beacons(hub: &tokio::sync::Mutex<pylon::fixture::Hub>) -> Vec<Beacon> {
    hub.lock()
        .await
        .stored
        .iter()
        .filter(|e| e.kind == 30_200)
        .filter_map(|e| parse_beacon(e).ok())
        .collect()
}

async fn until<T>(what: &str, secs: u64, mut check: impl AsyncFnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(value) = check().await {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The group head and its stages, once the relay holds all of them.
async fn group(
    hub: &tokio::sync::Mutex<pylon::fixture::Hub>,
    stages: usize,
    ok: impl Fn(&Beacon, &[Beacon]) -> bool,
) -> (Beacon, Vec<Beacon>) {
    until("the group's beacons", 30, async || {
        let all = beacons(hub).await;
        let head = all.iter().find(|b| b.group_head().is_some())?.clone();
        let staged: Vec<Beacon> = all.into_iter().filter(Beacon::is_stage).collect();
        (staged.len() == stages && ok(&head, &staged)).then_some((head, staged))
    })
    .await
}

/// The Decisions API stand-in; a request that names a model never asks it.
struct Never;

impl Decide for Never {
    fn decide<'a>(&'a self, _: &'a Question) -> Deciding<'a> {
        Box::pin(async { Err("not asked".to_string()) })
    }
}

async fn ask(relay: &str, home: &Path, artifact: &str) -> router::Routed {
    let mut settings = router::Settings::new(relay, home.to_path_buf());
    settings.try_wait = Duration::from_secs(15);
    settings.want = Some(Want::Artifact(artifact.into()));
    router::route(
        &Identity::generate(),
        &settings,
        &Never,
        &[Turn {
            role: "user".into(),
            content: "What is 2 + 2?".into(),
        }],
        None,
    )
    .await
}

fn pids(home: &Path) -> Vec<u32> {
    pylon::online::load_live(home)
        .and_then(|l| l.group)
        .map(|g| g.stages.iter().filter_map(|s| s.pid).collect())
        .unwrap_or_default()
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_launches_answers_through_the_router_and_stops_on_down() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let rig = Rig::new("four-single", &relay);
    let launched = cloud::launch(&rig.up).await.unwrap();
    let receipt = launched.receipt.clone();
    eprintln!(
        "launch receipt: {}",
        serde_json::to_string_pretty(&receipt).unwrap()
    );

    // Four machines through the guard, each with a ledger entry.
    let names: Vec<String> = (0..4).map(|i| format!("oa-v41-four-single-m{i}")).collect();
    assert_eq!(rig.gcloud("create"), names);
    assert_eq!(receipt["machines"].as_array().unwrap().len(), 4);
    assert_eq!(receipt["ledger"]["starts"].as_array().unwrap().len(), 4);
    assert_eq!(rig.ledger("start").len(), 4);
    for start in rig.ledger("start") {
        assert_eq!(start["machine_type"], "g4-standard-48");
        assert_eq!(start["provisioning"], "spot");
        assert!(start["price_per_hour"].as_f64().unwrap() > 1.0);
    }
    let artifact = receipt["group"]["artifact"].as_str().unwrap().to_string();
    assert_eq!(receipt["group"]["generation"], 1);
    let head_address = receipt["group"]["head"].as_str().unwrap().to_string();

    // One ready head with one slot and the declared limits, and four
    // stage beacons with their layers.
    let (head, stages) = group(&hub, 4, |head, stages| {
        head.group_head().unwrap().readiness == Some(Readiness::Ready)
            && stages.iter().all(|s| s.status == Status::Online)
    })
    .await;
    assert_eq!(head.address(), head_address);
    let declared = head.group_head().unwrap();
    assert_eq!(head.slots.total, 1);
    assert_eq!(declared.deadline_secs, Some(90));
    assert_eq!(declared.max_output_tokens, Some(6));
    assert_eq!(declared.artifact, artifact);
    let mut layers: Vec<(u32, u32)> = stages
        .iter()
        .map(|s| {
            let g = s.group.as_ref().unwrap();
            assert_eq!(g.role, GroupRole::Stage);
            (g.layers.unwrap().from, g.layers.unwrap().to)
        })
        .collect();
    layers.sort_unstable();
    assert_eq!(layers, [(0, 10), (10, 20), (20, 30), (30, 40)]);

    // The router sends a request for V4.1 to the head; the last stage's
    // tokens come back in order.
    let asker = tempfile::tempdir().unwrap();
    let routed = ask(&relay, asker.path(), &artifact).await;
    let Outcome::Answered(answer) = &routed.outcome else {
        panic!("no answer: {:?} {:?}", routed.outcome, routed.failed);
    };
    assert!(answer.pylon.contains(&head.provider));
    assert_eq!(
        answer.text.as_deref(),
        Some("<7000><7001><7002><7003><7004><7005>")
    );
    let prompts = rig.stage_events("prompt");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0]["stage"], "s0");
    assert_eq!(prompts[0]["prompt"][0]["content"], "What is 2 + 2?");

    // `down` drains the group, drops its beacons, and deletes every
    // machine through the guard.
    let stage_pids = pids(&rig.home());
    assert_eq!(stage_pids.len(), 4);
    let home = rig.home();
    let up = rig.up.clone();
    let serving = tokio::spawn(async move {
        launched
            .serve(&home, &up, std::future::pending::<()>())
            .await
    });
    let down = cloud::down_at(&rig.home(), Duration::from_secs(60))
        .await
        .unwrap();
    let ended = serving.await.unwrap();
    eprintln!(
        "stop receipt: {}",
        serde_json::to_string_pretty(&ended).unwrap()
    );
    assert_eq!(ended["reason"], "pylon cloud down");
    assert_eq!(down["ended"]["reason"], "pylon cloud down");
    assert_eq!(rig.gcloud("delete"), names);
    let stops = rig.ledger("stop");
    assert_eq!(stops.len(), 4);
    assert!(stops.iter().all(|s| s["reason"] == "deleted"));
    assert_eq!(ended["stops"].as_array().unwrap().len(), 4);
    assert!(ended["cost"].as_f64().unwrap() > 0.0);
    let (head, stages) = group(&hub, 4, |head, stages| {
        head.status == Status::Offline && stages.iter().all(|s| s.status == Status::Offline)
    })
    .await;
    assert_eq!(head.slots.free, 0);
    assert_eq!(stages.len(), 4);
    assert!(stage_pids.iter().all(|&pid| !alive(pid)));
    let record = cloud::load(&rig.home()).unwrap();
    assert_eq!(record.state, "stopped");
    assert!(record.machines.iter().all(|m| m.deleted));
    // Nothing is left: a second `down` says so.
    let again = cloud::down_at(&rig.home(), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(again["already"], true);
}

#[test]
fn dry_runs_print_the_guard_and_gcloud_commands_for_each_placement() {
    for (placement, machines, machine_type, disk) in [
        ("four-single", 4, "g4-standard-48", "350GB"),
        ("two-dual", 2, "g4-standard-96", "600GB"),
    ] {
        let rig = Rig::new(placement, "wss://relay.example");
        let mut up = rig.up.clone();
        up.worker = None;
        let plan = cloud::plan(&up).unwrap();
        let value = cloud::dry_run(&up, &plan).unwrap();
        eprintln!(
            "dry run {placement}: {}",
            serde_json::to_string_pretty(&value).unwrap()
        );
        let rows = value["machines"].as_array().unwrap();
        assert_eq!(rows.len(), machines);
        for (i, row) in rows.iter().enumerate() {
            let name = format!("oa-v41-{placement}-m{i}");
            let guard = row["guard"].as_str().unwrap();
            assert!(guard.contains(&format!(
                "cloud-budget.py create {name} --machine-type {machine_type} --zone us-east1-b --hours 1"
            )));
            assert!(
                guard
                    .contains("--spot --budget pylon-test -- --metadata-from-file=startup-script=")
            );
            let gcloud = row["gcloud"].as_str().unwrap();
            assert!(gcloud.starts_with(&format!("gcloud compute instances create {name}")));
            for flag in [
                "--provisioning-model=SPOT",
                "--max-run-duration=3600s",
                "--instance-termination-action=DELETE",
                "budget=pylon-test",
                "issue=12402",
                "pylon-group=v41-",
                "oa-worker=unset",
                "scripts/pylon/v41-stage-startup.sh",
                &format!("--boot-disk-size={disk}"),
            ] {
                assert!(gcloud.contains(flag), "{flag} not in {gcloud}");
            }
        }
        let layers: Vec<String> = rows
            .iter()
            .map(|r| {
                let gcloud = r["gcloud"].as_str().unwrap();
                let at = gcloud.find("oa-layers=").unwrap() + 10;
                gcloud[at..].split(',').next().unwrap().to_string()
            })
            .collect();
        match placement {
            "four-single" => assert_eq!(layers, ["0-10", "10-20", "20-30", "30-40"]),
            _ => assert_eq!(layers, ["0-10+10-20", "20-30+30-40"]),
        }
        // A dry run creates and records nothing.
        assert!(rig.gcloud("create").is_empty());
        assert!(rig.ledger("start").is_empty());
    }
}

#[tokio::test]
async fn refusals_create_nothing() {
    let refused = async |rig: &Rig| {
        let why = match cloud::launch(&rig.up).await {
            Ok(_) => panic!("launched"),
            Err(why) => why,
        };
        assert!(rig.gcloud("create").is_empty(), "{why}");
        assert!(rig.ledger("start").is_empty(), "{why}");
        assert!(!rig.home().join("share.json").exists(), "{why}");
        eprintln!("refused: {why}");
        why
    };

    // Four machines that each fit under the cap but not together.
    let mut rig = Rig::new("four-single", "wss://relay.example");
    rig.up.guard.cap = Some(10.0);
    rig.up.hours = 4.0;
    let why = refused(&rig).await;
    assert!(why.contains("over its $10.00 cap"), "{why}");
    // One machine over the cap on its own.
    rig.up.hours = 10.0;
    let why = refused(&rig).await;
    assert!(why.contains("> cap $10.00"), "{why}");

    // A machine type the guard has no price for.
    let mut rig = Rig::new("four-single", "wss://relay.example");
    rig.up.machine_type = Some("g4-standard-192".into());
    let why = refused(&rig).await;
    assert!(why.contains("price"), "{why}");

    // A placement that doesn't fit its machines.
    let mut rig = Rig::new("two-dual", "wss://relay.example");
    rig.up.machine_type = Some("g4-standard-48".into());
    let why = refused(&rig).await;
    assert!(why.contains("has 1 GPU"), "{why}");

    // A budget another host allocates for.
    let rig = Rig::new("four-single", "wss://relay.example");
    let ledger = rig.up.guard.ledger();
    std::fs::write(
        &ledger,
        format!(
            "{}\n",
            serde_json::json!({"event": "budget", "budget": "pylon-test", "cap": 3000.0,
                "at": "2026-10-20T00:00:00Z", "host": "other-host",
                "ledger": rig.path("budget").canonicalize().unwrap().join("pylon-test.jsonl")})
        ),
    )
    .unwrap();
    let why = refused(&rig).await;
    assert!(why.contains("host other-host allocates"), "{why}");

    // A second ledger path for the budget on this host.
    let rig = Rig::new("four-single", "wss://relay.example");
    let book = rig.path("guard-home/.openagents/cloud-budget-authority.json");
    std::fs::create_dir_all(book.parent().unwrap()).unwrap();
    std::fs::write(
        &book,
        r#"{"pylon-test": {"ledger": "/elsewhere/pylon-test.jsonl", "host": "pylon-test-host"}}"#,
    )
    .unwrap();
    let why = refused(&rig).await;
    assert!(why.contains("uses the ledger at /elsewhere"), "{why}");

    // A launch with no stage worker named.
    let mut rig = Rig::new("four-single", "wss://relay.example");
    rig.up.local_stages = None;
    rig.up.worker = None;
    let why = refused(&rig).await;
    assert!(why.contains("--worker"), "{why}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lost_stage_makes_the_group_unready_and_ends_it_visibly() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let rig = Rig::new("two-dual", &relay);
    let launched = cloud::launch(&rig.up).await.unwrap();
    let artifact = launched.receipt["group"]["artifact"]
        .as_str()
        .unwrap()
        .to_string();
    group(&hub, 4, |head, _| {
        head.group_head().unwrap().readiness == Some(Readiness::Ready)
    })
    .await;
    let home = rig.home();
    let up = rig.up.clone();
    let serving = tokio::spawn(async move {
        launched
            .serve(&home, &up, std::future::pending::<()>())
            .await
    });

    // Spot loss: stage s2's machine is gone, so its worker dies and can't
    // be started again.
    std::fs::write(rig.path("stages/always-die-s2"), "").unwrap();
    let s2 = pylon::online::load_live(&rig.home())
        .unwrap()
        .group
        .unwrap()
        .stages[2]
        .pid
        .unwrap();
    unsafe { libc::kill(-(s2 as i32), libc::SIGKILL) };

    // The group goes unready at a new generation and the stage's beacon
    // drops, so a request for V4.1 finds nothing rather than hanging.
    let (head, _) = group(&hub, 4, |head, stages| {
        head.group_head().unwrap().readiness != Some(Readiness::Ready)
            && stages
                .iter()
                .any(|s| s.pylon.ends_with("-s2") && s.status == Status::Offline)
    })
    .await;
    assert!(head.group_head().unwrap().generation >= 2);
    let asker = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let routed = ask(&relay, asker.path(), &artifact).await;
    assert!(matches!(routed.outcome, Outcome::Unavailable(_)));
    assert!(started.elapsed() < Duration::from_secs(10));

    // The restarts run out, the group stops for good, its beacons go
    // offline, and the machines are deleted.
    let ended = serving.await.unwrap();
    assert!(
        ended["reason"]
            .as_str()
            .unwrap()
            .starts_with("the group was lost"),
        "{ended}"
    );
    group(&hub, 4, |head, stages| {
        head.status == Status::Offline && stages.iter().all(|s| s.status == Status::Offline)
    })
    .await;
    assert_eq!(rig.gcloud("delete").len(), 2);
    assert_eq!(rig.ledger("stop").len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn expiry_and_the_spend_cap_end_the_group() {
    // Expiry: the guard gives each machine two minutes; the group ends
    // ahead of it.
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let mut rig = Rig::new("two-dual", &relay);
    rig.up.hours = 0.02;
    rig.up.expiry_margin = Duration::from_secs(117);
    let launched = cloud::launch(&rig.up).await.unwrap();
    let ended = launched
        .serve(&rig.home(), &rig.up, std::future::pending::<()>())
        .await;
    assert_eq!(ended["reason"], "the machines reached their expiry");
    assert_eq!(rig.gcloud("delete").len(), 2);
    group(&hub, 4, |head, _| head.status == Status::Offline).await;

    // The spend cap: a larger reserve leaves the budget below zero.
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let rig = Rig::new("two-dual", &relay);
    let launched = cloud::launch(&rig.up).await.unwrap();
    let reserve = std::process::Command::new("python3")
        .args(
            rig.up
                .guard
                .argv(&[
                    "reserve".into(),
                    "3000".into(),
                    "--why".into(),
                    "test".into(),
                ])
                .iter()
                .skip(1),
        )
        .envs(rig.up.guard.env.iter().map(|(k, v)| (k, v)))
        .output()
        .unwrap();
    assert!(reserve.status.success(), "{reserve:?}");
    let ended = launched
        .serve(&rig.home(), &rig.up, std::future::pending::<()>())
        .await;
    assert!(
        ended["reason"].as_str().unwrap().contains("passed its cap"),
        "{ended}"
    );
    assert_eq!(rig.gcloud("delete").len(), 2);
    group(&hub, 4, |head, _| head.status == Status::Offline).await;
}

#[test]
fn the_ssh_launcher_tunnels_the_control_connection_to_the_machine() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(repo().join("scripts/pylon/v41-stage-ssh.sh"))
        .args([
            "oa-v41-two-dual-m1",
            "us-east1-b",
            "openagentsgemini",
            "47102",
            "--",
            "--layers",
            "20-30",
            "--stage",
            "s2",
            "--generation",
            "3",
            "--front",
            "127.0.0.1:53111",
            "--token",
            "it's",
        ])
        .env("OA_GCLOUD", fixture("fake_gcloud.py"))
        .env("FAKE_GCLOUD_ROOT", dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "the fake refuses ssh");
    let calls = std::fs::read_to_string(dir.path().join("calls.jsonl")).unwrap();
    let args: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
    assert_eq!(
        args[..9],
        [
            "compute",
            "ssh",
            "oa-v41-two-dual-m1",
            "--zone",
            "us-east1-b",
            "--project",
            "openagentsgemini",
            "--quiet",
            "--"
        ]
    );
    assert!(args.contains(&"-R".into()));
    assert!(args.contains(&"127.0.0.1:47102:127.0.0.1:53111".into()));
    assert_eq!(
        args.last().unwrap(),
        "/opt/openagents/stage-run '--layers' '20-30' '--stage' 's2' '--generation' '3' '--front' '127.0.0.1:47102' '--token' 'it'\\''s'"
    );
}

#[test]
fn a_group_profile_is_version_6_settings_that_older_versions_refuse() {
    use pylon::share;
    let rig = Rig::new("two-dual", "wss://relay.example");
    let plan = cloud::plan(&rig.up).unwrap();
    let mut settings = share::Settings::off("v41-two-dual-head");
    settings.slots = 1;
    settings.group = Some(cloud::group_lane(&rig.up, &plan));
    let home = rig.home();
    share::save(&home, &settings).unwrap();
    let held = share::load(&home).unwrap().unwrap();
    assert_eq!(held.schema, share::GROUP_SCHEMA);
    let lane = held.group.as_ref().unwrap();
    assert_eq!(lane.stages.len(), 4);
    assert_eq!(lane.manifest, format!("sha256:{}", plan.placement.digest));
    lane.validate().unwrap();
    // A group under an older version, a group beside a text lane, and a
    // version this host doesn't know are all refused.
    for (schema, slots) in [
        (share::NATIVE_PROFILE_SCHEMA, 1),
        (share::GROUP_SCHEMA, 2),
        ("openagents.pylon.share.v7", 1),
    ] {
        let mut file = serde_json::to_value(&held).unwrap();
        file["schema"] = schema.into();
        file["slots"] = slots.into();
        std::fs::write(share::path(&home), serde_json::to_vec(&file).unwrap()).unwrap();
        assert!(share::load(&home).is_err(), "{schema} {slots}");
    }
    // A deadline past 90 seconds waits for W14 (#12401).
    let mut deadline = lane.clone();
    deadline.deadline_secs = 91;
    assert!(deadline.validate().unwrap_err().contains("90 seconds"));
}
