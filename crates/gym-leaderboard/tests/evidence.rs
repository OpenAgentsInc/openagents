//! The publication, regenerated from the repository's committed evidence,
//! must say what the reports say, fit its bounds, carry its labels, and
//! match the committed files byte for byte.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gym_leaderboard::contract::{Board, Cost, Label, Miss, StepKind, TaskStatus, TraceBundle};
use gym_leaderboard::{Output, PUBLISHED, bundle, check, evidence::Reader, generate, tb4_delegate};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn output() -> &'static Output {
    static OUT: OnceLock<Output> = OnceLock::new();
    OUT.get_or_init(|| generate(&root()).expect("the committed evidence generates"))
}

fn board(id: &str) -> &'static Board {
    output()
        .leaderboard
        .boards
        .iter()
        .find(|b| b.id == id)
        .expect("board")
}

fn bundle_of(attempt: &str) -> TraceBundle {
    let (_, bytes) = output()
        .bundles
        .iter()
        .find(|(p, _)| p.ends_with(&format!("/{attempt}.json")))
        .expect("bundle");
    serde_json::from_slice(bytes).expect("bundle parses")
}

#[test]
fn the_delegate_board_says_what_the_9776_report_says() {
    let b = board(tb4_delegate::BOARD_ID);
    assert_eq!(
        (b.totals.attempts, b.totals.passes, b.totals.beats),
        (28, 13, 4)
    );
    assert_eq!((b.totals.faults, b.totals.cost_unknown), (0, 20));
    let split = |name: &str| b.splits.iter().find(|s| s.name == name).unwrap().tally;
    assert_eq!((split("pass 1").passes, split("pass 1").beats), (5, 0));
    assert_eq!((split("pass 2").passes, split("pass 2").beats), (8, 4));
    let own = split("tasks with their own knowledge");
    let none = split("tasks without their own knowledge");
    assert_eq!((own.attempts, own.passes, own.beats), (24, 11, 4));
    assert_eq!((none.attempts, none.passes, none.beats), (4, 2, 0));

    let beats: BTreeSet<&str> = b
        .attempts
        .iter()
        .filter(|a| a.beat)
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(
        beats,
        BTreeSet::from([
            "coq-block-bound.p2",
            "gsea-proteomics.p2",
            "mp-checkpoint-consolidation.p2",
            "sound-change-cascade.p2",
        ])
    );
    let coq = b
        .attempts
        .iter()
        .find(|a| a.id == "coq-block-bound.p2")
        .unwrap();
    assert!(matches!(coq.cost, Cost::Reported { usd } if (usd - 2.0976).abs() < 0.0001));
    assert!((coq.seconds.unwrap() - 420.4).abs() < 0.01);
    let coq_bar = b
        .tasks
        .iter()
        .find(|t| t.task == "coq-block-bound")
        .unwrap();
    assert!((coq_bar.bar.cost_usd.unwrap() - 3.275).abs() < 0.0001);
    assert!(coq_bar.attempts.contains(&coq.id));

    // Every beat is in-sample; the two thin ones say so.
    for a in b.attempts.iter().filter(|a| a.beat) {
        assert!(a.labels.contains(&Label::InSample), "{}", a.id);
        assert!(a.misses.is_empty());
    }
    let thin: BTreeSet<&str> = b
        .attempts
        .iter()
        .filter(|a| a.labels.contains(&Label::ThinMargin))
        .map(|a| a.task.as_str())
        .collect();
    assert_eq!(
        thin,
        BTreeSet::from(["gsea-proteomics", "sound-change-cascade"])
    );

    // An unknown cost never beats, and says why.
    for a in b.attempts.iter().filter(|a| a.cost.known().is_none()) {
        assert!(!a.beat && a.misses.contains(&Miss::CostUnknown), "{}", a.id);
        assert!(a.labels.contains(&Label::CostBound));
    }

    let never: BTreeSet<&str> = b
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::NeverPassed)
        .map(|t| t.task.as_str())
        .collect();
    assert_eq!(
        never,
        BTreeSet::from([
            "distributed-dedup",
            "math-eval-grader",
            "production-planning",
            "risk-scorer-replay",
            "shadow-relay",
        ])
    );
    assert!((b.spend.reported_usd - 17.41).abs() < 0.01);
    assert!((b.spend.estimated_lower_bound_usd.unwrap() - 33.59).abs() < 0.01);
    for label in [
        Label::PreRegistered,
        Label::InSample,
        Label::ListPrice,
        Label::FewAttempts,
    ] {
        assert!(b.labels.contains(&label), "{label:?}");
    }
    assert_eq!(
        b.provenance.frozen_commit.as_deref().map(|c| &c[..10]),
        Some("e0414c3356")
    );
    assert!(b.provenance.evidence.iter().all(|e| e.sha256.len() == 64));
}

