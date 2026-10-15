//! Large file transfer through the CLI, including a lost reply and resume.
#![cfg(unix)]
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Command, Output},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
const PROJECT: &str = "prj_aaaaaaaaaaaaaaaa";
const UPLOAD: &str = "upl_bbbbbbbbbbbbbbbbbbbbbbbb";
const FILE: &str = "file_cccccccccccccccccccccccc";
const PART: usize = 8 * 1024 * 1024;
#[derive(Default)]
struct Kept {
    bytes: Vec<u8>,
    expected: usize,
    sha: String,
    parts: usize,
    starts: usize,
    interrupted: bool,
    complete: bool,
}
fn status(s: &Kept) -> Value {
    json!({"id":UPLOAD,"project":PROJECT,"part_bytes":PART,"uploaded_bytes":s.bytes.len(),"next_part":s.parts,"size_bytes":s.expected,"sha256":s.sha,"complete":s.complete})
}
fn cli(origin: &str, root: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["--json", "projects"])
        .args(args)
        .env_clear()
        .env("HOME", root)
        .env("OPENAGENTS_ORIGIN", origin)
        .env("OPENAGENTS_TOKEN", format!("sess_{}", "a".repeat(64)))
        .output()
        .unwrap()
}
#[test]
fn large_project_file_resumes_and_downloads_without_replacing_a_failed_output() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("render.mp4");
    let output = root.path().join("received.mp4");
    let bytes = vec![255u8; 11 * 1024 * 1024 + 17];
    std::fs::write(&input, &bytes).unwrap();
    let digest = hex(&Sha256::digest(&bytes));
    let kept = Arc::new(Mutex::new(Kept::default()));
    let state = kept.clone();
    let quit = Arc::new(AtomicBool::new(false));
    let stop = quit.clone();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let worker = std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            if first.is_empty() {
                continue;
            }
            let mut length = 0;
            let mut part_sha = None;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                    if name.eq_ignore_ascii_case("x-part-sha256") {
                        part_sha = Some(value.trim().to_owned());
                    }
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let words = first.split_whitespace().collect::<Vec<_>>();
            let method = words[0];
            let path = words[1].split('?').next().unwrap();
            let mut s = state.lock().unwrap();
            let (code, value) = match (method, path) {
                ("GET", "/v1/projects") => (
                    200,
                    json!({"data":[{"id":PROJECT,"name":"film","protection":"standard"}],"next":null}),
                ),
                ("POST", "/v1/files/uploads") => {
                    let request: Value = serde_json::from_slice(&body).unwrap();
                    s.starts += 1;
                    s.expected = request["size_bytes"].as_u64().unwrap() as usize;
                    s.sha = request["sha256"].as_str().unwrap().into();
                    (201, status(&s))
                }
                ("GET", p) if p == format!("/v1/files/uploads/{UPLOAD}") => {
                    if s.interrupted && !s.complete {
                        // The first process must stop, then a new one resumes.
                        s.interrupted = false;
                        (503, json!({"error":{"message":"Upload interrupted"}}))
                    } else {
                        (200, status(&s))
                    }
                }
                ("PUT", p) if p.starts_with(&format!("/v1/files/uploads/{UPLOAD}/parts/")) => {
                    assert_eq!(
                        part_sha.as_deref(),
                        Some(hex(&Sha256::digest(&body)).as_str())
                    );
                    s.bytes.extend_from_slice(&body);
                    s.parts += 1;
                    if s.parts == 1 && s.starts == 1 {
                        s.interrupted = true;
                        drop(s);
                        continue;
                    }
                    (200, status(&s))
                }
                ("POST", p) if p == format!("/v1/files/uploads/{UPLOAD}/complete") => {
                    assert_eq!(s.bytes.len(), s.expected);
                    assert_eq!(hex(&Sha256::digest(&s.bytes)), s.sha);
                    s.complete = true;
                    (
                        201,
                        json!({"id":FILE,"project":PROJECT,"object":"file","filename":"render.mp4","size_bytes":s.expected,"sha256":s.sha}),
                    )
                }
                ("GET", p) if p == format!("/v1/projects/{PROJECT}/attachments") => (
                    200,
                    json!({"data":[{"id":FILE,"type":"file","filename":"render.mp4","size_bytes":s.expected,"sha256":s.sha}],"next":null,"protection":"standard"}),
                ),
                ("GET", p) if p == format!("/v1/files/{FILE}/content") => {
                    assert!(words[1].contains(&format!("project={PROJECT}")));
                    let bytes = s.bytes.clone();
                    drop(s);
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .unwrap();
                    stream.write_all(&bytes).unwrap();
                    continue;
                }
                _ => panic!("unexpected {method} {path}"),
            };
            let body = value.to_string();
            drop(s);
            write!(stream,"HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let first = cli(
        &origin,
        root.path(),
        &["files", "add", PROJECT, input.to_str().unwrap()],
    );
    assert!(!first.status.success());
    assert!(
        format!(
            "{}{}",
            String::from_utf8_lossy(&first.stderr),
            String::from_utf8_lossy(&first.stdout)
        )
        .contains("resume")
    );
    assert_eq!(kept.lock().unwrap().bytes.len(), PART);
    let second = cli(
        &origin,
        root.path(),
        &["files", "add", PROJECT, input.to_str().unwrap()],
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("Resuming"));
    assert_eq!(kept.lock().unwrap().parts, 2);
    assert_eq!(kept.lock().unwrap().starts, 2);
    let got = cli(
        &origin,
        root.path(),
        &[
            "files",
            "get",
            PROJECT,
            FILE,
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert!(
        got.status.success(),
        "{}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(
        hex(&Sha256::digest(std::fs::read(&output).unwrap())),
        digest
    );
    // A changed server digest leaves the previous output intact.
    kept.lock().unwrap().sha = "d".repeat(64);
    let failed = cli(
        &origin,
        root.path(),
        &[
            "files",
            "get",
            PROJECT,
            FILE,
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert!(!failed.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    quit.store(true, Ordering::SeqCst);
    worker.join().unwrap();
}
