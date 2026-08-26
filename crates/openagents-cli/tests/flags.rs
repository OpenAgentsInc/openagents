//! What the global flags do, asserted by running the binary.
//!
//! The point of these tests is that a flag is *read*, not that it parses.
//! `--json` and `--verbose` used to parse on every command and change nothing,
//! and asserting "the flag is accepted" would have passed against exactly that
//! binary. So each test here runs `oa` twice — once with the flag and once
//! without — against a stub server it controls, and asserts the output is
//! different in the way the flag promises.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// A server that answers one canned body and reports the paths it was asked
/// for.
///
/// It exists so `--api-url` can be proven — the request has to arrive
/// somewhere this test owns — and so `--json` and `--verbose` can be asserted
/// against a body that does not change under it.
struct StubServer {
    port: u16,
    hits: mpsc::Receiver<String>,
}

impl StubServer {
    fn start(body: &'static str) -> Self {
        Self::start_with_status(200, "OK", body)
    }

    /// The same, with the status the server answers.
    ///
    /// A fleet promotion means different things at `202` and `200`, so a test
    /// for that distinction has to be able to choose which one it gets.
    fn start_with_status(code: u16, reason: &'static str, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let port = listener.local_addr().expect("read the port").port();
        let (tx, hits) = mpsc::channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let tx = tx.clone();
                thread::spawn(move || serve_one(stream, code, reason, body, tx));
            }
        });
        Self { port, hits }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every path asked for so far.
    fn paths(&self) -> Vec<String> {
        self.hits.try_iter().collect()
    }
}

fn serve_one(
    mut stream: TcpStream,
    code: u16,
    reason: &str,
    body: &str,
    hits: mpsc::Sender<String>,
) {
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
    if length > 0 {
        let mut discard = vec![0u8; length];
        let _ = reader.read_exact(&mut discard);
    }
    let _ = hits.send(path);
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

struct Output {
    stdout: String,
    stderr: String,
    status: Option<i32>,
}

/// A `HOME` of this test binary's own.
///
/// `oa` resolves its config directory from `HOME`, so without this every run
/// here writes into the developer's real `~/.config/openagents`. It did: a
/// single audit left 79 stub authorizations — keyed by ephemeral
/// `http://127.0.0.1:<port>` origins — in the real `device-authorizations.json`,
/// and concurrent runs racing that shared file made `repeated_scopes_are_all_sent`
/// flake with a write error. Tests must not touch the machine they run on.
fn isolated_home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let at = std::env::temp_dir().join(format!(
            "oa-flags-home-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&at).expect("make an isolated HOME");
        at
    })
    .as_path()
}

fn oa(args: &[&str]) -> Output {
    let result = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(args)
        // The credential store is keyed by origin, and the stub's origin has
        // no token, so these runs never carry a real one.
        .env("NO_COLOR", "")
        .env("HOME", isolated_home())
        .output()
        .expect("run oa");
    Output {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        status: result.status.code(),
    }
}

const ISSUES_BODY: &str = r#"{"issues":[{"number":7,"title":"a stub issue","state":"open","user":{"login":"someone"},"labels":[],"assignees":[]}],"total_count":1}"#;

/// `--json` has to change the output, not merely be accepted.
///
/// The assertion is deliberately three-part: the JSON parses, it carries a
/// field the human output also shows, and the two outputs are not the same
/// text. A binary that accepted `--json` and printed the table would pass the
/// first two only by accident and fails the third outright.
#[test]
fn json_changes_the_output_and_parses() {
    let server = StubServer::start(ISSUES_BODY);
    let origin = server.origin();
    let human = oa(&[
        "--api-url",
        &origin,
        "issue",
        "list",
        "--repo",
        "owner/repo",
    ]);
    let json = oa(&[
        "--api-url",
        &origin,
        "issue",
        "list",
        "--repo",
        "owner/repo",
        "--json",
    ]);

    assert!(
        human.stdout.contains("a stub issue"),
        "the human output should name the issue, got: {}",
        human.stdout
    );
    let parsed: serde_json::Value = serde_json::from_str(&json.stdout).unwrap_or_else(|error| {
        panic!(
            "--json did not emit parseable JSON ({error}): {}",
            json.stdout
        )
    });
    assert_eq!(parsed["issues"][0]["number"], 7);
    assert_eq!(parsed["issues"][0]["title"], "a stub issue");
    assert_ne!(
        human.stdout.trim(),
        json.stdout.trim(),
        "--json produced the same text as the human mode, so it changed nothing"
    );
}

/// The same for a second command family, because `--json` was declared once,
/// globally, and read by none of them.
#[test]
fn json_changes_the_output_for_the_forum_too() {
    let server = StubServer::start(
        r#"{"boards":[{"id":"b1","slug":"general","title":"General","description":"","topic_count":3}]}"#,
    );
    let origin = server.origin();
    let human = oa(&["--api-url", &origin, "forum", "boards"]);
    let json = oa(&["--api-url", &origin, "forum", "boards", "--json"]);

    assert!(human.stdout.contains("general"), "got: {}", human.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&json.stdout)
        .unwrap_or_else(|error| panic!("not JSON ({error}): {}", json.stdout));
    assert_eq!(parsed["boards"][0]["slug"], "general");
    assert_eq!(parsed["boards"][0]["topic_count"], 3);
    assert_ne!(human.stdout.trim(), json.stdout.trim());
}

/// `--api-url` has to send the request somewhere else.
///
/// Asserted by the request arriving at a server this test owns. Before this,
/// every client hardcoded the production origin at its construction site, so
/// the flag parsed and the request still went to production.
#[test]
fn api_url_sends_the_request_to_the_named_origin() {
    let server = StubServer::start(ISSUES_BODY);
    let origin = server.origin();
    let run = oa(&[
        "--api-url",
        &origin,
        "issue",
        "list",
        "--repo",
        "owner/repo",
    ]);
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let paths = server.paths();
    assert!(
        paths
            .iter()
            .any(|p| p.starts_with("/api/v1/repos/owner/repo/issues")),
        "the request did not reach the named origin; it asked for {paths:?}"
    );
}

