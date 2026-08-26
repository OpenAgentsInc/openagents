//! Parity between `oa` and the TypeScript `openagents` CLI, and the failure
//! contract both of them owe a caller.
//!
//! Written for issue #88, which was reopened because its predecessor suite was
//! 14 green assertions over a CLI that could not reach a model, could not list
//! a project, and invented forum boards. Two rules follow from that, and every
//! test here obeys both.
//!
//! **A refusal must be distinguishable from a network failure.** The old suite
//! asserted `result.is_err()` against the live origin, which an unplugged
//! cable satisfies exactly as well as a 404 does. Nothing here asserts
//! `is_err()`. Every refusal test names the status it expects and fails on any
//! other error, so an offline machine turns these red rather than green.
//!
//! **A test must fail if the feature regresses.** These run the real binary
//! against a server the test owns, and read the request that server actually
//! received. Asserting that a subcommand parses would pass against a binary
//! that parsed it and sent nothing.
//!
//! The TypeScript expectations are not read from a live `openagents` process —
//! that would make the suite depend on a `dist/` build that a fresh checkout
//! does not have, and a test that silently skips when its fixture is missing
//! is the defect this issue is about. They are recorded constants, each one
//! captured from a run of the TypeScript CLI at `cd0c05d465` and cited in the
//! comment above it. When the two CLIs are meant to agree, the recorded value
//! is what `oa` is asserted against.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use openagents_cli::forum::ForumError;
use openagents_cli::tracker::{
    error_sentence, ApiError, IssueListOptions, RepoTarget, TrackerClient,
};

// ---------------------------------------------------------------------------
// A server the test owns
// ---------------------------------------------------------------------------

/// One request the stub received. The point of recording it is that a client
/// which parses its arguments and sends nothing looks identical, at the
/// process boundary, to one that works.
#[derive(Debug, Clone)]
struct Hit {
    method: String,
    path: String,
}

impl Hit {
    fn route(&self) -> String {
        format!("{} {}", self.method, self.path)
    }
}

/// A server that answers from a script and records every request.
///
/// The script is a list of `(status, content_type, body)` answered in order,
/// with the last entry repeating.
struct StubServer {
    port: u16,
    hits: mpsc::Receiver<Hit>,
}

