use super::*;
use crate::RunLink;
use serde_json::json;

fn run(job: &str) -> RunLink {
    RunLink {
        cloud_job: job.into(),
        task: None,
    }
}
fn call(id: &str) -> CallIdentity {
    CallIdentity {
        id: id.into(),
        parent: None,
        run: run("job-1"),
        tool: "shell".into(),
        request: Some("turn-1".into()),
        operation: Some("proc-7".into()),
    }
}
/// A synthetic credential assembled at run time, so no credential-shaped
/// literal sits in the source.
fn secret(tag: &str) -> String {
    format!("{tag}-{}", "Zq9".repeat(12))
}
fn recorder(dir: &Path, redactor: Redactor, budget: u64) -> Recorder {
    Recorder::create(dir.join("ev"), "ev-1", Some(run("job-1")), redactor, budget).unwrap()
}
/// Every byte of every file under `dir`.
fn all_bytes(dir: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(all_bytes(&path));
        } else {
            out.extend(fs::read(&path).unwrap());
        }
    }
    out
}
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
fn retained(r: &Recorder, id: &str, stream: &str) -> Vec<u8> {
    fs::read(r.dir().join("streams").join(format!("{id}.{stream}"))).unwrap()
}
fn exit_ok() -> boat::CommandFrame {
    boat::CommandFrame::Exit {
        exit_code: Some(0),
        success: true,
        timed_out: false,
    }
}

#[test]
fn output_beyond_every_current_cap_is_retained_whole_with_paired_digests() {
    let tmp = tempfile::tempdir().unwrap();
    let mut redactor = Redactor::new();
    redactor.select(&secret("gh")).unwrap();
    let mut r = recorder(tmp.path(), redactor, 64 << 20);
    r.start_call(call("c1"), &json!({"cmd": "cargo build"}), 10)
        .unwrap();
    // 17 MiB: past the 16 KiB shell capture, the 1 MiB event cap, and the
    // 8 MiB ATIF file limit. Odd chunk sizes split lines and characters.
    let line = "compiling crate ✓ ok\n".as_bytes();
    let mut stdout = Vec::new();
    while stdout.len() < 17 << 20 {
        stdout.extend_from_slice(line);
    }
    let mut sent = 0;
    for (i, chunk) in stdout.chunks(1_000_003).enumerate() {
        let ack = r.output("c1", StreamName::Stdout, chunk).unwrap();
        sent += chunk.len() as u64;
        assert!(ack.input_end <= sent, "chunk {i}");
    }
    r.output("c1", StreamName::Stderr, b"warning: x\n").unwrap();
    for frame in [exit_ok()] {
        r.observe_boat("c1", &frame, 20).unwrap();
    }
    let sealed = r.finish(30).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Complete);
    assert_eq!(retained(&r, "c1", "stdout"), stdout);
    let manifest = load_manifest(r.dir(), &sealed.digest).unwrap();
    let c = &manifest.calls[0];
    assert_eq!(c.stdout.length, stdout.len() as u64);
    assert_eq!(c.stdout.digest, digest(&stdout));
    assert_eq!(c.stdout.state, StreamState::Complete);
    assert_eq!(c.outcome, CallOutcome::Succeeded);
    assert_eq!(c.identity.operation.as_deref(), Some("proc-7"));

    // Events are contiguous by sequence; output events tile each stream
    // exactly, and every chunk digest matches the retained range.
    let events = read_events(r.dir()).unwrap();
    assert_eq!(events.last().unwrap().seq, r.last_seq());
    let mut next = 0;
    for e in &events {
        if let Event::Output {
            stream: StreamName::Stdout,
            offset,
            length,
            digest: d,
            ..
        } = &e.event
        {
            assert_eq!(*offset, next);
            let range = *offset as usize..(*offset + *length) as usize;
            assert_eq!(&digest(&stdout[range]), d);
            next += length;
        }
    }
    assert_eq!(next, stdout.len() as u64);
}

#[test]
fn utf8_split_across_chunks_passes_through_byte_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let mut redactor = Redactor::new();
    redactor.select(&secret("tok")).unwrap();
    let mut r = recorder(tmp.path(), redactor, 1 << 20);
    r.start_call(call("c1"), &json!([]), 1).unwrap();
    let text = "naïve café — 日本語 🚀 end".as_bytes();
    // Split inside every multi-byte character, one byte at a time.
    for b in text {
        r.output("c1", StreamName::Stdout, std::slice::from_ref(b))
            .unwrap();
    }
    r.observe_boat("c1", &exit_ok(), 2).unwrap();
    assert_eq!(r.finish(3).unwrap().status, EvidenceStatus::Complete);
    assert_eq!(retained(&r, "c1", "stdout"), text);
}