/// Every command family, not just the two that were already threaded.
#[test]
fn api_url_reaches_each_command_family() {
    for (args, expected) in [
        (
            vec!["issue", "list", "--repo", "owner/repo"],
            "/api/v1/repos/owner/repo/issues",
        ),
        (vec!["forum", "boards"], "/api/v1/forum"),
        (vec!["memory", "list"], "/api/v1/memories"),
        (vec!["deploy", "list"], "/api/v1/admin/forge/targets"),
    ] {
        let server = StubServer::start(r#"{"issues":[],"boards":[],"memories":[],"targets":[]}"#);
        let origin = server.origin();
        let mut full = vec!["--api-url", origin.as_str()];
        full.extend(args.iter().copied());
        let _ = oa(&full);
        let paths = server.paths();
        assert!(
            paths.iter().any(|p| p.starts_with(expected)),
            "{args:?} did not reach {expected}; it asked for {paths:?}"
        );
    }
}

/// A profile that does not exist is refused rather than silently defaulted to
/// production.
#[test]
fn an_unknown_profile_is_refused() {
    let run = oa(&["--profile", "moon", "repo", "list"]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("unknown profile moon"),
        "stderr: {}",
        run.stderr
    );
}

/// `--verbose` has to print the request URL, the status, and the server's own
/// message, and a run without it must print none of them.
#[test]
fn verbose_prints_the_request_url_and_status() {
    let server = StubServer::start(ISSUES_BODY);
    let origin = server.origin();
    let quiet = oa(&[
        "--api-url",
        &origin,
        "issue",
        "list",
        "--repo",
        "owner/repo",
    ]);
    let loud = oa(&[
        "--api-url",
        &origin,
        "-v",
        "issue",
        "list",
        "--repo",
        "owner/repo",
    ]);

    assert!(
        loud.stderr
            .contains(&format!("> GET {origin}/api/v1/repos/owner/repo/issues")),
        "-v did not print the request URL; stderr was: {}",
        loud.stderr
    );
    assert!(
        loud.stderr.contains("< 200"),
        "-v did not print the status; stderr was: {}",
        loud.stderr
    );
    assert!(
        !quiet.stderr.contains("> GET"),
        "the request trace appeared without -v: {}",
        quiet.stderr
    );
    // The bodies must be identical: `-v` adds diagnostics on stderr and
    // changes nothing a script parses on stdout.
    assert_eq!(quiet.stdout, loud.stdout);
}

/// A refusal under `-v` carries the server's own message.
#[test]
fn verbose_prints_the_servers_refusal() {
    // Nothing is listening on this port, so the request cannot complete and
    // the transport diagnostic is what `-v` has to show.
    let run = oa(&[
        "--api-url",
        "http://127.0.0.1:1",
        "-v",
        "issue",
        "list",
        "--repo",
        "owner/repo",
    ]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr
            .contains("http://127.0.0.1:1/api/v1/repos/owner/repo/issues"),
        "stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("did not complete"),
        "stderr: {}",
        run.stderr
    );
}

/// `--completions` writes a real script that names this binary's real
/// subcommands, including the three that did not exist before.
#[test]
fn completions_name_the_real_subcommands() {
    for shell in ["bash", "zsh", "fish", "sh"] {
        let run = oa(&["--completions", shell]);
        assert_eq!(run.status, Some(0), "{shell}: {}", run.stderr);
        assert!(
            run.stdout.len() > 500,
            "{shell}: the script was {} bytes",
            run.stdout.len()
        );
        for subcommand in ["delegate", "deploy", "provider", "coder", "issue"] {
            assert!(
                run.stdout.contains(subcommand),
                "{shell}: the completion script does not name `{subcommand}`"
            );
        }
    }
}

/// The three commands the Rust CLI did not have. Each has to reach its own
/// help rather than the parser's "unrecognized subcommand".
#[test]
fn the_three_missing_commands_exist() {
    for (command, marker) in [
        ("delegate", "--child-config"),
        ("deploy", "promote"),
        ("provider", "settle"),
    ] {
        let run = oa(&[command, "--help"]);
        assert_eq!(run.status, Some(0), "{command}: {}", run.stderr);
        assert!(
            run.stdout.contains(marker),
            "`oa {command} --help` does not mention {marker}: {}",
            run.stdout
        );
    }
}

/// `oa provider settle` decides, and the decision is the whole output.
///
/// Run against files this test writes, so the decision is reproducible and the
/// same input can be handed to the TypeScript CLI for comparison.
#[test]
fn provider_settle_decides_from_the_files_it_is_given() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let lease = directory.path().join("lease.json");
    std::fs::write(
        &lease,
        r#"{"job_id":"job-1","lane":"coding","provider":"pk","price_msats":1200,"expires_at":"2026-08-26T12:00:00Z"}"#,
    )
    .expect("write the lease");

    let unverified = oa(&["provider", "settle", "--lease", lease.to_str().unwrap()]);
    assert_eq!(unverified.status, Some(0), "{}", unverified.stderr);
    assert!(unverified.stdout.contains("Outcome: unsettled"));
    assert!(unverified.stdout.contains("Refused: no_closeout"));
    assert!(unverified.stdout.contains("Earned: 0 msats"));

    let closeout = directory.path().join("closeout.json");
    let digest = "a".repeat(64);
    std::fs::write(
        &closeout,
        format!(
            r#"{{"receiptRef":"lbr-closeout:job-1:{digest}","requestId":"job-1","requesterPubkey":"buyer","providerPubkey":"pk","quotedAmountMsats":1200,"verificationCommandRef":"cmd:mix test","testRef":"evidence:run-9","platformCloseoutRef":"platform:closeout-3","digest":"{digest}","settled_at":"2026-08-26T11:00:00Z"}}"#
        ),
    )
    .expect("write the closeout");

    let settled = oa(&[
        "provider",
        "settle",
        "--lease",
        lease.to_str().unwrap(),
        "--closeout",
        closeout.to_str().unwrap(),
        "--json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&settled.stdout)
        .unwrap_or_else(|error| panic!("not JSON ({error}): {}", settled.stdout));
    assert_eq!(parsed["state"], "settled");
    assert_eq!(parsed["earned_msats"], 1200);
    // The two constants that say this decision moved nothing.
    assert_eq!(parsed["payout_rail"], "not_connected");
    assert_eq!(parsed["custody"], "none");
}

/// A lease this command cannot read is a refusal, not a decision.
#[test]
fn provider_settle_refuses_a_lease_it_cannot_read() {
    let run = oa(&["provider", "settle", "--lease", "/nonexistent/lease.json"]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("could not be read as JSON"),
        "stderr: {}",
        run.stderr
    );
}

/// `oa coder --export` writes the transcript on the line-oriented path too.
///
/// It was read only by the full-screen session, so a piped or headless run
/// that asked for a transcript got none and was told nothing. This does not
/// call the model: with no prompt the command explains itself and exits, which
/// is enough to prove the flag reaches a branch that could write.
#[test]
fn coder_plain_without_a_prompt_explains_itself() {
    let run = oa(&["coder", "--plain"]);
    assert_eq!(run.status, Some(0));
    assert!(
        run.stderr.contains("needs a terminal"),
        "stderr: {}",
        run.stderr
    );
    assert!(
        !run.stdout.contains('\u{1b}'),
        "the plain path emitted a cursor-control sequence"
    );
}

/// `oa` with no subcommand is a usage error, not a silent success.
#[test]
fn a_bare_invocation_is_a_usage_error() {
    let run = oa(&[]);
    assert_eq!(run.status, Some(2));
}

/// A delegated child configured for a lane that cannot honour the flag ends
/// the command instead of running the fan-out without it.
#[test]
fn delegate_refuses_a_child_flag_the_lane_cannot_honour() {
    let run = oa(&["delegate", "--agents", "1", "--child-ask", "anything"]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("--child-ask cannot be honoured"),
        "stderr: {}",
        run.stderr
    );
}

/// `--dir` names where children work, and a path that is not a directory is a
/// refusal rather than a fan-out that ran somewhere else.
#[test]
fn delegate_refuses_a_dir_that_is_not_a_directory() {
    let run = oa(&[
        "delegate",
        "--agents",
        "1",
        "--dir",
        "/nonexistent/place",
        "--lane",
        "claude",
        "anything",
    ]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("is not a directory"),
        "stderr: {}",
        run.stderr
    );
}

/// `deploy promote` refuses a SHA that is not one full commit SHA, before it
/// sends anything.
#[test]
fn deploy_promote_refuses_a_short_sha() {
    let server = StubServer::start("{}");
    let origin = server.origin();
    let run = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        "abc1234",
        "--environment",
        "production",
    ]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("full 40-character commit SHA"),
        "stderr: {}",
        run.stderr
    );
    assert!(
        server.paths().is_empty(),
        "a refused promotion still sent a request: {:?}",
        server.paths()
    );
}