#[test]
fn the_tb21_board_says_what_the_essay_says() {
    let b = board(gym_leaderboard::tb21_oos::BOARD_ID);
    let confirmed = b
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Confirmed)
        .count();
    assert_eq!((confirmed, b.tasks.len()), (30, 65));
    assert_eq!(
        (b.totals.attempts, b.totals.passes, b.totals.beats),
        (127, 83, 83)
    );
    let first = b
        .splits
        .iter()
        .find(|s| s.name == "first runs")
        .unwrap()
        .tally;
    assert_eq!((first.attempts, first.passes), (65, 31));
    assert!(b.headline.contains("2.9%"), "{}", b.headline);
    assert!(
        b.headline.contains("48%") && b.headline.contains("92%"),
        "{}",
        b.headline
    );
    assert!((b.spend.estimated_upper_bound_usd.unwrap() - 4.45).abs() < 0.01);
    assert!(b.labels.contains(&Label::OutOfSample) && b.labels.contains(&Label::KnowledgeOff));
    assert!(b.caveats.iter().any(|c| c.code == "reliability"));
}

#[test]
fn every_bundle_fits_its_bound_and_matched_no_credential_rule() {
    let credential = gym_leaderboard::scrub::credential_rules();
    assert_eq!(output().bundles.len(), 28);
    for (path, bytes) in &output().bundles {
        assert!(
            bytes.len() <= bundle::MAX_BUNDLE_BYTES,
            "{path}: {}",
            bytes.len()
        );
        let b: TraceBundle = serde_json::from_slice(bytes).unwrap();
        for rule in b.scrub.redactions.keys() {
            assert!(!credential.contains(&rule.as_str()), "{path}: {rule}");
        }
        assert!(!b.sources.is_empty());
    }
    assert!(output().leaderboard_bytes.len() <= gym_leaderboard::MAX_LEADERBOARD_BYTES);
}

#[test]
fn a_beat_bundle_steps_through_jev_the_delegate_and_the_verifier() {
    let b = bundle_of("coq-block-bound.p2");
    let jev = b.jev.as_ref().expect("Jev's decision");
    assert_eq!(jev.candidates.iter().filter(|c| c.kept).count(), 2);
    assert!(jev.candidates.iter().all(|c| (0.0..=1.0).contains(&c.p)));
    assert!(jev.requirements.iter().any(|r| r.flagged));
    assert!(
        b.briefing
            .as_ref()
            .is_some_and(|t| t.text.contains("Requirements Jev flags"))
    );
    assert!(b.instruction.text.contains("target_theorem"));
    let kinds: Vec<&StepKind> = b.steps.iter().map(|s| &s.kind).collect();
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, StepKind::Decision { name, .. } if name == "jev_briefing"))
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, StepKind::DelegateStarted { .. }))
    );
    assert!(kinds.iter().any(|k| matches!(k, StepKind::Command { .. })));
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, StepKind::DelegateEnded { error: false, .. }))
    );
    // The clock only moves forward.
    let times: Vec<u64> = b.steps.iter().filter_map(|s| s.at_ms).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]));
    let v = b.verifier.as_ref().unwrap();
    assert_eq!((v.passed, v.failed, v.reward), (4, 0, Some(1.0)));
    assert!(b.outcome.beat && b.outcome.passed);
}

#[test]
fn generation_is_deterministic() {
    let again = generate(&root()).unwrap();
    assert_eq!(again.leaderboard_bytes, output().leaderboard_bytes);
    assert_eq!(again.bundles, output().bundles);
}

#[test]
fn the_committed_publication_matches_the_evidence() {
    let problems = check(&root().join(PUBLISHED), output());
    assert!(
        problems.is_empty(),
        "run `gym-leaderboard build`: {problems:#?}"
    );
}

/// Copies the files `tb4_delegate::build` reads into a scratch root.
fn delegate_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let exp = "bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro";
    let mut files = vec![
        format!("{exp}/attempts.json"),
        format!("{exp}/tasks.json"),
        "docs/terminal-bench/2026-09-27-fable-delegate-repro.md".to_owned(),
    ];
    let attempts: Value =
        serde_json::from_slice(&std::fs::read(root().join(&files[0])).unwrap()).unwrap();
    let first = &attempts["attempts"][0];
    files.push(format!(
        "bench/terminal-bench/traces/{}/{}.episode/trajectory.atif.json",
        first["job"].as_str().unwrap(),
        first["trial"].as_str().unwrap()
    ));
    for rel in files {
        let to = dir.path().join(&rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(&rel), to).unwrap();
    }
    dir
}

