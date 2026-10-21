#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Stage-group lifecycle faults against synthetic stage processes.
//!
//! Each stage is `fixtures/stage_worker.py`, started and reaped by the
//! group's process-group owner. Faults are one-shot files in a private
//! directory; nothing reads the real home or reaches a network.

use nostr::pylon::Readiness;
use pylon::engine::Sampling;
use pylon::group::GroupReadiness;
use pylon::stage_group::{
    Failure, LinkPolicy, PROTOCOL, StageGroup, StageGroupConfig, StageGroupStatus, StageSpec,
    Transport,
};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const STAGES: [&str; 3] = ["s0", "s1", "s2"];

fn events(root: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The front's sampling settings; the last stage's tokens encode the seed.
fn sampling() -> Sampling {
    Sampling::new(0.6, 0.95, 7).unwrap()
}

/// The tokens the fixture's last stage samples for `steps`.
fn toks(steps: std::ops::Range<u64>) -> Vec<u32> {
    steps.map(|s| 7000 + s as u32).collect()
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

struct Rig {
    root: tempfile::TempDir,
    group: Option<StageGroup>,
}

impl Rig {
    async fn start(flags: &[(&str, &str)]) -> Self {
        Self::with(flags, |_| {}).await
    }

    async fn with(flags: &[(&str, &str)], change: impl FnOnce(&mut StageGroupConfig)) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("stage.py");
        std::fs::write(&script, include_bytes!("fixtures/stage_worker.py")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        for (flag, value) in flags {
            std::fs::write(root.path().join(flag), value).unwrap();
        }
        let mut config = StageGroupConfig {
            model: "deepseek-v4.1-fixture".into(),
            weights: "sha256:fixture".into(),
            runtime: "fixture-runtime/1".into(),
            protocol: PROTOCOL,
            build: None,
            stages: STAGES
                .iter()
                .map(|name| StageSpec {
                    name: (*name).into(),
                    program: script.clone(),
                    args: vec!["--root".into(), root.path().display().to_string()],
                })
                .collect(),
            transport: Transport::Front,
            links: LinkPolicy::default(),
            step_timeout: Duration::from_millis(1500),
            health_timeout: Duration::from_millis(600),
            startup_timeout: Duration::from_secs(5),
            stop_timeout: Duration::from_millis(600),
            max_restarts: 3,
            backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        };
        change(&mut config);
        let group = StageGroup::start(config, Arc::new(|s| eprintln!("group: {s}")))
            .await
            .unwrap();
        Self {
            root,
            group: Some(group),
        }
    }

    fn group(&self) -> &StageGroup {
        self.group.as_ref().unwrap()
    }

    fn status(&self) -> StageGroupStatus {
        self.group().status()
    }

    fn flag(&self, name: &str, value: &str) {
        std::fs::write(self.root.path().join(name), value).unwrap();
    }

    fn events(&self) -> Vec<serde_json::Value> {
        events(self.root.path())
    }

    async fn until(
        &self,
        what: &str,
        mut check: impl FnMut(&StageGroupStatus) -> bool,
    ) -> StageGroupStatus {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let status = self.status();
            if check(&status) {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "group did not reach {what}: {status:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn ready(&self, generation: u64) -> StageGroupStatus {
        self.until(&format!("readiness at generation {generation}"), |s| {
            s.generation == generation && s.admits()
        })
        .await
    }

    fn pids(&self) -> Vec<u32> {
        self.status()
            .stages
            .iter()
            .map(|s| s.pid.unwrap())
            .collect()
    }

    /// Run a request to completion and return its tokens.
    async fn complete(&self, steps: u64) -> Vec<u32> {
        let mut run = self.group().generate(steps, sampling()).await.unwrap();
        let done = tokio::time::timeout(Duration::from_secs(10), &mut run.done)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(done.tokens, steps);
        let mut tokens = vec![];
        while let Ok(token) = run.tokens.try_recv() {
            tokens.push(token);
        }
        tokens
    }

    /// Run a request that must fail; return its tokens and failure.
    async fn fail(&self, steps: u64) -> (String, Vec<u32>, Failure) {
        let mut run = self.group().generate(steps, sampling()).await.unwrap();
        let failure = tokio::time::timeout(Duration::from_secs(10), &mut run.done)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        let mut tokens = vec![];
        while let Ok(token) = run.tokens.try_recv() {
            tokens.push(token);
        }
        assert_eq!(failure.delivered, tokens.len() as u64);
        assert!(!failure.cancelled);
        (run.request, tokens, failure)
    }

    /// Steps each stage received for one request, in order.
    fn steps(&self, request: &str) -> Vec<(String, u64)> {
        self.events()
            .iter()
            .filter(|e| e["event"] == "step" && e["request"] == request)
            .map(|e| {
                (
                    e["stage"].as_str().unwrap().to_string(),
                    e["step"].as_u64().unwrap(),
                )
            })
            .collect()
    }

    async fn close(mut self) {
        let pids = self.pids_now();
        self.group.take().unwrap().stop().await;
        for pid in pids {
            assert!(!alive(pid), "stage process {pid} survived shutdown");
        }
        for event in self.events() {
            if let Some(pid) = event["late_pid"].as_u64() {
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            }
        }
    }

    fn pids_now(&self) -> Vec<u32> {
        self.status().stages.iter().filter_map(|s| s.pid).collect()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_group_is_advertised_only_after_every_stage_loads_checks_and_agrees() {
    let rig = Rig::start(&[("start-delay-s2", "700")]).await;
    // Two stages are up and checked; the third is still loading.
    rig.until("two ready stages", |s| {
        s.stages.iter().filter(|s| s.ready).count() == 2
    })
    .await;
    let status = rig.status();
    assert!(!status.ready && !status.admits());
    let head = GroupReadiness::status(rig.group());
    assert_eq!(head.readiness, Readiness::Starting);
    let refused = rig.group().generate(1, sampling()).await.unwrap_err();
    assert!(refused.contains("not ready"), "{refused}");
    let status = rig.ready(1).await;
    assert!(status.stages.iter().all(|s| s.ready && s.pid.is_some()));
    let head = GroupReadiness::status(rig.group());
    assert_eq!((head.readiness, head.generation), (Readiness::Ready, 1));
    assert_eq!(rig.complete(3).await, toks(0..3));
    let order: Vec<_> = rig.steps("1");
    let expected: Vec<_> = (0..3)
        .flat_map(|step| STAGES.iter().map(move |s| (s.to_string(), step)))
        .collect();
    assert_eq!(order, expected);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stage_with_other_weights_is_never_advertised_and_spends_the_budget() {
    let rig = Rig::with(&[("weights-s1", "sha256:other")], |c| {
        c.startup_timeout = Duration::from_millis(400);
        c.max_restarts = 2;
    })
    .await;
    let mut watch = rig.group().watch();
    let watcher = tokio::spawn(async move {
        let mut seen_ready = false;
        loop {
            seen_ready |= watch.borrow_and_update().ready;
            if watch.borrow().failed.is_some() || watch.changed().await.is_err() {
                return seen_ready;
            }
        }
    });
    let status = rig.until("budget exhaustion", |s| s.failed.is_some()).await;
    assert!(
        !watcher.await.unwrap(),
        "a disagreeing group was advertised"
    );
    assert!(
        status
            .failed
            .as_deref()
            .unwrap()
            .contains("repeated failures"),
        "{status:?}"
    );
    assert!(status.stages.iter().all(|s| s.pid.is_none()));
    let spawns = rig
        .events()
        .iter()
        .filter(|e| e["event"] == "spawn")
        .count();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        rig.events()
            .iter()
            .filter(|e| e["event"] == "spawn")
            .count(),
        spawns,
        "an exhausted group must not restart stages"
    );
    rig.close().await;
}

/// A replaced stage worker that runs another build or numerical profile
/// than the group never joins it, though its weights, runtime, and protocol
/// still match (#12407).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replaced_stage_with_another_build_or_profile_never_joins() {
    let flags = [
        ("build-s0", "aaaa"),
        ("build-s1", "aaaa"),
        ("build-s2", "aaaa"),
        ("profile-s0", "cuda-device-sm_120"),
        ("profile-s1", "cuda-device-sm_120"),
        ("profile-s2", "cuda-device-sm_120"),
    ];
    for (file, value) in [("build-s2", "bbbb"), ("profile-s2", "cpu-bf16")] {
        let rig = Rig::with(&flags, |c| {
            c.startup_timeout = Duration::from_secs(3);
            c.max_restarts = 2;
        })
        .await;
        rig.ready(1).await;
        let before = rig.pids();
        rig.flag(file, value);
        unsafe { libc::kill(before[2] as i32, libc::SIGKILL) };
        let status = rig
            .until("the replaced stage is refused", |s| {
                s.reason
                    .contains("the group runs build unbundled executable aaaa (sm_120)")
            })
            .await;
        assert!(!status.ready, "{file}: {status:?}");
        let status = rig.until("budget exhaustion", |s| s.failed.is_some()).await;
        assert!(!status.ready, "{file}");
        rig.close().await;
    }
}

/// Two bundles with the same executable and different CUDA libraries have
/// different bundle hashes, so a replaced stage that runs the other bundle
/// never joins a development group (#12410).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replaced_stage_with_the_same_executable_and_other_libraries_never_joins() {
    let flags = [
        ("build-s0", "aaaa"),
        ("build-s1", "aaaa"),
        ("build-s2", "aaaa"),
        ("bundle-s0", "b1"),
        ("bundle-s1", "b1"),
        ("bundle-s2", "b1"),
    ];
    let rig = Rig::with(&flags, |c| {
        c.startup_timeout = Duration::from_secs(3);
        c.max_restarts = 2;
    })
    .await;
    rig.ready(1).await;
    let before = rig.pids();
    rig.flag("bundle-s2", "b2");
    unsafe { libc::kill(before[2] as i32, libc::SIGKILL) };
    let status = rig
        .until("the replaced stage is refused", |s| {
            s.reason.contains("runs build bundle b2 (sm_120)")
                && s.reason.contains("the group runs build bundle b1 (sm_120)")
        })
        .await;
    assert!(!status.ready, "{status:?}");
    rig.close().await;
}

/// A pinned group admits only stages that announce the pinned bundle, even
/// when every stage agrees on another one (#12410).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pinned_group_admits_only_the_pinned_bundle() {
    let flags = [
        ("build-s0", "aaaa"),
        ("build-s1", "aaaa"),
        ("build-s2", "aaaa"),
        ("bundle-s0", "b2"),
        ("bundle-s1", "b2"),
        ("bundle-s2", "b2"),
    ];
    let rig = Rig::with(&flags, |c| {
        c.build = Some("b1".into());
        c.startup_timeout = Duration::from_secs(3);
        c.max_restarts = 1;
    })
    .await;
    let status = rig
        .until("every stage is refused", |s| {
            s.reason
                .contains("runs build bundle b2 (sm_120); the group is pinned to bundle b1")
        })
        .await;
    assert!(!status.ready, "{status:?}");
    assert_eq!(status.build.as_deref(), Some("b1"));
    rig.close().await;

    let pinned = [
        ("build-s0", "aaaa"),
        ("build-s1", "aaaa"),
        ("build-s2", "aaaa"),
        ("bundle-s0", "b1"),
        ("bundle-s1", "b1"),
        ("bundle-s2", "b1"),
    ];
    let rig = Rig::with(&pinned, |c| c.build = Some("b1".into())).await;
    let status = rig.ready(1).await;
    assert_eq!(status.build.as_deref(), Some("b1"));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn front_disconnect_holds_admission_until_every_stage_stops() {
    let rig = Rig::start(&[("slow-s1", "150")]).await;
    rig.ready(1).await;
    let mut run = rig.group().generate(20, sampling()).await.unwrap();
    assert_eq!(run.tokens.recv().await, Some(7000));
    // The caller goes away mid-request.
    drop(run.tokens);
    let busy = rig.status();
    assert!(busy.busy && !busy.admits());
    assert!(
        rig.group()
            .generate(1, sampling())
            .await
            .unwrap_err()
            .contains("busy")
    );
    let failure = run.done.await.unwrap().unwrap_err();
    assert!(failure.cancelled);
    assert!(failure.delivered >= 1);
    let status = rig.until("a free slot", |s| !s.busy).await;
    // Every stage confirmed it stopped before the slot freed, and none of
    // them was replaced.
    let stopped: Vec<_> = rig
        .events()
        .iter()
        .filter(|e| e["event"] == "stopped" && e["request"] == "1")
        .map(|e| e["stage"].as_str().unwrap().to_string())
        .collect();
    for stage in STAGES {
        assert!(stopped.iter().any(|s| s == stage), "{stage} did not stop");
    }
    assert_eq!(status.generation, 1);
    assert!(status.ready);
    let after = rig.steps("1").len();
    assert_eq!(rig.complete(2).await, toks(0..2));
    assert_eq!(
        rig.steps("1").len(),
        after,
        "the cancelled request ran again"
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancellation_keeps_the_slot_until_a_stage_that_ignores_it_is_reaped() {
    let rig = Rig::start(&[("slow-s1", "150"), ("ignore-cancel-s1", "")]).await;
    rig.ready(1).await;
    let stubborn = rig.pids()[1];
    let mut run = rig.group().generate(20, sampling()).await.unwrap();
    assert_eq!(run.tokens.recv().await, Some(7000));
    drop(run.tokens);
    let cancelled = Instant::now();
    // Watch the slot: it may free only after the stubborn stage is gone.
    let mut watch = rig.group().watch();
    loop {
        let busy = watch.borrow_and_update().busy;
        if !busy {
            assert!(!alive(stubborn), "the slot freed while a stage still ran");
            break;
        }
        tokio::time::timeout(Duration::from_secs(10), watch.changed())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(cancelled.elapsed() >= Duration::from_millis(500));
    std::fs::remove_file(rig.root.path().join("ignore-cancel-s1")).unwrap();
    let status = rig.ready(2).await;
    assert_ne!(status.stages[1].pid, Some(stubborn));
    assert_eq!(status.stages[1].replacements, 1);
    assert_eq!(rig.complete(2).await, toks(0..2));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_frozen_stage_that_still_answers_health_fails_the_request_once() {
    let rig = Rig::start(&[("freeze-s1", "1")]).await;
    rig.ready(1).await;
    let before = rig.pids();
    let (request, tokens, failure) = rig.fail(4).await;
    assert_eq!(tokens, toks(0..1));
    assert!(failure.reason.contains("s1"), "{failure}");
    assert!(failure.reason.contains("no progress"), "{failure}");
    // Only the frozen stage is replaced; the group moves to generation 2.
    let status = rig.ready(2).await;
    let after = rig.pids();
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[1]);
    assert_eq!(after[2], before[2]);
    assert!(!alive(before[1]));
    let fenced: Vec<_> = rig
        .events()
        .iter()
        .filter(|e| e["event"] == "fenced")
        .map(|e| {
            (
                e["stage"].as_str().unwrap().to_string(),
                e["generation"].as_u64(),
            )
        })
        .collect();
    assert!(fenced.contains(&("s0".into(), Some(2))));
    assert!(fenced.contains(&("s2".into(), Some(2))));
    assert_eq!(status.stages[1].replacements, 1);
    let ran = rig.steps(&request).len();
    assert_eq!(rig.complete(3).await, toks(0..3));
    assert_eq!(
        rig.steps(&request).len(),
        ran,
        "a failed request was replayed"
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stopped_process_fails_its_health_checks_and_is_replaced() {
    let rig = Rig::start(&[]).await;
    rig.ready(1).await;
    let before = rig.pids();
    unsafe { libc::kill(before[2] as i32, libc::SIGSTOP) };
    let status = rig.ready(2).await;
    assert_ne!(status.stages[2].pid, Some(before[2]));
    assert!(!alive(before[2]), "the stopped stage was not reaped");
    assert_eq!(rig.pids()[..2], before[..2]);
    assert_eq!(rig.complete(2).await, toks(0..2));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crash_before_a_step_acknowledgment_fails_the_request_visibly() {
    let rig = Rig::start(&[("crash-before-ack-s1", "0")]).await;
    rig.ready(1).await;
    let (request, tokens, failure) = rig.fail(3).await;
    assert!(tokens.is_empty());
    assert!(failure.reason.contains("s1"), "{failure}");
    rig.ready(2).await;
    assert_eq!(rig.complete(3).await, toks(0..3));
    // Stage s2 never saw the failed request: it is not retried anywhere.
    assert_eq!(
        rig.steps(&request),
        vec![("s0".to_string(), 0), ("s1".to_string(), 0)]
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crash_after_a_step_acknowledgment_never_replays_delivered_tokens() {
    let rig = Rig::start(&[("crash-after-ack-s2", "1"), ("slow-s0", "100")]).await;
    rig.ready(1).await;
    let (request, tokens, failure) = rig.fail(5).await;
    // The last stage acknowledged step 1, so its token reached the caller
    // before the crash was seen.
    assert_eq!(tokens, toks(0..2));
    assert_eq!(failure.delivered, 2);
    rig.ready(2).await;
    let steps = rig.steps(&request);
    for stage in STAGES {
        for step in 0..2 {
            assert_eq!(
                steps
                    .iter()
                    .filter(|(s, n)| s == stage && *n == step)
                    .count(),
                1,
                "{stage} step {step} ran more than once"
            );
        }
    }
    assert_eq!(rig.complete(2).await, toks(0..2));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replayed_and_out_of_date_messages_are_refused_without_breaking_the_request() {
    let rig = Rig::start(&[
        ("replay-ack-s0", "0"),
        ("stale-ack-s2", "1"),
        ("old-ack-s1", "2"),
    ])
    .await;
    rig.ready(1).await;
    assert_eq!(rig.complete(4).await, toks(0..4));
    let status = rig.status();
    assert!(status.refused >= 3, "{status:?}");
    assert_eq!(status.generation, 1);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lost_messages_fail_the_request_at_the_step_deadline() {
    let rig = Rig::with(&[("drop-ack-s1", "1")], |c| {
        c.step_timeout = Duration::from_millis(500);
    })
    .await;
    rig.ready(1).await;
    let (_, tokens, failure) = rig.fail(3).await;
    assert_eq!(tokens, toks(0..1));
    assert!(failure.reason.contains("s1"), "{failure}");
    rig.ready(2).await;
    // A lost step message: the stage never sees it.
    rig.flag("drop-step-s2", "0");
    let (_, tokens, failure) = rig.fail(3).await;
    assert!(tokens.is_empty());
    assert!(failure.reason.contains("s2"), "{failure}");
    rig.ready(3).await;
    assert_eq!(rig.complete(2).await, toks(0..2));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_late_old_stage_after_restart_is_refused() {
    // The first s1 leaves behind an escaped copy that speaks 1.2 s later,
    // with the first instance's identity and generation.
    let rig = Rig::start(&[("late-s1", "1200")]).await;
    rig.ready(1).await;
    let old = rig.pids()[1];
    unsafe { libc::kill(old as i32, libc::SIGKILL) };
    rig.ready(2).await;
    let refused = rig.status().refused;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !rig.events().iter().any(|e| e["event"] == "late-sent") {
        assert!(Instant::now() < deadline, "the late stage never spoke");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let status = rig
        .until("the late stage's refusal", |s| s.refused > refused)
        .await;
    assert!(
        status
            .last_refusal
            .as_deref()
            .is_some_and(|r| r.contains("not a running member") || r.contains("replaced")),
        "{status:?}"
    );
    assert_eq!(status.generation, 2);
    assert!(status.admits());
    // The late copy's acknowledgments for requests 1–4 never count.
    assert_eq!(rig.complete(4).await, toks(0..4));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stage_that_rejoins_with_an_old_generation_is_replaced() {
    let rig = Rig::with(&[], |c| c.startup_timeout = Duration::from_secs(2)).await;
    rig.ready(1).await;
    // The replacement for s0 claims the generation it replaced.
    rig.flag("stale-generation-s0", "");
    let old = rig.pids()[0];
    unsafe { libc::kill(old as i32, libc::SIGKILL) };
    let status = rig.ready(3).await;
    assert_eq!(status.stages[0].replacements, 2);
    assert!(status.refused >= 1);
    assert_eq!(rig.complete(1).await, toks(0..1));
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restarts_are_bounded_and_a_completed_request_resets_the_budget() {
    let rig = Rig::with(&[], |c| c.max_restarts = 2).await;
    rig.ready(1).await;
    // Two failures in a row stay within the budget.
    unsafe { libc::kill(rig.pids()[0] as i32, libc::SIGKILL) };
    rig.ready(2).await;
    unsafe { libc::kill(rig.pids()[1] as i32, libc::SIGKILL) };
    let status = rig.ready(3).await;
    assert_eq!(status.failures, 2);
    // A completed request resets it, as for the serial worker.
    assert_eq!(rig.complete(1).await, toks(0..1));
    rig.until("a reset budget", |s| s.failures == 0).await;
    // A stage that can no longer start spends the budget and stops the group.
    rig.flag("always-die-s2", "");
    let pids = rig.pids();
    unsafe { libc::kill(pids[2] as i32, libc::SIGKILL) };
    let status = rig.until("budget exhaustion", |s| s.failed.is_some()).await;
    assert!(!status.ready);
    for pid in pids {
        assert!(!alive(pid), "stage {pid} outlived the stopped group");
    }
    assert!(
        rig.group()
            .generate(1, sampling())
            .await
            .unwrap_err()
            .contains("stopped")
    );
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_prompt_reaches_the_first_stage_and_the_last_stage_can_end_the_answer() {
    let rig = Rig::start(&[("end-s2", "2")]).await;
    rig.ready(1).await;
    let prompt = serde_json::json!([{"role": "user", "content": "hi"}]);
    let mut run = rig
        .group()
        .generate_prompt(prompt.clone(), 5, sampling())
        .await
        .unwrap();
    let done = tokio::time::timeout(Duration::from_secs(10), &mut run.done)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    // The last stage sampled the end at step 2: three tokens, not five.
    assert_eq!(done.tokens, 3);
    let mut tokens = vec![];
    while let Ok(token) = run.tokens.try_recv() {
        tokens.push(token);
    }
    assert_eq!(tokens, toks(0..3));
    let mut text = String::new();
    while let Ok(piece) = run.text.try_recv() {
        text.push_str(&piece);
    }
    assert_eq!(text, "<7000><7001><7002>");
    let prompts: Vec<_> = rig
        .events()
        .into_iter()
        .filter(|e| e["event"] == "prompt")
        .collect();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0]["stage"], "s0");
    assert_eq!(prompts[0]["prompt"], prompt);
    // The group takes the next request.
    assert_eq!(rig.complete(2).await, toks(0..2));
    rig.close().await;
}
