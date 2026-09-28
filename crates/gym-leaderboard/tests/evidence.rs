//! The publication, regenerated from the repository's committed evidence,
//! must say what the reports say, fit its bounds, carry its labels, and
//! match the committed files byte for byte.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gym_leaderboard::contract::{Board, Cost, Label, Miss, StepKind, TaskStatus, TraceBundle};
use gym_leaderboard::{
    Output, PUBLISHED, bundle, check, evidence::Reader, generate, microcoder, reference_boards,
    study, tb4_delegate, tb4_delegate_dev, tb4_microcoder_kb, tb4_oos,
};
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
    assert_eq!(output().bundles.len(), 28 + 127 + 13 + 16);
    for (path, bytes) in &output().bundles {
        assert!(
            bytes.len() <= bundle::MAX_BUNDLE_BYTES,
            "{path}: {}",
            bytes.len()
        );
        let b: TraceBundle = serde_json::from_slice(bytes).unwrap();
        for (rule, n) in &b.scrub.redactions {
            if credential.contains(&rule.as_str()) {
                // Only a match a person inspected and recorded, still
                // reading the same source bytes.
                assert!(
                    output().reviews.iter().any(|r| &r.bundle == path
                        && &r.rule == rule
                        && r.matches == *n
                        && b.sources
                            .iter()
                            .any(|s| s.path == r.source && s.sha256 == r.source_sha256)),
                    "{path}: {rule}"
                );
            }
        }
        assert!(!b.sources.is_empty());
    }
    assert!(output().leaderboard_bytes.len() <= gym_leaderboard::MAX_LEADERBOARD_BYTES);
}

#[test]
fn every_tb21_attempt_has_a_trace() {
    let b = board(gym_leaderboard::tb21_oos::BOARD_ID);
    assert_eq!(b.attempts.len(), 127);
    for a in &b.attempts {
        let t = a
            .trace
            .as_ref()
            .unwrap_or_else(|| panic!("{}: no trace", a.id));
        let (_, bytes) = output()
            .bundles
            .iter()
            .find(|(p, _)| *p == t.path)
            .unwrap_or_else(|| panic!("{}: no bundle", t.path));
        assert_eq!(t.bytes, bytes.len() as u64);
        assert_eq!(t.sha256, gym_leaderboard::evidence::sha256_hex(bytes));
    }
}

#[test]
fn a_tb21_pass_bundle_steps_through_the_loop_jev_and_the_verifier() {
    let id = "prove-plus-comm-1790468849833";
    let a = board(gym_leaderboard::tb21_oos::BOARD_ID)
        .attempts
        .iter()
        .find(|a| a.id == id)
        .unwrap();
    let b = bundle_of(id);
    assert!(a.beat && b.outcome.beat && b.outcome.passed);
    // Its cost against the bar: Fable 5 xhigh's cost per trial.
    let cost = b.outcome.cost.known().unwrap();
    assert!((cost - 0.001_315_068).abs() < 1e-9, "{cost}");
    assert!((b.outcome.bar.cost_usd.unwrap() - 0.075).abs() < 1e-9);
    assert!(cost < b.outcome.bar.cost_usd.unwrap());
    // The record files, as the manifest recorded them.
    assert_eq!(b.sources.len(), 3);
    assert!(
        b.sources[0]
            .path
            .ends_with("coderos-4080-tb21/MANIFEST.json")
    );
    assert!(b.jev.is_none() && b.briefing.is_none());

    let kinds: Vec<&StepKind> = b.steps.iter().map(|s| &s.kind).collect();
    assert!(matches!(
        kinds[0],
        StepKind::DelegateStarted { agent, model } if agent == "Microcoder" && model.as_deref() == Some("gpt-6-luna")
    ));
    // Jev's judgments, with every answer's probability.
    let judged: Vec<&StepKind> = kinds
        .iter()
        .copied()
        .filter(|k| matches!(k, StepKind::Decision { name, .. } if name == "judged"))
        .collect();
    assert_eq!(judged.len(), 4);
    let StepKind::Decision {
        answers,
        cost_usd,
        question,
        ..
    } = judged[1]
    else {
        unreachable!()
    };
    let names: Vec<(&str, f64)> = answers.iter().map(|a| (a.name.as_str(), a.p)).collect();
    assert_eq!(
        names,
        vec![("done", 0.03), ("progress", 0.85), ("repeating", 0.08)]
    );
    assert!(cost_usd.is_some() && question.is_some());
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, StepKind::ModelStep { .. }))
            .count(),
        4
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| matches!(k, StepKind::Command { .. }))
            .count(),
        3
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, StepKind::Tests { passed, total, .. } if passed == total))
    );
    assert!(matches!(
        kinds.last().unwrap(),
        StepKind::Ended { reason, steps: Some(4) } if reason == "finished"
    ));
    let times: Vec<u64> = b.steps.iter().filter_map(|s| s.at_ms).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]));
    let v = b.verifier.as_ref().unwrap();
    assert_eq!((v.reward, v.passed, v.failed), (Some(1.0), 4, 0));
    assert!(v.tests.iter().all(|t| t.status == "passed"));
}

