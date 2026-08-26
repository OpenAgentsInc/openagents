//! The Computer subsystem (issue 79) and the API passthrough (issue 81).
//!
//! Two shapes of assertion are deliberately absent here. Nothing asserts
//! `x.is_empty() || !x.is_empty()`, and nothing asserts that a passthrough
//! response "is an object" — an error envelope is an object too, so that
//! assertion passed while every refused request returned a `{"status": N}`
//! stub. Each test below names the field the route actually returns, or the
//! refusal a closed policy actually produces.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use openagents_cli::api_passthrough::{
    admitted_method, api_error_details, decode_request_body, parse_request_fields,
    parse_request_headers, resolve_api_path, resolve_request_method, ApiPassthroughClient,
};
use openagents_cli::computer::{
    curated_allowlist, decide, execute_command, format_allowlist, gh_read_only_allowed,
    load_config, probe, probe_host, redact, serve, tier_allows, within_root, Cancellation,
    CommandRequest, ComputerClient, ComputerPaths, Decision, ExecutionLimits, Journal,
    MachineCredentials, PolicyConfig, RefusalReason, Tier,
};

// ---------------------------------------------------------------------------
// policy
// ---------------------------------------------------------------------------

fn config_at(directory: &std::path::Path, tier: Tier, roots: Vec<PathBuf>) -> PolicyConfig {
    PolicyConfig {
        tier,
        roots,
        ..PolicyConfig::closed(ComputerPaths::in_directory(directory))
    }
}

fn refusal(decision: &Decision) -> RefusalReason {
    match decision {
        Decision::Refused { reason, .. } => *reason,
        Decision::Allowed { .. } => panic!("expected a refusal, the command was allowed"),
    }
}

fn request(argv: &[&str], cwd: &std::path::Path) -> CommandRequest {
    CommandRequest {
        argv: argv.iter().map(|value| value.to_string()).collect(),
        cwd: cwd.display().to_string(),
    }
}

/// The policy this replaces was three unconditional `true`s, so it permitted
/// everything it was ever asked. The default now reaches nothing: no root is
/// declared, so no working directory is reachable, whatever the command is.
#[test]
fn test_default_policy_reaches_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let config = PolicyConfig::closed(ComputerPaths::in_directory(directory.path()));

    assert_eq!(config.tier, Tier::Probe);
    assert!(config.roots.is_empty());
    assert_eq!(
        refusal(&decide(
            &request(&["git", "status"], directory.path()),
            &config
        )),
        RefusalReason::RootNotDeclared
    );
    assert_eq!(
        refusal(&decide(&request(&["ls"], directory.path()), &config)),
        RefusalReason::RootNotDeclared
    );
}

/// The allowlist the policy command prints. The first and last lines, the
/// count, and `git`'s options are the contract the TypeScript CLI publishes.
#[test]
fn test_curated_allowlist_is_the_published_one() {
    let lines = format_allowlist();
    assert_eq!(lines.len(), curated_allowlist().len() + 1);
    assert_eq!(
        lines[0],
        "git: status, log, diff, branch, remote, show, rev-parse, ls-files, --version"
    );
    assert_eq!(
        lines[1],
        "uname: no options; path arguments inside declared roots"
    );
    assert_eq!(lines[lines.len() - 1], "gh: read-only queries only");
    assert!(lines.iter().any(|line| line == "npm: --version, ls"));
    assert!(lines
        .iter()
        .any(|line| line == "docker: ps, images, version"));
}

/// A declared root is not enough on its own. The probe tier is fixed discovery,
/// so even `ls` inside the root is refused until the owner raises the ceiling.
#[test]
fn test_probe_tier_refuses_a_curated_command_inside_a_declared_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let config = config_at(directory.path(), Tier::Probe, vec![root.clone()]);
    assert_eq!(
        refusal(&decide(&request(&["ls"], &root), &config)),
        RefusalReason::TierInsufficient
    );
    assert!(tier_allows(Tier::Curated, Tier::Probe));
    assert!(!tier_allows(Tier::Probe, Tier::Curated));
    assert!(tier_allows(Tier::Shell, Tier::Curated));
}

/// The curated tier is where the allowlist decides. Everything here is a
/// refusal the flat-`true` policy could not have produced.
#[test]
fn test_curated_tier_allows_the_allowlist_and_refuses_the_rest() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let config = config_at(directory.path(), Tier::Curated, vec![root.clone()]);

    assert!(decide(&request(&["git", "status"], &root), &config).allowed());
    assert!(decide(&request(&["git", "--version"], &root), &config).allowed());
    assert!(decide(&request(&["node", "--version"], &root), &config).allowed());

    // Not on the list at all.
    assert_eq!(
        refusal(&decide(
            &request(&["curl", "https://example.com"], &root),
            &config
        )),
        RefusalReason::NotAllowlisted
    );
    // On the list, but not with this subcommand: `git push` writes.
    assert_eq!(
        refusal(&decide(&request(&["git", "push"], &root), &config)),
        RefusalReason::NotAllowlisted
    );
    // On the list, but not with this option.
    assert_eq!(
        refusal(&decide(&request(&["node", "-e", "1"], &root), &config)),
        RefusalReason::NotAllowlisted
    );
    // Denied outright, and denied before the tier is consulted.
    assert_eq!(
        refusal(&decide(&request(&["sudo", "ls"], &root), &config)),
        RefusalReason::DeniedCommand
    );
    // A protected path, named anywhere in the argument vector.
    assert_eq!(
        refusal(&decide(&request(&["cat", "~/.ssh/id_rsa"], &root), &config)),
        RefusalReason::DeniedArgument
    );
    // Shell metacharacters never reach a shell, because there is no shell.
    assert_eq!(
        refusal(&decide(&request(&["ls", "; rm -rf /"], &root), &config)),
        RefusalReason::ShellMetacharacter
    );
    // A path argument that climbs out of every declared root.
    assert_eq!(
        refusal(&decide(
            &request(&["cat", "../../etc/hosts"], &root),
            &config
        )),
        RefusalReason::DeniedArgument
    );
    // A working directory outside every declared root.
    assert_eq!(
        refusal(&decide(
            &CommandRequest {
                argv: vec!["git".to_string(), "status".to_string()],
                cwd: "/tmp".to_string(),
            },
            &config
        )),
        RefusalReason::RootNotDeclared
    );
}