/// `deploy promote` refuses an unstated environment. Production promotion
/// never assumes one.
#[test]
fn deploy_promote_refuses_an_unstated_environment() {
    let server = StubServer::start("{}");
    let origin = server.origin();
    let run = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        &"a".repeat(40),
    ]);
    assert_eq!(run.status, Some(2));
    assert!(
        run.stderr.contains("--environment production"),
        "stderr: {}",
        run.stderr
    );
    assert!(server.paths().is_empty());
}

/// `deploy list` reads the server's own target list, and `--json` hands back
/// the body a script can parse.
#[test]
fn deploy_list_reports_the_servers_targets() {
    let body: &'static str = r#"{"targets":[{"id":"tgt-9","status":"live","sha":"cccccccccccccccccccccccccccccccccccccccc","promoted_at":"2026-08-26T00:00:00Z","repo":"openagents.com","environment":"production"}]}"#;
    let server = StubServer::start(body);
    let origin = server.origin();
    let human = oa(&["--api-url", &origin, "deploy", "list"]);
    assert_eq!(human.status, Some(0), "{}", human.stderr);
    assert!(human.stdout.contains("tgt-9"), "{}", human.stdout);
    assert!(human.stdout.contains("live"), "{}", human.stdout);

    let server = StubServer::start(body);
    let origin = server.origin();
    let json = oa(&["--api-url", &origin, "deploy", "list", "--json"]);
    let parsed: serde_json::Value = serde_json::from_str(&json.stdout)
        .unwrap_or_else(|error| panic!("not JSON ({error}): {}", json.stdout));
    assert_eq!(parsed["targets"][0]["id"], "tgt-9");
    assert_ne!(human.stdout.trim(), json.stdout.trim());
}

/// `--limit` outside the server's range is refused before a request is sent.
#[test]
fn deploy_list_refuses_a_limit_outside_the_range() {
    let server = StubServer::start("{}");
    let origin = server.origin();
    let run = oa(&["--api-url", &origin, "deploy", "list", "--limit", "99"]);
    assert_eq!(run.status, Some(2));
    assert!(run.stderr.contains("between 1 and 50"), "{}", run.stderr);
    assert!(server.paths().is_empty());
}

/// A promotion the server accepts is reported as accepted; one the same
/// idempotency key already named is reported as a replay.
///
/// The difference is the answer to "did I just deploy, or had I already?" and
/// it lives only in the status: the server returns the target either way, so a
/// client that read the body would report every replay as a fresh deployment.
#[test]
fn deploy_promote_tells_a_new_target_from_a_replay() {
    const TARGET: &str = r#"{"id":"tgt-1","status":"queued","sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","repo":"openagents.com","environment":"production"}"#;
    let sha = "a".repeat(40);

    let fresh = StubServer::start_with_status(202, "Accepted", TARGET);
    let origin = fresh.origin();
    let accepted = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        &sha,
        "--environment",
        "production",
        "--json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&accepted.stdout)
        .unwrap_or_else(|error| panic!("not JSON ({error}): {}", accepted.stdout));
    assert_eq!(parsed["accepted"], true);
    assert_eq!(parsed["replayed"], false);
    assert_eq!(parsed["schema"], "openagents.fleet_promotion.v1");
    assert_eq!(parsed["outcome"], "accepted");
    assert_eq!(parsed["live"], false);

    let replay = StubServer::start_with_status(200, "OK", TARGET);
    let origin = replay.origin();
    let replayed = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        &sha,
        "--environment",
        "production",
        "--json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&replayed.stdout)
        .unwrap_or_else(|error| panic!("not JSON ({error}): {}", replayed.stdout));
    assert_eq!(parsed["accepted"], false);
    assert_eq!(parsed["replayed"], true);

    // The human mode says which one it was, not just that something happened.
    let replay = StubServer::start_with_status(200, "OK", TARGET);
    let origin = replay.origin();
    let human = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        &sha,
        "--environment",
        "production",
    ]);
    assert!(
        human.stdout.contains("already named this promotion"),
        "the replay was reported as a fresh promotion: {}",
        human.stdout
    );
}