#[test]
fn a_credential_split_across_chunks_is_redacted_at_every_split() {
    let value = secret("ghp");
    let body = format!("token={value}\nagain:{value}");
    for split in 1..body.len() {
        let tmp = tempfile::tempdir().unwrap();
        let mut redactor = Redactor::new();
        redactor.select(&value).unwrap();
        let mut r = recorder(tmp.path(), redactor, 1 << 20);
        r.start_call(call("c1"), &json!({}), 1).unwrap();
        let (a, b) = body.as_bytes().split_at(split);
        let first = r.output("c1", StreamName::Stderr, a).unwrap();
        // Bytes that could begin the credential are held, not acknowledged.
        assert!(first.input_end <= a.len() as u64);
        r.output("c1", StreamName::Stderr, b).unwrap();
        r.observe_boat("c1", &exit_ok(), 2).unwrap();
        let sealed = r.finish(3).unwrap();
        assert_eq!(sealed.status, EvidenceStatus::CompleteWithRedactions);
        assert_eq!(
            retained(&r, "c1", "stderr"),
            b"token=[redacted]\nagain:[redacted]"
        );
        assert!(!contains(&all_bytes(r.dir()), value.as_bytes()), "{split}");
        let m = load_manifest(r.dir(), &sealed.digest).unwrap();
        assert_eq!(m.calls[0].stderr.redactions, 2);
    }
}

#[test]
fn engine_login_files_and_oauth_tokens_never_enter_evidence() {
    let home = tempfile::tempdir().unwrap();
    let access = secret("sk-ant-oat01");
    let refresh = secret("sk-ant-ort01");
    let codex_access = secret("eyJhbGciOi");
    let codex_refresh = secret("rt");
    let env_token = secret("sk-ant-oat01-env");
    let claude =
        json!({"claudeAiOauth": {"accessToken": access, "refreshToken": refresh, "expiresAt": 1}});
    let codex = json!({"OPENAI_API_KEY": null, "tokens": {"access_token": codex_access, "refresh_token": codex_refresh, "account_id": "acct"}});
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    let claude_text = serde_json::to_string_pretty(&claude).unwrap();
    fs::write(home.path().join(".claude/.credentials.json"), &claude_text).unwrap();
    fs::write(home.path().join(".codex/auth.json"), codex.to_string()).unwrap();
    let redactor = Redactor::engine_logins(home.path(), |name| {
        (name == "CLAUDE_CODE_OAUTH_TOKEN").then(|| env_token.clone())
    })
    .unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), redactor, 1 << 20);
    let args = json!({"cmd": format!("CLAUDE_CODE_OAUTH_TOKEN={env_token} claude -p hi"), access.clone(): 1});
    r.start_call(call("c1"), &args, 1).unwrap();
    let mut output = claude_text.clone().into_bytes();
    output.extend(format!("\n{codex_access} {codex_refresh} {env_token}\n").bytes());
    for chunk in output.chunks(7) {
        r.output("c1", StreamName::Stdout, chunk).unwrap();
    }
    r.result(
        "c1",
        CallResult::EngineError {
            message: format!("auth failed for {refresh}"),
        },
        2,
    )
    .unwrap();
    r.finish(3).unwrap();
    let everything = all_bytes(r.dir());
    for value in [&access, &refresh, &codex_access, &codex_refresh, &env_token] {
        assert!(!contains(&everything, value.as_bytes()));
    }
    let atif = r.atif_call("c1", 1 << 20).unwrap();
    let atif_text = serde_json::to_string(&atif.arguments).unwrap() + &atif.output;
    for value in [&access, &env_token, &codex_access] {
        assert!(!atif_text.contains(value.as_str()));
    }
    assert_eq!(atif.outcome, atif::Outcome::Failed);
    assert!(atif.extra["evidence"]["stdout"]["digest"].is_string());
    // A missing login file is fine; an unreadable one fails closed.
    assert!(Redactor::engine_logins(Path::new("/nonexistent-home"), |_| None).is_ok());
}

#[test]
fn an_engine_error_followed_by_an_apparent_success_stays_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), Redactor::new(), 1 << 20);
    r.start_call(call("c1"), &json!({}), 1).unwrap();
    r.observe_boat("c1", &boat::CommandFrame::Stdout("partial".into()), 2)
        .unwrap();
    r.observe_boat(
        "c1",
        &boat::CommandFrame::Error {
            error: Some("lost".into()),
            message: None,
            retryable: false,
        },
        3,
    )
    .unwrap();
    r.observe_boat("c1", &exit_ok(), 4).unwrap();
    assert_eq!(
        r.observe_boat("c1", &boat::CommandFrame::Stdout("late".into()), 5),
        Err(EvidenceError::Closed("c1".into()))
    );
    let c = r.call("c1").unwrap();
    assert_eq!(c.outcome, CallOutcome::Failed);
    assert_eq!(c.results.len(), 2);
    assert_eq!(c.stdout.state, StreamState::Gap);
    let sealed = r.finish(6).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Incomplete);
}