/// A denied binary stays denied at the top tier. Raising the ceiling widens
/// what the allowlist admits; it never unlocks `sudo`.
#[test]
fn test_shell_tier_still_refuses_denied_commands_and_asks_before_the_rest() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let mut config = config_at(directory.path(), Tier::Shell, vec![root.clone()]);

    assert_eq!(
        refusal(&decide(&request(&["sudo", "ls"], &root), &config)),
        RefusalReason::DeniedCommand
    );
    assert_eq!(
        decide(&request(&["make", "build"], &root), &config),
        Decision::Allowed {
            needs_confirmation: true
        }
    );
    config.pre_approved = vec!["make".to_string()];
    assert_eq!(
        decide(&request(&["make", "build"], &root), &config),
        Decision::Allowed {
            needs_confirmation: false
        }
    );
}

#[test]
fn test_gh_is_read_only() {
    let read = |args: &[&str]| {
        gh_read_only_allowed(&args.iter().map(|v| v.to_string()).collect::<Vec<_>>())
    };
    assert!(read(&["pr", "list"]));
    assert!(read(&["issue", "view"]));
    assert!(read(&["status"]));
    assert!(!read(&["pr", "merge"]));
    assert!(!read(&["api", "/user"]));
    assert!(!read(&["issue", "list", "--field", "x"]));
    assert!(!read(&[]));
}

#[test]
fn test_root_containment_is_lexical_and_does_not_admit_a_sibling() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("work");
    std::fs::create_dir_all(root.join("inner")).unwrap();
    assert!(within_root(&root, &root));
    assert!(within_root(&root.join("inner"), &root));
    assert!(!within_root(&directory.path().join("work-other"), &root));
    assert!(!within_root(&directory.path().join("elsewhere"), &root));
}

/// A configuration file that cannot be decoded is refused. Falling back to the
/// default would silently replace the owner's policy with a different one.
#[test]
fn test_unreadable_configuration_is_refused_rather_than_defaulted() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ComputerPaths::in_directory(directory.path());

    // Missing is a real answer: nothing is declared.
    let closed = load_config(&paths).expect("a missing file is the closed default");
    assert_eq!(closed.tier, Tier::Probe);
    assert!(closed.roots.is_empty());

    std::fs::write(&paths.config, "{ not json").unwrap();
    let refused = load_config(&paths);
    assert!(
        refused.is_err(),
        "invalid JSON must not read as the default"
    );

    std::fs::write(&paths.config, r#"{"tier":"root"}"#).unwrap();
    let refused = load_config(&paths).unwrap_err();
    assert!(
        refused.contains("unknown tier"),
        "the refusal must name the problem: {refused}"
    );

    std::fs::write(
        &paths.config,
        r#"{"tier":"curated","roots":["/tmp/declared"],"pre_approved":["make"]}"#,
    )
    .unwrap();
    let read = load_config(&paths).unwrap();
    assert_eq!(read.tier, Tier::Curated);
    assert_eq!(read.roots, vec![PathBuf::from("/tmp/declared")]);
    assert_eq!(read.pre_approved, vec!["make".to_string()]);
}

// ---------------------------------------------------------------------------
// journal
// ---------------------------------------------------------------------------

#[test]
fn test_journal_records_a_refusal_and_redacts_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let journal = Journal::at(directory.path().join("journal.ndjson"));

    assert!(
        journal.read(20).unwrap().is_empty(),
        "a machine that has been asked nothing has an empty journal"
    );

    let refused = CommandRequest {
        argv: vec!["curl".to_string(), "https://example.com".to_string()],
        cwd: "/declared/root".to_string(),
    };
    journal
        .append(
            "req-1",
            &refused,
            "not_allowlisted",
            "refused",
            "curl is not in the curated allowlist",
        )
        .unwrap();
    journal
        .append(
            "req-2",
            &CommandRequest {
                argv: vec!["echo".to_string(), "oa_pat_ABCDEF123456".to_string()],
                cwd: "/declared/root".to_string(),
            },
            "allowed",
            "completed",
            "Authorization: Bearer oa_pat_ABCDEF123456",
        )
        .unwrap();

    let entries = journal.read(20).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].request_id, "req-1");
    assert_eq!(entries[0].decision, "not_allowlisted");
    assert_eq!(entries[0].outcome, "refused");
    assert_eq!(entries[0].argv, vec!["curl", "https://example.com"]);
    assert_eq!(entries[0].detail, "curl is not in the curated allowlist");

    let redacted = &entries[1];
    assert!(
        !redacted.detail.contains("oa_pat_ABCDEF123456"),
        "the token survived the journal: {}",
        redacted.detail
    );
    assert!(!redacted.argv[1].contains("ABCDEF123456"));
    assert!(redacted.detail.contains("[REDACTED]"));

    // Only the tail is returned, newest last.
    let tail = journal.read(1).unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].request_id, "req-2");

    // And the file stays private to the owner.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(journal.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn test_journal_read_of_an_unreadable_file_is_an_error_not_an_empty_list() {
    let directory = tempfile::tempdir().unwrap();
    // A directory where the journal file should be: it exists and cannot be
    // read as a file, which must not read back as "nothing was asked".
    let path = directory.path().join("journal.ndjson");
    std::fs::create_dir(&path).unwrap();
    let journal = Journal::at(path);
    assert!(journal.read(20).is_err());
}

