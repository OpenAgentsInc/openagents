#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stderr
)]
//! A minimal front that speaks Pylon's stage-group control protocol, shared
//! by the process tests.

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use psionic_v41_stage::synthetic_manifest;
use serde_json::{Value, json};

pub const BIN: &str = env!("CARGO_BIN_EXE_psionic-v41-stage");

/// How long a stage may take to connect to the front, and a one-shot
/// command to finish. Every wait in these tests is bounded, so a stuck
/// process fails the test instead of hanging it.
pub const LIMIT: Duration = Duration::from_secs(180);

/// Runs `command` to completion within `limit`, killing it after that.
/// Output goes through files, so a full pipe can't stall the child.
pub fn run_bounded(command: &mut Command, limit: Duration) -> Output {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let scratch = tempfile::tempdir().unwrap();
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let (out, err) = (
        scratch.path().join(format!("{n}.out")),
        scratch.path().join(format!("{n}.err")),
    );
    let mut child = command
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{command:?} didn't finish in {limit:?}: {}",
                std::fs::read_to_string(&err).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Output {
        status,
        stdout: std::fs::read(&out).unwrap(),
        stderr: std::fs::read(&err).unwrap(),
    }
}

/// The device a CUDA stage child uses. A test machine may have one GPU, so
/// every child shares the device the test found, the parent's own
/// `CUDA_VISIBLE_DEVICES` or the first device, as `--gpu 0`.
fn visible_devices() -> String {
    std::env::var("CUDA_VISIBLE_DEVICES").unwrap_or_else(|_| "0".into())
}

pub fn weights(dir: &Path, split: &[(u32, u32)]) -> PathBuf {
    let out = run_bounded(
        Command::new(BIN).args(["synthetic", &dir.display().to_string()]),
        LIMIT,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let split: Vec<_> = split.iter().map(|(a, b)| *a..*b).collect();
    let manifest = synthetic_manifest(&split).unwrap();
    let path = dir.join("group-manifest.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    path
}

pub fn args(dir: &Path, manifest: &str, split: &[(u32, u32)], i: usize) -> Vec<String> {
    let (a, b) = split[i];
    let mut args = vec![
        "--weights".into(),
        dir.display().to_string(),
        "--layers".into(),
        format!("{a}-{b}"),
        // Every stage shares the one device `visible_devices` shows: `--gpu`
        // counts within an inherited mask (#12407).
        "--gpu".into(),
        "0".into(),
        "--manifest".into(),
        manifest.into(),
        "--events".into(),
        dir.join("events.jsonl").display().to_string(),
    ];
    if i == 0 {
        args.push("--embeddings".into());
    }
    if i + 1 == split.len() {
        args.push("--head".into());
    }
    args
}

pub struct Conn {
    pub reader: BufReader<TcpStream>,
    pub writer: TcpStream,
    pub hello: Value,
    /// The stage's standard error, for failure messages.
    pub logs: Vec<PathBuf>,
}

impl Conn {
    pub fn send(&mut self, message: &Value) {
        self.writer
            .write_all(format!("{message}\n").as_bytes())
            .unwrap();
    }

    /// The next message that isn't a pong.
    pub fn recv(&mut self) -> Value {
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line);
            let stage = self.hello["stage"].clone();
            let logs = || {
                self.logs
                    .iter()
                    .map(|l| std::fs::read_to_string(l).unwrap_or_default())
                    .chain(self.logs.first().and_then(|l| {
                        std::fs::read_to_string(l.with_file_name("events.jsonl")).ok()
                    }))
                    .collect::<Vec<_>>()
                    .join("\n---\n")
            };
            match read {
                Ok(0) => panic!("stage {stage} hung up; logs:\n{}", logs()),
                Ok(_) => {}
                Err(e) => panic!("stage {stage} didn't answer: {e}; logs:\n{}", logs()),
            }
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["kind"] != "pong" {
                return message;
            }
        }
    }
}

/// A minimal front: it starts the stages and drives one request.
pub struct Front {
    pub stages: Vec<Conn>,
    pub children: Vec<Child>,
    /// Requests run so far: each run is a new request, since a stage
    /// refuses a step of a request it has already taken.
    pub requests: u32,
}

impl Front {
    pub fn start(dir: &Path, manifest: &str, split: &[(u32, u32)], only: Option<usize>) -> Self {
        Self::start_with(dir, manifest, split, only, &[], "psionic-v41-stage/1")
    }

