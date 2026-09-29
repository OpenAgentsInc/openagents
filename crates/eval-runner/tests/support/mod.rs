//! What the runner's tests share: the fake agent, a fake door, a phone
//! that signs requests and reads answers, and a runner configuration in a
//! scratch directory. Nothing here reaches a model or a real relay.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eval_runner::config::{Config, Limits};
use eval_runner::runner::Runner;
use eval_runner::wire::memory::Memory;
use ext_eval::proxy::Secret;
use ext_eval::run::Door as RunDoor;
use nostr::domain::{Event, RelaySigner};
use nostr::eval_ext::hosted;
use nostr::execution::{self, Pending, Seal};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};

pub const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ext-eval/fixtures/repo-map-brief"
);

/// The fake agent, built once per test process.
pub fn fake_agent() -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let profile = Path::new(env!("CARGO_BIN_EXE_eval-runner"))
                .parent()
                .unwrap()
                .to_path_buf();
            let target = profile.parent().unwrap();
            let status = Command::new(env!("CARGO"))
                .args([
                    "build",
                    "--quiet",
                    "-p",
                    "ext-eval",
                    "--example",
                    "fake_agent",
                ])
                .env("CARGO_TARGET_DIR", target)
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .status()
                .unwrap();
            assert!(status.success(), "the fake agent builds");
            profile.join("examples").join("fake_agent")
        })
        .clone()
}

/// A fake Open Responses door on a loopback port: the overview answer
/// only with the fixture's skill in the instructions, a greeting for a
/// greeting.
pub struct FakeDoor {
    pub url: String,
    pub key: String,
}

pub fn door() -> FakeDoor {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let key = format!("door-key-{}-{}", std::process::id(), rand_hex());
    let expected = format!("bearer {key}");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let expected = expected.clone();
            std::thread::spawn(move || answer(stream, &expected));
        }
    });
    FakeDoor { url, key }
}