/// Copies one TB2.1 record and its manifest into a scratch root.
fn record_fixture(run: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let host = gym_leaderboard::tb21_oos::RUNS;
    for rel in [
        format!("{host}/MANIFEST.json"),
        format!("{host}/{run}/summary.json"),
        format!("{host}/{run}/events.jsonl"),
    ] {
        let to = dir.path().join(&rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(&rel), to).unwrap();
    }
    (dir, host.to_owned())
}

#[test]
fn a_record_that_differs_from_its_manifest_refuses_to_bundle() {
    let run = "prove-plus-comm-1790468849833";
    let (dir, host) = record_fixture(run);
    let b = board(gym_leaderboard::tb21_oos::BOARD_ID);
    let attempt = b.attempts.iter().find(|a| a.id == run).unwrap();
    let bar = &b.tasks.iter().find(|t| t.task == attempt.task).unwrap().bar;
    let reader = Reader::new(dir.path());
    let manifest = microcoder::Manifest::read(&reader, &host).unwrap();
    let input = microcoder::Input {
        board: &b.id,
        attempt,
        bar,
        manifest: &manifest,
        run,
    };
    microcoder::build(&reader, &input).expect("the retained record bundles");
    let events = dir.path().join(format!("{host}/{run}/events.jsonl"));
    let mut text = std::fs::read_to_string(&events).unwrap();
    text.push_str("{\"event\": \"ran\", \"step\": 9}\n");
    std::fs::write(&events, text).unwrap();
    let err = microcoder::build(&reader, &input).unwrap_err();
    assert!(err.0.contains("differs from MANIFEST.json"), "{err}");
}

#[test]
fn a_review_holds_only_while_its_source_is_unchanged() {
    // The committed publication's one reviewed match passes `check`.
    assert!(check(&root().join(PUBLISHED), output()).is_empty());
    // A review whose recorded digest no longer matches lapses: the match
    // fails `check` again, and the review is reported as matching nothing.
    let mut out = generate(&root()).unwrap();
    assert!(!out.reviews.is_empty());
    out.reviews[0].source_sha256 = "0".repeat(64);
    let problems = check(&root().join(PUBLISHED), &out);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("credential rule private-key")),
        "{problems:#?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("matches no bundle")),
        "{problems:#?}"
    );
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
        format!("{exp}/study.json"),
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

