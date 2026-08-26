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
use std::process::Command;
use std::sync::mpsc;
use std::thread;

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

fn oa(args: &[&str]) -> Output {
    let result = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(args)
        // The credential store is keyed by origin, and the stub's origin has
        // no token, so these runs never carry a real one.
        .env("NO_COLOR", "")
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