#[test]
fn test_redaction_removes_the_token_body_not_only_the_prefix() {
    let redacted = redact("Bearer oa_pat_998877_TOKENBODY and smct_MACHINEBODY");
    assert!(!redacted.contains("TOKENBODY"));
    assert!(!redacted.contains("MACHINEBODY"));
    assert!(!redacted.contains("998877"));
}

// ---------------------------------------------------------------------------
// executor
// ---------------------------------------------------------------------------

#[test]
fn test_executor_streams_bounded_scrubbed_output() {
    let directory = tempfile::tempdir().unwrap();
    let cancellation = Cancellation::default();
    let mut seen = String::new();
    let outcome = execute_command(
        &[
            "/bin/echo".to_string(),
            "token oa_pat_LEAKEDSECRET here".to_string(),
        ],
        &directory.path().display().to_string(),
        ExecutionLimits::default(),
        &cancellation,
        |chunk| seen.push_str(chunk),
    );
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.timed_out);
    assert!(seen.contains("token"));
    assert!(
        !seen.contains("LEAKEDSECRET"),
        "a token printed by the command reached the caller: {seen}"
    );
}

#[test]
fn test_executor_truncates_at_the_output_ceiling() {
    let directory = tempfile::tempdir().unwrap();
    let cancellation = Cancellation::default();
    let mut bytes = 0usize;
    let outcome = execute_command(
        &[
            "/usr/bin/head".to_string(),
            "-c".to_string(),
            "8192".to_string(),
            "/dev/zero".to_string(),
        ],
        &directory.path().display().to_string(),
        ExecutionLimits {
            timeout: Duration::from_secs(10),
            maximum_output_bytes: 64,
        },
        &cancellation,
        |chunk| bytes += chunk.len(),
    );
    assert!(bytes <= 64, "the output ceiling was crossed: {bytes} bytes");
    assert!(outcome.truncated, "a truncated run must say so");
}

#[test]
fn test_executor_stops_a_command_that_outlives_its_timeout() {
    let directory = tempfile::tempdir().unwrap();
    let cancellation = Cancellation::default();
    let outcome = execute_command(
        &["/bin/sleep".to_string(), "30".to_string()],
        &directory.path().display().to_string(),
        ExecutionLimits {
            timeout: Duration::from_millis(300),
            maximum_output_bytes: 1024,
        },
        &cancellation,
        |_| {},
    );
    assert!(outcome.timed_out, "the command outlived its timeout");
    assert!(outcome.duration_ms < 10_000);
}

#[test]
fn test_executor_reports_a_missing_binary_rather_than_succeeding() {
    let directory = tempfile::tempdir().unwrap();
    let outcome = execute_command(
        &["/nonexistent/binary-that-is-not-here".to_string()],
        &directory.path().display().to_string(),
        ExecutionLimits::default(),
        &Cancellation::default(),
        |_| {},
    );
    assert_eq!(outcome.exit_code, Some(127));
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

#[test]
fn test_probe_reports_this_host_and_the_roots_it_was_given() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("checkout")).unwrap();
    let roots = vec![directory.path().join("checkout")];
    let report = probe(&roots);

    assert_eq!(report.schema, "openagents.computer_probe.v1");
    assert!(report.host.cpu_count > 0);
    assert!(report.host.total_memory_bytes > 0);
    assert!(
        !report.host.hostname.is_empty(),
        "the probe names this host"
    );
    assert!(
        !report.host.release.is_empty(),
        "the probe reads the kernel release"
    );
    assert_eq!(report.coding_agents.len(), 11);
    assert_eq!(report.toolchains.len(), 14);
    assert_eq!(report.roots.len(), 1);
    assert_eq!(report.worktrees.len(), 1);
    assert!(report.worktrees[0].exists);
    assert!(!report.worktrees[0].git);

    // `git` is on this machine, and the probe reports its real path and version
    // rather than only a boolean.
    let git = report
        .toolchains
        .iter()
        .find(|tool| tool.name == "git")
        .unwrap();
    assert!(git.present);
    assert!(
        git.path.contains("git"),
        "the probe resolves a path: {}",
        git.path
    );
    assert!(
        git.version.starts_with("git version"),
        "the probe reads a version: {}",
        git.version
    );

    // The narrower host summary agrees with the full report.
    assert_eq!(probe_host().num_cpus, report.host.cpu_count);
}

// ---------------------------------------------------------------------------
// machine credential
// ---------------------------------------------------------------------------

#[test]
fn test_machine_credentials_round_trip_and_stay_separate_per_origin() {
    let directory = tempfile::tempdir().unwrap();
    let production = MachineCredentials::isolated("https://openagents.com", directory.path());
    let staging = MachineCredentials::isolated("https://staging.openagents.com", directory.path());

    assert!(production.get().unwrap().is_none());
    production
        .set(&openagents_cli::auth::Secret::new("smct_machine_token"))
        .unwrap();
    assert_eq!(
        production.get().unwrap().unwrap().expose(),
        "smct_machine_token"
    );
    assert!(
        staging.get().unwrap().is_none(),
        "a machine paired with production is not offered to staging"
    );

    assert!(production.remove().unwrap());
    assert!(production.get().unwrap().is_none());
    assert!(!production.remove().unwrap());
}