#[test]
fn the_development_board_says_what_the_9746_report_says() {
    let b = board(tb4_delegate_dev::BOARD_ID);
    assert_eq!(
        (b.totals.attempts, b.totals.passes, b.totals.beats),
        (13, 4, 2)
    );
    // One split per series: the arm changed between them.
    let per_series: Vec<(&str, u32, u32)> = b
        .splits
        .iter()
        .map(|s| (s.name.as_str(), s.tally.attempts, s.tally.beats))
        .collect();
    assert_eq!(
        per_series,
        vec![
            ("series 1", 3, 0),
            ("series 2", 1, 0),
            ("series 3", 3, 0),
            ("series 4", 1, 0),
            ("series 5", 2, 1),
            ("series 6", 2, 0),
            ("series 7", 1, 1),
        ]
    );
    let beats: Vec<&str> = b
        .attempts
        .iter()
        .filter(|a| a.beat)
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(beats, vec!["fin-saccr-rwa.s5a2", "fin-saccr-rwa.s7a1"]);
    assert!(b.headline.contains("2 of 13") && b.headline.contains("s5a2 and s7a1"));

    // s7a1: $0.9429 in 149.5 s against $1.2246 and 222.5 s, with Jev's one
    // decision keeping 5 of 12 candidates and flagging 3 of 6 requirements.
    let find = |id: &str| b.attempts.iter().find(|a| a.id == id).unwrap();
    let s7a1 = find("fin-saccr-rwa.s7a1");
    assert!(matches!(s7a1.cost, Cost::Reported { usd } if (usd - 0.9429).abs() < 0.0001));
    assert!((s7a1.seconds.unwrap() - 149.5).abs() < 0.01);
    let saccr = b.tasks.iter().find(|t| t.task == "fin-saccr-rwa").unwrap();
    assert!((saccr.bar.cost_usd.unwrap() - 1.2246).abs() < 0.0001);
    assert!((saccr.bar.seconds.unwrap() - 222.5).abs() < 0.05);
    let jev = s7a1.jev.as_ref().expect("s7a1's Jev decision");
    assert_eq!(
        (jev.kept, jev.candidates, jev.flagged, jev.requirements),
        (5, 12, 3, 6)
    );
    // s5a2 beat the bar with no Jev decision, and says so.
    let s5a2 = find("fin-saccr-rwa.s5a2");
    assert!(s5a2.jev.is_none());
    assert!(s5a2.caveats.contains(&"no_jev_decision".to_owned()));
    assert!(matches!(s5a2.cost, Cost::Reported { usd } if (usd - 0.8816).abs() < 0.0001));
    // Both wins are in-sample and tuned on the task, on the row and the board.
    for a in [s5a2, s7a1] {
        assert!(a.labels.contains(&Label::InSample), "{}", a.id);
        for code in ["in_sample", "tuned_on_task"] {
            assert!(a.caveats.contains(&code.to_owned()), "{}: {code}", a.id);
            assert!(b.caveats.iter().any(|c| c.code == code));
        }
    }
    assert!(b.labels.contains(&Label::InSample));
    // Series 1 recorded no verdict; its three attempts are recomputed.
    for a in b.attempts.iter().filter(|a| a.series == "series 1") {
        assert!(!a.beat && a.misses.contains(&Miss::CostUnknown), "{}", a.id);
    }
    // Every attempt has its retained episode bundled.
    assert!(
        b.attempts
            .iter()
            .all(|a| a.trace.as_ref().is_some_and(|t| t.bytes > 0))
    );
    let evidence: Vec<&str> = b
        .provenance
        .evidence
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert!(evidence.contains(&tb4_delegate_dev::REPLAYS));
}

#[test]
fn a_development_row_whose_verdict_the_numbers_dont_support_refuses_to_build() {
    let dir = tempfile::tempdir().unwrap();
    let exp = "bench/terminal-bench/experiments/2026-09-27-fable-delegate";
    let mut files: Vec<String> = [
        "attempts.json",
        "attempts-series2-5.json",
        "attempts-series6-7.json",
    ]
    .iter()
    .map(|f| format!("{exp}/{f}"))
    .collect();
    files.push(tb4_delegate_dev::REPLAYS.to_owned());
    files
        .push("bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/tasks.json".into());
    files.push("docs/terminal-bench/2026-09-27-fable-delegate.md".into());
    let row_files: Vec<String> = files[..3].to_vec();
    for f in row_files {
        let rows: Value = serde_json::from_slice(&std::fs::read(root().join(f)).unwrap()).unwrap();
        for row in rows.as_array().unwrap() {
            files.push(format!(
                "bench/terminal-bench/traces/{}/{}.episode/trajectory.atif.json",
                row["job"].as_str().unwrap(),
                row["trial"].as_str().unwrap()
            ));
        }
    }
    for rel in &files {
        let to = dir.path().join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), to).unwrap();
    }
    tb4_delegate_dev::build(&Reader::new(dir.path())).expect("the copied rows build");
    // s6a1 passed $0.042 over the cost bar; claim it beat.
    edit_json(
        &dir.path().join(format!("{exp}/attempts-series6-7.json")),
        |v| {
            v[0]["beat_the_bar"] = Value::Bool(true);
        },
    );
    let err = tb4_delegate_dev::build(&Reader::new(dir.path())).unwrap_err();
    assert!(err.0.contains("recomputed beat false"), "{err}");
}

