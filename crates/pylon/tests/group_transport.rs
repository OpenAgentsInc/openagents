#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The direct stage-to-stage transport (W12, #12399) against fake stages.
//!
//! Each stage is `fixtures/stage_worker.py` in its own process, with a
//! link listener on its own loopback port. Stage state goes from stage to
//! stage over `stage_link` connections (the fixture's `stage_link.py`
//! speaks the wire); only the last stage answers the front. Faults are
//! one-shot files in a private directory; nothing reaches a network.

use std::sync::Arc;
use std::time::{Duration, Instant};

use pylon::engine::Sampling;
use pylon::stage_group::{
    Failure, LinkPolicy, PROTOCOL, StageGroup, StageGroupConfig, StageGroupStatus, StageSpec,
    Transport,
};
use serde_json::Value;

const STAGES: [&str; 4] = ["s0", "s1", "s2", "s3"];

fn sampling() -> Sampling {
    Sampling::new(0.6, 0.95, 7).unwrap()
}

fn toks(steps: std::ops::Range<u64>) -> Vec<u32> {
    steps.map(|s| 7000 + s as u32).collect()
}

struct Rig {
    root: tempfile::TempDir,
    group: StageGroup,
}

impl Rig {
    async fn start(
        transport: Transport,
        flags: &[(&str, &str)],
        extra: &[&str],
        change: impl FnOnce(&mut StageGroupConfig),
    ) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("stage.py");
        std::fs::write(&script, include_bytes!("fixtures/stage_worker.py")).unwrap();
        std::fs::write(
            root.path().join("stage_link.py"),
            include_bytes!("fixtures/stage_link.py"),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        for (flag, value) in flags {
            std::fs::write(root.path().join(flag), value).unwrap();
        }
        let mut args = vec!["--root".to_string(), root.path().display().to_string()];
        if transport == Transport::Direct {
            args.extend(["--data-listen".into(), "127.0.0.1:0".into()]);
        }
        args.extend(extra.iter().map(|a| (*a).to_string()));
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
                    args: args.clone(),
                    identity: Default::default(),
                })
                .collect(),
            transport,
            links: LinkPolicy::default(),
            step_timeout: Duration::from_millis(1500),
            health_timeout: Duration::from_millis(800),
            startup_timeout: Duration::from_secs(8),
            stop_timeout: Duration::from_millis(800),
            max_restarts: 3,
            backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        };
        change(&mut config);
        let group = StageGroup::start(config, Arc::new(|s| eprintln!("group: {s}")))
            .await
            .unwrap();
        Self { root, group }
    }

    async fn direct(flags: &[(&str, &str)]) -> Self {
        Self::start(Transport::Direct, flags, &[], |_| {}).await
    }

    fn status(&self) -> StageGroupStatus {
        self.group.status()
    }

    fn flag(&self, name: &str, value: &str) {
        std::fs::write(self.root.path().join(name), value).unwrap();
    }

    fn unflag(&self, name: &str) {
        let _ = std::fs::remove_file(self.root.path().join(name));
    }

    fn events(&self, kind: &str) -> Vec<Value> {
        std::fs::read_to_string(self.root.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|e| e["event"] == kind)
            .collect()
    }

    fn at(&self, kind: &str, stage: &str) -> Vec<Value> {
        self.events(kind)
            .into_iter()
            .filter(|e| e["stage"] == stage)
            .collect()
    }

    async fn until(
        &self,
        what: &str,
        mut check: impl FnMut(&StageGroupStatus) -> bool,
    ) -> StageGroupStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
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

    /// Run a request to its end: the tokens that reached the caller and
    /// how it ended.
    async fn run(&self, steps: u64) -> (Vec<u32>, Result<pylon::stage_group::Done, Failure>) {
        let mut run = self.group.generate(steps, sampling()).await.unwrap();
        let done = tokio::time::timeout(Duration::from_secs(20), &mut run.done)
            .await
            .expect("the request ended")
            .unwrap();
        // Anything sent after the end would still be here: nothing is
        // replayed after tokens reached the caller.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut tokens = Vec::new();
        while let Ok(token) = run.tokens.try_recv() {
            tokens.push(token);
        }
        (tokens, done)
    }

    async fn stop(self) {
        self.group.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stage_state_goes_stage_to_stage_and_only_the_last_stage_answers() {
    let rig = Rig::direct(&[]).await;
    let status = rig.ready(1).await;
    assert_eq!(status.transport, Transport::Direct);
    // Every stage but the last linked to the next stage.
    for (i, stage) in STAGES.iter().enumerate() {
        assert_eq!(rig.at("linked", stage).len(), usize::from(i < 3), "{stage}");
    }

    let (tokens, done) = rig.run(5).await;
    assert_eq!(tokens, toks(0..5));
    assert_eq!(done.unwrap().tokens, 5);

    // The front sent each step only to the first stage; the others got
    // their state from the stage before them.
    assert_eq!(rig.events("step").len(), 5);
    assert!(rig.events("step").iter().all(|e| e["stage"] == "s0"));
    for stage in ["s1", "s2", "s3"] {
        let data = rig.at("data", stage);
        assert_eq!(data.len(), 5, "{stage}");
        assert!(data.iter().all(|e| e["bytes"] == 44_300));
    }
    // Per-hop bytes: 44,300 of state plus the frame's own fields and the
    // head's sampling settings.
    let sizes: Vec<u64> = rig
        .events("forwarded")
        .iter()
        .map(|e| e["bytes"].as_u64().unwrap())
        .collect();
    assert_eq!(sizes.len(), 15);
    let per_hop = sizes[0];
    assert!(sizes.iter().all(|&b| b == per_hop));
    let extra = per_hop - 44_300 - stage_link::FRAME_OVERHEAD as u64;
    assert!(extra < 200, "{extra}");
    let status = rig.status();
    for (i, stage) in status.stages.iter().enumerate() {
        let hops = if i < 3 { 5 } else { 0 };
        assert_eq!(stage.forwarded, hops, "{}", stage.name);
        assert_eq!(stage.forwarded_bytes, hops * per_hop, "{}", stage.name);
    }
    eprintln!(
        "direct transport: {per_hop} bytes per hop per token: 44,300 of stage state, {} of framing and MAC, {extra} of request ID and settings",
        stage_link::FRAME_OVERHEAD
    );
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loopback_latency_per_token_front_and_direct() {
    const TOKENS: u64 = 60;
    let mut results = Vec::new();
    // The fake front transport carries no stage state, so the direct
    // transport runs twice: with 32 bytes of state (the transport alone)
    // and with V4.1's 44,300.
    for (transport, extra) in [
        (Transport::Front, &[][..]),
        (Transport::Direct, &["--state-bytes", "32"][..]),
        (Transport::Direct, &[][..]),
    ] {
        let rig = Rig::start(transport, &[], extra, |_| {}).await;
        rig.ready(1).await;
        let started = Instant::now();
        let mut run = rig.group.generate(TOKENS, sampling()).await.unwrap();
        let mut gaps = Vec::new();
        let mut last = started;
        while let Some(token) = run.tokens.recv().await {
            let now = Instant::now();
            gaps.push(now - last);
            last = now;
            if u64::from(token - 7000) + 1 == TOKENS {
                break;
            }
        }
        (&mut run.done).await.unwrap().unwrap();
        let total = started.elapsed();
        gaps.sort();
        let median = gaps[gaps.len() / 2];
        eprintln!(
            "{transport:?} transport {extra:?}, 4 fake stages on loopback: {TOKENS} tokens in {total:?}, median {median:?} per token"
        );
        results.push(median);
        rig.stop().await;
    }
    assert_eq!(results.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dropped_link_fails_the_request_visibly_and_the_group_relinks() {
    let rig = Rig::direct(&[("drop-link-s1", "2")]).await;
    rig.ready(1).await;
    let (tokens, done) = rig.run(6).await;
    let failure = done.unwrap_err();
    eprintln!("dropped link: {failure}");
    // Two tokens reached the caller, and nothing after them.
    assert_eq!(tokens, toks(0..2));
    assert_eq!(failure.delivered, 2);
    assert!(!failure.cancelled);
    assert!(failure.reason.contains("stage s1 failed"), "{failure}");
    assert!(failure.reason.contains("closed the link"), "{failure}");
    assert_eq!(rig.at("link-failed", "s2")[0]["peer"], "prev");

    // A new generation relinks every stage with a new key and serves.
    rig.ready(2).await;
    assert_eq!(rig.at("linked", "s0").len(), 2);
    let (tokens, done) = rig.run(6).await;
    assert_eq!(tokens, toks(0..6));
    assert_eq!(done.unwrap().generation, 2);
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_old_generation_cannot_rejoin_the_chain() {
    // s0 keeps a copy of itself with generation 1's key; s3 crashes, so
    // the group moves to generation 2 before the copy comes back.
    let rig = Rig::direct(&[("late-link-s0", "1500"), ("crash-before-ack-s3", "1")]).await;
    rig.ready(1).await;
    let (tokens, done) = rig.run(4).await;
    assert_eq!(tokens, toks(0..1));
    assert_eq!(done.unwrap_err().delivered, 1);
    rig.ready(2).await;
    let late = loop {
        if let Some(late) = rig.events("late-link").pop() {
            break late;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    eprintln!("late copy of s0: {late}");
    assert_eq!(late["generation"], 1);
    assert!(late["result"].as_str().unwrap().contains("generation"));
    let refused = rig.at("link-refused", "s1");
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert_eq!(refused[0]["claimed"]["generation"], 1);
    // The current chain is untouched.
    let (tokens, _) = rig.run(3).await;
    assert_eq!(tokens, toks(0..3));
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_duplicated_frame_is_refused_and_never_run_twice() {
    let rig = Rig::direct(&[("dup-frame-s0", "1")]).await;
    rig.ready(1).await;
    let (tokens, done) = rig.run(4).await;
    assert_eq!(tokens, toks(0..4));
    assert_eq!(done.unwrap().tokens, 4);
    // s1 ran step 1 once and told the front it refused the copy.
    let steps: Vec<u64> = rig
        .at("data", "s1")
        .iter()
        .map(|e| e["step"].as_u64().unwrap())
        .collect();
    assert_eq!(steps, [0, 1, 2, 3]);
    let status = rig.status();
    assert_eq!(status.refused, 1);
    assert!(
        status
            .last_refusal
            .as_deref()
            .unwrap()
            .contains("stage s1: step 1 chunk 0 of request 1 arrived twice")
    );
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_partitioned_middle_stage_fails_the_step_where_it_stopped() {
    // s1 stops reading its inbound link at step 2 but still answers the
    // front's health checks.
    let rig = Rig::direct(&[("partition-s1", "2")]).await;
    rig.ready(1).await;
    let started = Instant::now();
    let (tokens, done) = rig.run(6).await;
    let failure = done.unwrap_err();
    eprintln!("partition after {:?}: {failure}", started.elapsed());
    assert_eq!(tokens, toks(0..2));
    assert_eq!(failure.delivered, 2);
    assert!(
        failure
            .reason
            .contains("stage s1 failed: step 2 made no progress"),
        "{failure}"
    );
    rig.ready(2).await;
    let (tokens, _) = rig.run(3).await;
    assert_eq!(tokens, toks(0..3));
    rig.stop().await;
}

/// Settings for the backpressure tests: step zero in 12 frames of 256 KiB,
/// small socket buffers, and a queue of two.
const BIG: &[&str] = &["--state-bytes", "262144", "--buffers", "65536"];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_stage_backs_up_a_bounded_queue_and_the_answer_still_arrives() {
    let rig = Rig::start(
        Transport::Direct,
        &[("chunks-s0", "12"), ("link-slow-s2", "60")],
        BIG,
        |c| {
            c.links = LinkPolicy {
                queue: 2,
                send_timeout_ms: 5_000,
            };
            c.step_timeout = Duration::from_secs(10);
        },
    )
    .await;
    rig.ready(1).await;
    let (tokens, done) = rig.run(2).await;
    assert_eq!(tokens, toks(0..2));
    done.unwrap();
    let peaks: Vec<u64> = rig
        .at("forwarded", "s1")
        .iter()
        .map(|e| e["peak"].as_u64().unwrap())
        .collect();
    eprintln!("s1's queue while s2 was slow: {peaks:?}");
    assert_eq!(peaks.len(), 13);
    assert_eq!(peaks.iter().max(), Some(&2), "the queue filled and held");
    // Each stage ran each of step zero's frames once.
    assert_eq!(rig.at("data", "s3").len(), 13);
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stalled_stage_fails_its_link_and_the_request_visibly() {
    let rig = Rig::start(
        Transport::Direct,
        &[("chunks-s0", "12"), ("partition-s2", "0")],
        BIG,
        |c| {
            c.links = LinkPolicy {
                queue: 2,
                send_timeout_ms: 400,
            };
            c.step_timeout = Duration::from_secs(10);
        },
    )
    .await;
    rig.ready(1).await;
    let started = Instant::now();
    let (tokens, done) = rig.run(3).await;
    let failure = done.unwrap_err();
    eprintln!("stall after {:?}: {failure}", started.elapsed());
    assert!(tokens.is_empty());
    assert_eq!(failure.delivered, 0);
    assert!(failure.reason.contains("stage s2 failed"), "{failure}");
    assert!(failure.reason.contains("isn't keeping up"), "{failure}");
    assert!(started.elapsed() < Duration::from_secs(5));
    rig.unflag("chunks-s0");
    rig.ready(2).await;
    let (tokens, _) = rig.run(3).await;
    assert_eq!(tokens, toks(0..3));
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancellation_passes_down_the_chain_and_holds_the_slot_until_every_stage_stops() {
    let rig = Rig::direct(&[]).await;
    rig.ready(1).await;
    rig.flag("slow-s2", "300");
    let mut run = rig.group.generate(10, sampling()).await.unwrap();
    assert_eq!(run.tokens.recv().await, Some(7000));
    drop(run.tokens);
    let failure = (&mut run.done).await.unwrap().unwrap_err();
    assert!(failure.cancelled);
    assert_eq!(failure.delivered, 1);
    // The slot stays held until every stage says it stopped.
    let busy = rig.status();
    assert!(busy.busy, "{busy:?}");
    rig.until("every stage to stop", |s| !s.busy).await;
    for stage in STAGES {
        assert_eq!(rig.at("stopped", stage).len(), 1, "{stage}");
    }
    // The first stage started the cancellation; each later stage got it
    // from the stage before and passed it on.
    assert_eq!(rig.at("cancel-passed", "s0").len(), 1);
    for stage in ["s1", "s2"] {
        assert_eq!(rig.at("cancel-frame", stage).len(), 1, "{stage}");
        assert_eq!(rig.at("cancel-passed", stage).len(), 1, "{stage}");
    }
    assert_eq!(rig.at("cancel-frame", "s3").len(), 1);
    rig.unflag("slow-s2");
    let (tokens, _) = rig.run(2).await;
    assert_eq!(tokens, toks(0..2));
    assert_eq!(rig.status().generation, 1);
    rig.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stage_without_a_link_address_never_makes_a_direct_group_ready() {
    // Stages without --data-listen announce no address.
    let rig = Rig::start(Transport::Direct, &[], &[], |c| {
        for stage in &mut c.stages {
            stage
                .args
                .retain(|a| a != "--data-listen" && a != "127.0.0.1:0");
        }
        c.startup_timeout = Duration::from_millis(600);
        c.max_restarts = 1;
    })
    .await;
    let status = rig.until("the group to stop", |s| s.failed.is_some()).await;
    assert!(!status.ready);
    assert!(
        status.reason.contains("no link address")
            || status.failed.as_deref().unwrap().contains("ready in time"),
        "{status:?}"
    );
    rig.stop().await;
}