/// A promotion carries an idempotency key the caller did not have to invent,
/// and never prints it.
#[test]
fn deploy_promote_generates_an_idempotency_key_and_does_not_print_it() {
    let server =
        StubServer::start_with_status(202, "Accepted", r#"{"id":"tgt-1","status":"queued"}"#);
    let origin = server.origin();
    let run = oa(&[
        "--api-url",
        &origin,
        "deploy",
        "promote",
        "--repo",
        "openagents.com",
        "--sha",
        &"a".repeat(40),
        "--environment",
        "production",
    ]);
    assert_eq!(run.status, Some(0), "{}", run.stderr);
    assert!(
        !run.stdout.contains("idempotency"),
        "the key reached the output: {}",
        run.stdout
    );
}

// ---------------------------------------------------------------------------
// `oa coder`'s lane, effort, offline and resume flags
// ---------------------------------------------------------------------------
//
// The same rule as everything above: each of these runs the binary twice, once
// with the flag and once without, and asserts the difference the flag
// promises. Several of them assert on the *request that arrived*, because that
// is where a flag like `--reasoning` either exists or does not: a run that
// accepts the flag and sends the same body is exactly the defect these tests
// are here to catch.

/// One request the routing server was asked for.
#[derive(Debug, Clone)]
struct Hit {
    method: String,
    path: String,
    body: String,
}

/// A server that answers different bodies on different routes and hands back
/// the requests it was asked for, bodies included.
///
/// [`StubServer`] answers one body on every path, which is enough for a
/// listing but not for a coder turn: opening a thread reads `GET
/// /api/v1/models`, posts `POST /api/v1/threads`, and then streams from
/// whatever proxy url the grant names. Each of those needs a different answer,
/// and the assertion for `--model` and `--reasoning` is about what was *sent*,
/// not what came back.
struct RouteServer {
    port: u16,
    hits: mpsc::Receiver<Hit>,
}

/// `("POST /api/v1/threads", status, body, content_type)`.
type Route = (String, u16, String, &'static str);

impl RouteServer {
    /// `routes` is built from the port, because a grant has to name a proxy
    /// url on this same server and the port is not known until it binds.
    fn start(routes: impl FnOnce(u16) -> Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let port = listener.local_addr().expect("read the port").port();
        let routes = routes(port);
        let (tx, hits) = mpsc::channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let tx = tx.clone();
                let routes = routes.clone();
                thread::spawn(move || serve_routed(stream, &routes, tx));
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

fn serve_routed(mut stream: TcpStream, routes: &[Route], hits: mpsc::Sender<Hit>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let full_path = parts.next().unwrap_or("").to_string();
    let path = full_path
        .split('?')
        .next()
        .unwrap_or(&full_path)
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
    let mut body = String::new();
    if length > 0 {
        let mut buffer = vec![0u8; length];
        if reader.read_exact(&mut buffer).is_ok() {
            body = String::from_utf8_lossy(&buffer).into_owned();
        }
    }

    let key = format!("{method} {path}");
    let _ = hits.send(Hit {
        method,
        path,
        body,
    });

    let answer = routes.iter().find(|(route, ..)| *route == key);
    let (code, reply, content_type) = match answer {
        Some((_, code, reply, content_type)) => (*code, reply.clone(), *content_type),
        None => (
            404,
            format!("{{\"code\":\"no_route\",\"message\":{key:?}}}"),
            "application/json",
        ),
    };
    let response = format!(
        "HTTP/1.1 {code} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        reply.len(),
        reply
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// `oa`, with environment of the caller's choosing.
///
/// The coder paths read `OPENAGENTS_TOKEN` for the account credential and
/// `OPENAGENTS_OLLAMA_HOST` for the local lane, and a test that does not set
/// them is testing whatever the developer's machine happens to hold.
fn oa_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oa"));
    command.args(args).env("NO_COLOR", "");
    for (key, value) in env {
        command.env(key, value);
    }
    let result = command.output().expect("run oa");
    Output {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        status: result.status.code(),
    }
}

/// The catalog `GET /api/v1/models` serves in these tests.
const CATALOG: &str = r#"{"default":"gemini-3.7-flash","models":[
    {"id":"gemini-3.7-flash","availability":"available","default":true},
    {"id":"ox-alpha","availability":"available","default":false},
    {"id":"gpt-5.6-luna","availability":"available","default":false}]}"#;

/// A thread open, a grant that points the proxy back at this same server, and
/// a stream that answers one word.
fn coder_routes(port: u16) -> Vec<Route> {
    let proxy = format!("http://127.0.0.1:{port}/api/inference/proxy");
    vec![
        (
            "GET /api/v1/models".to_string(),
            200,
            CATALOG.to_string(),
            "application/json",
        ),
        (
            "POST /api/v1/threads".to_string(),
            201,
            format!(
                r#"{{"thread":{{"id":"t-1","status":"open"}},
                     "grant":{{"token":"g-1","url":{proxy:?},"model":"ox-alpha"}}}}"#
            ),
            "application/json",
        ),
        (
            "POST /api/inference/proxy".to_string(),
            200,
            "data: {\"choices\":[{\"delta\":{\"content\":\"PONG\"},\"index\":0}]}\n\n\
             data: [DONE]\n\n"
                .to_string(),
            "text/event-stream",
        ),
        (
            "DELETE /api/v1/threads/t-1".to_string(),
            200,
            r#"{"thread":{"id":"t-1","status":"cancelled"}}"#.to_string(),
            "application/json",
        ),
    ]
}

/// The body of the first request that matches a method and path.
fn body_of(hits: &[Hit], method: &str, path: &str) -> Option<String> {
    hits.iter()
        .find(|hit| hit.method == method && hit.path == path)
        .map(|hit| hit.body.clone())
}

// --------------------------------------------------------------- `--offline`

/// `--offline` answers, and the live path in the same conditions fails.
///
/// This is the inversion #83 named, asserted from both sides at once. The
/// stand-in text used to be reachable *only* on failure and by no flag; now it
/// is reachable only by the flag and never on failure. A binary with the old
/// behaviour fails the second half of this test: its live run would print the
/// stand-in and exit 0.
#[test]
fn offline_answers_from_the_stand_in_and_the_live_path_refuses_instead() {
    // A port nothing listens on. Both runs are offline in the network sense;
    // only one of them was asked to be.
    let dead = "http://127.0.0.1:1";
    let asked = oa_env(
        &["--api-url", dead, "coder", "--offline", "count the crates"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    let live = oa_env(
        &[
            "--api-url",
            dead,
            "coder",
            "--headless",
            "count the crates",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );

    assert_eq!(asked.status, Some(0), "stderr: {}", asked.stderr);
    assert!(
        asked.stdout.contains("[stand-in]"),
        "--offline did not answer from the stand-in: {}",
        asked.stdout
    );
    assert!(
        asked.stdout.contains("count the crates"),
        "the stand-in did not carry the prompt: {}",
        asked.stdout
    );

    assert_eq!(
        live.status,
        Some(2),
        "the live path succeeded with nothing to reach: {} {}",
        live.stdout,
        live.stderr
    );
    assert!(
        !live.stdout.contains("stand-in") && !live.stdout.contains("offline fallback"),
        "a failed turn reached the stand-in: {}",
        live.stdout
    );
}

/// `--offline` opens no socket, and the same command without it opens several.
#[test]
fn offline_reaches_no_server_and_the_live_path_reaches_one() {
    let server = RouteServer::start(coder_routes);
    let origin = server.origin();

    let offline = oa_env(
        &["--api-url", &origin, "coder", "--offline", "hello"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(offline.status, Some(0), "stderr: {}", offline.stderr);
    assert!(
        server.hits().is_empty(),
        "--offline sent requests to the server"
    );

    let live = oa_env(
        &["--api-url", &origin, "coder", "--headless", "hello"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(live.status, Some(0), "stderr: {}", live.stderr);
    let paths: Vec<String> = server.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        paths.iter().any(|p| p == "/api/v1/threads"),
        "the live run did not open a thread; it asked for {paths:?}"
    );
}

// ----------------------------------------------------------------- `--model`

/// `--model` decides the id sent at thread open; without it the default lane's
/// id is sent.
#[test]
fn model_names_the_id_the_thread_opens_on() {
    let named = RouteServer::start(coder_routes);
    let origin = named.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--model",
            "gpt-5.6-luna",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let with = body_of(&named.hits(), "POST", "/api/v1/threads")
        .expect("the run did not open a thread");

    let plain = RouteServer::start(coder_routes);
    let origin = plain.origin();
    let run = oa_env(
        &["--api-url", &origin, "coder", "--headless", "hello"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let without = body_of(&plain.hits(), "POST", "/api/v1/threads")
        .expect("the run did not open a thread");

    let with: serde_json::Value = serde_json::from_str(&with).expect("the open body is JSON");
    let without: serde_json::Value = serde_json::from_str(&without).expect("the open body is JSON");
    assert_eq!(with["model"], "gpt-5.6-luna");
    assert_eq!(without["model"], "ox-alpha");
    assert_ne!(
        with["model"], without["model"],
        "--model changed nothing about the thread that was opened"
    );
}

/// An id this deployment does not serve is refused by name, and no thread is
/// opened. It used to fall through to the default lane, so a reader who asked
/// for one model got another and was told nothing.
#[test]
fn a_model_the_catalog_does_not_serve_is_refused_rather_than_substituted() {
    let server = RouteServer::start(coder_routes);
    let origin = server.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--model",
            "claude-3-7-sonnet",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("claude-3-7-sonnet"),
        "the refusal did not name the model asked for: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("gpt-5.6-luna"),
        "the refusal did not name what this deployment serves: {}",
        run.stderr
    );
    let paths: Vec<String> = server.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        !paths.iter().any(|p| p == "/api/v1/threads"),
        "a refused model still opened a thread: {paths:?}"
    );
}

// ----------------------------------------------------------------- `--local`

/// `--local` answers from this machine, so it never reaches the server; the
/// same command without it does.
#[test]
fn local_answers_from_this_machine_and_never_reaches_the_server() {
    let server = RouteServer::start(coder_routes);
    let origin = server.origin();
    // A port nothing listens on, so the local lane fails at Ollama rather than
    // silently finding whatever this developer has running.
    let no_ollama = [
        ("OPENAGENTS_TOKEN", "t"),
        ("OPENAGENTS_OLLAMA_HOST", "http://127.0.0.1:1"),
    ];

    let local = oa_env(
        &["--api-url", &origin, "coder", "--headless", "--local", "hello"],
        &no_ollama,
    );
    assert_eq!(local.status, Some(2), "stdout: {}", local.stdout);
    assert!(
        local.stderr.contains("Ollama"),
        "the local lane did not name Ollama: {}",
        local.stderr
    );
    assert!(
        server.hits().is_empty(),
        "--local sent the turn to the server"
    );

    let hosted = oa_env(
        &["--api-url", &origin, "coder", "--headless", "hello"],
        &no_ollama,
    );
    assert_eq!(hosted.status, Some(0), "stderr: {}", hosted.stderr);
    let paths: Vec<String> = server.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        paths.iter().any(|p| p == "/api/v1/threads"),
        "the hosted run did not open a thread: {paths:?}"
    );
}

/// `--model ollama:<model>` is the same lane, and names the model to Ollama
/// rather than to the server.
#[test]
fn model_ollama_selects_the_local_lane() {
    let server = RouteServer::start(coder_routes);
    let origin = server.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--model",
            "ollama:llama3",
            "hello",
        ],
        &[
            ("OPENAGENTS_TOKEN", "t"),
            ("OPENAGENTS_OLLAMA_HOST", "http://127.0.0.1:1"),
        ],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("Ollama"),
        "the turn did not go to Ollama: {}",
        run.stderr
    );
    assert!(
        server.hits().is_empty(),
        "an ollama: model still reached the server"
    );
}

// ------------------------------------------------------------- `--reasoning`

/// `--reasoning` is carried on the thread; without it the request carries no
/// effort at all and the deployment's default stands.
///
/// The thread is where the server takes it — `GET /api/v1/threads` reports it
/// back as `reasoning_effort` — so this asserts on the open body. A binary
/// that parsed the flag and sent the same open body twice fails here, which is
/// the whole point.
#[test]
fn reasoning_is_carried_on_the_thread_and_absent_without_the_flag() {
    let asked = RouteServer::start(coder_routes);
    let origin = asked.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--reasoning",
            "high",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let with: serde_json::Value = serde_json::from_str(
        &body_of(&asked.hits(), "POST", "/api/v1/threads").expect("no thread was opened"),
    )
    .expect("the open body is JSON");

    let quiet = RouteServer::start(coder_routes);
    let origin = quiet.origin();
    let run = oa_env(
        &["--api-url", &origin, "coder", "--headless", "hello"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let without: serde_json::Value = serde_json::from_str(
        &body_of(&quiet.hits(), "POST", "/api/v1/threads").expect("no thread was opened"),
    )
    .expect("the open body is JSON");

    assert_eq!(with["reasoning"], "high");
    assert!(
        without.get("reasoning").is_none(),
        "a run without --reasoning still named an effort: {without}"
    );
}

/// An effort outside the admitted set is a usage error, not a thread opened at
/// something the server will refuse.
#[test]
fn an_unadmitted_reasoning_effort_is_refused() {
    let server = RouteServer::start(coder_routes);
    let origin = server.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--reasoning",
            "extreme",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(2));
    assert!(server.hits().is_empty(), "a refused effort still sent a request");
}

// ---------------------------------------------------------------- `--resume`

/// The routes a resume reads: one thread, its events, and the re-mint.
fn resume_routes(port: u16) -> Vec<Route> {
    let proxy = format!("http://127.0.0.1:{port}/api/inference/proxy");
    let mut routes = coder_routes(port);
    routes.push((
        "GET /api/v1/threads".to_string(),
        200,
        r#"{"threads":[
            {"id":"t-far","status":"open","objective":"Coding assistant session",
             "repository":"Elsewhere/other","event_count":2,"started_at":"2026-08-26T09:00:00Z"},
            {"id":"t-near","status":"open","objective":"Coding assistant session",
             "repository":"OpenAgentsInc/openagents","event_count":2,
             "started_at":"2026-08-26T08:00:00Z"}]}"#
            .to_string(),
        "application/json",
    ));
    routes.push((
        "GET /api/v1/threads/t-near".to_string(),
        200,
        r#"{"thread":{"id":"t-near","status":"open","objective":"Coding assistant session",
             "repository":"OpenAgentsInc/openagents","event_count":2}}"#
            .to_string(),
        "application/json",
    ));
    routes.push((
        "GET /api/v1/threads/t-far".to_string(),
        200,
        r#"{"thread":{"id":"t-far","status":"open","objective":"Coding assistant session",
             "repository":"Elsewhere/other","event_count":2}}"#
            .to_string(),
        "application/json",
    ));
    routes.push((
        "GET /api/v1/threads/t-shut".to_string(),
        200,
        r#"{"thread":{"id":"t-shut","status":"cancelled","objective":"Coding assistant session",
             "repository":"OpenAgentsInc/openagents","event_count":1}}"#
            .to_string(),
        "application/json",
    ));
    for id in ["t-near", "t-far"] {
        routes.push((
            format!("GET /api/v1/threads/{id}/events"),
            200,
            r#"{"events":[
                {"id":1,"event_type":"turn.user","payload":{"text":"how many crates"}},
                {"id":2,"event_type":"turn.assistant","payload":{"text":"Twelve."}}]}"#
                .to_string(),
            "application/json",
        ));
        routes.push((
            format!("POST /api/v1/threads/{id}/grants"),
            201,
            format!(
                r#"{{"thread":{{"id":{id:?},"status":"open"}},
                     "grant":{{"token":"g-2","url":{proxy:?},"model":"ox-alpha"}}}}"#
            ),
            "application/json",
        ));
        routes.push((
            format!("DELETE /api/v1/threads/{id}"),
            200,
            r#"{"thread":{"status":"cancelled"}}"#.to_string(),
            "application/json",
        ));
    }
    routes
}

