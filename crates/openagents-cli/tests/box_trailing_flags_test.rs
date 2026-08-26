//! A flag written after the command must not reach the box.
//!
//! `oa box exec` and `oa box run` capture the rest of the line for the box,
//! and that captured `oa`'s own `--conversation` too: the id went into the
//! remote shell's argv, the flag was never read, and nothing said so
//! (forge issue #109).
//!
//! These run the real binary against a stub server, so they assert the two
//! things a unit test cannot: that the refusal happens before any request is
//! sent, and that a trailing argument the guard leaves alone still arrives at
//! the box unchanged.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;

/// One canned body for every route: the conversation lookup reads
/// `conversation_id`, the command run reads `result`.
const BODY: &str = r#"{"conversation_id":"cnv_1","result":{"box_id":"bx_1","exit_code":0,"stdout":"ok","stderr":"","timed_out":false},"run":{"id":"run_1","box_id":"bx_1","state":"running"}}"#;

/// What the server was asked for.
struct Hit {
    path: String,
    body: String,
}

struct StubServer {
    port: u16,
    hits: mpsc::Receiver<Hit>,
}

impl StubServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let port = listener.local_addr().expect("read the port").port();
        let (tx, hits) = mpsc::channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let tx = tx.clone();
                thread::spawn(move || serve_one(stream, tx));
            }
        });
        Self { port, hits }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn hits(&self) -> Vec<Hit> {
        self.hits.try_iter().collect()
    }
}

fn serve_one(mut stream: TcpStream, hits: mpsc::Sender<Hit>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }
        if header.trim().is_empty() {
            break;
        }
        if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        let _ = reader.read_exact(&mut body);
    }
    let _ = hits.send(Hit {
        path,
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        BODY.len(),
        BODY
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

struct Output {
    stderr: String,
    status: Option<i32>,
}

fn oa(args: &[&str]) -> Output {
    let result = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(args)
        .env("NO_COLOR", "")
        .output()
        .expect("run oa");
    Output {
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        status: result.status.code(),
    }
}

/// The invocation from the issue: refused, named, and nothing sent.
#[test]
fn a_trailing_conversation_flag_is_refused_before_anything_is_sent() {
    let server = StubServer::start();
    let origin = server.origin();
    let output = oa(&[
        "--api-url",
        &origin,
        "box",
        "exec",
        "bx_8bhkse3n",
        "echo hi",
        "--conversation",
        "3dd6d813-0000-4000-8000-000000000000",
    ]);

    assert_ne!(
        output.status,
        Some(0),
        "a swallowed flag must not be a success; stderr was: {}",
        output.stderr
    );
    assert!(
        output.stderr.contains("--conversation"),
        "the refusal must name the flag that was about to be swallowed; it said: {}",
        output.stderr
    );
    assert!(
        output.stderr.contains("oa box exec"),
        "the refusal must name the subcommand whose flag it is; it said: {}",
        output.stderr
    );

    let hits = server.hits();
    assert!(
        hits.is_empty(),
        "nothing may be sent for a refused invocation, and the conversation id \
         must never reach a remote shell; the server was asked for {:?}",
        hits.iter().map(|hit| &hit.path).collect::<Vec<_>>()
    );
}

/// `oa box run` has the same shape and the same refusal.
#[test]
fn box_run_refuses_a_trailing_conversation_flag_too() {
    let server = StubServer::start();
    let origin = server.origin();
    let output = oa(&[
        "--api-url",
        &origin,
        "box",
        "run",
        "bx_1",
        "cargo test",
        "--conversation",
        "abc",
    ]);

    assert_ne!(output.status, Some(0), "stderr was: {}", output.stderr);
    assert!(
        output.stderr.contains("--conversation") && output.stderr.contains("oa box run"),
        "the refusal must name the flag and the subcommand; it said: {}",
        output.stderr
    );
    assert!(server.hits().is_empty(), "nothing may be sent");
}

/// A flag that belongs to the remote program still reaches the box, verbatim.
#[test]
fn a_remote_programs_own_flag_still_reaches_the_box() {
    let server = StubServer::start();
    let origin = server.origin();
    let output = oa(&[
        "--api-url",
        &origin,
        "box",
        "exec",
        "bx_1",
        "ls",
        "--color=auto",
    ]);

    assert_eq!(
        output.status,
        Some(0),
        "`ls --color=auto` is not one of this subcommand's flags; stderr was: {}",
        output.stderr
    );
    let sent = server
        .hits()
        .into_iter()
        .find(|hit| hit.path.ends_with("/commands"))
        .expect("the command must have been sent to the box");
    assert!(
        sent.body.contains("ls --color=auto"),
        "the command must arrive unchanged; the body was: {}",
        sent.body
    );
}

/// `--` is the caller saying where the boundary is, and it is honoured.
#[test]
fn an_explicit_separator_sends_the_flag_through() {
    let server = StubServer::start();
    let origin = server.origin();
    let output = oa(&[
        "--api-url",
        &origin,
        "box",
        "exec",
        "bx_1",
        "--",
        "mytool",
        "--conversation",
        "abc",
    ]);

    assert_eq!(
        output.status,
        Some(0),
        "`--` says the flag is the command's; stderr was: {}",
        output.stderr
    );
    let sent = server
        .hits()
        .into_iter()
        .find(|hit| hit.path.ends_with("/commands"))
        .expect("the command must have been sent to the box");
    assert!(
        sent.body.contains("mytool --conversation abc"),
        "the command must arrive unchanged; the body was: {}",
        sent.body
    );
}