// ---------------------------------------------------------------------------
// the controller client
// ---------------------------------------------------------------------------

/// A status read that the server refuses is an error. It is never reported as
/// "not paired", which would send the owner to `computer pair` for a problem
/// that has nothing to do with pairing.
#[tokio::test]
async fn test_computer_status_refuses_rather_than_reporting_unpaired() {
    let client = ComputerClient::new("https://openagents.com/no-such-surface");
    let result = client
        .status(&openagents_cli::auth::Secret::new("smct_not_a_real_token"))
        .await;
    let refusal = result
        .expect_err("a 404 is not an unpaired machine")
        .to_string();
    assert!(
        refusal.contains("could not read this Computer status"),
        "the refusal must name what failed: {refusal}"
    );
}

/// The live controller surface answers a machine token it does not know with a
/// 401, and only that is reported as "no longer active".
#[tokio::test]
async fn test_live_controller_reports_an_unknown_machine_token_as_inactive() {
    let client = ComputerClient::new("https://openagents.com");
    let status = client
        .status(&openagents_cli::auth::Secret::new("smct_not_a_real_token"))
        .await
        .expect("the live controller answers");
    assert!(
        status.is_none(),
        "an unknown machine token is not an active machine"
    );
}

// ---------------------------------------------------------------------------
// the outbound channel
// ---------------------------------------------------------------------------

/// A Phoenix-shaped controller socket, so the client's join, hello, framing,
/// policy, journal, and exit reporting all run against a live peer.
struct StubController {
    origin: String,
    frames: Receiver<serde_json::Value>,
}

fn start_stub_controller(machine_id: &str, run_payload: serde_json::Value) -> StubController {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, frames) = channel();
    let topic = format!("computer:{machine_id}");

    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let Ok(mut socket) = tungstenite::accept(stream) else {
            return;
        };
        // phx_join, then the reply the client waits for before it sends hello.
        let _ = socket.read();
        let reply =
            serde_json::json!(["1", "1", topic, "phx_reply", {"status": "ok", "response": {}}]);
        let _ = socket.send(tungstenite::Message::Text(reply.to_string().into()));
        // hello
        let _ = socket.read();
        let ask = serde_json::json!([serde_json::Value::Null, "9", topic, "run", run_payload]);
        let _ = socket.send(tungstenite::Message::Text(ask.to_string().into()));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            match socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        let terminal = value
                            .get(3)
                            .and_then(|event| event.as_str())
                            .map(|event| event == "refused" || event == "exit")
                            .unwrap_or(false);
                        if sender.send(value).is_err() {
                            return;
                        }
                        if terminal {
                            break;
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = socket.close(None);
        // Drain the close handshake so the client sees a clean end.
        while socket.read().is_ok() {}
    });

    StubController {
        origin: format!("http://127.0.0.1:{port}"),
        frames,
    }
}

fn next_frame(frames: &Receiver<serde_json::Value>, event: &str) -> serde_json::Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
    while std::time::Instant::now() < deadline {
        match frames.recv_timeout(Duration::from_secs(25)) {
            Ok(frame) => {
                if frame.get(3).and_then(|value| value.as_str()) == Some(event) {
                    return frame.get(4).cloned().unwrap_or(serde_json::Value::Null);
                }
            }
            Err(_) => break,
        }
    }
    panic!("the client never sent a {event} frame");
}

/// A command the allowlist does not carry is refused over the wire, and the
/// refusal lands in the local journal with the reason that produced it. The
/// journal never leaves this machine, so this is the only place the owner can
/// read what was asked of it.
#[test]
fn test_up_refuses_a_command_outside_the_allowlist_and_journals_it() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let config = config_at(directory.path(), Tier::Curated, vec![root.clone()]);
    let journal = Journal::at(directory.path().join("journal.ndjson"));

    let stub = start_stub_controller(
        "machine-1",
        serde_json::json!({
            "request_id": "req-refused",
            "argv": ["curl", "https://example.com"],
            "cwd": root.display().to_string(),
        }),
    );

    let reason = serve(
        &stub.origin,
        &openagents_cli::auth::Secret::new("smct_stub"),
        "machine-1",
        &serde_json::json!({"agent_version": "test"}),
        &config,
        &journal,
        |_| {},
    );

    let refused = next_frame(&stub.frames, "refused");
    assert_eq!(
        refused.get("reason").and_then(|v| v.as_str()),
        Some("not_allowlisted"),
        "the server was told why: {refused}"
    );
    assert_eq!(
        refused.get("request_id").and_then(|v| v.as_str()),
        Some("req-refused")
    );
    assert!(refused
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("curated allowlist"));
    // The stub serves one connection and then stops listening, so the client
    // sees the peer close, retries within its bound, and reports that it ran
    // out of retries rather than claiming a clean shutdown.
    assert!(
        reason.starts_with("transport_retry_exhausted:"),
        "the ending names what happened: {reason}"
    );

    let entries = journal.read(50).unwrap();
    let recorded = entries
        .iter()
        .find(|entry| entry.request_id == "req-refused" && entry.outcome == "refused")
        .expect("the refusal is in the local journal");
    assert_eq!(recorded.decision, "not_allowlisted");
    assert_eq!(recorded.argv, vec!["curl", "https://example.com"]);
    assert_eq!(recorded.cwd, root.display().to_string());
    eprintln!(
        "journal entry: {}",
        serde_json::to_string(recorded).unwrap()
    );
}

