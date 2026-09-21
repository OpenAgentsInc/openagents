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
    std::fs::write(&manifest,serde_json::to_vec(&json!({"v":1,"slug":"test-check","name":"Fixture check","transport":"subprocess","detect":{"binary":"/bin/sh","version":["/bin/sh","--version"]},"enforces":[],"cannot_enforce":[],"sees_repository":true,"cost":"local","invoke":["/bin/sh"],"isolation":["directory"]})).unwrap()).unwrap();
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
    for (name, expected) in [("pass", 0), ("unverifiable", 3), ("stale", 2)] {
        if name == "unverifiable" {
            requirements.plan.checks[0].acceptance = Acceptance::Suite {
                suite_digest: "suite".into(),
                input_digest: observed.digest(),
            };
        }
        if name == "stale" {
            requirements.tip = "0".repeat(40);
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
                "verify",
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
}
