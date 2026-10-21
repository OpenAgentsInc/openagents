#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The worker's handoff under a stalled executor (audit F7): the real
//! serving loop, the real link, and the CPU executor behind a gate. It runs
//! alone in this test binary, so the process's memory is its own.
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use psionic_models::{
    deepseek_v41_stage::{V41GroupManifest, V41StageGeometry},
    stage_state::{
        RowRule, Section, StageExecutor, StageInbox, StageMessage, StageWorker, StepKind, encode,
    },
};
use serde_json::{Value, json};
use stage_link::{Frame, Hop, Key, Kind, Limits};

use psionic_v41_stage::serve::{InboxBounds, Stage, serve};
use psionic_v41_stage::{
    Model, RUNTIME, Sampling, StageBackend, cpu_worker, load_model, synthetic_manifest,
    write_synthetic,
};

/// A gate the executor waits at until the test opens it.
#[derive(Clone, Default)]
struct Gate(Arc<(Mutex<(bool, usize)>, Condvar)>);

impl Gate {
    fn enter(&self) {
        let (lock, wake) = &*self.0;
        let mut state = lock.lock().unwrap();
        state.1 += 1;
        wake.notify_all();
        let _state = wake.wait_while(state, |s| !s.0).unwrap();
    }

    fn wait_entered(&self, n: usize, limit: Duration) {
        let (lock, wake) = &*self.0;
        let state = lock.lock().unwrap();
        let (state, _) = wake.wait_timeout_while(state, limit, |s| s.1 < n).unwrap();
        assert!(state.1 >= n, "the executor never started step {n}");
    }

    fn open(&self) {
        let (lock, wake) = &*self.0;
        lock.lock().unwrap().0 = true;
        wake.notify_all();
    }
}

struct OpenOnDrop(Gate);

impl Drop for OpenOnDrop {
    fn drop(&mut self) {
        self.0.open();
    }
}

/// The real CPU executor, stalled at the gate before each step.
struct Stalled<E> {
    inner: E,
    gate: Gate,
}

impl<E: StageExecutor> StageExecutor for Stalled<E> {
    type Error = E::Error;

    fn step(&mut self, message: &StageMessage) -> Result<Vec<Section>, Self::Error> {
        self.gate.enter();
        self.inner.step(message)
    }

    fn reset(&mut self, request_id: &str) {
        self.inner.reset(request_id);
    }
}

impl<E: StageBackend> StageBackend for Stalled<E> {
    fn geometry(&self) -> V41StageGeometry {
        self.inner.geometry()
    }
}

fn rss_kib() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

/// Stage 0's real state for `request`'s first step at `generation`.
fn first_hop(
    model: &'static Model,
    manifest: &V41GroupManifest,
    generation: u64,
    request: &str,
) -> Vec<u8> {
    let mut first = cpu_worker(
        model,
        manifest,
        &manifest.stages[0],
        model.arithmetic,
        64,
        generation,
    )
    .unwrap();
    let message = StageMessage {
        manifest_digest: manifest.digest(),
        generation,
        request_id: request.into(),
        attempt: 1,
        stage: 0,
        layers: 0..20,
        step: 0,
        kind: StepKind::Prefill,
        position: 0,
        tokens: vec![3, 5, 0, 6],
        sections: Vec::new(),
    };
    let sections = first.step(message.clone()).unwrap();
    encode(&message.next_hop(1, 20..40, sections), &Default::default()).unwrap()
}

/// A valid encoded prefill of `tokens` tokens for stage 1, at `step`.
fn big_hop(stage: &StageInbox, manifest: &V41GroupManifest, step: u64, tokens: u32) -> Vec<u8> {
    let sections = stage
        .contract()
        .sections
        .iter()
        .map(|spec| {
            let rows = match spec.rows {
                RowRule::PerToken => tokens,
                RowRule::Append { ratio } => tokens / ratio.max(1),
                RowRule::Fixed(rows) => rows,
            };
            let mut shape = vec![rows];
            shape.extend_from_slice(&spec.inner);
            let elements: usize = shape.iter().map(|d| *d as usize).product();
            Section {
                kind: spec.kind,
                dtype: spec.dtype,
                shape,
                first_row: 0,
                data: vec![0; elements * spec.dtype.bytes()],
            }
        })
        .collect();
    let message = StageMessage {
        manifest_digest: manifest.digest(),
        generation: 1,
        request_id: "r".into(),
        attempt: 1,
        stage: 1,
        layers: 20..40,
        step,
        kind: StepKind::Prefill,
        position: 0,
        tokens: vec![0; tokens as usize],
        sections,
    };
    encode(&message, &Default::default()).unwrap()
}