/// An allowed command runs, streams its real output, and reports a real exit.
#[test]
fn test_up_serves_a_bounded_allowed_request() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let config = config_at(directory.path(), Tier::Curated, vec![root.clone()]);
    let journal = Journal::at(directory.path().join("journal.ndjson"));

    let stub = start_stub_controller(
        "machine-2",
        serde_json::json!({
            "request_id": "req-allowed",
            "argv": ["git", "--version"],
            "cwd": root.display().to_string(),
        }),
    );

    serve(
        &stub.origin,
        &openagents_cli::auth::Secret::new("smct_stub"),
        "machine-2",
        &serde_json::json!({"agent_version": "test"}),
        &config,
        &journal,
        |_| {},
    );

    let mut chunks = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
    let exit = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "no exit frame arrived"
        );
        let frame = stub
            .frames
            .recv_timeout(Duration::from_secs(25))
            .expect("the client sends chunk and exit frames");
        match frame.get(3).and_then(|value| value.as_str()) {
            Some("chunk") => chunks.push_str(
                frame
                    .get(4)
                    .and_then(|payload| payload.get("text"))
                    .and_then(|text| text.as_str())
                    .unwrap_or_default(),
            ),
            Some("exit") => break frame.get(4).cloned().unwrap(),
            Some("refused") => panic!("git --version was refused: {frame}"),
            _ => {}
        }
    };

    assert!(
        chunks.contains("git version"),
        "the command's own output reached the server: {chunks:?}"
    );
    assert_eq!(
        exit.get("status").and_then(|v| v.as_str()),
        Some("completed")
    );
    assert_eq!(exit.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(exit.get("timed_out").and_then(|v| v.as_bool()), Some(false));

    let entries = journal.read(50).unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry.request_id == "req-allowed" && entry.outcome == "completed"));
}

/// Transport loss retries with bounded backoff and then stops. It does not
/// reconnect forever, and it says the retries were exhausted rather than
/// reporting a clean close.
#[test]
fn test_up_retries_transport_loss_with_bounded_backoff() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (counted, attempts) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            // Accept the connection and drop it before the handshake completes.
            drop(stream);
            if counted.send(()).is_err() {
                return;
            }
        }
    });

    let directory = tempfile::tempdir().unwrap();
    let config = PolicyConfig::closed(ComputerPaths::in_directory(directory.path()));
    let journal = Journal::at(directory.path().join("journal.ndjson"));
    let mut events = Vec::new();

    let started = std::time::Instant::now();
    let reason = serve(
        &format!("http://127.0.0.1:{port}"),
        &openagents_cli::auth::Secret::new("smct_stub"),
        "machine-3",
        &serde_json::json!({}),
        &config,
        &journal,
        |event| events.push(event.to_string()),
    );
    let elapsed = started.elapsed();

    assert!(
        reason.starts_with("transport_retry_exhausted:"),
        "a bounded retry ends by saying so: {reason}"
    );
    assert_eq!(
        events.len(),
        3,
        "three bounded reconnects, not an unbounded loop: {events:?}"
    );
    assert!(events[0].starts_with("reconnect:"));

    let mut connections = 0;
    while attempts.try_recv().is_ok() {
        connections += 1;
    }
    assert_eq!(connections, 4, "one attempt plus three retries");

    // 250ms, then 500ms, then 1s. The backoff grows and is bounded.
    assert!(
        elapsed >= Duration::from_millis(1_700),
        "the retries did not back off: {elapsed:?}"
    );
    assert!(elapsed < Duration::from_secs(30));

    assert!(
        journal
            .read(50)
            .unwrap()
            .iter()
            .filter(|entry| entry.decision == "transport")
            .count()
            >= 4,
        "every transport ending is recorded locally"
    );
}

// ---------------------------------------------------------------------------
// the whole command, through the binary
// ---------------------------------------------------------------------------