fn answer(mut stream: std::net::TcpStream, expected: &str) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut length = 0;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let lower = line.trim().to_ascii_lowercase();
        if lower.is_empty() {
            break;
        }
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
        if let Some(value) = lower.strip_prefix("authorization:") {
            authorized = value.trim() == expected;
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let prompt = request["input"].to_string().to_ascii_lowercase();
    let skilled = request["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("name its main language");
    let text = if prompt.contains("repository") {
        if skilled {
            "It is a Rust crate of six files; the largest is src/tables.rs."
        } else {
            "I would have to look at the files."
        }
    } else {
        "Welcome aboard, glad you are here!"
    };
    let (status, body) = if authorized {
        (
            "200 OK",
            json!({"output": [{"type": "message", "content": [{"type": "output_text", "text": text}]}]})
                .to_string(),
        )
    } else {
        ("401 Unauthorized", "{}".to_string())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
}

pub fn rand_hex() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A fresh key.
pub fn secret() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

pub fn hex(secret: &SecretKey) -> String {
    secret.display_secret().to_string()
}

/// The runner's configuration in `dir`, with the fixture extension as its
/// catalog and the fake agent and door.
pub fn config(dir: &Path, door: &FakeDoor, limits: Limits) -> Config {
    Config {
        relay: "memory".into(),
        blossom: None,
        bucket: None,
        key_file: dir.join("runner-key"),
        state: dir.join("state"),
        catalog: vec![PathBuf::from(FIXTURE)],
        coder: fake_agent(),
        questions: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../questions"),
        door: RunDoor {
            name: "default".into(),
            url: door.url.clone(),
            key: Secret::new(door.key.clone()),
            model: "fake/model".into(),
        },
        decision: None,
        limits,
        temp_root: {
            let tmp = dir.join("tmp");
            std::fs::create_dir_all(&tmp).unwrap();
            tmp
        },
    }
}

/// A runner over the in-memory relay.
pub fn memory_runner(
    dir: &Path,
    door: &FakeDoor,
    limits: Limits,
) -> (Arc<Runner>, Arc<Memory>, SecretKey) {
    let key = secret();
    let identity = coder::relay::Identity::from_text(&hex(&key), "key").unwrap();
    let memory = Arc::new(Memory::default());
    let runner = Runner::new(
        config(dir, door, limits),
        identity,
        memory.clone(),
        memory.clone(),
    )
    .expect("the runner starts");
    (runner, memory, key)
}

/// A trainer's phone: signs hosted requests and reads the answers.
pub struct Phone {
    pub secret: SecretKey,
    pub signer: RelaySigner,
}

impl Phone {
    pub fn new() -> Self {
        let secret = secret();
        let signer = RelaySigner::from_secret_hex(&hex(&secret)).unwrap();
        Self { secret, signer }
    }

    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }

    /// A signed request to `runner` with `input`.
    pub fn request(&self, runner: &str, input: &Value) -> (Event, Value) {
        let now = eval_runner::unix_now();
        let body = hosted::request_body(runner, &rand_hex(), input, now).unwrap();
        self.seal(runner, &body)
    }

    /// Seals any body as a request to `runner`.
    pub fn seal(&self, runner: &str, body: &Value) -> (Event, Value) {
        let deadline = body["deadline"]
            .as_u64()
            .unwrap_or(eval_runner::unix_now() + 3_600);
        let peer = XOnlyPublicKey::from_str(runner).unwrap();
        let seal = Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: eval_runner::unix_now(),
        };
        let event = seal
            .event(
                execution::REQUEST_KIND,
                hosted::request_tags(runner, deadline),
                body,
            )
            .unwrap();
        (event, body.clone())
    }

    /// A cancel control for the execute `request` carried.
    pub fn cancel(&self, runner: &str, request: &Event, body: &Value) -> Event {
        let control = json!({
            "v": execution::SCHEMA,
            "requires": [],
            "type": "cancel",
            "request": body["request"],
            "attempt": body["attempt"],
            "run": body["run"],
            "reason": "the person tapped Stop",
        });
        let peer = XOnlyPublicKey::from_str(runner).unwrap();
        let seal = Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: eval_runner::unix_now(),
        };
        seal.event(
            execution::REQUEST_KIND,
            vec![
                nostr::domain::Tag::new(vec!["p".into(), runner.into()]),
                nostr::domain::Tag::new(vec!["e".into(), request.id.clone()]),
            ],
            &control,
        )
        .unwrap()
    }

    /// Every answer to `request` among `events`, decrypted and bound.
    pub fn answers(
        &self,
        runner: &str,
        request: &Event,
        body: &Value,
        events: &[Event],
    ) -> Vec<(u16, Value)> {
        let pending = Pending {
            execute_event: &request.id,
            worker: runner,
            customer: self.signer.pubkey(),
            request: body["request"].as_str().unwrap_or_default(),
            attempt: 1,
        };
        events
            .iter()
            .filter_map(|event| {
                execution::bind_worker_event(event, &pending, &self.secret)
                    .ok()
                    .map(|payload| (event.kind, payload))
            })
            .collect()
    }
}

/// Waits until `find` returns something, polling `events`.
pub fn wait_for<T>(within: Duration, mut find: impl FnMut() -> Option<T>) -> Option<T> {
    let started = Instant::now();
    while started.elapsed() < within {
        if let Some(found) = find() {
            return Some(found);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// The first `26920` result among `answers`.
pub fn result_of(answers: &[(u16, Value)]) -> Option<Value> {
    answers
        .iter()
        .find(|(kind, payload)| *kind == execution::RESULT_KIND && payload["type"] == "result")
        .map(|(_, payload)| payload.clone())
}

/// Every file under `dir` whose bytes hold `needle`.
pub fn files_holding(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_holding(&path, needle));
        } else if std::fs::read(&path)
            .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        {
            found.push(path);
        }
    }
    found
}

/// Every file named `name` under `dir`.
pub fn files_named(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_named(&path, name));
        } else if path.file_name().is_some_and(|file| file == name) {
            found.push(path);
        }
    }
    found
}