    /// Starts the stages with `extra` arguments and checks they announce
    /// `runtime`.
    pub fn start_with(
        dir: &Path,
        manifest: &str,
        split: &[(u32, u32)],
        only: Option<usize>,
        extra: &[&str],
        runtime: &str,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let indices: Vec<usize> = only.map_or((0..split.len()).collect(), |i| vec![i]);
        let mut children = Vec::new();
        let mut logs = Vec::new();
        for &i in &indices {
            let mut a = args(dir, manifest, split, i);
            a.extend(extra.iter().map(|x| (*x).to_owned()));
            a.extend([
                "--stage".into(),
                format!("s{i}"),
                "--generation".into(),
                "1".into(),
                "--front".into(),
                address.clone(),
                "--token".into(),
                format!("t{i}"),
            ]);
            let log = dir.join(format!("stage-{i}-{}.log", std::process::id()));
            children.push(
                Command::new(BIN)
                    .args(&a)
                    .env("CUDA_VISIBLE_DEVICES", visible_devices())
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(std::fs::File::create(&log).unwrap())
                    .spawn()
                    .unwrap(),
            );
            logs.push(log);
        }
        let mut front = Self {
            stages: Vec::new(),
            children,
            requests: 0,
        };
        // A stage that exits or never connects fails the test with its
        // log, rather than leaving `accept` waiting forever.
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + LIMIT;
        for _ in &indices {
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("accept: {e}"),
                }
                for (child, log) in front.children.iter_mut().zip(&logs) {
                    if let Some(status) = child.try_wait().unwrap() {
                        panic!(
                            "a stage exited with {status} before connecting: {}",
                            std::fs::read_to_string(log).unwrap_or_default()
                        );
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "no stage connected in {LIMIT:?}: {:?}",
                    logs.iter()
                        .map(|l| std::fs::read_to_string(l).unwrap_or_default())
                        .collect::<Vec<_>>()
                );
                std::thread::sleep(Duration::from_millis(20));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut conn = Conn {
                reader: BufReader::new(stream.try_clone().unwrap()),
                writer: stream,
                hello: Value::Null,
                logs: logs.clone(),
            };
            conn.hello = conn.recv();
            assert_eq!(conn.hello["kind"], "hello");
            assert_eq!(conn.hello["runtime"], runtime);
            conn.send(&json!({"kind": "check", "nonce": 1, "input": 20}));
            assert_eq!(conn.recv()["output"], 41);
            front.stages.push(conn);
        }
        front
            .stages
            .sort_by_key(|c| c.hello["stage"].as_str().unwrap().to_owned());
        front
    }

    /// Runs `steps` steps of one request and returns the tokens and text.
    pub fn run(&mut self, prompt: &Value, steps: u64) -> (Vec<u32>, String) {
        self.run_sampled(
            prompt,
            steps,
            &json!({"temperature": 0.0, "top_p": 1.0, "seed": 0}),
        )
    }

    /// Runs `steps` steps of one request with the last stage's `sampling`.
    pub fn run_sampled(
        &mut self,
        prompt: &Value,
        steps: u64,
        sampling: &Value,
    ) -> (Vec<u32>, String) {
        let n = self.stages.len();
        self.requests += 1;
        let request = format!("r{}", self.requests);
        let (mut tokens, mut text) = (Vec::new(), String::new());
        for step in 0..steps {
            for i in 0..n {
                let mut message = json!({
                    "kind": "step", "generation": 1, "request_id": request, "attempt": 1,
                    "stage": i, "step": step,
                });
                if i == 0 && step == 0 {
                    message["prompt"] = prompt.clone();
                }
                if i == 0 && step > 0 {
                    message["token"] = json!(tokens.last().unwrap());
                }
                if i + 1 < n {
                    message["next"] = self.stages[i + 1].hello["listen"].clone();
                } else {
                    message["sampling"] = sampling.clone();
                }
                self.stages[i].send(&message);
                let ack = self.stages[i].recv();
                assert_eq!(
                    (ack["kind"].as_str(), ack["step"].as_u64()),
                    (Some("ack"), Some(step))
                );
                if i + 1 == n {
                    tokens.push(ack["token"].as_u64().unwrap() as u32);
                    text.push_str(ack["text"].as_str().unwrap());
                    if ack["end"] == true {
                        return (tokens, text);
                    }
                }
            }
        }
        (tokens, text)
    }
}

impl Drop for Front {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn reference(dir: &Path, prompt: &Value, steps: u64) -> (Vec<u32>, String) {
    reference_with(dir, prompt, steps, &[])
}

/// The unsplit model's answer, with extra `reference` arguments.
pub fn reference_with(
    dir: &Path,
    prompt: &Value,
    steps: u64,
    extra: &[&str],
) -> (Vec<u32>, String) {
    let out = run_bounded(
        Command::new(BIN)
            .args([
                "reference",
                "--weights",
                &dir.display().to_string(),
                "--prompt",
                &prompt.to_string(),
                "--steps",
                &steps.to_string(),
            ])
            .args(extra)
            .env("CUDA_VISIBLE_DEVICES", visible_devices()),
        LIMIT,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let answer: Value = serde_json::from_slice(&out.stdout).unwrap();
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

pub fn events(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