/// A stand-in for the controller: the `/controller/status` route and the
/// Phoenix socket on one origin, so `oa computer status` and `oa computer up`
/// run end to end against a peer.
///
/// The socket is served once. A second upgrade is answered `403`, which is a
/// decision rather than transport loss, so the client stops instead of
/// reconnecting into a loop.
fn start_controller_host(
    machine_id: &'static str,
    run_payload: serde_json::Value,
) -> (String, Receiver<serde_json::Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, frames) = channel();
    let topic = format!("computer:{machine_id}");
    let served = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut peeked = [0u8; 2048];
            let count = stream.peek(&mut peeked).unwrap_or(0);
            let head = String::from_utf8_lossy(&peeked[..count]).to_ascii_lowercase();

            if !head.contains("upgrade: websocket") {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let payload = serde_json::json!({
                    "machine_id": machine_id,
                    "name": "stub-machine",
                    "status": "active",
                    "token_expires_at": "2099-01-01T00:00:00Z",
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                continue;
            }

            if served.swap(true, std::sync::atomic::Ordering::SeqCst) {
                let _ = stream.write_all(
                    b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                );
                let _ = stream.flush();
                continue;
            }

            let Ok(mut socket) = tungstenite::accept(stream) else {
                continue;
            };
            let _ = socket.read();
            let reply =
                serde_json::json!(["1", "1", topic, "phx_reply", {"status": "ok", "response": {}}]);
            let _ = socket.send(tungstenite::Message::Text(reply.to_string().into()));
            let _ = socket.read();
            let ask = serde_json::json!([serde_json::Value::Null, "9", topic, "run", run_payload]);
            let _ = socket.send(tungstenite::Message::Text(ask.to_string().into()));

            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            while std::time::Instant::now() < deadline {
                match socket.read() {
                    Ok(tungstenite::Message::Text(text)) => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            let event = value
                                .get(3)
                                .and_then(|event| event.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let _ = sender.send(value);
                            if event == "refused" || event == "exit" {
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            let _ = socket.close(None);
        }
    });

    (format!("http://127.0.0.1:{port}"), frames)
}

/// Lay out a private `HOME` holding a Computer policy and a machine token, so
/// the binary reads real files without touching the developer's own.
fn stub_home(directory: &std::path::Path, origin: &str, tier: &str, root: &std::path::Path) {
    let config_directory = directory.join(".config").join("openagents");
    std::fs::create_dir_all(&config_directory).unwrap();
    std::fs::write(
        config_directory.join("computer.json"),
        serde_json::json!({
            "tier": tier,
            "roots": [root.display().to_string()],
            "pre_approved": [],
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        config_directory.join("cli-credentials.json"),
        serde_json::json!({
            "version": 1,
            "tokens": { format!("computer:{origin}"): "smct_stub_machine_token" },
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn test_the_binary_reports_real_pairing_state_without_printing_a_secret() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (origin, _frames) = start_controller_host("machine-status", serde_json::json!({}));
    stub_home(directory.path(), &origin, "curated", &root);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", &origin, "--json", "computer", "status"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "computer status failed: {output:?}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["state"], serde_json::json!("paired"));
    assert_eq!(value["paired"], serde_json::json!(true));
    assert_eq!(value["machine_id"], serde_json::json!("machine-status"));
    assert_eq!(value["tier"], serde_json::json!("curated"));
    assert_eq!(
        value["paths"]["journal"],
        serde_json::json!(directory
            .path()
            .join(".config/openagents/journal.ndjson")
            .display()
            .to_string())
    );
    assert!(
        !stdout.contains("smct_"),
        "the machine token reached the output: {stdout}"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("smct_"));
}

/// `oa computer up` end to end: it opens the outbound connection, serves the
/// bounded request the peer asks for, and writes what it decided to the local
/// journal.
#[test]
fn test_the_binary_serves_a_bounded_request_over_an_outbound_connection() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (origin, frames) = start_controller_host(
        "machine-up",
        serde_json::json!({
            "request_id": "req-binary",
            "argv": ["git", "--version"],
            "cwd": root.display().to_string(),
        }),
    );
    stub_home(directory.path(), &origin, "curated", &root);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", &origin, "computer", "up"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();

    let mut chunks = String::new();
    let mut exit = None;
    while let Ok(frame) = frames.recv_timeout(Duration::from_secs(5)) {
        match frame.get(3).and_then(|value| value.as_str()) {
            Some("chunk") => chunks.push_str(
                frame[4]
                    .get("text")
                    .and_then(|text| text.as_str())
                    .unwrap_or_default(),
            ),
            Some("exit") => {
                exit = Some(frame[4].clone());
                break;
            }
            Some("refused") => panic!("the binary refused an allowed command: {frame}"),
            _ => {}
        }
    }
    let exit = exit.expect("the binary reported a terminal exit to the peer");
    assert!(
        chunks.contains("git version"),
        "output reached the peer: {chunks:?}"
    );
    assert_eq!(exit["status"], serde_json::json!("completed"));
    assert_eq!(exit["exit_code"], serde_json::json!(0));

    // The second upgrade is refused, so the command stops rather than
    // reconnecting forever, and it says so on stderr with a non-zero status.
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        stderr.contains("oa: the Computer connection stopped"),
        "the ending is reported on stderr: {stderr}"
    );

    let journal = Journal::at(directory.path().join(".config/openagents/journal.ndjson"));
    let entries = journal.read(50).unwrap();
    assert!(entries
        .iter()
        .any(|entry| entry.request_id == "req-binary" && entry.outcome == "completed"));
}

/// The refusal path through the binary, and the journal entry it leaves.
#[test]
fn test_the_binary_refuses_a_command_outside_the_allowlist() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (origin, frames) = start_controller_host(
        "machine-refuse",
        serde_json::json!({
            "request_id": "req-binary-refused",
            "argv": ["curl", "https://example.com"],
            "cwd": root.display().to_string(),
        }),
    );
    stub_home(directory.path(), &origin, "curated", &root);

    let _ = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", &origin, "computer", "up"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();

    let refused = next_frame(&frames, "refused");
    assert_eq!(refused["reason"], serde_json::json!("not_allowlisted"));
    assert_eq!(
        refused["request_id"],
        serde_json::json!("req-binary-refused")
    );

    let journal = Journal::at(directory.path().join(".config/openagents/journal.ndjson"));
    let recorded = journal
        .read(50)
        .unwrap()
        .into_iter()
        .find(|entry| entry.request_id == "req-binary-refused" && entry.outcome == "refused")
        .expect("the refusal is journaled locally");
    assert_eq!(recorded.decision, "not_allowlisted");

    // And `oa computer journal` reads it back.
    let listed = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["computer", "journal"])
        .env("HOME", directory.path())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&listed.stdout).to_string();
    assert!(
        text.contains("not_allowlisted/refused") && text.contains("curl https://example.com"),
        "the journal command shows the refusal: {text}"
    );
}

/// Without a machine token there is nothing to serve, and the command says so
/// rather than pretending to run a daemon. The version this replaces printed
/// `Computer agent daemon launched.` and exited zero.
#[test]
fn test_the_binary_refuses_to_serve_when_it_is_not_paired() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join(".config/openagents")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", "http://127.0.0.1:1", "computer", "up"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        stderr.contains("not paired") && stderr.contains("computer pair"),
        "the refusal names the next step: {stderr}"
    );
}