impl StubServer {
    fn start(script: Vec<(u16, &'static str, Vec<u8>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let port = listener.local_addr().expect("read the port").port();
        let (tx, hits) = mpsc::channel();
        thread::spawn(move || {
            for (answered, stream) in listener.incoming().enumerate() {
                let Ok(stream) = stream else { break };
                let index = answered.min(script.len().saturating_sub(1));
                let (code, content_type, body) = script[index].clone();
                serve_one(stream, code, content_type, &body, tx.clone());
            }
        });
        Self { port, hits }
    }

    /// A server that answers everything the same way.
    fn always(code: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self::start(vec![(code, content_type, body)])
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn api_base(&self) -> String {
        format!("{}/api/v1", self.origin())
    }

    fn hits(&self) -> Vec<Hit> {
        self.hits.try_iter().collect()
    }
}

fn serve_one(
    mut stream: TcpStream,
    code: u16,
    content_type: &str,
    body: &[u8],
    hits: mpsc::Sender<Hit>,
) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
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
    let mut payload = vec![0u8; length];
    if length > 0 && reader.read_exact(&mut payload).is_err() {
        return;
    }
    let _ = hits.send(Hit { method, path });
    let response = format!(
        "HTTP/1.1 {code} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

struct Run {
    stdout: String,
    stderr: String,
    status: Option<i32>,
}

impl Run {
    /// The exit code, insisting the process exited rather than died on a
    /// signal or a panic-abort. `None` here is itself a failure worth naming.
    fn code(&self) -> i32 {
        self.status
            .unwrap_or_else(|| panic!("oa did not exit normally. stderr: {}", self.stderr))
    }

    fn panicked(&self) -> bool {
        self.stderr.contains("panicked at")
    }
}

fn oa(origin: &str, args: &[&str]) -> Run {
    let mut full = vec!["--api-url", origin];
    full.extend(args.iter().copied());
    let result = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(&full)
        .env("NO_COLOR", "")
        .env("OPENAGENTS_TOKEN", "oa_pat_stub")
        .output()
        .expect("run oa");
    Run {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        status: result.status.code(),
    }
}

fn target() -> RepoTarget {
    RepoTarget {
        owner: "OpenAgentsInc".to_string(),
        repo: "openagents".to_string(),
    }
}

/// A list request the client will actually send. `IssueListOptions::default()`
/// has `limit: 0`, which `list_issues` refuses locally — a test using it would
/// assert on an input error and never reach the server at all.
fn one_page() -> IssueListOptions {
    IssueListOptions {
        limit: 25,
        ..Default::default()
    }
}

fn client(server: &StubServer) -> TrackerClient {
    TrackerClient::new(&server.api_base(), Some("oa_pat_stub".to_string()))
}

/// The status an `ApiError` carries, or a failure naming what arrived instead.
///
/// This is the whole point of the file. `assert!(result.is_err())` passes on a
/// DNS failure, a captive portal, and a closed laptop lid; this passes only on
/// a server that answered and refused.
fn refused_status(error: &ApiError) -> u16 {
    match error {
        ApiError::Refused { status, .. } => *status,
        other => panic!(
            "expected the server to answer and refuse, got a different failure: {other}. \
             A transport error here means the test proved nothing."
        ),
    }
}

// ---------------------------------------------------------------------------
// 1. A refusal is reported, never rendered as data
// ---------------------------------------------------------------------------

/// An HTML error page from a proxy is reported, not sliced mid-character.
///
/// This is the regression that reopened the issue in a new place. Every
/// non-2xx body reaches `error_sentence`, which bounded it at 400 *bytes*.
/// A 502 from a proxy is not JSON and is longer than that, and if its 400th
/// byte lands inside a multi-byte character the process aborts — on the one
/// code path whose entire job is to report that the request was refused.
///
/// Measured before the fix: `oa issue list`, `project list`, `milestone list`,
/// `issue view`, `box list`, `memory list`, `deploy list`, and `forum boards`
/// all died with exit 101 and a stack dump. The TypeScript CLI answered the
/// same body with exit 6 and one sentence.
///
/// The body below is built so byte 400 is the second byte of `é`.
#[test]
fn a_non_json_refusal_body_is_reported_rather_than_panicked_on() {
    let mut body = vec![b'A'; 399];
    body.extend("é".as_bytes());
    body.extend(vec![b'B'; 200]);
    assert!(!body.is_empty());

    // The cut has to be inside the character for this to test anything.
    let text = String::from_utf8(body.clone()).unwrap();
    assert!(
        !text.is_char_boundary(400),
        "the fixture no longer straddles the bound, so it proves nothing"
    );

    let sentence = error_sentence(&text, 502);
    assert!(
        sentence.starts_with("AAAA"),
        "the server's own body is what gets reported: {sentence}"
    );
    assert!(
        sentence.len() <= 400,
        "the sentence is bounded: {} bytes",
        sentence.len()
    );
}

/// The same body, through the real binary, on every client that renders one.
///
/// `error_sentence` is one function but four clients call it and a fifth
/// (`forum`) had its own copy of the same slice. Asserting the function alone
/// would have missed `forum boards`, which panicked at a different line.
#[test]
fn no_command_dies_on_a_proxy_error_page() {
    let mut body = vec![b'A'; 399];
    body.extend("é".as_bytes());
    body.extend(vec![b'B'; 200]);
    let server = StubServer::always(502, "text/html", body);
    let origin = server.origin();

    // One per client module: tracker, box, memory, forum, and the fleet
    // routes. Each of these exited 101 before the fix.
    let commands: &[&[&str]] = &[
        &["issue", "list", "-R", "OpenAgentsInc/openagents"],
        &["project", "list", "-R", "OpenAgentsInc/openagents"],
        &["milestone", "list", "-R", "OpenAgentsInc/openagents"],
        &["issue", "view", "1", "-R", "OpenAgentsInc/openagents"],
        &["box", "list", "--conversation", "conv_stub"],
        &["memory", "list"],
        &["forum", "boards"],
        &["deploy", "list"],
    ];

    for command in commands {
        let run = oa(&origin, command);
        assert!(
            !run.panicked(),
            "oa {} panicked while reporting a refusal:\n{}",
            command.join(" "),
            run.stderr
        );
        assert_ne!(
            run.code(),
            101,
            "oa {} aborted rather than reported: {}",
            command.join(" "),
            run.stderr
        );
        assert_ne!(
            run.code(),
            0,
            "oa {} reported success on a 502",
            command.join(" ")
        );
        assert!(
            run.stdout.trim().is_empty(),
            "oa {} wrote data to stdout on a 502: {}",
            command.join(" "),
            run.stdout
        );
    }
}

/// A refused list is an error carrying the status, never an empty list.
///
/// The predecessor of this file asserted `listed.is_err()`. That passes with
/// no network at all. This names 404, so a transport failure fails the test.
#[tokio::test]
async fn a_refused_list_carries_the_status_and_yields_no_rows() {
    let server = StubServer::always(
        404,
        "application/json",
        br#"{"message":"Not Found"}"#.to_vec(),
    );
    let tracker = client(&server);

    let listed = tracker
        .list_issues(&target(), &one_page())
        .await
        .expect_err("a 404 must not produce rows");
    assert_eq!(refused_status(&listed), 404);
    assert!(
        listed.to_string().contains("Not Found"),
        "the server's own message is what gets reported: {listed}"
    );

    let projects = tracker
        .list_projects(&target(), false)
        .await
        .expect_err("a 404 must not produce boards");
    assert_eq!(refused_status(&projects), 404);
}

/// The status is carried through distinctly, not flattened to "it failed".
///
/// `project list` returned `Ok(Vec::new())` on every non-2xx once, so a
/// permission problem and a missing repository and a working empty board were
/// the same output. Each status has to survive to the caller.
#[tokio::test]
async fn each_refusal_status_survives_to_the_caller() {
    for status in [400u16, 401, 403, 404, 409, 422, 500, 502] {
        let server = StubServer::always(
            status,
            "application/json",
            format!(r#"{{"message":"refused with {status}"}}"#).into_bytes(),
        );
        let error = client(&server)
            .list_issues(&target(), &one_page())
            .await
            .expect_err("a non-2xx must not produce rows");
        assert_eq!(
            refused_status(&error),
            status,
            "the client reported a different status than the server sent"
        );
        assert!(
            error
                .to_string()
                .contains(&format!("refused with {status}")),
            "the server's message was dropped: {error}"
        );
    }
}

/// The forum client refuses rather than substituting a board list.
///
/// `forum.rs` answered any non-2xx with a hardcoded `general`/`dev` pair. The
/// `dev` board does not exist on this forum, so the CLI printed two boards
/// that were never served to it. The assertion that caught nothing was
/// `assert!(!boards.is_empty())`; this asserts the refusal instead.
#[tokio::test]
async fn a_refused_board_list_is_an_error_not_a_substitute_list() {
    let server = StubServer::always(406, "application/json", br#"{"message":"nope"}"#.to_vec());
    let error = openagents_cli::forum::ForumClient::new(&server.api_base(), None)
        .list_boards()
        .await
        .expect_err("a 406 must not produce boards");
    match &error {
        ForumError::Refused { status, .. } => assert_eq!(*status, 406),
        other => panic!("expected a refusal carrying the status, got {other}"),
    }
    let rendered = error.to_string();
    assert!(
        !rendered.contains("dev") && !rendered.contains("General"),
        "a refusal must not name boards: {rendered}"
    );
}

// ---------------------------------------------------------------------------
// 2. The route asked for is the route the parity depends on
// ---------------------------------------------------------------------------

/// `project list` asks for `projectsV2`, which is the route that exists.
///
/// It asked for `/projects` and turned the 404 into `Ok(Vec::new())`, so it
/// printed nothing and exited 0 against a repository with four boards. Reading
/// the request the stub received is the only assertion that separates "asked
/// correctly" from "asked wrongly and hid the answer".
///
/// The TypeScript client builds the same path at
/// `packages/openagents-cli/src/project-client.ts:60`.
#[test]
fn project_list_asks_for_projects_v2() {
    let server = StubServer::always(200, "application/json", br#"{"projects":[]}"#.to_vec());
    let run = oa(
        &server.origin(),
        &["project", "list", "-R", "OpenAgentsInc/openagents"],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);

    let routes: Vec<String> = server.hits().iter().map(Hit::route).collect();
    assert_eq!(
        routes,
        vec!["GET /api/v1/repos/OpenAgentsInc/openagents/projectsV2"],
        "project list asked for the wrong route"
    );
}

/// `forum boards` asks for `/api/v1/forum`.
///
/// It asked for `/api/v1/forum/boards`, which answers 406, and the fabricated
/// fallback hid that. The TypeScript client uses `/forum`
/// (`packages/openagents-cli/src/forum-client.ts:151`).
#[test]
fn forum_boards_asks_for_the_forum_route() {
    let server = StubServer::always(200, "application/json", br#"{"boards":[]}"#.to_vec());
    let run = oa(&server.origin(), &["forum", "boards"]);
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);

    let routes: Vec<String> = server.hits().iter().map(Hit::route).collect();
    assert_eq!(routes, vec!["GET /api/v1/forum"]);
}

// ---------------------------------------------------------------------------
// 3. Fields, not shapes
// ---------------------------------------------------------------------------

/// The rendered row carries the fields the route returns.
///
/// Asserting that a listing "returns rows" passed against the two fabricated
/// trace sessions and the two fabricated forum boards. These assert the values
/// the server sent, so a client that renders its own defaults fails.
#[test]
fn a_listing_renders_the_fields_the_server_sent() {
    let body = br#"{"issues":[
        {"number":4242,"title":"a title only this server knows","state":"open",
         "user":{"login":"AtlantisPleb","id":14167547},
         "openagents":{"blocked":true,"progress":"in_progress"}}
    ],"total_count":1}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(
        &server.origin(),
        &["issue", "list", "-R", "OpenAgentsInc/openagents"],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    assert!(
        run.stdout.contains("#4242"),
        "the number the server sent is missing: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains("a title only this server knows"),
        "the title the server sent is missing: {}",
        run.stdout
    );
    // `blocked` is rendered as a suffix by both CLIs. Recorded from the
    // TypeScript CLI at cd0c05d465:
    //   #105   open    Consolidate the coder TUI …  [blocked]
    assert!(
        run.stdout.contains("[blocked]"),
        "the blocked flag the server sent is missing: {}",
        run.stdout
    );
}

/// `--json` prints the server's body, not a re-rendering of it.
///
/// The tracker's contract is that `--json` prints exactly what the server
/// sent. A client that decoded into its own struct and re-encoded would drop
/// every field it did not model, which is how `forum search --json` lost
/// `board`, `url`, `pinned`, `tip_count`, and `tip_sats`.
#[test]
fn json_output_preserves_fields_the_client_does_not_model() {
    let body = br#"{"issues":[{"number":1,"title":"t","state":"open",
        "a_field_no_client_models":"survives"}],"total_count":1}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(
        &server.origin(),
        &["--json", "issue", "list", "-R", "OpenAgentsInc/openagents"],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    let parsed: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("--json must print one JSON document");
    assert_eq!(
        parsed["issues"][0]["a_field_no_client_models"], "survives",
        "an unmodelled field was dropped: {}",
        run.stdout
    );
}

/// `--json` changes the output. A flag that is parsed and ignored does not.
///
/// `--json` was declared globally and read nowhere, so every command accepted
/// it and printed the same human text. Comparing the two runs is what catches
/// that; asserting the JSON run "produces output" does not.
///
/// Each command gets a body of its own shape. A shared body would leave the
/// commands that cannot read it printing nothing either way, and two empty
/// strings compare equal — the test would pass for the wrong reason on the
/// commands it was least able to check.
#[test]
fn the_json_flag_is_read_and_not_merely_accepted() {
    let cases: &[(&[&str], &[u8])] = &[
        (
            &["issue", "list", "-R", "OpenAgentsInc/openagents"],
            br#"{"issues":[{"number":7,"title":"an issue","state":"open"}],"total_count":1}"#,
        ),
        (
            &["project", "list", "-R", "OpenAgentsInc/openagents"],
            br#"{"projects":[{"number":3,"title":"a board","state":"open"}]}"#,
        ),
        (
            &["memory", "list"],
            br#"{"memories":[{"id":"m1","bucket":"user","body":"a memory",
                "created_at":"2026-08-26T00:00:00Z","source_ref":null,
                "superseded_by":null}]}"#,
        ),
        (
            &["forum", "boards"],
            br#"{"boards":[{"slug":"general","name":"General","topic_count":0}]}"#,
        ),
    ];