struct FrontConn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl FrontConn {
    fn send(&mut self, message: &Value) {
        self.writer
            .write_all(format!("{message}\n").as_bytes())
            .unwrap();
    }

    /// The next message of `kind`, skipping others.
    fn next(&mut self, kind: &str) -> Value {
        loop {
            let mut line = String::new();
            assert!(
                self.reader.read_line(&mut line).unwrap() > 0,
                "the stage hung up"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["kind"] == kind {
                return message;
            }
        }
    }

    fn link(&mut self, generation: u64, key: &Key) {
        self.send(&json!({
            "kind": "link", "generation": generation, "key": key.to_hex(), "group": "g",
            "stage": 1, "stages": 2, "next": null, "queue": 2, "send_timeout_ms": 1000,
        }));
        assert_eq!(self.next("linked")["generation"], generation);
    }
}

fn state(step: u64, generation: u64, request: &str, state: Vec<u8>) -> Frame {
    let sampling = Sampling::GREEDY;
    Frame {
        kind: Kind::State,
        generation,
        request_id: request.into(),
        attempt: 1,
        step,
        meta: serde_json::to_vec(&json!({"sampling": sampling, "final": false, "mode": "chat"}))
            .unwrap(),
        state,
    }
}

#[test]
fn a_stalled_executor_holds_the_link_to_its_bounds_and_recovers() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_path_buf();
    write_synthetic(&dir, "synthetic").unwrap();
    let model: &'static Model = Box::leak(Box::new(load_model(&dir).unwrap()));
    let manifest = synthetic_manifest(&[0..20, 20..40]).unwrap();
    let (inbox, executor) = cpu_worker(
        model,
        &manifest,
        &manifest.stages[1],
        model.arithmetic,
        64,
        1,
    )
    .unwrap()
    .into_parts();
    let gate = Gate::default();
    let big = big_hop(&inbox, &manifest, 1, 8192);
    let bounds = InboxBounds {
        frames: 3,
        bytes: 3 << 19,
        wait: Duration::from_secs(2),
    };
    // The byte bound, not the frame bound, stops the third frame.
    assert!(
        big.len() > 600_000 && 3 * big.len() > bounds.bytes,
        "{}",
        big.len()
    );
    let worker = StageWorker::new(
        inbox,
        Stalled {
            inner: executor,
            gate: gate.clone(),
        },
    );
    let front = TcpListener::bind("127.0.0.1:0").unwrap();
    let stage = Stage {
        name: "s1".into(),
        token: "t1".into(),
        weights: model.digest.clone(),
        manifest: manifest.clone(),
        index: 1,
        vocabulary: &model.vocabulary,
        eos: model.config.eos_token,
        default_token: model.config.pad_token,
        front: front.local_addr().unwrap().to_string(),
        listen: "127.0.0.1:0".into(),
        data_listen: Some("127.0.0.1:0".into()),
        data_advertise: None,
        runtime: RUNTIME,
        profile: psionic_v41_stage::profile_name(
            psionic_v41_stage::Backend::Cpu,
            Some(model.arithmetic),
        ),
        device: serde_json::json!({"class": "cpu-test"}),
        events: Some(dir.join("events.jsonl")),
        max_context: 64,
        max_step_tokens: 64,
        prefill_chunk: None,
        max_input: None,
        inbox: bounds,
        record: None,
    };
    let first = first_hop(model, &manifest, 1, "r");
    let recovery = first_hop(model, &manifest, 2, "r2");
    // The test drives the stage from another thread; the stage serves
    // on this one until the front hangs up.
    let driver = std::thread::spawn(move || {
        // A failed check still frees the executor, so the stage ends.
        let _free = OpenOnDrop(gate.clone());
        let (stream, _) = front.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let mut conn = FrontConn {
            reader: BufReader::new(stream.try_clone().unwrap()),
            writer: stream,
        };
        let hello = conn.next("hello");
        assert_eq!(hello["limits"]["max_context_tokens"], 64);
        assert_eq!(hello["limits"]["max_input_tokens"], 63);
        assert_eq!(hello["limits"]["modes"], json!(["thinking", "chat"]));
        let data = hello["data"].as_str().unwrap().to_owned();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let key = Key::generate().unwrap();
        conn.link(1, &key);
        let limits = Limits {
            max_state: 256 << 20,
            queue: 2,
            send_timeout: Duration::from_secs(1),
            ..Limits::default()
        };
        let hop = |generation| Hop {
            group: "g".into(),
            generation,
            from: 0,
            to: 1,
        };
        let mut sender = runtime
            .block_on(stage_link::connect(data.as_str(), &key, &hop(1), &limits))
            .unwrap();
        runtime
            .block_on(sender.send(&state(0, 1, "r", first)))
            .unwrap();
        gate.wait_entered(1, Duration::from_secs(30));
        // The executor is stuck in step 0. Flood the link with valid
        // 690 KB frames: unbounded, 600 of them would queue 400 MB.
        let before = rss_kib();
        let (mut sent, mut failed) = (0, None);
        for step in 1..=600 {
            let started = Instant::now();
            match runtime.block_on(sender.send(&state(step, 1, "r", big.clone()))) {
                Ok(_) => sent += 1,
                Err(error) => {
                    failed = Some((started.elapsed(), error.to_string()));
                    break;
                }
            }
        }
        let grown = rss_kib().saturating_sub(before);
        let (waited, error) = failed.expect("the link never pushed back");
        // The sender blocks, then fails within its one-second deadline.
        assert!(waited < Duration::from_secs(3), "{waited:?}: {error}");
        // Only the inbox, the sender's queue, and socket buffers hold
        // frames: a few, not hundreds.
        assert!(sent < 40, "{sent} frames were accepted");
        assert!(grown < 64 << 10, "the process grew {grown} KiB");
        // Cancel the request while its state is queued, then free the
        // executor: the cancellation goes before the queued state.
        conn.send(&json!({"kind": "cancel", "request_id": "r", "attempt": 1}));
        gate.open();
        assert_eq!(conn.next("ack")["step"], 0);
        assert_eq!(conn.next("stopped")["request_id"], "r");
        // The broken link is reported: by the reader that gave up
        // waiting (`self`), or that found the sender gone (`prev`).
        let lost = conn.next("link_failed");
        assert!(lost["peer"] == "self" || lost["peer"] == "prev", "{lost}");
        // Recovery: the front fences, links again, and a new request
        // runs.
        conn.send(&json!({"kind": "fence", "generation": 2}));
        assert_eq!(conn.next("hello")["generation"], 2);
        let key = Key::generate().unwrap();
        conn.link(2, &key);
        let mut sender = runtime
            .block_on(stage_link::connect(data.as_str(), &key, &hop(2), &limits))
            .unwrap();
        runtime
            .block_on(sender.send(&state(0, 2, "r2", recovery)))
            .unwrap();
        let ack = conn.next("ack");
        assert_eq!(
            (ack["request_id"].as_str(), ack["step"].as_u64()),
            (Some("r2"), Some(0))
        );
        assert!(ack["token"].is_u64());
        sent
    });
    serve(stage, worker, 1).unwrap();
    let sent = match driver.join() {
        Ok(sent) => sent,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    let events: Vec<Value> = std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let closed = events
        .iter()
        .find(|e| e["event"] == "front-closed")
        .unwrap();
    let (frames, bytes) = (
        closed["peak_frames"].as_u64().unwrap(),
        closed["peak_bytes"].as_u64().unwrap(),
    );
    assert!(frames <= 2 && bytes <= 3 << 19, "{closed}; {sent} sent");
    // The stop came before any queued state was looked at.
    let stopped = events.iter().position(|e| e["event"] == "stopped").unwrap();
    let first_stale = events
        .iter()
        .position(|e| e["event"] == "refused" && e["step"].as_u64() > Some(0));
    assert!(first_stale.is_none_or(|i| i > stopped), "{events:#?}");
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "step" && e["mode"] == "chat"),
        "{events:#?}"
    );
}