#[test]
fn the_shared_fact_board_says_what_the_showcase_and_the_gym_say() {
    use tb4_microcoder_kb::{BEFORE, WITH};
    let b = board(tb4_microcoder_kb::BOARD_ID);
    assert_eq!(
        b.kind,
        gym_leaderboard::contract::BoardKind::CostBelowCheapestWin
    );
    let split = |name: &str| b.splits.iter().find(|s| s.name == name).unwrap().tally;

    // The Gym's own claims, recomputed from the same records: the showcase
    // prints them as "8 of 9", "3 of 8", and "5 of 9" graded runs.
    let root = root();
    let knowledge = gym::runs_microcoder::Knowledge::read(&root.join("knowledge"));
    let runs = gym::runs_microcoder::read_all(
        &[root.join(tb4_microcoder_kb::RUNS)],
        &knowledge,
        i64::MAX / 4,
    );
    let replays: Value =
        serde_json::from_slice(&std::fs::read(root.join(tb4_delegate_dev::REPLAYS)).unwrap())
            .unwrap();
    let highlights = gym::runs_beats_winner::beats_winner(&gym::runs_highlights::Inputs {
        runs: &runs,
        answers: &std::collections::HashMap::new(),
        reference: None,
        marks: &gym::runs_marks::Marks::default(),
        fable: Some(&replays),
    });
    for (task, passes, graded, low, high, bar) in [
        ("embedding-drift-monitor", 8, 9, 0.0165, 0.34, 0.74),
        ("fin-saccr-rwa", 3, 8, 0.0404, 0.0518, 1.22),
        ("gsea-proteomics", 5, 9, 0.0491, 0.0691, 0.69),
    ] {
        let with = split(&format!("{task}, {WITH}"));
        assert_eq!(
            (with.passes, with.beats, with.attempts),
            (passes, passes, graded),
            "{task}"
        );
        let claim = highlights
            .iter()
            .find(|h| h.task.as_deref() == Some(task) && h.claim.starts_with("[in-sample"))
            .unwrap();
        assert!(
            claim.claim.contains(&format!(
                "passed {task} in {passes} of {graded} graded runs"
            )),
            "{}",
            claim.claim
        );
        let costs: Vec<f64> = b
            .attempts
            .iter()
            .filter(|a| a.task == task && a.beat)
            .filter_map(|a| a.cost.known())
            .collect();
        let (min, max) = costs
            .iter()
            .fold((f64::MAX, 0.0_f64), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        assert!(
            (min - low).abs() < 0.00005 && (max - high).abs() < 0.005,
            "{task}: {min} {max}"
        );
        let row = b.tasks.iter().find(|t| t.task == task).unwrap();
        assert!((row.bar.cost_usd.unwrap() - bar).abs() < 0.005, "{task}");
        assert_eq!(row.status, TaskStatus::Beat);
    }
    // Before the entry, no graded run passed; without any knowledge, the
    // showcase's 0 of 6 on embedding-drift-monitor and 0 of 1 on
    // gsea-proteomics.
    assert_eq!(split(BEFORE).passes, 0);
    let off = |task: &str| {
        let set: Vec<_> = b
            .attempts
            .iter()
            .filter(|a| a.task == task && a.labels.contains(&Label::KnowledgeOff))
            .collect();
        (set.iter().filter(|a| a.passed).count(), set.len())
    };
    assert_eq!(off("embedding-drift-monitor"), (0, 6));
    assert_eq!(off("gsea-proteomics"), (0, 1));

    // The same-task caveat is on the board and on every beat's row, and
    // it cites the #9776 reproduction's own numbers.
    let same = b.caveats.iter().find(|c| c.code == "same_task").unwrap();
    assert!(same.text.contains("4 of 28 attempts"), "{}", same.text);
    for a in b.attempts.iter().filter(|a| a.beat) {
        assert!(a.caveats.contains(&"same_task".to_owned()), "{}", a.id);
        assert!(
            a.labels.contains(&Label::InSample) && a.labels.contains(&Label::KnowledgeAssisted)
        );
        assert!(a.trace.as_ref().is_some_and(|t| t.bytes > 0), "{}", a.id);
    }
    for label in [Label::InSample, Label::KnowledgeAssisted] {
        assert!(b.labels.contains(&label));
    }
    // The Codex-login pass is list price; the rest are billed.
    assert!(
        b.attempts
            .iter()
            .any(|a| a.cost_basis == Some(gym_leaderboard::contract::CostBasis::ListPrice))
    );
    assert_eq!(b.spend.basis, gym_leaderboard::contract::CostBasis::Mixed);
    assert!(b.caveats.iter().any(|c| c.code == "retained_records_only"));
}

#[test]
fn the_tb4_out_of_sample_board_says_no_held_out_pass_yet() {
    let b = board(tb4_oos::BOARD_ID);
    assert_eq!(
        (
            b.totals.attempts,
            b.totals.passes,
            b.totals.beats,
            b.totals.faults
        ),
        (72, 0, 0, 2)
    );
    for s in &b.splits {
        assert_eq!((s.tally.attempts, s.tally.passes), (24, 0), "{}", s.name);
    }
    // The negative headline is the code's, from the counts.
    assert_eq!(
        b.headline,
        "No held-out TB4 pass yet: 0 of 72 graded held-out runs passed across 3 rounds (0 of 24 in round 2, 0 of 24 in round 3, 0 of 24 in round 4), so no cost win and no confirmed out-of-sample win."
    );
    assert!(b.labels.contains(&Label::PreRegistered) && b.labels.contains(&Label::OutOfSample));
    // Exclusions and coverage are on the board.
    for code in ["coverage", "exclusions", "fable_fails_pool", "round_1"] {
        assert!(b.caveats.iter().any(|c| c.code == code), "{code}");
    }
    let coverage = &b
        .caveats
        .iter()
        .find(|c| c.code == "coverage")
        .unwrap()
        .text;
    assert!(coverage.contains("freecad-platform-drawing"), "{coverage}");
    assert!(b.caveats.iter().any(|c| c.text.contains("ks-solver-cpp")));
    // Unknown costs stay unknown and never beat.
    for a in b.attempts.iter().filter(|a| a.cost.known().is_none()) {
        assert!(a.misses.contains(&Miss::CostUnknown) && a.labels.contains(&Label::CostBound));
    }
}

/// Copies the TB4 study's inputs into a scratch root.
fn study_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in [
        tb4_oos::RESULTS,
        "docs/terminal-bench/2026-09-26-out-of-sample-study.md",
        tb4_delegate_dev::REPLAYS,
    ] {
        let to = dir.path().join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), to).unwrap();
    }
    dir
}

