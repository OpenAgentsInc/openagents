#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]
//! Stage processes over loopback, driven by a minimal front that speaks
//! Pylon's stage-group control protocol. Pylon's own front runs these
//! processes in `crates/pylon/tests/group_stage_worker.rs`.

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use psionic_models::stage_state::{CodecLimits, StageMessage, StepKind, encode};
use psionic_v41_stage::{cpu_worker, load_model, serve::write_frame, synthetic_manifest};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_psionic-v41-stage");

fn weights(dir: &Path, split: &[(u32, u32)]) -> PathBuf {
    let status = Command::new(BIN)
        .args(["synthetic", &dir.display().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let split: Vec<_> = split.iter().map(|(a, b)| *a..*b).collect();
    let manifest = synthetic_manifest(&split).unwrap();
    let path = dir.join("group-manifest.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    path
}

fn args(dir: &Path, manifest: &str, split: &[(u32, u32)], i: usize) -> Vec<String> {
    let (a, b) = split[i];
    let mut args = vec![
        "--weights".into(),
        dir.display().to_string(),
        "--layers".into(),
        format!("{a}-{b}"),
        "--gpu".into(),
        i.to_string(),
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

struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    hello: Value,
}

impl Conn {
    fn send(&mut self, message: &Value) {
        self.writer
            .write_all(format!("{message}\n").as_bytes())
            .unwrap();
    }

    /// The next message that isn't a pong.
    fn recv(&mut self) -> Value {
        loop {
            let mut line = String::new();
            assert!(
                self.reader.read_line(&mut line).unwrap() > 0,
                "stage hung up"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["kind"] != "pong" {
                return message;
            }
        }
    }
}

/// A minimal front: it starts the stages and drives one request.
struct Front {
    stages: Vec<Conn>,
    children: Vec<Child>,
}

impl Front {
    fn start(dir: &Path, manifest: &str, split: &[(u32, u32)], only: Option<usize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let indices: Vec<usize> = only.map_or((0..split.len()).collect(), |i| vec![i]);
        let mut children = Vec::new();
        for &i in &indices {
            let mut a = args(dir, manifest, split, i);
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
            children.push(
                Command::new(BIN)
                    .args(&a)
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
        }
        let mut stages: Vec<Conn> = Vec::new();
        for _ in &indices {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut conn = Conn {
                reader: BufReader::new(stream.try_clone().unwrap()),
                writer: stream,
                hello: Value::Null,
            };
            conn.hello = conn.recv();
            assert_eq!(conn.hello["kind"], "hello");
            assert_eq!(conn.hello["runtime"], "psionic-v41-stage/1");
            conn.send(&json!({"kind": "check", "nonce": 1, "input": 20}));
            assert_eq!(conn.recv()["output"], 41);
            stages.push(conn);
        }
        stages.sort_by_key(|c| c.hello["stage"].as_str().unwrap().to_owned());
        Self { stages, children }
    }

    /// Runs `steps` steps of one request and returns the tokens and text.
    fn run(&mut self, prompt: &Value, steps: u64) -> (Vec<u32>, String) {
        let n = self.stages.len();
        let (mut tokens, mut text) = (Vec::new(), String::new());
        for step in 0..steps {
            for i in 0..n {
                let mut message = json!({
                    "kind": "step", "generation": 1, "request_id": "r", "attempt": 1,
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
                    message["sampling"] = json!({"temperature": 0.0, "top_p": 1.0, "seed": 0});
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

fn reference(dir: &Path, prompt: &Value, steps: u64) -> (Vec<u32>, String) {
    let out = Command::new(BIN)
        .args([
            "reference",
            "--weights",
            &dir.display().to_string(),
            "--prompt",
            &prompt.to_string(),
            "--steps",
            &steps.to_string(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
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

fn events(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[test]
fn groups_of_two_to_four_processes_answer_like_the_unsplit_model() {
    let prompt = json!([{"role": "user", "content": "tok3 tok5 tok0 tok6 tok4 tok7"}]);
    for split in [
        vec![(0, 20), (20, 40)],
        vec![(0, 2), (2, 21), (21, 40)],
        vec![(0, 10), (10, 20), (20, 30), (30, 40)],
    ] {
        let root = tempfile::tempdir().unwrap();
        let manifest = weights(root.path(), &split);
        // Stages find the manifest by its digest in the weights directory,
        // as `pylon cloud up` names it.
        let digest = format!(
            "sha256:{}",
            serde_json::from_slice::<psionic_models::deepseek_v41_stage::V41GroupManifest>(
                &std::fs::read(&manifest).unwrap()
            )
            .unwrap()
            .digest_hex()
        );
        let mut front = Front::start(root.path(), &digest, &split, None);
        let expected = reference(root.path(), &prompt, 10);
        assert_eq!(front.run(&prompt, 10), expected, "{split:?}");
    }
}

fn refusal(args: &[String]) -> String {
    let out = Command::new(BIN).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    String::from_utf8(out.stderr).unwrap()
}

fn with_front(mut a: Vec<String>) -> Vec<String> {
    a.extend(
        [
            "--stage",
            "s0",
            "--generation",
            "1",
            "--front",
            "127.0.0.1:9",
            "--token",
            "t",
        ]
        .map(String::from),
    );
    a
}

#[test]
fn missing_weights_and_a_mismatched_layer_range_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest = weights(root.path(), &split);
    let manifest = manifest.display().to_string();
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let mut a = args(&empty, &manifest, &split, 0);
    a.truncate(8);
    a.push("--embeddings".into());
    assert!(refusal(&with_front(a)).contains("weights are missing"));
    let mut a = args(root.path(), &manifest, &split, 0);
    a[3] = "0-10".into();
    assert!(refusal(&with_front(a)).contains("don't match a stage of the manifest"));
    let mut a = args(root.path(), &manifest, &split, 1);
    a.push("--embeddings".into());
    assert!(refusal(&with_front(a)).contains("command line disagrees"));
    let a = args(root.path(), "sha256:00", &split, 0);
    assert!(refusal(&with_front(a)).contains("no group manifest with digest"));
    let mut a = args(root.path(), &manifest, &split, 0);
    a.extend(["--backend".into(), "cuda".into()]);
    assert!(refusal(&with_front(a)).contains("no CUDA stage executor"));
}

#[test]
fn wrong_generation_and_stale_state_are_refused_before_use() {
    let root = tempfile::tempdir().unwrap();
    let split = [(0, 20), (20, 40)];
    let manifest_path = weights(root.path(), &split);
    let mut front = Front::start(
        root.path(),
        &manifest_path.display().to_string(),
        &split,
        Some(1),
    );
    // Make stage 0's real output in this process.
    let model: &'static _ = Box::leak(Box::new(load_model(root.path()).unwrap()));
    let manifest = synthetic_manifest(&[0..20, 20..40]).unwrap();
    let mut first = cpu_worker(
        model,
        &manifest,
        &manifest.stages[0],
        model.arithmetic,
        64,
        1,
    )
    .unwrap();
    let message = StageMessage {
        manifest_digest: manifest.digest(),
        generation: 1,
        request_id: "r".into(),
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
    let hop = message.next_hop(1, 20..40, sections);
    let stage = &mut front.stages[0];
    let listen = stage.hello["listen"].as_str().unwrap().to_owned();
    let mut link = TcpStream::connect(&listen).unwrap();
    let limits = CodecLimits::default();
    stage.send(&json!({
        "kind": "step", "generation": 1, "request_id": "r", "attempt": 1, "stage": 1, "step": 0,
        "sampling": {"temperature": 0.0, "top_p": 1.0, "seed": 0},
    }));
    // From a generation the group has left.
    let mut old = hop.clone();
    old.generation = 0;
    write_frame(&mut link, &encode(&old, &limits).unwrap()).unwrap();
    // From another group.
    let mut other = hop.clone();
    other.manifest_digest = [9; 32];
    write_frame(&mut link, &encode(&other, &limits).unwrap()).unwrap();
    // A step the stage has already taken: its state is stale.
    let mut early = hop.clone();
    early.position = 4;
    write_frame(&mut link, &encode(&early, &limits).unwrap()).unwrap();
    // The real state: the stage acknowledges with a token.
    write_frame(&mut link, &encode(&hop, &limits).unwrap()).unwrap();
    let ack = stage.recv();
    assert_eq!(ack["kind"], "ack");
    assert!(ack["token"].is_u64());
    // The same state again is a duplicate.
    write_frame(&mut link, &encode(&hop, &limits).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let reasons: Vec<String> = events(root.path())
        .into_iter()
        .filter(|e| e["event"] == "refused")
        .map(|e| e["reason"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(reasons.len(), 4, "{reasons:#?}");
    assert!(reasons[0].contains("generation 0 is older"), "{reasons:#?}");
    assert!(reasons[1].contains("manifest"), "{reasons:#?}");
    // The stale and duplicate messages name the step the stage expects.
    assert!(
        reasons[2..]
            .iter()
            .all(|r| r.contains("position") || r.contains("step")),
        "{reasons:#?}"
    );
}
