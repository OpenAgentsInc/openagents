use serde_json::json;

use super::card::{Budget, Card, Phase, Pitfall, Span};
use super::events::{self, Follower, Kind};
use super::judge::{self, Judgment, Rules, Run};

fn card() -> Card {
    Card {
        schema: super::card::SCHEMA.to_string(),
        task: "example".to_string(),
        strategy: "Read, check, fix, verify.".to_string(),
        phases: vec![
            Phase {
                id: "read".to_string(),
                what: "Read the files.".to_string(),
                fable_seconds: Span {
                    start: 0.0,
                    end: 30.0,
                },
                ..Phase::default()
            },
            Phase {
                id: "fix".to_string(),
                what: "Fix the code.".to_string(),
                fable_seconds: Span {
                    start: 30.0,
                    end: 90.0,
                },
                ..Phase::default()
            },
        ],
        pitfalls: vec![Pitfall {
            id: "self_check".to_string(),
            what: "Trusts its own check.".to_string(),
        }],
        budget: Budget {
            first_read_done_s: 30.0,
            first_edit_s: 40.0,
            first_check_s: 20.0,
            done_s: 100.0,
        },
        ..Card::default()
    }
}

fn line(value: serde_json::Value) -> String {
    value.to_string()
}

fn tool(at: u64, name: &str, arguments: serde_json::Value, output: &str) -> String {
    line(
        json!({"record": "step", "step": {"at": at, "source": "Agent", "message": "",
        "call": {"id": "c", "name": name, "arguments": arguments, "output": output}}}),
    )
}

#[test]
fn parses_every_record_kind() {
    let session = line(
        json!({"record": "session", "at": 5, "session": {"id": "s", "model": "m", "directive": "d"}}),
    );
    assert!(matches!(
        events::parse(&session, "episode").unwrap().kind,
        Kind::Session { .. }
    ));
    let start = line(
        json!({"record": "step", "step": {"at": 6, "source": "System", "message": "invocation inv-2 started",
        "extensions": {"invocation": {"event": "start", "id": "inv-2", "parent": "inv-1", "component": "task.requirements",
        "name": "requirement map", "implementation": {"name": "span kinds by Jev", "digest": "abcdef0123456789"}}}}}),
    );
    match events::parse(&start, "episode").unwrap().kind {
        Kind::Start {
            component,
            implementation,
            ..
        } => {
            assert_eq!(component, "task.requirements");
            assert_eq!(implementation, "span kinds by Jev");
        }
        other => panic!("{other:?}"),
    }
    let jev = line(
        json!({"record": "step", "step": {"at": 7, "source": "Agent", "message": "", "milliseconds": 900,
        "call": {"id": "j", "name": "jev_probe", "arguments": {"state": {"a": 1}, "questions": {"q": {}}},
        "output": "{\"q\":{\"type\":\"noul\",\"noul\":0.9}}"},
        "extensions": {"jev_usage": {"input_tokens": 1000}}}}),
    );
    match events::parse(&jev, "episode").unwrap().kind {
        Kind::Jev {
            answers,
            input_tokens,
            ..
        } => {
            assert_eq!(answers["q"]["noul"], 0.9);
            assert_eq!(input_tokens, Some(1000));
        }
        other => panic!("{other:?}"),
    }
    let think = line(
        json!({"record": "step", "step": {"at": 8, "source": "Agent", "message": "", "reasoning": "**Plan**",
        "tokens": [100, 20], "milliseconds": 1500, "extensions": {"microluna.usage.v1": {"cost_usd": 0.002}}}}),
    );
    assert!(
        matches!(events::parse(&think, "microluna-1-1").unwrap().kind, Kind::Think { usd, .. } if (usd - 0.002).abs() < 1e-9)
    );
    let call = tool(9, "run_command", json!({"command": "ls"}), "[exit 0]\na");
    let event = events::parse(&call, "microluna-1-1").unwrap();
    assert!(event.is_action());
    let end = line(json!({"record": "end", "at": 10, "state": "ended"}));
    assert!(matches!(
        events::parse(&end, "episode").unwrap().kind,
        Kind::End { .. }
    ));
}