    for (command, body) in cases {
        let plain_server = StubServer::always(200, "application/json", body.to_vec());
        let plain = oa(&plain_server.origin(), command);
        assert_eq!(
            plain.code(),
            0,
            "oa {} failed against its own fixture: {}",
            command.join(" "),
            plain.stderr
        );
        assert!(
            !plain.stdout.trim().is_empty(),
            "oa {} printed nothing, so the comparison below would be two \
             empty strings and prove nothing",
            command.join(" ")
        );

        let json_server = StubServer::always(200, "application/json", body.to_vec());
        let mut with_flag = vec!["--json"];
        with_flag.extend(command.iter().copied());
        let json = oa(&json_server.origin(), &with_flag);

        assert_ne!(
            plain.stdout,
            json.stdout,
            "oa {} produced identical output with and without --json, \
             so the flag is accepted and ignored",
            command.join(" ")
        );
        serde_json::from_str::<serde_json::Value>(&json.stdout).unwrap_or_else(|error| {
            panic!(
                "oa --json {} did not print JSON ({error}): {}",
                command.join(" "),
                json.stdout
            )
        });
    }
}

/// `trace list` accepts `--json` and prints human text anyway.
///
/// Recorded from `openagents trace list --json` at cd0c05d465:
///
/// ```text
/// {"schema":"openagents.trace_list.v1","stores":[{"root":"…","kind":
///  "openagents_export","present":true,"matched":68,…}]}
/// ```
///
/// `oa trace list --json` prints the same table it prints without the flag.
/// `run_trace` takes a `json` parameter and reads it in one of its four arms.
/// The same holds for `trace show`, `trace redact`, `plugin search`,
/// `plugin inspect`, `plugin run`, `api`, `coder`, `delegate`, and `update`.
#[test]
#[ignore = "#88: oa trace list ignores --json and prints the human table. \
            Run with --ignored to see it; delete the attribute when the flag \
            is read."]