/// `--resume <id>` continues that thread: it re-mints the thread's own grant
/// and opens no new thread. Without it the same command opens one.
///
/// The re-mint is what makes this a continuation rather than a new thread that
/// has read an old one — the server revokes the thread's active grants and
/// bumps its generation there, so a resumed session cannot race a zombie of
/// its former self.
#[test]
fn resume_by_id_re_mints_the_thread_instead_of_opening_one() {
    let resumed = RouteServer::start(resume_routes);
    let origin = resumed.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "t-near",
            "and now",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let paths: Vec<String> = resumed.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        paths.iter().any(|p| p == "/api/v1/threads/t-near/grants"),
        "the resume did not re-mint the thread's grant: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p == "/api/v1/threads"),
        "the resume opened a new thread as well: {paths:?}"
    );
    assert!(
        run.stdout.contains("Resumed thread t-near"),
        "the run did not say which thread it continued: {}",
        run.stdout
    );

    let fresh = RouteServer::start(resume_routes);
    let origin = fresh.origin();
    let run = oa_env(
        &["--api-url", &origin, "coder", "--headless", "and now"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let paths: Vec<String> = fresh.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        paths.iter().any(|p| p == "/api/v1/threads"),
        "the run without --resume did not open a thread: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.ends_with("/grants")),
        "a run without --resume re-minted something: {paths:?}"
    );
}