#[test]
fn the_follower_reads_only_complete_new_lines() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("artifacts")).unwrap();
    let log = dir.path().join("artifacts/microluna-1-1.atif.jsonl");
    let first = tool(1, "run_command", json!({"command": "ls"}), "a");
    std::fs::write(&log, format!("{first}\n{{\"record\":")).unwrap();
    let mut follower = Follower::default();
    assert_eq!(follower.poll(dir.path()).len(), 1);
    assert!(follower.poll(dir.path()).is_empty());
    let second = tool(2, "run_command", json!({"command": "pwd"}), "b");
    std::fs::write(&log, format!("{first}\n{second}\n")).unwrap();
    let events = follower.poll(dir.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].at, 2);
}

fn run_with(actions: &[(u64, &str, serde_json::Value, &str)]) -> Run {
    let mut run = Run::default();
    let session = line(
        json!({"record": "session", "at": 1_000, "session": {"id": "s", "model": "m", "directive": "d"}}),
    );
    run.see(&events::parse(&session, "episode").unwrap());
    for (at, name, arguments, output) in actions {
        run.see(
            &events::parse(&tool(*at, name, arguments.clone(), output), "microluna-1-1").unwrap(),
        );
    }
    run
}

#[test]
fn code_rules_stop_a_slow_a_stuck_and_a_repeating_run() {
    let card = card();
    // The idle rule would fire first here; these checks are about the others.
    let rules = Rules {
        idle_s: f64::INFINITY,
        ..Rules::default()
    };
    // Over time: 4 x 100 s.
    let run = run_with(&[(
        2_000,
        "write_file",
        json!({"path": "a", "contents": "x"}),
        "ok",
    )]);
    assert!(judge::rules(&run, &card, &rules, 1_000 + 399_000).is_none());
    assert_eq!(
        judge::rules(&run, &card, &rules, 1_000 + 401_000)
            .unwrap()
            .rule,
        "over_time"
    );
    // No edit past max(4 x 40 s, 120 s).
    let run = run_with(&[(2_000, "run_command", json!({"command": "ls"}), "a")]);
    assert!(judge::rules(&run, &card, &rules, 1_000 + 159_000).is_none());
    assert_eq!(
        judge::rules(&run, &card, &rules, 1_000 + 161_000)
            .unwrap()
            .rule,
        "no_edit"
    );
    // The same command and output three times.
    let same = json!({"command": "python3 check.py"});
    let run = run_with(&[
        (
            2_000,
            "write_file",
            json!({"path": "a", "contents": "x"}),
            "ok",
        ),
        (3_000, "run_command", same.clone(), "FAIL"),
        (4_000, "run_command", same.clone(), "FAIL"),
        (5_000, "run_command", same, "FAIL"),
    ]);
    assert_eq!(
        judge::rules(&run, &card, &rules, 5_000).unwrap().rule,
        "repeating"
    );
}

#[test]
fn five_minutes_without_an_action_stop_the_run() {
    let card = card();
    let rules = Rules::default();
    let run = run_with(&[(
        2_000,
        "write_file",
        json!({"path": "a", "contents": "x"}),
        "ok",
    )]);
    assert!(judge::rules(&run, &card, &rules, 2_000 + 299_000).is_none());
    assert_eq!(
        judge::rules(&run, &card, &rules, 2_000 + 301_000)
            .unwrap()
            .rule,
        "idle"
    );
}

#[test]
fn a_named_pitfall_votes_at_a_lower_stop_answer() {
    let rules = Rules::default();
    let pitfall = Judgment {
        on_track: Some(0.5),
        deviation: Some("none".to_string()),
        pitfall: Some("self_check".to_string()),
        stop: Some(0.72),
        ..Judgment::default()
    };
    assert!(judge::votes(&pitfall, &rules));
    assert!(!judge::votes(
        &Judgment {
            pitfall: None,
            ..pitfall
        },
        &rules
    ));
}

#[test]
fn two_refused_finishes_stop_the_run() {
    let card = card();
    let mut run = run_with(&[(
        2_000,
        "write_file",
        json!({"path": "a", "contents": "x"}),
        "ok",
    )]);
    for at in [3_000, 4_000] {
        let note = line(
            json!({"record": "step", "step": {"at": at, "source": "System",
            "message": "The host turned this finish back: the checks fail."}}),
        );
        run.see(&events::parse(&note, "microluna-1-1").unwrap());
    }
    assert_eq!(
        judge::rules(&run, &card, &Rules::default(), 4_000)
            .unwrap()
            .rule,
        "finish_refused"
    );
}

