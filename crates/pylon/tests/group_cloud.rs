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
use pylon::cloud_head::HeadPlacement;
use pylon::engine::{Sampling, Turn};
use pylon::group::Want;
use pylon::identity::Identity;
use pylon::router::{self, Decide, Deciding, Outcome, Question};
use pylon::stage_group::Transport;
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
        std::fs::write(
            path("stage_link.py"),
            include_bytes!("fixtures/stage_link.py"),
        )
        .unwrap();
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
        // The fixture stages run no build bundle.
        up.development = true;
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
    assert_eq!(receipt["build"]["launch"], "development");
    assert_eq!(cloud::load(&rig.home()).unwrap().launch, "development");
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

    // A machine type the guard has no price for: a price table without
    // the g4-standard-192.
    let mut rig = Rig::new("four-single", "wss://relay.example");
    let mut prices: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo().join("scripts/dev/cloud-prices.json")).unwrap(),
    )
    .unwrap();
    prices["machines"]
        .as_object_mut()
        .unwrap()
        .remove("g4-standard-192");
    let table = rig.path("prices.json");
    std::fs::write(&table, prices.to_string()).unwrap();
    rig.up
        .guard
        .env
        .push(("OA_CLOUD_PRICES".into(), table.display().to_string()));
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
    // Past 90 seconds the head takes only pylon.deadline.v1 jobs (W14,
    // #12401); a beacon's deadline is at most 3600 seconds.
    let mut deadline = lane.clone();
    deadline.deadline_secs = 600;
    assert!(deadline.validate().is_ok());
    deadline.deadline_secs = 3_601;
    assert!(deadline.validate().unwrap_err().contains("3600 seconds"));
}

/// The head's ssh session, run on this computer: a wrapper that records its
/// process and starts `pylon cloud head` with a private home, as
/// `/opt/openagents/head-run` does on the head's machine.
fn in_zone_head(rig: &Rig) -> (PathBuf, Vec<String>) {
    use std::os::unix::fs::PermissionsExt;
    let script = rig.path("head-session.sh");
    std::fs::create_dir_all(rig.path("head-home")).unwrap();
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ >'{}'\nexec env HOME='{}' '{}' cloud head --home '{}'\n",
            rig.path("head.pid").display(),
            rig.path("head-home").display(),
            env!("CARGO_BIN_EXE_pylon"),
            rig.path("remote-head").display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, vec![])
}

/// The create call the fake gcloud got for `name`.
fn created(rig: &Rig, name: &str) -> Vec<String> {
    std::fs::read_to_string(rig.path("gcloud/calls.jsonl"))
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Vec<String>>(l).ok())
        .find(|args| args.get(2).map(String::as_str) == Some("create") && args[3] == name)
        .unwrap()
}