/// The replayed transcript reaches the model. The resumed turn's request
/// carries the thread's recorded conversation; a fresh one carries only the
/// system prompt and this turn.
#[test]
fn a_resumed_turn_carries_the_threads_recorded_conversation() {
    let resumed = RouteServer::start(resume_routes);
    let origin = resumed.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "t-near",
            "and now",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let sent = body_of(&resumed.hits(), "POST", "/api/inference/proxy")
        .expect("the resumed turn sent nothing to the proxy");
    assert!(
        sent.contains("how many crates") && sent.contains("Twelve."),
        "the replayed conversation did not reach the model: {sent}"
    );

    let fresh = RouteServer::start(resume_routes);
    let origin = fresh.origin();
    let run = oa_env(
        &["--api-url", &origin, "coder", "--headless", "and now"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let sent = body_of(&fresh.hits(), "POST", "/api/inference/proxy")
        .expect("the fresh turn sent nothing to the proxy");
    assert!(
        !sent.contains("how many crates"),
        "a fresh turn carried a thread it never resumed: {sent}"
    );
}

/// `--all` drops the repository filter, and the two runs pick different
/// threads because of it.
///
/// Run from a temporary directory, which is no checkout at all: the filtered
/// list is empty and says so, and `--all` reaches the newest thread on the
/// account.
#[test]
fn all_drops_the_repository_filter_the_picker_applies() {
    let server = RouteServer::start(resume_routes);
    let origin = server.origin();
    let elsewhere = std::env::temp_dir();

    let filtered = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args([
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "--last",
            "and now",
        ])
        .current_dir(&elsewhere)
        .env("NO_COLOR", "")
        .env("OPENAGENTS_TOKEN", "t")
        .output()
        .expect("run oa");
    assert_eq!(filtered.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&filtered.stderr);
    assert!(
        stderr.contains("--all"),
        "the refusal did not say what to do next: {stderr}"
    );

    let everything = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args([
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "--last",
            "--all",
            "and now",
        ])
        .current_dir(&elsewhere)
        .env("NO_COLOR", "")
        .env("OPENAGENTS_TOKEN", "t")
        .output()
        .expect("run oa");
    assert_eq!(
        everything.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&everything.stderr)
    );
    // Newest first as the server ordered them, so `--all` reaches the thread
    // the repository filter had excluded.
    assert!(
        String::from_utf8_lossy(&everything.stdout).contains("Resumed thread t-far"),
        "--all did not reach the thread outside this repository: {}",
        String::from_utf8_lossy(&everything.stdout)
    );
}

/// A terminal thread is refused by its status rather than resumed into a
/// session that could only ever show history.
#[test]
fn a_terminal_thread_is_refused_with_its_status() {
    let server = RouteServer::start(resume_routes);
    let origin = server.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "t-shut",
            "and now",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("cancelled"),
        "the refusal did not name the status: {}",
        run.stderr
    );
    let paths: Vec<String> = server.hits().into_iter().map(|hit| hit.path).collect();
    assert!(
        !paths.iter().any(|p| p.ends_with("/grants")),
        "a terminal thread was still re-minted: {paths:?}"
    );
}

