#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The stage-group lifecycle against the real V4.1 stage worker.
//!
//! These run W10's front against `psionic-v41-stage` (W11) with the tiny
//! synthetic model, over loopback, instead of the Python fixture. Build the
//! worker in the Psionic workspace and point `PSIONIC_V41_STAGE` at it:
//!
//! ```text
//! cargo build --manifest-path crates/psionic/Cargo.toml -p psionic-v41-stage
//! PSIONIC_V41_STAGE=$CARGO_TARGET_DIR/debug/psionic-v41-stage cargo test -p pylon --test group_stage_worker
//! ```
//!
//! Without it, each test says so and passes. A qualification run sets
//! `PSIONIC_V41_STAGE_REQUIRED=1`, and then a missing or broken worker fails
//! every test instead:
//!
//! - `PSIONIC_V41_STAGE_SHA256` pins the exact binary under test;
//! - `PSIONIC_V41_STAGE_BACKEND=cuda` runs every stage and the reference on
//!   the GPU, and a machine with no GPU fails;
//! - `PSIONIC_V41_STAGE_REPORT=FILE` appends one JSON line per test, and
//!   each test prints the executed, skipped, and failed counts so far.

use pylon::engine::{Sampling, request_limit_reason};
use pylon::stage_group::{
    LinkPolicy, PROTOCOL, PromptMode, StageGroup, StageGroupConfig, StageGroupStatus, StageSpec,
    Transport,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// A qualification run: missing prerequisites fail instead of skipping.
fn required() -> bool {
    env("PSIONIC_V41_STAGE_REQUIRED").as_deref() == Some("1")
}

/// The stages run on the GPU.
fn cuda() -> bool {
    env("PSIONIC_V41_STAGE_BACKEND").as_deref() == Some("cuda")
}

fn runtime() -> &'static str {
    if cuda() {
        "psionic-v41-stage/1+cuda"
    } else {
        "psionic-v41-stage/1"
    }
}

/// The worker under test, or why it can't run here.
fn worker() -> Result<PathBuf, String> {
    let path = PathBuf::from(
        env("PSIONIC_V41_STAGE")
            .ok_or("PSIONIC_V41_STAGE is not set to a built psionic-v41-stage")?,
    );
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("PSIONIC_V41_STAGE={} can't be read: {e}", path.display()))?;
    if let Some(pin) = env("PSIONIC_V41_STAGE_SHA256") {
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != pin.trim_start_matches("sha256:") {
            return Err(format!(
                "{} has SHA-256 {actual}; the run pins {pin}",
                path.display()
            ));
        }
    }
    if cuda() {
        let gpus = Command::new("nvidia-smi").arg("-L").output();
        match gpus {
            Ok(out) if out.status.success() && !out.stdout.is_empty() => {}
            _ => {
                return Err("PSIONIC_V41_STAGE_BACKEND=cuda, but no GPU answers nvidia-smi".into());
            }
        }
    }
    Ok(path)
}

static EXECUTED: AtomicUsize = AtomicUsize::new(0);
static SKIPPED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn report(test: &str, outcome: &str, detail: &str) {
    let counter = match outcome {
        "executed" => &EXECUTED,
        "skipped" => &SKIPPED,
        _ => &FAILED,
    };
    counter.fetch_add(1, Ordering::SeqCst);
    let (executed, skipped, failed) = (
        EXECUTED.load(Ordering::SeqCst),
        SKIPPED.load(Ordering::SeqCst),
        FAILED.load(Ordering::SeqCst),
    );
    eprintln!(
        "group_stage_worker: {test} {outcome}{}{detail}; executed {executed}, skipped {skipped}, failed {failed}",
        if detail.is_empty() { "" } else { ": " }
    );
    if let Some(path) = env("PSIONIC_V41_STAGE_REPORT") {
        use std::io::Write;
        let line = serde_json::json!({
            "test": test,
            "outcome": outcome,
            "detail": detail,
            "backend": if cuda() { "cuda" } else { "cpu" },
            "required": required(),
        });
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path);
        if let Ok(mut file) = file {
            let _ = file.write_all(format!("{line}\n").as_bytes());
        }
    }
}