#[test]
fn budget_exhaustion_keeps_what_fits_and_seals_incomplete() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), Redactor::new(), 100);
    r.start_call(call("c1"), &json!({"a": 1}), 1).unwrap();
    let used = r.call("c1").unwrap().arguments_bytes;
    r.output("c1", StreamName::Stdout, &[b'x'; 150]).unwrap();
    r.output("c1", StreamName::Stdout, b"more").unwrap();
    r.observe_boat("c1", &exit_ok(), 2).unwrap();
    // A second call's arguments do not fit either.
    r.start_call(call("c2"), &json!({"b": 2}), 3).unwrap();
    let sealed = r.finish(4).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Incomplete);
    let m = load_manifest(r.dir(), &sealed.digest).unwrap();
    assert!(m.budget.exhausted);
    assert_eq!(m.budget.used, 100);
    let s = &m.calls[0].stdout;
    assert_eq!(s.state, StreamState::Truncated);
    assert_eq!(s.length, 100 - used);
    assert_eq!(s.dropped_bytes, 150 - (100 - used) + 4);
    assert_eq!(m.calls[1].arguments, None);
    assert_eq!(m.calls[1].outcome, CallOutcome::Unresolved);
    assert!(
        !sealed
            .passed()
            .eq(&crate::transition::VerificationObservation::Passed {
                evidence_digest: sealed.digest.clone(),
                evidence: EvidenceStatus::Complete,
            })
    );
}

#[test]
fn nested_child_runs_are_archived_and_their_completeness_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), Redactor::new(), 1 << 20);
    r.start_call(call("verify"), &json!({"tool": "verifier"}), 1)
        .unwrap();
    let mut child = r
        .child("verify", "child-1", run("job-child"), 4096)
        .unwrap();
    let mut inner = call("check-1");
    inner.parent = Some("verify".into());
    inner.run = run("job-child");
    child
        .start_call(inner, &json!({"cmd": "cargo test"}), 2)
        .unwrap();
    child
        .observe_boat("check-1", &boat::CommandFrame::Stdout("ok\n".into()), 3)
        .unwrap();
    child.observe_boat("check-1", &exit_ok(), 4).unwrap();
    let link = r.archive_child(child, 5).unwrap();
    assert_eq!(link.status, EvidenceStatus::Complete);
    let child_dir = r.dir().join("children/child-1");
    let child_manifest = load_manifest(&child_dir, &link.manifest_digest).unwrap();
    assert_eq!(
        child_manifest.calls[0].identity.parent.as_deref(),
        Some("verify")
    );
    r.observe_boat("verify", &exit_ok(), 6).unwrap();
    let sealed = r.finish(7).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Complete);
    assert_eq!(
        load_manifest(r.dir(), &sealed.digest).unwrap().children,
        vec![link]
    );

    // A reserved child that is never archived, and an incomplete child,
    // each leave the parent incomplete.
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), Redactor::new(), 1 << 20);
    r.start_call(call("verify"), &json!({}), 1).unwrap();
    let _forgotten = r.child("verify", "child-1", run("job-child"), 10).unwrap();
    let mut gapped = r.child("verify", "child-2", run("job-child"), 10).unwrap();
    gapped.start_call(call("x"), &json!({}), 1).unwrap();
    assert_eq!(
        r.archive_child(gapped, 2).unwrap().status,
        EvidenceStatus::Incomplete
    );
    r.observe_boat("verify", &exit_ok(), 3).unwrap();
    let sealed = r.finish(4).unwrap();
    let m = load_manifest(r.dir(), &sealed.digest).unwrap();
    assert_eq!(m.status, EvidenceStatus::Incomplete);
    assert!(
        m.reasons
            .iter()
            .any(|x| x.contains("child-1 was not archived"))
    );
    assert!(
        m.reasons
            .iter()
            .any(|x| x.contains("child-2 is incomplete"))
    );
}

#[test]
fn unfinished_calls_seal_as_explicitly_unresolved_and_records_are_fixed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = recorder(tmp.path(), Redactor::new(), 1 << 20);
    r.start_call(call("c1"), &json!({}), 1).unwrap();
    r.output("c1", StreamName::Stdout, b"still running")
        .unwrap();
    let sealed = r.finish(2).unwrap();
    assert_eq!(sealed.status, EvidenceStatus::Incomplete);
    let c = r.call("c1").unwrap();
    assert_eq!(c.outcome, CallOutcome::Unresolved);
    assert_eq!(c.stdout.state, StreamState::Gap);
    assert_eq!(r.finish(3).unwrap(), sealed);
    assert_eq!(
        r.output("c1", StreamName::Stdout, b"x"),
        Err(EvidenceError::Finalized)
    );
    assert!(matches!(
        r.start_call(call("c2"), &json!({}), 4),
        Err(EvidenceError::Finalized)
    ));
    // A second recorder never reuses a directory, and a changed manifest
    // no longer matches its cited digest.
    assert!(Recorder::create(r.dir(), "ev-1", None, Redactor::new(), 1).is_err());
    fs::write(r.dir().join("manifest.json"), b"{}").unwrap();
    assert!(load_manifest(r.dir(), &sealed.digest).is_err());
}