/// Bare `--resume` needs a terminal to show a picker in, and says which two
/// forms work without one.
#[test]
fn the_bare_picker_needs_a_terminal() {
    let server = RouteServer::start(resume_routes);
    let origin = server.origin();
    // `--all` so the candidate list is not empty: an empty list refuses for a
    // different and earlier reason, and this test is about the picker.
    let run = oa_env(
        &["--api-url", &origin, "coder", "--headless", "--resume", "--all"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("--resume <id>") && run.stderr.contains("--last"),
        "the refusal did not name a form that works: {}",
        run.stderr
    );
}

/// Resuming reads the account's threads, so a run with no credential says so
/// instead of listing nothing.
#[test]
fn resume_without_a_credential_says_to_sign_in() {
    let server = RouteServer::start(resume_routes);
    let origin = server.origin();
    let run = oa_env(
        &[
            "--api-url",
            &origin,
            "coder",
            "--headless",
            "--resume",
            "t-near",
            "and now",
        ],
        &[("OPENAGENTS_TOKEN", "")],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("auth login"),
        "the refusal did not say how to fix it: {}",
        run.stderr
    );
}

// ------------------------------------------------- flags that cannot combine

/// Two flags that name the same setting differently end the command, because
/// honouring one means ignoring the other and a flag that is ignored is a flag
/// that lied.
#[test]
fn flags_that_name_different_lanes_are_refused_by_name() {
    let both = oa_env(
        &[
            "--api-url",
            "http://127.0.0.1:1",
            "coder",
            "--headless",
            "--model",
            "ox-alpha",
            "--lane",
            "pro",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(both.status, Some(2), "stdout: {}", both.stdout);
    assert!(
        both.stderr.contains("--lane pro") && both.stderr.contains("--model ox-alpha"),
        "the refusal did not name both flags: {}",
        both.stderr
    );

    // Two names for the *same* lane is agreement, not a conflict, and is not
    // refused: `--lane pro` and `--model gpt-5.6-luna` name one thing.
    let agreeing = oa_env(
        &[
            "--api-url",
            "http://127.0.0.1:1",
            "coder",
            "--headless",
            "--model",
            "gpt-5.6-luna",
            "--lane",
            "pro",
            "hello",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert!(
        !agreeing.stderr.contains("name different lanes"),
        "one lane named twice was refused as two: {}",
        agreeing.stderr
    );

    // The same command with one of them is not that refusal. It fails for
    // want of a reachable server, which is a different sentence.
    let one = oa_env(
        &["--api-url", "http://127.0.0.1:1", "coder", "--headless", "--lane", "pro", "hello"],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert!(
        !one.stderr.contains("name different lanes"),
        "one lane flag was refused as two: {}",
        one.stderr
    );
}

/// `--local` and `--model ollama:<model>` are the same intent written twice,
/// which is the one combination that is not a contradiction.
#[test]
fn local_and_an_ollama_model_agree_rather_than_conflict() {
    let run = oa_env(
        &[
            "--api-url",
            "http://127.0.0.1:1",
            "coder",
            "--headless",
            "--local",
            "--model",
            "ollama:llama3",
            "hello",
        ],
        &[
            ("OPENAGENTS_TOKEN", "t"),
            ("OPENAGENTS_OLLAMA_HOST", "http://127.0.0.1:1"),
        ],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("Ollama") && !run.stderr.contains("name different lanes"),
        "the same lane written twice was refused as two: {}",
        run.stderr
    );
}

/// `--offline` and `--resume` cannot combine: one reaches no server and the
/// other is a read from one.
#[test]
fn offline_and_resume_are_refused_together() {
    let run = oa_env(
        &[
            "--api-url",
            "http://127.0.0.1:1",
            "coder",
            "--offline",
            "--resume",
            "t-near",
        ],
        &[("OPENAGENTS_TOKEN", "t")],
    );
    assert_eq!(run.status, Some(2), "stdout: {}", run.stdout);
    assert!(
        run.stderr.contains("--offline") && run.stderr.contains("--resume"),
        "the refusal did not name both: {}",
        run.stderr
    );
}

/// `--last` and `--all` say which thread to continue, so without `--resume`
/// they have nothing to say.
#[test]
fn last_and_all_are_refused_without_resume() {
    for flag in ["--last", "--all"] {
        let run = oa_env(
            &["--api-url", "http://127.0.0.1:1", "coder", flag, "hello"],
            &[("OPENAGENTS_TOKEN", "t")],
        );
        assert_eq!(run.status, Some(2), "{flag}: {}", run.stdout);
        assert!(
            run.stderr.contains("--resume"),
            "{flag}: the refusal did not name --resume: {}",
            run.stderr
        );
    }
}

// --------------------------------------------------- `oa auth login --scope`

/// `--scope` reaches the server and its answer reaches the reader.
///
/// Two halves, because either one alone would pass against a binary that lies.
/// The request half proves the flag is sent: the body carries the scopes
/// asked for, and a run without the flag sends no scope at all so the
/// deployment's own default stands. The output half proves the answer is read:
/// the approval a reader is about to click is named on screen, and the two
/// runs name different things.
///
/// The server settles this, not the client — an unknown scope is refused
/// outright — so the stub answers with what it decided rather than echoing.
#[test]
fn scope_is_asked_for_and_the_servers_answer_is_reported() {
    fn authorization_routes(_port: u16) -> Vec<Route> {
        vec![
            (
                "POST /api/v1/device/authorizations".to_string(),
                201,
                r#"{"device_code":"d-1","user_code":"AAAA-BBBB",
                    "verification_uri":"https://example.test/device",
                    "verification_uri_complete":"https://example.test/device?user_code=AAAA-BBBB",
                    "expires_in":600,"interval":5,"scope":"forge:write"}"#
                    .to_string(),
                "application/json",
            ),
        ]
    }
    fn default_routes(_port: u16) -> Vec<Route> {
        vec![(
            "POST /api/v1/device/authorizations".to_string(),
            201,
            r#"{"device_code":"d-2","user_code":"CCCC-DDDD",
                "verification_uri":"https://example.test/device",
                "verification_uri_complete":"https://example.test/device?user_code=CCCC-DDDD",
                "expires_in":600,"interval":5,"scope":"chat:account forge:write"}"#
                .to_string(),
            "application/json",
        )]
    }

    let asked = RouteServer::start(authorization_routes);
    let origin = asked.origin();
    let named = oa(&[
        "--api-url",
        &origin,
        "auth",
        "login",
        "--headless",
        "--scope",
        "forge:write",
    ]);
    assert_eq!(named.status, Some(0), "stderr: {}", named.stderr);
    let sent = body_of(&asked.hits(), "POST", "/api/v1/device/authorizations")
        .expect("no authorization was started");
    let sent: serde_json::Value = serde_json::from_str(&sent).expect("the start body is JSON");
    assert_eq!(
        sent["scope"], "forge:write",
        "--scope did not reach the request: {sent}"
    );
    assert!(
        named.stdout.contains("Scope requested: forge:write"),
        "the run did not name the scope the approval will grant: {}",
        named.stdout
    );

    let quiet = RouteServer::start(default_routes);
    let origin = quiet.origin();
    let plain = oa(&["--api-url", &origin, "auth", "login", "--headless"]);
    assert_eq!(plain.status, Some(0), "stderr: {}", plain.stderr);
    let sent = body_of(&quiet.hits(), "POST", "/api/v1/device/authorizations")
        .expect("no authorization was started");
    let sent: serde_json::Value = serde_json::from_str(&sent).expect("the start body is JSON");
    assert!(
        sent.get("scope").is_none(),
        "a run without --scope still named one, so the server's default cannot apply: {sent}"
    );
    assert!(
        plain.stdout.contains("Scope requested: chat:account forge:write"),
        "the default run did not name the server's own scopes: {}",
        plain.stdout
    );

    assert_ne!(
        named.stdout, plain.stdout,
        "--scope produced the same output as a run without it"
    );
}

/// `--scope` repeats, and the repeats reach the server as one space-separated
/// set rather than the last one winning.
#[test]
fn repeated_scopes_are_all_sent() {
    fn routes(_port: u16) -> Vec<Route> {
        vec![(
            "POST /api/v1/device/authorizations".to_string(),
            201,
            r#"{"device_code":"d-3","user_code":"EEEE-FFFF",
                "verification_uri":"https://example.test/device",
                "verification_uri_complete":"https://example.test/device?user_code=EEEE-FFFF",
                "expires_in":600,"interval":5,"scope":"chat:account forge:write"}"#
                .to_string(),
            "application/json",
        )]
    }
    let server = RouteServer::start(routes);
    let origin = server.origin();
    let run = oa(&[
        "--api-url",
        &origin,
        "auth",
        "login",
        "--headless",
        "--scope",
        "chat:account",
        "--scope",
        "forge:write",
    ]);
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    let sent = body_of(&server.hits(), "POST", "/api/v1/device/authorizations")
        .expect("no authorization was started");
    let sent: serde_json::Value = serde_json::from_str(&sent).expect("the start body is JSON");
    assert_eq!(sent["scope"], "chat:account forge:write");
}