fn trace_list_honours_the_json_flag() {
    let server = StubServer::always(200, "application/json", b"{}".to_vec());
    let run = oa(&server.origin(), &["--json", "trace", "list"]);
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    serde_json::from_str::<serde_json::Value>(&run.stdout).unwrap_or_else(|error| {
        panic!(
            "trace list --json did not print JSON ({error}): {}",
            run.stdout
        )
    });
}

// ---------------------------------------------------------------------------
// 4. The exit-code contract
// ---------------------------------------------------------------------------

/// A refusal never exits 0, and never aborts.
///
/// This is the property both CLIs already hold and the one a caller most needs.
/// The *value* they exit with is where they part company, which the next test
/// pins.
#[test]
fn no_refusal_exits_zero() {
    for status in [400u16, 401, 403, 404, 409, 422, 500] {
        let server = StubServer::always(
            status,
            "application/json",
            format!(r#"{{"message":"refused {status}"}}"#).into_bytes(),
        );
        let run = oa(
            &server.origin(),
            &["issue", "list", "-R", "OpenAgentsInc/openagents"],
        );
        assert_ne!(run.code(), 0, "HTTP {status} exited 0");
        assert_ne!(run.code(), 101, "HTTP {status} aborted: {}", run.stderr);
        assert!(
            run.stdout.trim().is_empty(),
            "HTTP {status} wrote data to stdout: {}",
            run.stdout
        );
    }
}

/// `oa` collapses every server refusal to exit 2. The TypeScript CLI does not.
///
/// This test records the divergence rather than blessing it. The TypeScript
/// ladder is deliberate and published in
/// `packages/openagents-cli/src/errors.ts:238-305`:
///
/// | condition          | openagents | oa |
/// | ------------------ | ---------: | -: |
/// | 400, 422           |          2 |  2 |
/// | 401, 403           |          3 |  2 |
/// | 404                |          4 |  2 |
/// | 409                |          5 |  2 |
/// | 5xx                |          6 |  2 |
///
/// Measured against production at cd0c05d465: `openagents deploy list` exits
/// 3 and `oa deploy list` exits 2; `openagents issue view 999999` exits 4 and
/// `oa` exits 2.
///
/// A caller cannot tell an expired token from a typo from a missing repository
/// from a server outage. When the ladder lands in `oa`, this test is the file
/// that has to change, which is the point: the collapse becomes a deliberate
/// edit rather than a silent default.
#[test]
fn every_server_refusal_currently_exits_two() {
    for status in [401u16, 403, 404, 409, 500, 502] {
        let server = StubServer::always(
            status,
            "application/json",
            format!(r#"{{"message":"refused {status}"}}"#).into_bytes(),
        );
        let run = oa(
            &server.origin(),
            &["issue", "list", "-R", "OpenAgentsInc/openagents"],
        );
        assert_eq!(
            run.code(),
            2,
            "HTTP {status} exited {}. If the #88 exit ladder has landed, \
             update this test to the new expectation rather than deleting it.",
            run.code()
        );
    }
}

/// A refusal writes to stderr and leaves stdout clean.
///
/// A `--json` consumer piping stdout must not receive prose. `oa` writes
/// `oa: …` to stderr on every failure, which is right; what it does not yet do
/// is write a JSON error object to stdout under `--json`, the way the
/// TypeScript CLI does. Recorded from `openagents box list --json` at
/// cd0c05d465:
///
/// ```text
/// {"code":"api_error","message":"This deployment does not report a
///  conversation for the account. …","exit_code":3}
/// ```
///
/// Until that lands, the contract this pins is the weaker one: stdout stays
/// empty, so a consumer sees a parse failure on empty input rather than prose
/// masquerading as data.
#[test]
fn a_refusal_keeps_prose_off_stdout() {
    let server = StubServer::always(
        403,
        "application/json",
        br#"{"message":"forbidden"}"#.to_vec(),
    );
    let run = oa(
        &server.origin(),
        &["--json", "issue", "list", "-R", "OpenAgentsInc/openagents"],
    );
    assert_ne!(run.code(), 0);
    assert!(
        run.stdout.trim().is_empty(),
        "prose reached stdout under --json: {}",
        run.stdout
    );
    assert!(
        run.stderr.contains("forbidden"),
        "the server's message never reached the user: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// 5. Recorded parity with the TypeScript CLI
// ---------------------------------------------------------------------------

/// The two CLIs render a repository listing the same way.
///
/// Recorded from `openagents repo list` against production at cd0c05d465:
///
/// ```text
/// moneya/wardrobe
/// OpenAgentsInc/openagents
/// ```
///
/// `oa` appends `\t(branch: main)` to every row. That is a divergence in a
/// command whose output is routinely piped, and this test is what makes it
/// visible: it asserts the recorded TypeScript shape, so it fails until the
/// two agree.
#[test]
#[ignore = "#88: oa appends a branch column the TypeScript CLI does not. \
            Run with --ignored to see the divergence; delete the attribute \
            when the two renderings agree."]
fn repo_list_renders_the_same_row_as_the_typescript_cli() {
    let body = br#"{"repositories":[
        {"id":"1","name":"wardrobe","full_name":"moneya/wardrobe",
         "owner":{"id":1,"login":"moneya","type":"User"},
         "private":false,"visibility":"public","default_branch":"main",
         "lifecycle_state":"ready",
         "clone_url":"https://openagents.com/moneya/wardrobe.git",
         "html_url":"https://openagents.com/moneya/wardrobe",
         "permissions":{"admin":false,"push":false,"pull":true}}
    ]}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(&server.origin(), &["repo", "list"]);
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    assert_eq!(run.stdout, "moneya/wardrobe\n");
}

/// A refused `box list` reports the server's sentence, as the TypeScript does.
///
/// Recorded from `openagents box list` against production at cd0c05d465:
///
/// ```text
/// openagents: This deployment does not report a conversation for the
/// account. Pass --conversation <conversation_id> to name the conversation
/// to use.
/// ```
///
/// `oa` prefixes the route and status — `oa: The API refused the request to
/// resolve user conversation (HTTP 401): …` — which is more informative and
/// not a parity break. What both must do, and what this asserts, is carry the
/// server's own sentence rather than an empty list and exit 0. `box_client.rs`
/// returned `Ok(Vec::new())` here once.
#[test]
fn a_refused_box_list_reports_the_servers_sentence() {
    const SENTENCE: &str = "This deployment does not report a conversation for the account.";
    let server = StubServer::always(
        401,
        "application/json",
        format!(r#"{{"message":"{SENTENCE}"}}"#).into_bytes(),
    );
    let run = oa(&server.origin(), &["box", "list"]);
    assert_ne!(run.code(), 0, "a refused box list exited 0");
    assert!(
        run.stdout.trim().is_empty(),
        "a refused box list printed rows: {}",
        run.stdout
    );
    assert!(
        run.stderr.contains(SENTENCE),
        "the server's sentence never reached the user: {}",
        run.stderr
    );
}

/// A global flag after a positional argument is a flag, not data.
///
/// `oa memory add "text" --json` stored the memory as `text --json`, because
/// the positional is `trailing_var_arg`. It is the same defect as issue #109
/// (`box exec` swallowing a trailing `--conversation`), and it silently
/// corrupts what gets written. The TypeScript CLI reads the flag from either
/// position.
///
/// Measured against production at cd0c05d465: `oa memory add "parity audit
/// scratch RS (delete me)" --json` created a memory whose body ended
/// `(delete me) --json`.
#[test]
#[ignore = "#88: oa memory add absorbs a trailing --json into the memory body. \
            Run with --ignored to see it; delete the attribute when fixed."]
fn a_trailing_global_flag_is_not_stored_as_data() {
    let server = StubServer::always(
        201,
        "application/json",
        br#"{"memory":{"id":"m1","bucket":"user","body":"remember this"}}"#.to_vec(),
    );
    let run = oa(
        &server.origin(),
        &["memory", "add", "remember this", "--json"],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    serde_json::from_str::<serde_json::Value>(&run.stdout).unwrap_or_else(|error| {
        panic!(
            "the trailing --json was swallowed rather than read ({error}): {}",
            run.stdout
        )
    });
}

// ---------------------------------------------------------------------------
// 6. The forum read half: one topic, later pages, and the fields search drops
// ---------------------------------------------------------------------------

/// `forum topic` asks for the route the TypeScript client asks for.
///
/// The path is built at `packages/openagents-cli/src/forum-client.ts:176`:
/// `${API_VERSION_PATH}/forum/topics/${encodeURIComponent(id)}`. Reading the
/// request the stub received is what separates "parsed the subcommand" from
/// "asked the server for the topic".
#[test]
fn forum_topic_asks_for_the_topic_route() {
    let body = br#"{"topic":{"id":"9946bf38-788b","title":"Why not?","state":"open",
        "slug":"why-not","posts_count":1},
        "posts":[{"id":"p1","post_number":1,"state":"visible",
                  "author":{"display_name":"Sneaky","ref":"agent:user_b3ce"},
                  "body_text":"A quiet observation."}],
        "pagination":{"total":1,"page":1,"per_page":50,"total_pages":1}}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(&server.origin(), &["forum", "topic", "9946bf38-788b"]);
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);

    let routes: Vec<String> = server.hits().iter().map(Hit::route).collect();
    assert_eq!(routes, vec!["GET /api/v1/forum/topics/9946bf38-788b"]);

    // The title, then one line per post. Recorded from `openagents forum topic
    // 9946bf38-788b-45f3-b17b-b0e36bb8dc60` at 0.4.0:
    //   Why are you not running agents around the clock?
    //   #1 Sneaky: A quiet observation for the agents arriving today: …
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines[0], "Why not?");
    assert_eq!(lines[1], "#1 Sneaky: A quiet observation.");
}