#[test]
fn jev_stops_only_after_votes_in_a_row() {
    let rules = Rules::default();
    let off = Judgment {
        on_track: Some(0.1),
        deviation: Some("looping".to_string()),
        stop: Some(0.5),
        ..Judgment::default()
    };
    assert!(judge::votes(&off, &rules));
    let good = Judgment {
        on_track: Some(0.1),
        deviation: Some("good".to_string()),
        stop: Some(0.5),
        ..Judgment::default()
    };
    assert!(!judge::votes(&good, &rules));
    let mut run = run_with(&[
        (2_000, "run_command", json!({"command": "a"}), "1"),
        (3_000, "run_command", json!({"command": "b"}), "2"),
        (4_000, "run_command", json!({"command": "c"}), "3"),
        (5_000, "run_command", json!({"command": "d"}), "4"),
        (6_000, "run_command", json!({"command": "e"}), "5"),
    ]);
    // Votes cast before the fifth action don't count.
    run.judgments.push(Judgment {
        vote: true,
        actions: 3,
        ..off.clone()
    });
    run.judgments.push(Judgment {
        vote: true,
        actions: 4,
        ..off.clone()
    });
    assert!(judge::jev_stop(&run, &rules).is_none());
    run.judgments.push(Judgment {
        vote: true,
        actions: 5,
        ..off.clone()
    });
    assert!(judge::jev_stop(&run, &rules).is_none());
    run.judgments.push(Judgment {
        vote: true,
        actions: 6,
        ..off
    });
    assert_eq!(judge::jev_stop(&run, &rules).unwrap().rule, "jev");
}

#[test]
fn two_votes_in_three_or_a_steady_stop_answer_stop_the_run() {
    let rules = Rules::default();
    let mut run = run_with(&[
        (2_000, "run_command", json!({"command": "a"}), "1"),
        (3_000, "run_command", json!({"command": "b"}), "2"),
        (4_000, "run_command", json!({"command": "c"}), "3"),
        (5_000, "run_command", json!({"command": "d"}), "4"),
        (6_000, "run_command", json!({"command": "e"}), "5"),
    ]);
    let quiet = Judgment {
        stop: Some(0.3),
        ..Judgment::default()
    };
    // A vote, a miss, then a vote: two of the last three.
    run.judgments = vec![
        Judgment {
            vote: true,
            actions: 5,
            ..quiet.clone()
        },
        Judgment {
            actions: 6,
            ..quiet.clone()
        },
    ];
    assert!(judge::jev_stop(&run, &rules).is_none());
    run.judgments.push(Judgment {
        vote: true,
        actions: 7,
        ..quiet.clone()
    });
    assert!(
        judge::jev_stop(&run, &rules)
            .unwrap()
            .why
            .contains("2 of its last 3")
    );
    // A stop answer of 0.7 or more four times in a row, with no votes.
    let high = Judgment {
        stop: Some(0.73),
        ..quiet
    };
    run.judgments = (5..8)
        .map(|n| Judgment {
            actions: n,
            ..high.clone()
        })
        .collect();
    assert!(judge::jev_stop(&run, &rules).is_none());
    run.judgments.push(Judgment { actions: 8, ..high });
    assert!(
        judge::jev_stop(&run, &rules)
            .unwrap()
            .why
            .contains("in a row")
    );
}

#[test]
fn the_request_carries_the_card_and_the_recent_actions() {
    let card = card();
    let run = run_with(&[(
        2_000,
        "run_command",
        json!({"command": "cat app/main.py"}),
        "print(1)",
    )]);
    let (state, questions) = judge::request(&run, &card, 3_000);
    assert_eq!(state["phases"][1]["id"], "fix");
    assert_eq!(state["recent_actions"][0]["input"], "cat app/main.py");
    let questions = serde_json::to_value(&questions).unwrap();
    for id in ["on_track", "phase", "deviation", "pitfall", "stop"] {
        assert!(questions.get(id).is_some(), "{id} missing from {questions}");
    }
}

#[test]
fn edits_are_writes_patches_and_in_place_commands() {
    assert!(judge::is_edit("write_file", "a"));
    assert!(judge::is_edit("run_command", "sed -i 's/a/b/' x.py"));
    assert!(!judge::is_edit("run_command", "python3 x.py"));
}