/// `logout` removes the machine token, and `status` then reports the local
/// state rather than a stale pairing.
#[test]
fn test_the_binary_logout_removes_the_machine_token() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (origin, _frames) = start_controller_host("machine-logout", serde_json::json!({}));
    stub_home(directory.path(), &origin, "curated", &root);

    let removed = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", &origin, "--json", "computer", "logout"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();
    assert!(removed.status.success());
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&removed.stdout)).unwrap();
    assert_eq!(value["removed"], serde_json::json!(true));

    let after = std::process::Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(["--api-url", &origin, "--json", "computer", "status"])
        .env("HOME", directory.path())
        .env_remove("OPENAGENTS_TOKEN")
        .output()
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&after.stdout)).unwrap();
    assert_eq!(value["state"], serde_json::json!("local"));
    assert_eq!(value["paired"], serde_json::json!(false));
}

// ---------------------------------------------------------------------------
// api passthrough: path resolution
// ---------------------------------------------------------------------------

const ORIGIN: &str = "https://openagents.com";

/// The bug this issue was reopened for. Both spellings of the same route now
/// resolve to it; the version this replaces turned `/api/v1/user` into
/// `/api/v1/api/v1/user` and 404ed on the one form its help text advertised.
#[test]
fn test_absolute_and_relative_paths_name_the_same_route() {
    assert_eq!(
        resolve_api_path(ORIGIN, "/api/v1/user").unwrap(),
        "/api/v1/user"
    );
    assert_eq!(resolve_api_path(ORIGIN, "user").unwrap(), "/api/v1/user");
    assert_eq!(
        resolve_api_path(ORIGIN, "repos/OpenAgentsInc/openagents/issues").unwrap(),
        "/api/v1/repos/OpenAgentsInc/openagents/issues"
    );
    assert_eq!(
        resolve_api_path(ORIGIN, "/api/v1/repos/OpenAgentsInc/openagents/issues").unwrap(),
        "/api/v1/repos/OpenAgentsInc/openagents/issues"
    );
    // A query survives resolution.
    assert_eq!(
        resolve_api_path(ORIGIN, "issues?state=closed").unwrap(),
        "/api/v1/issues?state=closed"
    );
    // A complete URL on the configured origin is accepted.
    assert_eq!(
        resolve_api_path(ORIGIN, "https://openagents.com/api/v1/user").unwrap(),
        "/api/v1/user"
    );
}

#[test]
fn test_a_path_that_leaves_the_api_is_refused() {
    // The website, not the API.
    let refusal = resolve_api_path(ORIGIN, "/user").unwrap_err();
    assert!(
        refusal.contains("must start with /api/"),
        "the refusal must say how to write it: {refusal}"
    );
    // Another origin entirely.
    assert!(resolve_api_path(ORIGIN, "https://example.com/api/v1/user")
        .unwrap_err()
        .contains("leaves the configured API origin"));
    // A protocol-relative path is another origin in disguise.
    assert!(resolve_api_path(ORIGIN, "//example.com/api/v1/user").is_err());
    // A relative path that climbs out of the API prefix.
    assert!(resolve_api_path(ORIGIN, "../../admin")
        .unwrap_err()
        .contains("resolves outside"));
    assert!(resolve_api_path(ORIGIN, "   ").is_err());
    assert!(resolve_api_path(ORIGIN, "ftp://openagents.com/api/v1/user").is_err());
}

#[test]
fn test_an_unknown_method_is_refused_rather_than_performed_as_a_get() {
    assert_eq!(admitted_method("post").unwrap(), "POST");
    assert_eq!(admitted_method(" delete ").unwrap(), "DELETE");
    let refusal = admitted_method("POSTT").unwrap_err();
    assert!(
        refusal.contains("not a supported method"),
        "the refusal must name the problem: {refusal}"
    );
    assert!(admitted_method("HEAD").is_err());
    assert!(admitted_method("OPTIONS").is_err());
    assert_eq!(resolve_request_method(None, false), "GET");
    assert_eq!(resolve_request_method(None, true), "POST");
    assert_eq!(resolve_request_method(Some("PATCH"), true), "PATCH");
}