#[test]
fn a_study_table_the_numbers_dont_support_refuses_to_build() {
    let dir = study_fixture();
    tb4_oos::build(&Reader::new(dir.path())).expect("the copied tables build");
    let path = dir.path().join(tb4_oos::RESULTS);
    let page = std::fs::read_to_string(&path).unwrap();
    // A failing run whose row claims a cost win.
    let row = page
        .lines()
        .find(|l| l.starts_with("| `atrx-vep-crispr`") && l.ends_with("| no | no |"))
        .unwrap();
    let edited = page.replacen(row, &row.replace("| no | no |", "| yes | no |"), 1);
    std::fs::write(&path, edited).unwrap();
    let err = tb4_oos::build(&Reader::new(dir.path())).unwrap_err();
    assert!(err.0.contains("recomputed Cost win false"), "{err}");
    // A generated tally that the rows don't add up to.
    let edited = page.replacen(
        "**So far:** 0 passes in 24 graded held-out runs",
        "**So far:** 1 passes in 24 graded held-out runs",
        1,
    );
    std::fs::write(&path, edited).unwrap();
    let err = tb4_oos::build(&Reader::new(dir.path())).unwrap_err();
    assert!(err.0.contains("generated tallies"), "{err}");
}

#[test]
fn reference_boards_are_labeled_snapshots_and_never_merged() {
    use gym_leaderboard::contract::BoardKind;
    for (id, rel) in reference_boards::SNAPSHOTS {
        let b = board(id);
        assert_eq!(b.kind, BoardKind::Reference);
        let snap = b.snapshot.as_ref().expect("a snapshot");
        assert_eq!(snap.host, "hub.harborframework.com");
        let date = &snap.fetched_at[..10];
        // Snapshot, fetch time, and host, in the headline and a caveat.
        assert!(
            b.headline.contains("snapshot")
                && b.headline.contains(date)
                && b.headline.contains(&snap.host),
            "{}",
            b.headline
        );
        let caveat = &b
            .caveats
            .iter()
            .find(|c| c.code == "snapshot")
            .unwrap()
            .text;
        assert!(caveat.contains("not live") && caveat.contains(&snap.fetched_at));
        assert!(b.caveats.iter().any(|c| c.code == "reconciliation"));
        // No beats, attempts, or tasks of its own.
        assert!(b.attempts.is_empty() && b.tasks.is_empty() && b.splits.is_empty());
        assert_eq!(b.totals, gym_leaderboard::contract::Tally::default());
        let doc: Value = serde_json::from_slice(&std::fs::read(root().join(rel)).unwrap()).unwrap();
        let entries = doc["entries"].as_array().unwrap();
        assert_eq!(b.reference_rows.len(), entries.len());
        for (row, entry) in b.reference_rows.iter().zip(entries) {
            assert_eq!(
                row.per_task_consistent,
                entry["per_task"]["consistent"].as_bool()
            );
            assert_eq!(
                row.per_task_cost_consistent,
                entry["per_task"]["cost_consistent"].as_bool()
            );
        }
    }
    let tb4 = board(reference_boards::SNAPSHOTS[0].0);
    assert_eq!(
        tb4.reference_rows
            .iter()
            .filter(|r| r.per_task_consistent == Some(false))
            .count(),
        1
    );
    // Subject boards carry no reference rows.
    for b in &output().leaderboard.boards {
        if b.kind != BoardKind::Reference {
            assert!(
                b.reference_rows.is_empty() && b.snapshot.is_none(),
                "{}",
                b.id
            );
        }
    }
}