/// Reports a test that ran: executed, or failed when it panicked.
struct Ran(&'static str);

impl Drop for Ran {
    fn drop(&mut self) {
        let outcome = if std::thread::panicking() {
            "failed"
        } else {
            "executed"
        };
        report(self.0, outcome, "");
    }
}

macro_rules! worker_or_skip {
    ($test:literal) => {
        match worker() {
            Ok(path) => (path, Ran($test)),
            Err(why) if required() => {
                report($test, "failed", &why);
                panic!("PSIONIC_V41_STAGE_REQUIRED=1, but {why}");
            }
            Err(why) => {
                report($test, "skipped", &why);
                return;
            }
        }
    };
}

fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn run(bin: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new(bin).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// A synthetic weights directory named `name`, with the group manifest for
/// `split` in it; returns the weights digest and the manifest digest.
fn weights(bin: &Path, dir: &Path, name: &str, split: &[(u32, u32)]) -> (String, String) {
    let d = dir.display().to_string();
    run(bin, &["synthetic", &d, name]);
    let split_arg = split
        .iter()
        .map(|(a, b)| format!("{a}-{b}"))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = run(bin, &["manifest", "--split", &split_arg]);
    std::fs::write(dir.join("group-manifest.json"), &manifest).unwrap();
    let digest = |bytes: &[u8]| format!("sha256:{:x}", Sha256::digest(bytes));
    (
        digest(&std::fs::read(dir.join("synthetic.json")).unwrap()),
        digest(&manifest),
    )
}

fn stage_args(dir: &Path, manifest: &str, index: usize, split: &[(u32, u32)]) -> Vec<String> {
    let (a, b) = split[index];
    // A test machine may have one GPU: every CUDA stage shares the first.
    let gpu = if cuda() { 0 } else { index };
    let mut args = vec![
        "--weights".into(),
        dir.display().to_string(),
        "--layers".into(),
        format!("{a}-{b}"),
        "--gpu".into(),
        gpu.to_string(),
        "--manifest".into(),
        manifest.into(),
        "--events".into(),
        dir.join("events.jsonl").display().to_string(),
    ];
    if index == 0 {
        args.push("--embeddings".into());
    }
    if index + 1 == split.len() {
        args.push("--head".into());
    }
    if cuda() {
        args.extend(["--backend".into(), "cuda".into()]);
    }
    args
}

struct Rig {
    _root: tempfile::TempDir,
    dir: PathBuf,
    bin: PathBuf,
    group: Option<StageGroup>,
}

impl Rig {
    async fn start(bin: PathBuf, split: &[(u32, u32)]) -> Self {
        Self::with(bin, split, |_, _| {}).await
    }

    async fn with(
        bin: PathBuf,
        split: &[(u32, u32)],
        change: impl FnOnce(&mut StageGroupConfig, &Path),
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("weights");
        let (digest, manifest) = weights(&bin, &dir, "synthetic", split);
        let mut config = StageGroupConfig {
            model: "deepseek-v4.1-synthetic".into(),
            weights: digest,
            runtime: runtime().into(),
            protocol: PROTOCOL,
            build: None,
            stages: (0..split.len())
                .map(|i| StageSpec {
                    name: format!("s{i}"),
                    program: bin.clone(),
                    args: stage_args(&dir, &manifest, i, split),
                })
                .collect(),
            transport: Transport::Front,
            links: LinkPolicy::default(),
            step_timeout: Duration::from_secs(10),
            health_timeout: Duration::from_millis(800),
            startup_timeout: Duration::from_secs(20),
            stop_timeout: Duration::from_millis(800),
            max_restarts: 3,
            backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        };
        change(&mut config, root.path());
        let group = StageGroup::start(config, Arc::new(|s| eprintln!("group: {s}")))
            .await
            .unwrap();
        Self {
            _root: root,
            dir,
            bin,
            group: Some(group),
        }
    }

    fn group(&self) -> &StageGroup {
        self.group.as_ref().unwrap()
    }

    fn status(&self) -> StageGroupStatus {
        self.group().status()
    }

    async fn until(
        &self,
        what: &str,
        mut check: impl FnMut(&StageGroupStatus) -> bool,
    ) -> StageGroupStatus {
        let deadline = Instant::now() + Duration::from_secs(30);
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

    /// The group's answer: tokens, text, and how many tokens it delivered.
    async fn answer(
        &self,
        prompt: Option<serde_json::Value>,
        steps: u64,
        sampling: Sampling,
    ) -> (Vec<u32>, String) {
        let mut run = match prompt {
            Some(prompt) => self
                .group()
                .generate_prompt(prompt, steps, sampling)
                .await
                .unwrap(),
            None => self.group().generate(steps, sampling).await.unwrap(),
        };
        let done = tokio::time::timeout(Duration::from_secs(60), &mut run.done)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let mut tokens = vec![];
        while let Ok(token) = run.tokens.try_recv() {
            tokens.push(token);
        }
        let mut text = String::new();
        while let Ok(piece) = run.text.try_recv() {
            text.push_str(&piece);
        }
        assert_eq!(done.tokens, tokens.len() as u64);
        (tokens, text)
    }

    /// What the unsplit model answers, from the worker's `reference` mode.
    fn reference(
        &self,
        prompt: &serde_json::Value,
        steps: u64,
        (temperature, top_p, seed): (f64, f64, u32),
    ) -> (Vec<u32>, String) {
        let out = run(
            &self.bin,
            &[
                "reference",
                "--weights",
                &self.dir.display().to_string(),
                "--prompt",
                &prompt.to_string(),
                "--steps",
                &steps.to_string(),
                "--temperature",
                &temperature.to_string(),
                "--top-p",
                &top_p.to_string(),
                "--seed",
                &seed.to_string(),
                "--backend",
                if cuda() { "cuda" } else { "cpu" },
            ],
        );
        let answer: serde_json::Value = serde_json::from_slice(&out).unwrap();
        (
            answer["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_u64().unwrap() as u32)
                .collect(),
            answer["text"].as_str().unwrap().to_owned(),
        )
    }

    fn events(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.dir.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    async fn close(mut self) {
        let pids: Vec<u32> = self.status().stages.iter().filter_map(|s| s.pid).collect();
        self.group.take().unwrap().stop().await;
        for pid in pids {
            assert!(!alive(pid), "stage process {pid} survived shutdown");
        }
    }
}

fn greedy() -> Sampling {
    Sampling::new(0.0, 1.0, 0).unwrap()
}

fn prompt() -> serde_json::Value {
    serde_json::json!([{"role": "user", "content": "tok3 tok5 tok0 tok6 tok4 tok7"}])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loopback_groups_of_two_to_four_stages_answer_like_the_unsplit_model() {
    let (bin, _ran) =
        worker_or_skip!("loopback_groups_of_two_to_four_stages_answer_like_the_unsplit_model");
    for split in [
        vec![(0, 20), (20, 40)],
        vec![(0, 7), (7, 21), (21, 40)],
        vec![(0, 10), (10, 20), (20, 30), (30, 40)],
    ] {
        let rig = Rig::start(bin.clone(), &split).await;
        rig.ready(1).await;
        let expected = rig.reference(&prompt(), 12, (0.0, 1.0, 0));
        assert_eq!(expected.0.len(), 12);
        assert_eq!(rig.answer(Some(prompt()), 12, greedy()).await, expected);
        // Sampled, with the front's temperature, top-p, and seed.
        let sampled = Sampling::new(0.9, 0.95, 11).unwrap();
        let expected = rig.reference(&prompt(), 12, (0.9, 0.95, 11));
        assert_eq!(rig.answer(Some(prompt()), 12, sampled).await, expected);
        let status = rig.status();
        assert_eq!((status.generation, status.refused), (1, 0), "{status:?}");
        rig.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_last_stage_ends_the_answer_and_its_text_reaches_the_caller() {
    let (bin, _ran) =
        worker_or_skip!("the_last_stage_ends_the_answer_and_its_text_reaches_the_caller");
    let split = [(0, 14), (14, 40)];
    let rig = Rig::start(bin, &split).await;
    rig.ready(1).await;
    let prompt = serde_json::json!([{"role": "user", "content": "tok3 tok5 hello tok6"}]);
    let expected = rig.reference(&prompt, 12, (0.9, 0.95, 7));
    // This prompt and seed reach the end token before the limit.
    assert_eq!(expected.0.last(), Some(&1));
    assert!(expected.0.len() < 12);
    let got = rig
        .answer(
            Some(prompt.clone()),
            12,
            Sampling::new(0.9, 0.95, 7).unwrap(),
        )
        .await;
    assert_eq!(got, expected);
    assert!(got.1.starts_with("tok"), "{got:?}");
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_killed_stage_is_replaced_fenced_and_the_group_answers_the_same() {
    let (bin, _ran) =
        worker_or_skip!("a_killed_stage_is_replaced_fenced_and_the_group_answers_the_same");
    let split = [(0, 10), (10, 25), (25, 40)];
    let rig = Rig::start(bin, &split).await;
    rig.ready(1).await;
    let before = rig.answer(Some(prompt()), 6, greedy()).await;
    let pids = rig.pids();
    unsafe { libc::kill(pids[1] as i32, libc::SIGKILL) };
    let status = rig.ready(2).await;
    let after = rig.pids();
    assert_eq!((after[0], after[2]), (pids[0], pids[2]));
    assert_ne!(after[1], pids[1]);
    assert_eq!(status.stages[1].replacements, 1);
    let fenced: Vec<_> = rig
        .events()
        .into_iter()
        .filter(|e| e["event"] == "fenced" && e["generation"] == 2)
        .map(|e| e["stage"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        fenced.contains(&"s0".into()) && fenced.contains(&"s2".into()),
        "{:#?}",
        rig.events()
    );
    // The survivors dropped the old generation's state; the answer repeats.
    assert_eq!(rig.answer(Some(prompt()), 6, greedy()).await, before);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stopped_stage_fails_its_health_checks_and_is_replaced() {
    let (bin, _ran) = worker_or_skip!("a_stopped_stage_fails_its_health_checks_and_is_replaced");
    let split = [(0, 20), (20, 40)];
    let rig = Rig::start(bin, &split).await;
    rig.ready(1).await;
    let expected = rig.answer(None, 3, greedy()).await;
    let before = rig.pids();
    unsafe { libc::kill(before[1] as i32, libc::SIGSTOP) };
    let status = rig.ready(2).await;
    assert_ne!(status.stages[1].pid, Some(before[1]));
    assert!(!alive(before[1]), "the stopped stage was not reaped");
    assert_eq!(rig.answer(None, 3, greedy()).await, expected);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_caller_that_goes_away_holds_the_slot_until_every_stage_stops() {
    let (bin, _ran) =
        worker_or_skip!("a_caller_that_goes_away_holds_the_slot_until_every_stage_stops");
    let split = [(0, 13), (13, 27), (27, 40)];
    let rig = Rig::start(bin, &split).await;
    rig.ready(1).await;
    let mut run = rig
        .group()
        .generate_prompt(prompt(), 200, greedy())
        .await
        .unwrap();
    assert!(run.tokens.recv().await.is_some());
    drop(run.tokens);
    let failure = run.done.await.unwrap().unwrap_err();
    assert!(failure.cancelled);
    let status = rig.until("a free slot", |s| !s.busy).await;
    assert!(status.ready);
    assert_eq!(status.generation, 1);
    let stopped: Vec<_> = rig
        .events()
        .into_iter()
        .filter(|e| e["event"] == "stopped" && e["request"] == "1")
        .map(|e| e["stage"].as_str().unwrap().to_owned())
        .collect();
    for stage in ["s0", "s1", "s2"] {
        assert!(
            stopped.iter().any(|s| s == stage),
            "{stage} did not stop: {:#?}",
            rig.events()
        );
    }
    let expected = rig.reference(&prompt(), 4, (0.0, 1.0, 0));
    assert_eq!(rig.answer(Some(prompt()), 4, greedy()).await, expected);
    rig.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stage_with_other_weights_is_never_advertised() {
    let (bin, _ran) = worker_or_skip!("a_stage_with_other_weights_is_never_advertised");
    let split = [(0, 20), (20, 40)];
    let rig = Rig::with(bin.clone(), &split, |config, root| {
        // Stage s1 loads a different synthetic model directory.
        let other = root.join("other");
        let (_, manifest) = weights(&bin, &other, "other", &split);
        config.stages[1].args = stage_args(&other, &manifest, 1, &split);
        config.startup_timeout = Duration::from_millis(1500);
        config.max_restarts = 2;
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
    assert!(status.stages.iter().all(|s| s.pid.is_none()));
    rig.close().await;
}

/// W12's direct transport against the real worker: the front sends each
/// step to stage 0 only, stage state goes stage to stage over authenticated
/// `stage_link` connections, and only the last stage answers.
fn direct(config: &mut StageGroupConfig) {
    config.transport = Transport::Direct;
    for stage in &mut config.stages {
        stage
            .args
            .extend(["--data-listen".into(), "127.0.0.1:0".into()]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_groups_pass_state_stage_to_stage_and_answer_like_the_unsplit_model() {
    let (bin, _ran) = worker_or_skip!(
        "direct_groups_pass_state_stage_to_stage_and_answer_like_the_unsplit_model"
    );
    for split in [
        vec![(0, 20), (20, 40)],
        vec![(0, 10), (10, 20), (20, 30), (30, 40)],
    ] {
        let rig = Rig::with(bin.clone(), &split, |config, _| direct(config)).await;
        let status = rig.ready(1).await;
        assert_eq!(status.transport, Transport::Direct);
        let expected = rig.reference(&prompt(), 12, (0.0, 1.0, 0));
        assert_eq!(rig.answer(Some(prompt()), 12, greedy()).await, expected);
        let expected = rig.reference(&prompt(), 12, (0.9, 0.95, 11));
        assert_eq!(
            rig.answer(Some(prompt()), 12, Sampling::new(0.9, 0.95, 11).unwrap())
                .await,
            expected
        );
        let status = rig.status();
        assert_eq!((status.generation, status.failures), (1, 0), "{status:?}");
        // Every stage but the last passed each of the 24 steps on. A
        // request's last `forwarded` report can reach the front after the
        // last stage's answer has ended it, so it may not be counted.
        let last = split.len() - 1;
        for (i, stage) in status.stages.iter().enumerate() {
            if i == last {
                assert_eq!(stage.forwarded, 0, "{status:?}");
            } else {
                assert!((22..=24).contains(&stage.forwarded), "{status:?}");
                assert!(stage.forwarded_bytes > 22 * 100, "{status:?}");
            }
        }
        rig.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_group_relinks_after_a_killed_stage_and_cancels_down_the_chain() {
    let (bin, _ran) =
        worker_or_skip!("a_direct_group_relinks_after_a_killed_stage_and_cancels_down_the_chain");
    let split = [(0, 10), (10, 25), (25, 40)];
    let rig = Rig::with(bin, &split, |config, _| direct(config)).await;
    rig.ready(1).await;
    let before = rig.answer(Some(prompt()), 6, greedy()).await;
    let pids = rig.pids();
    unsafe { libc::kill(pids[1] as i32, libc::SIGKILL) };
    rig.ready(2).await;
    assert_eq!(rig.answer(Some(prompt()), 6, greedy()).await, before);
    let linked: Vec<_> = rig
        .events()
        .into_iter()
        .filter(|e| e["event"] == "linked" && e["generation"] == 2)
        .map(|e| e["stage"].as_str().unwrap().to_owned())
        .collect();
    for stage in ["s0", "s1", "s2"] {
        assert!(linked.iter().any(|s| s == stage), "{linked:?}");
    }
    // A caller that goes away: stage 0 sends the cancellation down the chain.
    let mut run = rig
        .group()
        .generate_prompt(prompt(), 200, greedy())
        .await
        .unwrap();
    assert!(run.tokens.recv().await.is_some());
    drop(run.tokens);
    assert!(run.done.await.unwrap().unwrap_err().cancelled);
    let status = rig.until("a free slot", |s| !s.busy).await;
    assert!(status.ready && status.generation == 2, "{status:?}");
    let events = rig.events();
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "cancel-passed" && e["stage"] == "s0"),
        "{events:#?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "cancel-frame" && e["stage"] == "s1"),
        "{events:#?}"
    );
    assert_eq!(rig.answer(Some(prompt()), 6, greedy()).await, before);
    rig.close().await;
}

/// Every stage takes `extra` arguments.
fn with_args(config: &mut StageGroupConfig, extra: &[&str]) {
    for stage in &mut config.stages {
        stage.args.extend(extra.iter().map(|a| (*a).to_owned()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_oversized_request_fails_alone_and_the_group_stays_ready() {
    let (bin, _ran) = worker_or_skip!("an_oversized_request_fails_alone_and_the_group_stays_ready");
    let split = [(0, 10), (10, 25), (25, 40)];
    for transport in [Transport::Front, Transport::Direct] {
        let rig = Rig::with(bin.clone(), &split, |config, _| {
            with_args(config, &["--max-context", "64", "--max-input-tokens", "4"]);
            if transport == Transport::Direct {
                direct(config);
            }
        })
        .await;
        let status = rig.ready(1).await;
        let limits = status.limits.clone().expect("the stages announce limits");
        assert_eq!(
            (limits.max_context_tokens, limits.max_input_tokens),
            (64, 4),
            "{limits:?}"
        );
        assert_eq!(limits.modes, [PromptMode::Thinking, PromptMode::Chat]);
        // Six prompt tokens: stage 0 rejects the request before any stage
        // runs it, and only the request fails.
        let run = rig
            .group()
            .generate_prompt(prompt(), 8, greedy())
            .await
            .unwrap();
        let failure = tokio::time::timeout(Duration::from_secs(20), run.done)
            .await
            .expect("the request ended before its step deadline")
            .unwrap()
            .unwrap_err();
        assert!(failure.request_limit, "{failure:?}");
        assert!(!failure.cancelled && failure.delivered == 0);
        let reason = request_limit_reason(&failure.reason).expect("a marked limit");
        assert!(reason.contains("at most 4"), "{reason}");
        // The stages stop the request before the slot frees.
        rig.until("a free slot", |s| !s.busy).await;
        // An answer longer than the context is refused at admission.
        let refused = rig
            .group()
            .generate_prompt(prompt(), 64, greedy())
            .await
            .unwrap_err();
        assert!(request_limit_reason(&refused).is_some(), "{refused}");
        // The group stays at its generation with no failures, and answers.
        let status = rig.status();
        assert_eq!((status.generation, status.failures), (1, 0), "{status:?}");
        assert!(status.stages.iter().all(|s| s.replacements == 0));
        let short = serde_json::json!([{"role": "user", "content": "tok3 tok5"}]);
        let expected = rig.reference(&short, 4, (0.0, 1.0, 0));
        assert_eq!(rig.answer(Some(short), 4, greedy()).await, expected);
        let events = rig.events();
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "rejected" && e["stage"] == "s0"),
            "{events:#?}"
        );
        rig.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_prompt_mode_reaches_every_stage() {
    let (bin, _ran) = worker_or_skip!("the_prompt_mode_reaches_every_stage");
    let split = [(0, 10), (10, 25), (25, 40)];
    for transport in [Transport::Front, Transport::Direct] {
        let rig = Rig::with(bin.clone(), &split, |config, _| {
            if transport == Transport::Direct {
                direct(config);
            }
        })
        .await;
        rig.ready(1).await;
        let mut run = rig
            .group()
            .generate_prompt_in(prompt(), PromptMode::Chat, 3, greedy())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(60), &mut run.done)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let steps: Vec<_> = rig
            .events()
            .into_iter()
            .filter(|e| e["event"] == "step")
            .collect();
        for stage in ["s0", "s1", "s2"] {
            let modes: Vec<_> = steps
                .iter()
                .filter(|e| e["stage"] == stage)
                .map(|e| e["mode"].clone())
                .collect();
            assert!(!modes.is_empty(), "{stage}: {steps:#?}");
            assert!(modes.iter().all(|m| m == "chat"), "{stage}: {modes:?}");
        }
        rig.close().await;
    }
}