#[test]
fn dry_runs_plan_a_head_in_the_zone_and_direct_links() {
    // The head on the stage-0 machine, with direct links.
    let rig = Rig::new("four-single", "wss://relay.example");
    let mut up = rig.up.clone();
    up.worker = None;
    up.local_stages = None;
    up.head = HeadPlacement::Stage0;
    up.transport = Transport::Direct;
    up.pylon = Some("gs://pylon-test/pylon".into());
    let plan = cloud::plan(&up).unwrap();
    let value = cloud::dry_run(&up, &plan).unwrap();
    eprintln!(
        "dry run four-single, head on stage 0: {}",
        serde_json::to_string_pretty(&value).unwrap()
    );
    let rows = value["machines"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for (i, row) in rows.iter().enumerate() {
        let gcloud = row["gcloud"].as_str().unwrap();
        assert_eq!(row["head"], i == 0);
        assert!(gcloud.contains("ssh-keys="), "{gcloud}");
        assert_eq!(gcloud.contains("oa-group-head=1"), i == 0, "{gcloud}");
        assert_eq!(gcloud.contains("oa-pylon=gs://pylon-test/pylon"), i == 0);
        assert_eq!(gcloud.contains("v41-stage-ssh.sh"), i == 0);
    }
    let head = &value["head"];
    assert_eq!(head["placement"], "stage0");
    assert_eq!(head["machine"], "oa-v41-four-single-m0");
    assert_eq!(head["transport"], "direct");
    assert!(head["session"].as_str().unwrap().ends_with(
        "scripts/pylon/v41-head-ssh.sh oa-v41-four-single-m0 us-east1-b openagentsgemini"
    ));
    let launches: Vec<&str> = head["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["launch"].as_str().unwrap())
        .collect();
    let host = |i: usize| format!("oa-v41-four-single-m{i}.us-east1-b.c.openagentsgemini.internal");
    // A cloud group runs every stage on CUDA and expects its runtime.
    assert!(launches.iter().all(|l| l.contains("--backend cuda")));
    assert!(launches[0].starts_with("/opt/openagents/stage-run --layers 0-10"));
    assert!(launches[0].ends_with(&format!(
        "--data-listen 0.0.0.0:47200 --data-advertise {}:47200",
        host(0)
    )));
    for (i, launch) in launches.iter().enumerate().skip(1) {
        assert!(
            launch.starts_with(&format!(
                "/opt/openagents/stage-ssh --ssh '{{head}}/group-ssh' oa-stage@{} {} --",
                host(i),
                47_100 + i
            )),
            "{launch}"
        );
        assert!(launch.ends_with(&format!(
            "--data-listen 0.0.0.0:{} --data-advertise {}:{}",
            47_200 + i,
            host(i),
            47_200 + i
        )));
    }

    // A small head machine of its own: on demand, no GPU, no weights.
    let rig = Rig::new("two-dual", "wss://relay.example");
    let mut up = rig.up.clone();
    up.worker = None;
    up.local_stages = None;
    up.head = HeadPlacement::Vm;
    let plan = cloud::plan(&up).unwrap();
    let value = cloud::dry_run(&up, &plan).unwrap();
    eprintln!(
        "dry run two-dual, head on its own machine: {}",
        serde_json::to_string_pretty(&value).unwrap()
    );
    let rows = value["machines"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let head = &rows[2];
    assert_eq!(head["name"], "oa-v41-two-dual-head");
    assert_eq!(head["machine_type"], "e2-small");
    assert_eq!(head["head"], true);
    let gcloud = head["gcloud"].as_str().unwrap();
    for flag in [
        "--machine-type=e2-small",
        "oa-role=head",
        "oa-group-head=1",
        "--image-family=debian-12",
        "--boot-disk-size=20GB",
    ] {
        assert!(gcloud.contains(flag), "{flag} not in {gcloud}");
    }
    assert!(!gcloud.contains("SPOT"), "{gcloud}");
    assert!(!gcloud.contains("--accelerator"), "{gcloud}");
    assert!(head["cost_to_expiry"].as_f64().unwrap() < 1.0);
    let total: f64 = rows
        .iter()
        .map(|r| r["cost_to_expiry"].as_f64().unwrap())
        .sum();
    assert!((value["total_after"].as_f64().unwrap() - total).abs() < 0.02);
    assert_eq!(value["head"]["machine"], "oa-v41-two-dual-head");
    assert_eq!(value["head"]["transport"], "front");
    // Every stage is on another machine, so each goes over ssh.
    for stage in value["head"]["stages"].as_array().unwrap() {
        assert!(
            stage["launch"]
                .as_str()
                .unwrap()
                .starts_with("/opt/openagents/stage-ssh --ssh")
        );
    }
    assert!(rig.gcloud("create").is_empty());
    assert!(rig.ledger("start").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_head_on_the_stage_zero_machine_serves_with_direct_links_and_stops_on_down() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let mut rig = Rig::new("four-single", &relay);
    rig.up.head = HeadPlacement::Stage0;
    rig.up.transport = Transport::Direct;
    rig.up.pylon = Some("gs://pylon-test/pylon".into());
    rig.up.local_head = Some(in_zone_head(&rig));
    let launched = cloud::launch(&rig.up).await.unwrap();
    let receipt = launched.receipt.clone();
    eprintln!(
        "launch receipt: {}",
        serde_json::to_string_pretty(&receipt).unwrap()
    );
    assert_eq!(receipt["group"]["head_placement"], "stage0");
    assert_eq!(receipt["group"]["head_machine"], "oa-v41-four-single-m0");
    assert_eq!(receipt["group"]["transport"], "direct");
    assert_eq!(rig.gcloud("create").len(), 4);
    // The stage machines carry the group key's public half.
    let keys = std::fs::read_to_string(rig.home().join("cloud-v41-four-single/ssh-keys")).unwrap();
    assert!(keys.starts_with("oa-stage:ssh-ed25519 "), "{keys}");
    assert!(
        created(&rig, "oa-v41-four-single-m2")
            .iter()
            .any(|a| a.contains("ssh-keys=") && a.contains("cloud-v41-four-single/ssh-keys"))
    );
    // This computer runs no pylon of its own; the head in the zone holds
    // the group key and publishes under its own identity.
    assert!(pylon::share::load(&rig.home()).unwrap().is_none());
    let key = rig.path("remote-head/group-ssh");
    assert!(
        std::fs::read_to_string(&key)
            .unwrap()
            .contains("PRIVATE KEY")
    );
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let remote = Identity::load_or_create(&rig.path("remote-head/provider.key")).unwrap();
    let (head, _) = group(&hub, 4, |head, stages| {
        head.group_head().unwrap().readiness == Some(Readiness::Ready)
            && stages.iter().all(|s| s.status == Status::Online)
    })
    .await;
    assert_eq!(head.provider, remote.pubkey());
    assert_eq!(receipt["group"]["head"].as_str().unwrap(), head.address());

    let artifact = receipt["group"]["artifact"].as_str().unwrap().to_string();
    let asker = tempfile::tempdir().unwrap();
    let routed = ask(&relay, asker.path(), &artifact).await;
    let Outcome::Answered(answer) = &routed.outcome else {
        panic!("no answer: {:?} {:?}", routed.outcome, routed.failed);
    };
    assert_eq!(
        answer.text.as_deref(),
        Some("<7000><7001><7002><7003><7004><7005>")
    );
    // The steps reached only the first stage; the others got stage state
    // from the stage before them.
    assert!(rig.stage_events("step").iter().all(|e| e["stage"] == "s0"));
    for stage in ["s1", "s2", "s3"] {
        assert_eq!(
            rig.stage_events("data")
                .iter()
                .filter(|e| e["stage"] == stage)
                .count(),
            6,
            "{stage}"
        );
    }

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
    assert_eq!(ended["reason"], "pylon cloud down");
    assert_eq!(down["ended"]["reason"], "pylon cloud down");
    assert_eq!(rig.gcloud("delete").len(), 4);
    group(&hub, 4, |head, stages| {
        head.status == Status::Offline && stages.iter().all(|s| s.status == Status::Offline)
    })
    .await;
    let pid: i32 = std::fs::read_to_string(rig.path("head.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(!alive(pid as u32), "the head's session is over");
    let (_, text) = cloud::status_at(&rig.home()).unwrap();
    assert!(
        text.contains("on oa-v41-four-single-m0, direct transport"),
        "{text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lost_head_ends_the_group_and_deletes_every_machine() {
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    let mut rig = Rig::new("two-dual", &relay);
    rig.up.head = HeadPlacement::Vm;
    rig.up.local_head = Some(in_zone_head(&rig));
    let launched = cloud::launch(&rig.up).await.unwrap();
    assert_eq!(rig.gcloud("create").len(), 3);
    assert_eq!(
        launched.receipt["group"]["head_machine"],
        "oa-v41-two-dual-head"
    );
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
    // The head's machine is gone: its session ends without a word.
    let pid: i32 = std::fs::read_to_string(rig.path("head.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    unsafe { libc::kill(pid, libc::SIGKILL) };
    let ended = tokio::time::timeout(Duration::from_secs(30), serving)
        .await
        .unwrap()
        .unwrap();
    eprintln!("lost head: {ended}");
    assert!(
        ended["reason"]
            .as_str()
            .unwrap()
            .starts_with("the group was lost: the head's session ended"),
        "{ended}"
    );
    assert_eq!(rig.gcloud("delete").len(), 3);
    assert_eq!(rig.ledger("stop").len(), 3);
    // Its stages lose their front and exit.
    until("the stages to exit", 10, async || {
        let left = rig.stage_events("front-closed").len();
        (left == 4).then_some(())
    })
    .await;
}

#[tokio::test]
async fn a_head_in_the_zone_needs_the_pylon_binary() {
    let (relay, _hub) = pylon::fixture::relay().await.unwrap();
    let mut rig = Rig::new("two-dual", &relay);
    rig.up.head = HeadPlacement::Stage0;
    let Err(why) = cloud::launch(&rig.up).await else {
        panic!("launched");
    };
    assert!(why.contains("--pylon"), "{why}");
    assert!(rig.gcloud("create").is_empty());
}

#[test]
fn the_launchers_reach_stages_and_the_head_inside_the_zone() {
    // A head in the zone starts a stage over plain ssh with the group key.
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(repo().join("scripts/pylon/v41-stage-ssh.sh"))
        .args([
            "--ssh",
            "/home/me/.openagents/v41-head/group-ssh",
            "oa-stage@oa-v41-two-dual-m1.us-east1-b.c.openagentsgemini.internal",
            "47102",
            "--",
            "--layers",
            "20-30",
            "--front",
            "127.0.0.1:53111",
        ])
        .env("OA_SSH", fixture("fake_gcloud.py"))
        .env("FAKE_GCLOUD_ROOT", dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "the fake refuses ssh");
    let calls = std::fs::read_to_string(dir.path().join("calls.jsonl")).unwrap();
    let args: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
    assert_eq!(
        args[..3],
        ["-i", "/home/me/.openagents/v41-head/group-ssh", "-tt"]
    );
    assert!(args.contains(&"BatchMode=yes".into()));
    assert!(args.contains(&"127.0.0.1:47102:127.0.0.1:53111".into()));
    assert_eq!(
        args[args.len() - 2],
        "oa-stage@oa-v41-two-dual-m1.us-east1-b.c.openagentsgemini.internal"
    );
    assert_eq!(
        args.last().unwrap(),
        "/opt/openagents/stage-run '--layers' '20-30' '--front' '127.0.0.1:47102'"
    );

    // The budget host waits for a new machine's head to answer ssh before
    // it starts the session, and gives up after the wait.
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(repo().join("scripts/pylon/v41-head-ssh.sh"))
        .args(["oa-v41-two-dual-head", "us-east1-b", "openagentsgemini"])
        .env("OA_GCLOUD", fixture("fake_gcloud.py"))
        .env("FAKE_GCLOUD_ROOT", dir.path())
        .env("OA_HEAD_SSH_WAIT", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not start its head within 1 s"));
    let calls = std::fs::read_to_string(dir.path().join("calls.jsonl")).unwrap();
    let probe: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
    // Ready means this boot's ready mark (v41-stage-startup.sh, #12407).
    assert_eq!(
        probe.last().unwrap(),
        "test -x /opt/openagents/head-run && test \"$(head -n 1 /mnt/stage/ready 2>/dev/null)\" = \"boot $(cat /proc/sys/kernel/random/boot_id)\""
    );
    assert!(!calls.contains("\"-T\""));

    // Then it starts the head over gcloud ssh, with its input and output as
    // the session's.
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(repo().join("scripts/pylon/v41-head-ssh.sh"))
        .args(["oa-v41-two-dual-head", "us-east1-b", "openagentsgemini"])
        .env("OA_GCLOUD", fixture("fake_gcloud.py"))
        .env("FAKE_GCLOUD_ROOT", dir.path())
        .env("OA_HEAD_SSH_WAIT", "0")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let calls = std::fs::read_to_string(dir.path().join("calls.jsonl")).unwrap();
    let args: Vec<String> = serde_json::from_str(calls.lines().next().unwrap()).unwrap();
    assert_eq!(
        args[..9],
        [
            "compute",
            "ssh",
            "oa-v41-two-dual-head",
            "--zone",
            "us-east1-b",
            "--project",
            "openagentsgemini",
            "--quiet",
            "--"
        ]
    );
    assert!(args.contains(&"-T".into()));
    assert_eq!(args.last().unwrap(), "/opt/openagents/head-run");
}

#[test]
fn a_cloud_group_runs_cuda_stages_unless_told_otherwise() {
    let parse = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
    };
    let (up, _) = parse(&["two-dual"]).unwrap();
    assert_eq!(up.stage_backend.as_deref(), Some("cuda"));
    assert_eq!(up.stage_runtime, cloud::RUNTIME_CUDA);
    let (up, _) = parse(&["two-dual", "--backend", "cpu"]).unwrap();
    assert_eq!(up.stage_backend.as_deref(), Some("cpu"));
    assert_eq!(up.stage_runtime, cloud::RUNTIME);
    assert!(parse(&["two-dual", "--backend", "rocm"]).is_err());
}

#[test]
fn chunk_capacities_reach_stages_and_must_fit_the_measured_memory_profile() {
    let words: Vec<String> = [
        "four-single",
        "--max-context",
        "32768",
        "--max-step-tokens",
        "2048",
        "--prefill-chunk",
        "2048",
        "--max-input-tokens",
        "24576",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (mut up, _) = cloud::parse_up(
        &words,
        &repo(),
        Path::new("/home/me"),
        "wss://relay.example",
    )
    .unwrap();
    for placement in ["four-single", "two-dual", "one-quad"] {
        up.placement = placement.into();
        let plan = cloud::plan(&up).unwrap();
        let lane = cloud::group_lane(&up, &plan);
        for stage in lane.stages {
            for (flag, value) in [
                ("--max-context", "32768"),
                ("--max-step-tokens", "2048"),
                ("--prefill-chunk", "2048"),
                ("--max-input-tokens", "24576"),
            ] {
                assert!(stage.args.windows(2).any(|a| a == [flag, value]));
            }
        }
    }
    up.max_context = 131072;
    assert!(cloud::plan(&up).unwrap_err().contains("memory profile"));
    up.max_context = 32768;
    up.max_step_tokens = Some(8192);
    assert!(cloud::plan(&up).is_err());
    up.max_step_tokens = Some(0);
    assert!(cloud::plan(&up).is_err());
}

/// A launch can pin the worker and pylon builds; every machine's metadata
/// then carries the pins its startup script checks (#12407).
#[test]
fn build_pins_reach_every_machine() {
    let parse = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
    };
    let (worker, pylon) = ("A".repeat(64), "b".repeat(64));
    let (mut up, _) = parse(&[
        "two-dual",
        "--worker-build",
        &worker,
        "--pylon-build",
        &pylon,
        "--head",
        "vm",
        "--pylon",
        "gs://pylon-test/pylon",
    ])
    .unwrap();
    up.worker = Some("gs://pylon-test/stage-worker".into());
    let plan = cloud::plan(&up).unwrap();
    assert_eq!(plan.machines.len(), 3);
    for machine in &plan.machines {
        let metadata = machine
            .guard
            .iter()
            .find(|a| a.starts_with("--metadata="))
            .unwrap();
        assert!(
            metadata.contains(&format!("oa-build={}", "a".repeat(64))),
            "{metadata}"
        );
        assert!(
            metadata.contains(&format!("oa-pylon-build={pylon}")),
            "{metadata}"
        );
    }
    assert!(parse(&["two-dual", "--worker-build", "abc"]).is_err());
    assert!(parse(&["two-dual", "--pylon-build", &"g".repeat(64)]).is_err());

    // The group configuration carries the worker pin, which the head
    // compares with every stage's announced bundle (#12410).
    let lane = cloud::group_lane(&up, &plan);
    assert_eq!(lane.build.as_deref(), Some("a".repeat(64).as_str()));
    assert_eq!(lane.stage_config().build, lane.build);
    cloud::check_pins(&up).unwrap();
    assert_eq!(cloud::build_summary(&up)["launch"], "pinned");
}

/// A qualified launch needs the worker pin, and the pylon pin for a head in
/// the zone; a development launch takes neither and says so (#12410).
#[test]
fn a_launch_without_build_pins_must_be_a_development_launch() {
    let parse = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
        .unwrap()
        .0
    };
    let pin = "c".repeat(64);
    let why = cloud::check_pins(&parse(&["two-dual"])).unwrap_err();
    assert!(why.contains("--worker-build"), "{why}");
    let why = cloud::check_pins(&parse(&[
        "two-dual",
        "--worker-build",
        &pin,
        "--head",
        "vm",
    ]))
    .unwrap_err();
    assert!(why.contains("--pylon-build"), "{why}");
    cloud::check_pins(&parse(&["two-dual", "--worker-build", &pin])).unwrap();
    let up = parse(&["two-dual", "--development"]);
    cloud::check_pins(&up).unwrap();
    assert_eq!(cloud::build_summary(&up)["launch"], "development");
    let plan = cloud::plan(&up).unwrap();
    assert_eq!(cloud::group_lane(&up, &plan).build, None);
    assert!(
        cloud::check_pins(&parse(&[
            "two-dual",
            "--development",
            "--worker-build",
            &pin
        ]))
        .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unpinned_launch_creates_nothing() {
    let (relay, _hub) = pylon::fixture::relay().await.unwrap();
    let mut rig = Rig::new("four-single", &relay);
    rig.up.development = false;
    let Err(why) = cloud::launch(&rig.up).await else {
        panic!("an unpinned launch started");
    };
    assert!(why.contains("--worker-build"), "{why}");
    assert!(rig.gcloud("create").is_empty());
}

/// The one-machine placement with its head on that machine (W11, #12398):
/// the stage programs live there, so the budget host checks only the
/// lane's settings; the flags set the head's limits and sampling.
#[test]
fn a_one_machine_group_heads_itself_and_takes_its_limits() {
    let words: Vec<String> = [
        "one-quad",
        "--head",
        "stage0",
        "--transport",
        "direct",
        "--temperature",
        "0",
        "--ready-secs",
        "3600",
        "--step-secs",
        "75",
    ]
    .iter()
    .map(|w| (*w).to_owned())
    .collect();
    let (up, _) = cloud::parse_up(
        &words,
        &repo(),
        Path::new("/home/me"),
        "wss://relay.example",
    )
    .unwrap();
    assert_eq!(up.ready_timeout, Duration::from_secs(3600));
    assert_eq!(up.step_timeout, Duration::from_secs(75));
    assert_eq!(up.sampling, Sampling::new(0.0, 0.95, 0).unwrap());
    let plan = cloud::plan(&up).unwrap();
    assert_eq!(plan.machines.len(), 1);
    assert_eq!(plan.machines[0].machine_type, "g4-standard-192");
    let lane = cloud::group_lane(&up, &plan);
    assert!(
        lane.stages
            .iter()
            .all(|s| s.program == Path::new("/opt/openagents/stage-run"))
    );
    assert!(lane.validate().unwrap_err().contains("missing its program"));
    lane.validate_elsewhere().unwrap();
    let step = |secs: &str| {
        let words: Vec<String> = ["one-quad", "--step-secs", secs]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
    };
    assert!(step("76").is_err());
}

/// `--mode chat` heads the group in the model's chat mode (W2, #12389), as
/// the oracle's p2 prompt needs; a thinking-mode lane serializes as before.
#[test]
fn a_cloud_group_head_takes_its_prompt_mode() {
    let parse = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
    };
    let (up, _) = parse(&["one-quad"]).unwrap();
    let lane = cloud::group_lane(&up, &cloud::plan(&up).unwrap());
    assert!(serde_json::to_value(&lane).unwrap().get("mode").is_none());
    let (up, _) = parse(&["one-quad", "--mode", "chat"]).unwrap();
    let lane = cloud::group_lane(&up, &cloud::plan(&up).unwrap());
    assert_eq!(lane.mode, pylon::stage_group::PromptMode::Chat);
    assert_eq!(serde_json::to_value(&lane).unwrap()["mode"], "chat");
    assert!(parse(&["one-quad", "--mode", "terse"]).is_err());
}

/// `--weights` points every stage machine at a copy of the checkpoint in
/// the zone's region (W16, #12403); the default stays the original.
#[test]
fn a_cloud_group_takes_a_regional_weights_copy() {
    let parse = |words: &[&str]| {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        cloud::parse_up(
            &words,
            &repo(),
            Path::new("/home/me"),
            "wss://relay.example",
        )
    };
    let (up, _) = parse(&["one-quad"]).unwrap();
    assert_eq!(up.weights, pylon::placement::WEIGHTS);
    let (up, _) = parse(&["one-quad", "--weights", "gs://copy/v41/1bae0e1"]).unwrap();
    assert_eq!(up.weights, "gs://copy/v41/1bae0e1/");
    let plan = cloud::plan(&up).unwrap();
    let args = serde_json::to_string(&cloud::dry_run(&up, &plan).unwrap()).unwrap();
    assert!(args.contains("oa-weights=gs://copy/v41/1bae0e1/"), "{args}");
    assert!(parse(&["one-quad", "--weights", "/local/dir"]).is_err());
}