/// A topic the server refuses is a refusal, not an empty topic.
#[test]
fn a_refused_topic_read_exits_non_zero_with_the_server_status() {
    let server = StubServer::always(
        404,
        "application/json",
        br#"{"error":"not_found"}"#.to_vec(),
    );
    let run = oa(
        &server.origin(),
        &["forum", "topic", "deadbeef-0000-0000-0000-000000000000"],
    );
    assert_ne!(run.code(), 0, "a 404 must not read as a topic");
    assert!(
        run.stderr.contains("404"),
        "the status the server sent is missing: {}",
        run.stderr
    );
    assert!(
        run.stdout.trim().is_empty(),
        "a refused read printed topic-shaped output: {}",
        run.stdout
    );
}

/// `--page 2` is sent, and the page it returns is not the page 1 returned.
///
/// The regression this guards is silence, not absence: `forum topics` returned
/// the server's first page — 25 rows of a 107-topic board — with nothing saying
/// the other four pages existed. Asserting that rows came back passes against
/// exactly that bug, so this asserts three things it cannot satisfy: the page
/// number reaches the query string, page 2 differs from page 1, and both runs
/// report the server's own total.
#[test]
fn forum_topics_pages_and_says_how_much_it_is_not_showing() {
    let page_one = br#"{"topics":[{"id":"415e16a7-183c","title":"first page topic",
        "state":"open","slug":"a","posts_count":132}],
        "pagination":{"total":107,"page":1,"per_page":25,"total_pages":5}}"#;
    let page_two = br#"{"topics":[{"id":"9e7b4f18-0000","title":"second page topic",
        "state":"open","slug":"b","posts_count":12}],
        "pagination":{"total":107,"page":2,"per_page":25,"total_pages":5}}"#;
    let server = StubServer::start(vec![
        (200, "application/json", page_one.to_vec()),
        (200, "application/json", page_two.to_vec()),
    ]);

    let first = oa(
        &server.origin(),
        &["forum", "topics", "--board", "product-promises"],
    );
    assert_eq!(first.code(), 0, "stderr: {}", first.stderr);
    let second = oa(
        &server.origin(),
        &[
            "forum",
            "topics",
            "--board",
            "product-promises",
            "--page",
            "2",
        ],
    );
    assert_eq!(second.code(), 0, "stderr: {}", second.stderr);

    let routes: Vec<String> = server.hits().iter().map(Hit::route).collect();
    assert_eq!(
        routes,
        vec![
            "GET /api/v1/forum/topics?forum=product-promises",
            "GET /api/v1/forum/topics?forum=product-promises&page=2",
        ],
        "the page number never reached the server"
    );

    assert!(
        first.stdout.contains("first page topic"),
        "page 1 rows: {}",
        first.stdout
    );
    assert!(
        second.stdout.contains("second page topic"),
        "page 2 rows: {}",
        second.stdout
    );
    assert_ne!(
        first.stdout, second.stdout,
        "page 2 returned page 1; the flag was parsed and dropped"
    );
    assert!(
        !second.stdout.contains("first page topic"),
        "page 2 still carries page 1's rows: {}",
        second.stdout
    );

    // The total, and the next page to ask for, are the server's own numbers.
    assert!(
        first.stdout.contains("Page 1 of 5 — 107 topics"),
        "page 1 did not say how much of the board it was not showing: {}",
        first.stdout
    );
    assert!(
        first.stdout.contains("--page 2"),
        "page 1 named no next page: {}",
        first.stdout
    );
    assert!(
        second.stdout.contains("Page 2 of 5 — 107 topics"),
        "page 2 did not report its place: {}",
        second.stdout
    );
}

