//! The operator CLI binds a reviewed artifact to an approved bounded verifier.

use coder::capability::{Entry, Source, Trust};
use coder::verification::{Acceptance, Check, Plan, SCHEMA};
use coder_project::{artifact, git};
use serde_json::json;
use std::path::Path;
use std::process::Command;

fn scratch(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .current_dir(path)
        .args([
            "--git-dir=.coder-git",
            "--work-tree=.",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
async fn cli_requires_pinned_artifacts_and_never_accepts_missing_suite_evidence() {
    if !coder::delegate::boundary_supported() {
        eprintln!("skipping: artifact verification requires an enforcing boundary");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let host = tempfile::tempdir().unwrap();
    let repo = root.path();
    std::fs::create_dir(repo.join("capabilities")).unwrap();
    let manifest = repo.join("capabilities/test-check.json");
    std::fs::write(&manifest, serde_json::to_vec(&capability::executor_document("test-check", "/bin/sh", vec!["/bin/sh".into(), "--version".into()], serde_json::json!({"name":"Fixture check","invoke":["/bin/sh"],"isolation":["directory"]}))).unwrap()).unwrap();
    std::fs::write(repo.join("candidate.txt"), "old\n").unwrap();
    git(repo, &["init", "--quiet"]).await.unwrap();
    git(repo, &["add", "."]).await.unwrap();
    git(
        repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Seed",
        ],
    )
    .await
    .unwrap();
    let base = git(repo, &["rev-parse", "HEAD"])
        .await
        .unwrap()
        .trim()
        .to_string();
    let work = repo.join(".coder/worktrees/test");
    std::fs::create_dir_all(work.join("capabilities")).unwrap();
    std::fs::copy(repo.join("candidate.txt"), work.join("candidate.txt")).unwrap();
    std::fs::copy(&manifest, work.join("capabilities/test-check.json")).unwrap();
    scratch(&work, &["init", "--quiet"]);
    std::fs::write(work.join(".coder-git/info/exclude"), ".coder-git/\n").unwrap();
    scratch(&work, &["add", "."]);
    scratch(&work, &["commit", "--quiet", "-m", "Seed"]);
    std::fs::write(work.join("candidate.txt"), "new\n").unwrap();
    scratch(&work, &["add", "candidate.txt"]);
    scratch(&work, &["commit", "--quiet", "-m", "Candidate"]);
    let owned = vec!["candidate.txt".into()];
    let observed = artifact::inspect(repo, &work, &base, &owned).await.unwrap();
    let store = host.path().join("trust.json");
    Trust::load(&store)
        .unwrap()
        .approve(Some(repo), "test-check", &[])
        .unwrap();
    let entry = Entry::load(&manifest, Source::Repository).unwrap();
    let mut requirements = artifact::Verification {
        base,
        tip: observed.tip.clone(),
        owned_paths: owned,
        plan: Plan {
            schema: SCHEMA.into(),
            input_digest: observed.digest(),
            seconds: 5,
            allow_unrestricted_reads: true,
            allow_network: true,
            checks: vec![Check {
                id: "candidate-check".into(),
                manifest,
                manifest_digest: entry.digest,
                arguments: vec!["-c".into(), "test \"$(cat candidate.txt)\" = new".into()],
                seconds: 2,
                output_bytes: 4096,
                acceptance: Acceptance::ExitSuccess,
            }],
        },
    };
    let plan = host.path().join("plan.json");
    for (name, expected) in [
        ("pass", 0),
        ("suite-refuses-exit", 3),
        ("unverifiable", 3),
        ("suite-pass", 0),
        ("stale", 2),
    ] {
        if name == "unverifiable" {
            requirements.plan.checks[0].acceptance = Acceptance::Suite {
                suite_digest: "suite".into(),
                input_digest: observed.digest(),
            };
        }
        if name == "stale" {
            requirements.tip = "0".repeat(40);
        }
        if name == "suite-pass" {
            let evidence = json!({"schema":SCHEMA,"suite_digest":"suite","input_digest":observed.digest(),"verdict":"passed"});
            requirements.plan.checks[0].arguments[1] = format!("printf '%s' '{evidence}'");
        }
        std::fs::write(&plan, serde_json::to_vec(&requirements).unwrap()).unwrap();
        let out = host.path().join(name);
        let result = Command::new(env!("CARGO_BIN_EXE_coder-project"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", host.path())
            .env("CODER_CAPABILITY_TRUST", &store)
            .env("CODER_PROGRAM_EFFECTS", "reads,network,subprocesses,spend")
            .args([
                if name.starts_with("suite-") {
                    "run-suite"
                } else {
                    "verify"
                },
                repo.to_str().unwrap(),
                work.to_str().unwrap(),
                plan.to_str().unwrap(),
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("result.json")).unwrap()).unwrap();
        assert_eq!(report["integration_accepted"], false);
        assert!(out.join("trace.atif.jsonl").exists());
    }
    // Exercise the real review CLI without making a paid decision call. Empty
    // findings need no judgment; malformed or failed evidence must stop first.
    std::fs::create_dir(repo.join("questions")).unwrap();
    std::fs::write(
        repo.join("questions/review-finding.json"),
        include_str!("../../../questions/review-finding.json"),
    )
    .unwrap();
    requirements.tip = observed.tip.clone();
    requirements.plan.checks[0].acceptance = Acceptance::ExitSuccess;
    requirements.plan.checks[0].arguments[1] = "exit 0".into();
    let reviewer = coder::review::Reviewer {
        manifest: requirements.plan.checks[0].manifest.clone(),
        manifest_digest: requirements.plan.checks[0].manifest_digest.clone(),
        arguments: vec!["-c".into(), String::new()],
        seconds: 2,
        output_bytes: 4096,
    };
    let mut review = artifact::Review {
        schema: "openagents.review-plan.v1".into(),
        verification: requirements,
        reviewer,
        policy: coder::review::Policy {
            confirm_at: 0.8,
            dismiss_below: 0.2,
        },
        excluded: vec![],
    };
    for (name, command, expected_outcome, code) in [
        (
            "review-empty",
            "printf '{\"schema\":\"openagents.review-findings.v1\",\"base\":\"%s\",\"tip\":\"%s\",\"input_digest\":\"%s\",\"findings\":[]}' \"$CODER_REVIEW_BASE\" \"$CODER_REVIEW_TIP\" \"$CODER_REVIEW_INPUT_DIGEST\"",
            "answered",
            0,
        ),
        ("review-missing", "printf '{}'", "unverifiable", 3),
        ("review-failed", "exit 1", "failed", 3),
        (
            "review-truncated",
            "while :; do printf xxxxxxxxxxxxxxxxxxxxxxxxx; done",
            "unverifiable",
            3,
        ),
    ] {
        review.reviewer.arguments[1] = command.into();
        std::fs::write(&plan, serde_json::to_vec(&review).unwrap()).unwrap();
        let out = host.path().join(name);
        let result = Command::new(env!("CARGO_BIN_EXE_coder-project"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", host.path())
            .env("CODER_CAPABILITY_TRUST", &store)
            .env("CODER_PROGRAM_EFFECTS", "reads,network,subprocesses,spend")
            .env("CODER_DECISION_PROFILE", "direct_local")
            .env("CODER_DECISION_URL", "http://127.0.0.1:9")
            .env("CODER_DECISION_MODEL", "fixture")
            .args([
                "review-changes",
                repo.to_str().unwrap(),
                work.to_str().unwrap(),
                plan.to_str().unwrap(),
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(code),
            "{name}: {} {}",
            String::from_utf8_lossy(&result.stderr),
            String::from_utf8_lossy(&result.stdout)
        );
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("result.json")).unwrap()).unwrap();
        assert_eq!(
            report["review"]["evidence"]["outcome"], expected_outcome,
            "{report}"
        );
        assert_eq!(report["integration_accepted"], false);
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        let body = loop {
            let n = stream.read(&mut buffer).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let size: usize = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse()
                    .unwrap();
                if bytes.len() >= end + 4 + size {
                    break bytes[end + 4..end + 4 + size].to_vec();
                }
            }
        };
        let asked: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(asked["questions"].as_object().unwrap().len(), 3);
        assert!(
            asked["state"]["diff"][0]["text"]
                .as_str()
                .unwrap()
                .contains("+new")
        );
        let body = json!({"model":"fixture","answers":{"f1":{"type":"noul","noul":0.95},"f2":{"type":"noul","noul":0.05},"f3":{"type":"noul","noul":0.5}}}).to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let findings: Vec<_> = (0..3).map(|n| json!({"path":"candidate.txt","span":{"start":1,"end":1},"severity":"test","summary":format!("Fixture finding {n}")})).collect();
    let document = json!({"schema":coder::review::FINDINGS_SCHEMA,"base":review.verification.base,"tip":observed.tip,"input_digest":observed.digest(),"findings":findings});
    review.reviewer.arguments[1] = format!("printf '%s' '{document}'");
    std::fs::write(&plan, serde_json::to_vec(&review).unwrap()).unwrap();
    let out = host.path().join("review-judgments");
    let result = Command::new(env!("CARGO_BIN_EXE_coder-project"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", host.path())
        .env("CODER_CAPABILITY_TRUST", &store)
        .env("CODER_PROGRAM_EFFECTS", "reads,network,subprocesses,spend")
        .env("CODER_DECISION_PROFILE", "direct_local")
        .env("CODER_DECISION_URL", endpoint)
        .env("CODER_DECISION_MODEL", "fixture")
        .args([
            "review-changes",
            repo.to_str().unwrap(),
            work.to_str().unwrap(),
            plan.to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&result.stdout)
    );
    server.join().unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("result.json")).unwrap()).unwrap();
    for field in ["confirmed", "dismissed", "unresolved"] {
        assert_eq!(report["review"][field], 1);
    }
    assert_eq!(report["review"]["findings"][0]["probability"], 0.95);
    assert_eq!(report["integration_accepted"], false);
}