#[test]
fn test_request_fields_and_headers_are_parsed_not_guessed() {
    let fields =
        parse_request_fields(&["title=Hello".to_string(), "body=World".to_string()]).unwrap();
    assert_eq!(fields["title"], serde_json::json!("Hello"));
    assert_eq!(fields["body"], serde_json::json!("World"));
    // Every value is a JSON string; `--input` carries anything else.
    let typed = parse_request_fields(&["count=3".to_string()]).unwrap();
    assert_eq!(typed["count"], serde_json::json!("3"));

    assert!(parse_request_fields(&["nope".to_string()]).is_err());
    assert!(parse_request_fields(&["=value".to_string()]).is_err());
    assert!(
        parse_request_fields(&["a=1".to_string(), "a=2".to_string()])
            .unwrap_err()
            .contains("more than once")
    );

    let headers = parse_request_headers(&["X-Trace:  abc  ".to_string()]).unwrap();
    assert_eq!(headers, vec![("x-trace".to_string(), "abc".to_string())]);
    // The session owns the authorization header.
    assert!(
        parse_request_headers(&["Authorization: Bearer x".to_string()])
            .unwrap_err()
            .contains("Remove --header authorization")
    );
    assert!(parse_request_headers(&["no colon".to_string()]).is_err());
    assert!(parse_request_headers(&["bad name: x".to_string()]).is_err());

    assert!(decode_request_body("  ", "standard input").is_err());
    assert!(decode_request_body("not json", "standard input").is_err());
    assert_eq!(
        decode_request_body(r#"{"a":[1,2]}"#, "standard input").unwrap(),
        serde_json::json!({"a": [1, 2]})
    );
}

#[test]
fn test_error_details_read_the_servers_own_envelope() {
    let body = serde_json::json!({
        "code": "not_found",
        "message": "Repository not found",
        "request_id": "GM9-abc",
    });
    let details = api_error_details(Some(&body));
    assert_eq!(details.message.as_deref(), Some("Repository not found"));
    assert_eq!(details.code.as_deref(), Some("not_found"));
    assert_eq!(details.request_id.as_deref(), Some("GM9-abc"));

    // A body with no envelope yields nothing rather than an invented message.
    assert_eq!(
        api_error_details(Some(&serde_json::json!({"ok": true}))).message,
        None
    );
    assert_eq!(api_error_details(None).message, None);
}

// ---------------------------------------------------------------------------
// api passthrough: transport
// ---------------------------------------------------------------------------

/// A stub API that answers with the request line and every header it received,
/// so header injection is asserted against a peer rather than against the
/// client's own intent.
fn start_echo_api() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buffer = [0u8; 8192];
            let Ok(count) = stream.read(&mut buffer) else {
                continue;
            };
            let request = String::from_utf8_lossy(&buffer[..count]).to_string();
            let mut lines = request.split("\r\n");
            let start = lines.next().unwrap_or_default().to_string();
            let mut headers = serde_json::Map::new();
            for line in lines {
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    headers.insert(
                        name.trim().to_ascii_lowercase(),
                        serde_json::Value::String(value.trim().to_string()),
                    );
                }
            }
            let body = request
                .split_once("\r\n\r\n")
                .map(|(_, body)| body.to_string())
                .unwrap_or_default();
            let payload = serde_json::json!({
                "request_line": start,
                "headers": serde_json::Value::Object(headers),
                "body": body,
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn test_headers_and_body_reach_the_server() {
    let origin = start_echo_api();
    let client = ApiPassthroughClient::new(&origin, Some("oa_pat_test".to_string()));
    let response = client
        .send(
            "POST",
            "memories",
            &[("x-trace".to_string(), "header-proof".to_string())],
            Some(&serde_json::json!({"body": "hello"})),
        )
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    let echoed = response.body.expect("the stub answers with JSON");
    assert_eq!(
        echoed["request_line"].as_str().unwrap(),
        "POST /api/v1/memories HTTP/1.1",
        "the method and the resolved path both reached the server"
    );
    assert_eq!(
        echoed["headers"]["x-trace"],
        serde_json::json!("header-proof")
    );
    assert_eq!(
        echoed["headers"]["authorization"],
        serde_json::json!("Bearer oa_pat_test"),
        "authentication is forwarded from the session"
    );
    assert_eq!(
        echoed["body"].as_str().unwrap(),
        r#"{"body":"hello"}"#,
        "the JSON body reached the server"
    );
}

/// The replacement for `assert!(res.is_object())`. That assertion held while
/// every refused request returned `{"status": N}` — an object — so it could
/// not tell a route from an error. This names the field the route returns.
#[tokio::test]
async fn test_api_passthrough_returns_the_repository_the_route_names() {
    let client = ApiPassthroughClient::new(ORIGIN, None);
    let value = client
        .execute_request("GET", "repos/OpenAgentsInc/openagents", None)
        .await
        .expect("the public repository route answers");
    assert_eq!(
        value.get("full_name").and_then(|v| v.as_str()),
        Some("OpenAgentsInc/openagents")
    );
    assert!(!value
        .get("default_branch")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .is_empty());
    assert_eq!(
        value.get("visibility").and_then(|v| v.as_str()),
        Some("public")
    );
}

/// A route the server does not serve is an error carrying the server's own
/// message, not a value. The `{"status": 404}` stub this replaces satisfied
/// every assertion a caller could write about the success path.
#[tokio::test]
async fn test_api_passthrough_refuses_a_route_the_server_does_not_serve() {
    let client = ApiPassthroughClient::new(ORIGIN, None);
    let refused = client
        .execute_request("GET", "repos/OpenAgentsInc/no-such-repository-here", None)
        .await;
    let message = refused
        .expect_err("a 404 must not read as a value")
        .to_string();
    assert!(
        message.contains("404"),
        "the refusal names the status: {message}"
    );
    assert!(
        message.contains("Repository not found"),
        "the refusal carries the server's own message: {message}"
    );

    // And the envelope keeps the server's body so the command can print it.
    let response = client
        .send(
            "GET",
            "repos/OpenAgentsInc/no-such-repository-here",
            &[],
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.status, 404);
    assert!(!response.successful());
    assert_eq!(
        response
            .body
            .as_ref()
            .and_then(|body| body.get("code"))
            .and_then(|code| code.as_str()),
        Some("not_found")
    );
}

/// A host that answers nothing is a transport failure, and it stays one. It
/// does not become an empty body or a plausible status.
#[tokio::test]
async fn test_api_passthrough_reports_transport_failure() {
    // A port nothing is listening on.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let client = ApiPassthroughClient::new(&format!("http://127.0.0.1:{port}"), None);
    let refusal = client
        .send("GET", "user", &[], None)
        .await
        .expect_err("nothing is listening");
    assert!(
        refusal.contains("could not reach"),
        "the refusal names the transport: {refusal}"
    );
}

/// The client keeps the origin whichever way it was given, so an absolute path
/// is not prefixed twice.
#[test]
fn test_client_reduces_a_legacy_api_base_to_its_origin() {
    assert_eq!(
        ApiPassthroughClient::new("https://openagents.com/api/v1", None).origin,
        "https://openagents.com"
    );
    assert_eq!(
        ApiPassthroughClient::new("https://openagents.com", None).origin,
        "https://openagents.com"
    );
}