/// `forum topics --json` carries the server's pagination block.
#[test]
fn forum_topics_json_carries_the_servers_pagination() {
    let body = br#"{"topics":[{"id":"415e16a7-183c","title":"t","state":"open",
        "slug":"a","posts_count":2}],
        "pagination":{"total":107,"page":1,"per_page":25,"total_pages":5}}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(
        &server.origin(),
        &["--json", "forum", "topics", "--board", "product-promises"],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    let parsed: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("--json must print one JSON document");
    assert_eq!(parsed["pagination"]["total"], 107);
    assert_eq!(parsed["pagination"]["total_pages"], 5);
    assert_eq!(parsed["pagination"]["page"], 1);
    assert_eq!(parsed["pagination"]["per_page"], 25);
}

/// `forum search` keeps the six fields it dropped, in both renderings.
///
/// The client decoded into a struct that modelled eight fields and re-encoded
/// it, so `board`, `url`, `pinned`, `tip_count`, `tip_sats`, and `actor_ref`
/// never reached `--json`, and the `[board]` suffix never reached the human
/// line. Every value asserted below is one this fixture sent.
#[test]
fn forum_search_keeps_the_fields_the_server_sent() {
    let body = br#"{"query":"acceptance gate","topics":[
        {"id":"9946bf38-788b","slug":"why-not","title":"Why not?","state":"open",
         "author":{"ref":"agent:user_b3ce","display_name":"Sneaky","is_agent":true},
         "url":"https://openagents.com/forum/t/9946bf38","actor_ref":"agent:user_b3ce",
         "pinned":true,"tip_count":3,"tip_sats":210,"posts_count":14,
         "board":{"title":"Work Requests","slug":"work-requests"}}],
        "pagination":{"total":1,"page":1,"per_page":25,"total_pages":1},"board":null}"#;

    let human_server = StubServer::always(200, "application/json", body.to_vec());
    let human = oa(
        &human_server.origin(),
        &["forum", "search", "acceptance gate"],
    );
    assert_eq!(human.code(), 0, "stderr: {}", human.stderr);
    // Recorded from `openagents forum search "acceptance gate"` at 0.4.0:
    //   9946bf38 — Why are you not running agents … — Sneaky [work-requests]
    assert_eq!(
        human.stdout.trim(),
        "9946bf38 — Why not? — Sneaky [work-requests]"
    );

    let routes: Vec<String> = human_server.hits().iter().map(Hit::route).collect();
    assert_eq!(routes, vec!["GET /api/v1/forum/topics?q=acceptance%20gate"]);

    let json_server = StubServer::always(200, "application/json", body.to_vec());
    let json = oa(
        &json_server.origin(),
        &["--json", "forum", "search", "acceptance gate"],
    );
    assert_eq!(json.code(), 0, "stderr: {}", json.stderr);
    let parsed: serde_json::Value =
        serde_json::from_str(&json.stdout).expect("--json must print one JSON document");
    let row = &parsed["topics"][0];
    assert_eq!(row["board"]["slug"], "work-requests");
    assert_eq!(row["url"], "https://openagents.com/forum/t/9946bf38");
    assert_eq!(row["pinned"], true);
    assert_eq!(row["tip_count"], 3);
    assert_eq!(row["tip_sats"], 210);
    assert_eq!(row["actor_ref"], "agent:user_b3ce");
}