#[tokio::test]
async fn replay_with_jev_off_stops_by_code_rules_and_writes_a_report() {
    let trial = tempfile::tempdir().unwrap();
    let logs = trial.path().join("agent/episode");
    std::fs::create_dir_all(logs.join("artifacts")).unwrap();
    let session = line(
        json!({"record": "session", "at": 1_000, "session": {"id": "e", "model": "m", "directive": "d"}}),
    );
    let task = line(
        json!({"record": "step", "step": {"at": 1_001, "source": "User", "message": "Fix the bug."}}),
    );
    std::fs::write(
        logs.join("episode.atif.jsonl"),
        format!("{session}\n{task}\n"),
    )
    .unwrap();
    let mut lines = Vec::new();
    for n in 0..5u64 {
        lines.push(tool(
            2_000 + n * 60_000,
            "run_command",
            json!({"command": format!("ls {n}")}),
            "x",
        ));
    }
    std::fs::write(
        logs.join("artifacts/microluna-1-1.atif.jsonl"),
        lines.join("\n") + "\n",
    )
    .unwrap();
    std::fs::create_dir_all(trial.path().join("verifier")).unwrap();
    std::fs::write(trial.path().join("verifier/reward.txt"), "0\n").unwrap();
    std::fs::write(trial.path().join("result.json"), "{}").unwrap();
    let card_path = trial.path().join("card.json");
    std::fs::write(&card_path, serde_json::to_string(&card()).unwrap()).unwrap();
    let out = trial.path().join("out");
    let args: Vec<String> = [
        "replay",
        "--card",
        card_path.to_str().unwrap(),
        "--jev",
        "off",
        "--out",
        out.to_str().unwrap(),
        trial.path().to_str().unwrap(),
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let code = super::command(&args).await.unwrap();
    assert_eq!(code, 3);
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["stop"]["rule"], "no_edit");
    assert_eq!(report["reward"], 0.0);
    assert!(
        std::fs::read_to_string(out.join("report.md"))
            .unwrap()
            .contains("no_edit")
    );
}

#[test]
fn scores_come_from_score_lines_and_reach_the_request() {
    assert_eq!(
        judge::scores_in("x\nSCORE 228 780\nSCORE 0 0\nSCORE a b"),
        vec![(228.0, 780.0), (0.0, 0.0)]
    );
    let run = run_with(&[(
        2_000,
        "run_command",
        json!({"command": "score.sh"}),
        "[exit 0]\nSCORE 155 780",
    )]);
    let (state, _) = judge::request(&run, &card(), 3_000);
    assert_eq!(state["score_history"][0]["score"], "155/780");
}

#[test]
fn votes_before_a_turned_back_finish_do_not_count() {
    let rules = Rules::default();
    let mut run = run_with(&[
        (2_000, "run_command", json!({"command": "a"}), "1"),
        (3_000, "run_command", json!({"command": "b"}), "2"),
        (4_000, "run_command", json!({"command": "c"}), "3"),
        (5_000, "run_command", json!({"command": "d"}), "4"),
        (6_000, "run_command", json!({"command": "e"}), "5"),
    ]);
    let vote = Judgment {
        vote: true,
        stop: Some(0.9),
        ..Judgment::default()
    };
    run.judgments.push(Judgment {
        t: 5.0,
        actions: 5,
        ..vote.clone()
    });
    let note = line(
        json!({"record": "step", "step": {"at": 7_000, "source": "System",
        "message": "The host turned this finish back: keep working."}}),
    );
    run.see(&events::parse(&note, "microluna-1-1").unwrap());
    run.judgments.push(Judgment {
        t: 7.0,
        actions: 6,
        ..vote.clone()
    });
    assert!(judge::jev_stop(&run, &rules).is_none());
    run.judgments.push(Judgment {
        t: 8.0,
        actions: 7,
        ..vote
    });
    assert!(judge::jev_stop(&run, &rules).is_some());
}

#[test]
fn the_fire_experiment_protocol_is_frozen_at_its_digest() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for experiment in super::experiment::EXPERIMENTS {
        let bytes = std::fs::read(root.join(experiment.protocol)).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            digest, experiment.protocol_sha256,
            "{}",
            experiment.protocol
        );
    }
}