fn edit_json(path: &Path, edit: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    edit(&mut v);
    std::fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
}

#[test]
fn a_recorded_verdict_the_numbers_dont_support_refuses_to_build() {
    let dir = delegate_fixture();
    let path = dir
        .path()
        .join("bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/attempts.json");
    edit_json(&path, |v| {
        // Claim a beat for a deadline-stopped attempt with unknown cost.
        let row = v["attempts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["delegate"]["total_cost_usd"].is_null())
            .unwrap();
        row["beat_the_bar"] = Value::Bool(true);
    });
    let err = tb4_delegate::build(&Reader::new(dir.path())).unwrap_err();
    assert!(err.0.contains("recomputed beat false"), "{err}");
}

#[test]
fn tallies_that_disagree_with_the_rows_refuse_to_build() {
    let dir = delegate_fixture();
    let path = dir
        .path()
        .join("bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/attempts.json");
    edit_json(&path, |v| v["tallies"]["pass2"]["beats"] = Value::from(5));
    let err = tb4_delegate::build(&Reader::new(dir.path())).unwrap_err();
    assert!(err.0.contains("beats"), "{err}");
}

/// Copies one retained episode's bundled files into a scratch root.
fn episode_fixture(job: &gym_leaderboard::tb4_delegate::TraceJob) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "retention.json",
        "trajectory.atif.json",
        "artifacts/briefing-jev.json",
        "artifacts/delegate-1.briefing.md",
        "verifier/ctrf.json",
        "verifier/reward.txt",
        "verifier/test-stdout.txt",
    ] {
        let rel = format!("{}/{name}", job.episode);
        let to = dir.path().join(&rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(&rel), to).unwrap();
    }
    dir
}

fn first_job() -> gym_leaderboard::tb4_delegate::TraceJob {
    let (_, jobs) = tb4_delegate::build(&Reader::new(root())).unwrap();
    jobs.into_iter().find(|j| j.attempt.beat).unwrap()
}

fn input(job: &gym_leaderboard::tb4_delegate::TraceJob) -> bundle::Input<'_> {
    bundle::Input {
        board: tb4_delegate::BOARD_ID,
        attempt: &job.attempt,
        bar: &job.bar,
        episode: &job.episode,
        own_entries: &job.own_entries,
    }
}

#[test]
fn a_trace_that_changed_after_retention_refuses_to_bundle() {
    let job = first_job();
    let dir = episode_fixture(&job);
    let path = dir
        .path()
        .join(format!("{}/trajectory.atif.json", job.episode));
    edit_json(&path, |v| v["session_id"] = Value::from("edited"));
    let err = bundle::build(&Reader::new(dir.path()), &input(&job)).unwrap_err();
    assert!(err.0.contains("differs from retention.json"), "{err}");
}

#[test]
fn a_planted_credential_is_redacted_and_fails_the_check() {
    let job = first_job();
    let dir = episode_fixture(&job);
    let token = format!("ghp_{}", "Q7".repeat(18));
    let path = dir
        .path()
        .join(format!("{}/trajectory.atif.json", job.episode));
    edit_json(&path, |v| {
        let step = v["steps"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["extra"]["executor_event"]["event"]["kind"] == "command_completed")
            .unwrap();
        step["extra"]["executor_event"]["event"]["output"] =
            Value::from(format!("export GITHUB_TOKEN={token}"));
    });
    // Re-record the edited file's digest, as a retention run would have.
    let bytes = std::fs::read(&path).unwrap();
    let sha = gym_leaderboard::evidence::sha256_hex(&bytes);
    edit_json(
        &dir.path().join(format!("{}/retention.json", job.episode)),
        |v| {
            for f in v["files"].as_array_mut().unwrap() {
                if f["path"] == "trajectory.atif.json" {
                    f["sha256"] = Value::from(sha.clone());
                }
            }
        },
    );
    let b = bundle::build(&Reader::new(dir.path()), &input(&job)).unwrap();
    let text = serde_json::to_string(&b).unwrap();
    assert!(!text.contains(&token));
    assert_eq!(b.scrub.redactions.get("github-token"), Some(&1));

    // `check` refuses a publication carrying that bundle.
    let mut out = generate(&root()).unwrap();
    let mut bytes = serde_json::to_vec(&b).unwrap();
    bytes.push(b'\n');
    out.bundles[0].1 = bytes;
    let problems = check(&root().join(PUBLISHED), &out);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("credential rule github-token")),
        "{problems:#?}"
    );
}