/// `forum search --board` narrows the query the way the TypeScript client does.
///
/// `packages/openagents-cli/src/forum-client.ts:167` appends `&forum=<slug>`
/// after `q`, in that order.
#[test]
fn forum_search_can_narrow_to_one_board() {
    let server = StubServer::always(200, "application/json", br#"{"topics":[]}"#.to_vec());
    let run = oa(
        &server.origin(),
        &[
            "forum",
            "search",
            "gate",
            "--board",
            "work-requests",
            "--page",
            "3",
        ],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    let routes: Vec<String> = server.hits().iter().map(Hit::route).collect();
    assert_eq!(
        routes,
        vec!["GET /api/v1/forum/topics?q=gate&forum=work-requests&page=3"]
    );
}

/// An empty page of a board that has topics is not reported as an empty board.
///
/// The live route answers `?page=9` on a five-page board with `topics: []` and
/// the same pagination block. Printing "No topics found." there claims the
/// server said the board is empty, which it did not.
#[test]
fn an_empty_page_does_not_claim_the_board_is_empty() {
    let body = br#"{"topics":[],
        "pagination":{"total":107,"page":9,"per_page":25,"total_pages":5}}"#;
    let server = StubServer::always(200, "application/json", body.to_vec());
    let run = oa(
        &server.origin(),
        &[
            "forum",
            "topics",
            "--board",
            "product-promises",
            "--page",
            "9",
        ],
    );
    assert_eq!(run.code(), 0, "stderr: {}", run.stderr);
    assert!(
        run.stdout.contains("107") && run.stdout.contains("page 9"),
        "the server's own numbers are missing: {}",
        run.stdout
    );
    assert!(
        !run.stdout.contains("No topics found."),
        "an empty page claimed the board is empty: {}",
        run.stdout
    );
}

/// A body with no `topics` array is a malformed answer, not an empty board.
#[test]
fn a_two_hundred_without_topics_is_refused_rather_than_rendered_empty() {
    let server = StubServer::always(200, "application/json", br#"{"ok":true}"#.to_vec());
    let run = oa(&server.origin(), &["forum", "topics", "--board", "general"]);
    assert_ne!(
        run.code(),
        0,
        "a body with no `topics` array printed a board listing: {}",
        run.stdout
    );
    assert!(
        !run.stdout.contains("No topics found."),
        "an unreadable body was rendered as an empty board: {}",
        run.stdout
    );
}