#[test]
fn the_9776_board_is_unchanged_after_the_port() {
    // Pinned from the hand-written adapter before #9845 moved the board to
    // a study descriptor and the generic adapter.
    // (The board as the adapter returns it, before bundles are attached;
    // `the_committed_publication_matches_the_evidence` covers the rest.)
    let (b, _) = tb4_delegate::build(&Reader::new(root())).unwrap();
    assert_eq!(
        atif::digest(&serde_json::to_value(&b).unwrap()),
        "4dbcfaf27d6a02381d66fd6cea31a887ba8386ba39e607ab4766f2aabef7d74f"
    );
}

const FIXTURE: &str = "bench/terminal-bench/experiments/2099-01-01-fixture";

fn fixture_row(
    id: &str,
    task: &str,
    series: &str,
    reward: f64,
    usd: Option<f64>,
    verdict: bool,
) -> Value {
    serde_json::json!({
        "id": id, "task": task, "series": series, "trial": format!("trial-{id}"),
        "reward": reward, "seconds": 100.0,
        "cost": match usd {
            Some(usd) => serde_json::json!({"kind": "reported", "usd": usd}),
            None => serde_json::json!({"kind": "unknown", "lower_bound_usd": 0.4, "upper_bound_usd": null}),
        },
        "bar": {"cost_usd": 1.0, "seconds": null, "cost_trial": "ref-1", "time_trial": null,
                "deadline_seconds": null, "reference_passes": 3, "reference_trials": 5},
        "verdict": verdict,
        "knowledge": {"kept": ["a.entry"], "own": ["a.entry"]},
        "how_it_ended": "finished",
    })
}

