//! The stdin guards on `--body-file -` and `--token-stdin` (#178), and the
//! tool-path stdin fix (#180): a spawn through the coder tool registry must
//! fail fast instead of hanging when stdin never arrives.
//!
//! Every test here points `--api-url` at a localhost stub, not at production.
//! The piped-content test used to run against the live tracker because its
//! only assertion was "the stdin guard did not refuse" — and on a machine
//! with a real `forge:write` token that meant every green test run *created
//! an issue on openagents.com*. Eighteen issues titled `x` with the body
//! `a real body` (#215–#236) were filed by this test before anyone noticed.
//! A write path is asserted on the wire, the way `milestone_test.rs` does:
//! what method, what path, what body — against a server this file owns.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Where the CLI binary under test comes from: the crate's own build.
fn cli_path() -> std::path::PathBuf {
    // Cargo provides the exact path to the integration test's sibling binary.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_openagents"))
}

/// One canned body for every route. The guard tests refuse before any request
/// is sent, and the piped-content test only POSTs a new issue, so one reply
/// covers every request this file can produce.
const CREATED_BODY: &str = r#"{"number":478,"title":"x","state":"open"}"#;

/// What the stub server was asked for.
#[derive(Debug)]
struct Hit {
    method: String,
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
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
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
        method,
        path,
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    let response = format!(
        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        CREATED_BODY.len(),
        CREATED_BODY
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// A child `oa` pointed at `origin`, with a dummy token in the environment so
/// the real credential in the OS store is never consulted (find_token reads
/// OPENAGENTS_TOKEN first).
fn spawn_at(origin: &str, args: &[&str], stdin: Stdio) -> std::process::Child {
    Command::new(cli_path())
        .args(["--api-url", origin])
        .args(args)
        .env("OPENAGENTS_TOKEN", "stub-token-for-tests")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the CLI")
}

fn collect(child: std::process::Child) -> String {
    let output = child.wait_with_output().expect("collect the output");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Spawn the CLI with stdin **closed** (what #180 now guarantees at the tool
/// path) and assert it exits inside the bound — the no-hang property.
fn assert_exits_with_closed_stdin(origin: &str, args: &[&str], bound: Duration) -> String {
    let start = Instant::now();
    let mut child = spawn_at(origin, args, Stdio::null());
    loop {
        match child.try_wait().expect("wait on the CLI") {
            Some(status) => {
                assert!(!status.success(), "expected a refusal, got success");
                return collect(child);
            }
            None if start.elapsed() > bound => {
                let _ = child.kill();
                panic!(
                    "the CLI did not exit within {bound:?} with closed stdin: \
                     the stdin hang (#178) is back"
                );
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// The legitimate pipeline case #178 pins: EOF arrives with content, so no
/// guard fires and the create goes out. Asserted on the wire against the stub
/// server — never against the live tracker. This test used to reach production
/// and file an issue titled `x` on every run; the wire assertions are what
/// keep the guard honest without the litter.
#[test]
fn body_file_dash_with_piped_content_creates_on_the_stub_not_on_production() {
    let server = StubServer::start();
    let origin = server.origin();
    let mut child = spawn_at(
        &origin,
        // An explicit target: the default repo comes from the checkout's git
        // origin, and pinning a fictional one keeps this test self-contained.
        &[
            "issue",
            "create",
            "--title",
            "x",
            "--body-file",
            "-",
            "-R",
            "octavia/project",
        ],
        Stdio::piped(),
    );
    // The exact bytes the old test sent — kept so the wire assertion pins
    // what a piped body looks like, trailing newline included.
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(b"a real body\n")
        .expect("write the body");
    let text = collect(child);

    assert!(
        text.contains("Created issue #478"),
        "a piped body must create (against the stub) and report the number: {text}"
    );

    let hits = server.hits();
    assert_eq!(hits.len(), 1, "exactly one request, to the stub: {text}");
    let hit = &hits[0];
    assert_eq!(hit.method, "POST", "issue create is a POST: {:#?}", hit);
    assert!(
        hit.path.ends_with("/api/v1/repos/octavia/project/issues"),
        "create must hit the issues route, got {}",
        hit.path
    );
    let sent: serde_json::Value =
        serde_json::from_str(&hit.body).expect("the create body is valid JSON");
    assert_eq!(sent["title"], "x");
    assert_eq!(
        sent["body"], "a real body\n",
        "the piped body must arrive verbatim (the stdin read does not trim)"
    );
}

#[test]
fn body_file_dash_with_closed_stdin_refuses_instead_of_hanging() {
    // The exact call that hung the session in #178: no stdin ever arrives.
    let server = StubServer::start();
    let origin = server.origin();
    let message = assert_exits_with_closed_stdin(
        &origin,
        &["issue", "create", "--title", "x", "--body-file", "-"],
        Duration::from_secs(10),
    );
    assert!(
        message.to_lowercase().contains("stdin") || message.to_lowercase().contains("empty"),
        "the refusal should name stdin or the empty body: {message}"
    );
    assert!(
        server.hits().is_empty(),
        "a refused invocation must not send anything: the stub was asked for {:?}",
        server
            .hits()
            .iter()
            .map(|hit| &hit.path)
            .collect::<Vec<_>>()
    );
}

#[test]
fn body_file_dash_with_empty_pipe_refuses_with_empty_body_guidance() {
    let server = StubServer::start();
    let origin = server.origin();
    let mut child = spawn_at(
        &origin,
        &["issue", "create", "--title", "x", "--body-file", "-"],
        Stdio::piped(),
    );
    // Closing stdin immediately delivers EOF with no content.
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait");
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("the body read from stdin is empty"),
        "the refusal should say the body was empty: {text}"
    );
    assert!(
        server.hits().is_empty(),
        "an empty body must be refused before anything is sent"
    );
}

#[test]
fn body_file_dash_from_a_tty_refuses_immediately() {
    // A pty makes stdin a terminal. Without the TTY guard the CLI would wait
    // for a human to press Ctrl-D forever. The child is spawned with a real
    // pty as its controlling terminal via Python; macOS can refuse concurrent
    // pty opens under a full workspace run, so a probe failure to *open* the
    // pty is a quiet skip — the closed-stdin test above already pins the
    // no-hang property.
    let probe = std::process::Command::new("python3")
        .arg("-c")
        .arg("import os, fcntl, termios; m, s = os.openpty(); print('ok')")
        .stdin(Stdio::null())
        .output()
        .expect("probe the pty system");
    if !probe.status.success() {
        return;
    }
    // Spawn through Python's pty module the way a real session would: the
    // child owns the slave as its controlling terminal and stdio. portable-
    // pty's own `spawn_command` was flaky here under the test harness.
    // The origin points at 127.0.0.1:1 (nothing listens there): the TTY
    // refusal must fire before any network work, so an unreachable server is
    // the sentinel — a success here would mean the guard did not fire.
    let exe = env!("CARGO_BIN_EXE_openagents");
    let script = format!(
        "import os, sys, time, fcntl, termios\n\
         master, slave = os.openpty()\n\
         pid = os.fork()\n\
         if pid == 0:\n\
         \x20   os.setsid()\n\
         \x20   fcntl.ioctl(slave, termios.TIOCSCTTY, 0)\n\
         \x20   os.dup2(slave, 0); os.dup2(slave, 1); os.dup2(slave, 2)\n\
         \x20   os.close(master); os.close(slave)\n\
         \x20   os.execv({exe:?}, [{exe:?}, '--api-url', 'http://127.0.0.1:1',\n\
         \x20                       'issue', 'create', '--title', 'x',\n\
         \x20                       '--body-file', '-', '-R', 'octavia/project'])\n\
         os.close(slave)\n\
         start = time.time()\n\
         while time.time() - start < 8:\n\
         \x20   done, status = os.waitpid(pid, os.WNOHANG)\n\
         \x20   if done:\n\
         \x20       print(os.waitstatus_to_exitcode(status))\n\
         \x20       sys.exit(0)\n\
         \x20   time.sleep(0.2)\n\
         print('HUNG')\n\
         os.kill(pid, 9)\n\
         sys.exit(1)\n"
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run the pty probe");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.contains("HUNG"),
        "the CLI hung on a TTY stdin; the TTY guard is missing"
    );
    assert!(
        text.trim().ends_with('2'),
        "a TTY stdin must be refused (exit 2), got: {text}"
    );
}

#[test]
fn token_stdin_with_closed_stdin_refuses_instead_of_hanging() {
    // `auth login --token-stdin` reads the token before it touches the
    // network, so the default origin is never reached; still pointed at a
    // dead port so a regression that dials first fails loudly instead of
    // quietly using a real credential.
    let message = assert_exits_with_closed_stdin(
        "http://127.0.0.1:1",
        &["auth", "login", "--token-stdin"],
        Duration::from_secs(10),
    );
    assert!(
        message.to_lowercase().contains("stdin") || message.to_lowercase().contains("token"),
        "the refusal should name stdin or the token: {message}"
    );
}