/// A scratch root holding only a study descriptor and its rows.
fn study_only(edit: impl FnOnce(&mut Value, &mut Value)) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut descriptor = serde_json::json!({
        "schema": "openagents.gym.study.v1",
        "board": "fixture-study",
        "title": "A fixture study",
        "benchmark": {"name": "Terminal-Bench", "version": "4.0"},
        "question": "Does the fixture pass under the bar?",
        "issues": [9845],
        "subject": {"agent": "Microcoder", "arm": "on", "model": "gpt-6-luna"},
        "reference": {"name": "Fable 5.1 low", "rule": "Below the cheapest win.", "conditions": "Elsewhere."},
        "rule": "cost_below_cheapest_win",
        "labels": ["knowledge_assisted", "list_price"],
        "headline": "Beat {reference}'s cheapest win on {beats} of {attempts} attempts ({series_beats_in}).",
        "caveats": [
            {"code": "in_sample", "text": "{beats_in_sample} of {beats} beats kept knowledge from the same task.", "on_beats": true},
            {"code": "cost_unknown", "when": "cost_unknown", "text": "{cost_unknown} of {attempts} costs are unknown."},
        ],
        "rows": {"path": format!("{FIXTURE}/rows.json")},
    });
    let mut rows = serde_json::json!({
        "schema": "openagents.gym.attempt-row.v1",
        "rows": [
            fixture_row("t1.a1", "t1", "screen", 1.0, Some(0.2), true),
            fixture_row("t1.a2", "t1", "screen", 0.0, Some(0.3), false),
            fixture_row("t2.a1", "t2", "screen", 1.0, None, false),
        ],
        "tallies": {"screen": {"attempts": 3, "passes": 2, "beats": 1, "faults": 0, "cost_unknown": 1}},
    });
    edit(&mut descriptor, &mut rows);
    std::fs::create_dir_all(dir.path().join(FIXTURE)).unwrap();
    std::fs::write(
        dir.path().join(format!("{FIXTURE}/study.json")),
        descriptor.to_string(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join(format!("{FIXTURE}/rows.json")),
        rows.to_string(),
    )
    .unwrap();
    dir
}

#[test]
fn a_study_with_only_a_descriptor_and_rows_generates_a_board() {
    let dir = study_only(|_, _| {});
    let reader = Reader::new(dir.path());
    assert_eq!(
        study::discover(&reader).unwrap(),
        vec![format!("{FIXTURE}/study.json")]
    );
    let (b, jobs) = study::build(&reader, &format!("{FIXTURE}/study.json")).unwrap();
    assert!(jobs.is_empty());
    assert_eq!(b.id, "fixture-study");
    assert_eq!(
        (
            b.totals.attempts,
            b.totals.passes,
            b.totals.beats,
            b.totals.cost_unknown
        ),
        (3, 2, 1, 1)
    );
    assert_eq!(
        b.headline,
        "Beat Fable 5.1 low's cheapest win on 1 of 3 attempts (1 of 3 in screen)."
    );
    assert_eq!(
        b.caveats[0].text,
        "1 of 1 beats kept knowledge from the same task."
    );
    assert_eq!(b.caveats[1].text, "1 of 3 costs are unknown.");
    let beat = b.attempts.iter().find(|a| a.beat).unwrap();
    assert_eq!(beat.caveats, vec!["in_sample".to_owned()]);
    assert!(beat.labels.contains(&Label::InSample));
    // The unknown cost doesn't beat, and the board says it's a bound.
    let unknown = b.attempts.iter().find(|a| a.id == "t2.a1").unwrap();
    assert!(!unknown.beat && unknown.misses == vec![Miss::CostUnknown]);
    assert!(b.labels.contains(&Label::CostBound));
    assert_eq!(b.tasks.len(), 2);
    assert_eq!(b.provenance.evidence.len(), 1);
}

#[test]
fn a_study_that_names_an_unknown_rule_or_disagrees_with_its_rows_refuses() {
    let build = |dir: &tempfile::TempDir| {
        study::build(&Reader::new(dir.path()), &format!("{FIXTURE}/study.json")).unwrap_err()
    };
    let err = build(&study_only(|d, _| {
        d["rule"] = Value::from("cheaper_than_yesterday")
    }));
    assert!(err.0.contains("unknown rule"), "{err}");
    let err = build(&study_only(|d, _| d["rule"] = Value::from("reference")));
    assert!(err.0.contains("unknown rule"), "{err}");
    let err = build(&study_only(|_, r| {
        r["rows"][1]["verdict"] = Value::Bool(true)
    }));
    assert!(err.0.contains("recomputed beat false"), "{err}");
    let err = build(&study_only(|_, r| {
        r["tallies"]["screen"]["beats"] = Value::from(2)
    }));
    assert!(err.0.contains("beats"), "{err}");
    let err = build(&study_only(|d, _| {
        d["headline"] = Value::from("{beats} of {everything}")
    }));
    assert!(err.0.contains("unknown placeholder"), "{err}");
    let err = build(&study_only(|_, r| {
        r["rows"][1]["bar"]["cost_usd"] = Value::from(2.0)
    }));
    assert!(err.0.contains("differs from another row"), "{err}");
}
