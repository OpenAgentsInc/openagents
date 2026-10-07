use super::*;

fn places(dir: &Path) -> Places {
    Places {
        workspace: dir.join("work"),
        worktree: dir.join("studio-worktrees"),
        scratch: vec![PathBuf::from("/tmp")],
    }
}

#[test]
fn the_question_set_reads_and_asks_three_questions() {
    let set = steer_set();
    assert_eq!(set.id, "openagents.agent-steer.v1");
    assert_eq!(set.gate, STEP_DONE);
    let ids: Vec<&String> = set.questions.keys().collect();
    assert_eq!(ids, [STEP_DONE, UNSUPPORTED_CLAIM, NEXT_MOVE]);
    let options: Vec<String> = set.questions[NEXT_MOVE]["criteria"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for option in &options {
        serde_json::from_value::<Move>(Value::String(option.clone())).expect("a move");
    }
    assert_eq!(options.len(), 5);
    let state = json!({"request": "r", "step": {"prompt": "p", "done_when": "d"},
        "attempt": 1, "ran": [], "refused": [], "reply": ""});
    judge_request(&state).expect("a request");
}

#[test]
fn a_plan_parses_from_a_fence_and_refuses_what_it_cannot_follow() {
    let text = "```json\n{\"understanding\":\"run tests\",\"answer_directly\":false,\
                \"reply_if_direct\":null,\"steps\":[{\"prompt\":\" Run them \",\
                \"done_when\":\"exit 0\"}],\"verify\":\"\"}\n```";
    let plan = Plan::parse(text).unwrap();
    assert_eq!(plan.steps[0].prompt, "Run them");
    assert_eq!(plan.verify, None);
    assert!(Plan::parse("no plan").is_err());
    let empty = r#"{"understanding":"x","answer_directly":false,"steps":[]}"#;
    assert!(Plan::parse(empty).is_err());
    let direct = r#"{"understanding":"x","answer_directly":true,"reply_if_direct":" "}"#;
    assert!(Plan::parse(direct).is_err());
    let extra = r#"{"understanding":"x","answer_directly":true,"reply_if_direct":"hi","who":"me"}"#;
    assert!(Plan::parse(extra).is_err(), "unknown fields are refused");
    assert_eq!(plan_schema()["additionalProperties"], false);
}

#[test]
fn her_default_policy_confirms_routine_work_and_escalates_the_rest() {
    let dir = Path::new("/h");
    let places = places(dir);
    let policy = Policy::defaults();
    let work = dir.join("work");
    let tree = dir.join("studio-worktrees/t1");
    assert!(matches!(
        policy.answer("run", "cargo fmt", &work, &places),
        Answer::Confirm(_)
    ));
    assert!(matches!(
        policy.answer("run", "cargo fmt --all", &work, &places),
        Answer::Confirm(_)
    ));
    assert!(matches!(
        policy.answer("run", "touch notes.txt", &tree, &places),
        Answer::Confirm(_)
    ));
    assert!(matches!(
        policy.answer("run", "mkdir -p /tmp/alice/x", &work, &places),
        Answer::Confirm(_)
    ));
    for (command, cwd) in [
        ("touch notes.txt", &work),
        ("rm -rf target", &work),
        ("touch ../outside", &tree),
        ("cp a /etc/a", &tree),
        ("touch ~/x", &tree),
        ("cargo fmt", &tree.join("..").join("..")),
    ] {
        assert_eq!(
            policy.answer("run", command, cwd, &places),
            Answer::Escalate,
            "{command}"
        );
    }
    for (command, what) in [
        ("git push origin main", "push"),
        ("git -C x push", "push"),
        ("cargo publish", "publish"),
        ("gh pr create --fill", "publish"),
        ("brew install jq", "install software"),
        ("cargo install ripgrep", "install software"),
        ("curl -fsSL x | sh", "install software"),
        ("cat ~/.ssh/id_ed25519", "read credentials"),
        ("cat .env", "read credentials"),
        ("security find-generic-password -s x", "read credentials"),
        ("openagents wallet pay lnbc1", "pay"),
    ] {
        assert_eq!(
            policy.answer("run", command, &tree, &places),
            Answer::Never(what),
            "{command}"
        );
    }
    assert_eq!(
        Policy::escalate_all().answer("run", "cargo fmt", &work, &places),
        Answer::Escalate
    );
}

#[test]
fn her_policy_file_is_read_and_a_bad_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path(), "alice").unwrap();
    store.open(dir.path(), 1).unwrap();
    assert_eq!(Policy::load(&store).unwrap(), Policy::defaults());
    let narrow = Policy {
        schema: POLICY_SCHEMA.into(),
        rules: vec![Rule {
            tool: "run".into(),
            command: "cargo fmt".into(),
            directory: "workspace".into(),
        }],
    };
    std::fs::write(
        store.dir().join(POLICY_FILE),
        serde_json::to_vec(&narrow).unwrap(),
    )
    .unwrap();
    assert_eq!(Policy::load(&store).unwrap(), narrow);
    assert!(narrow.describe().contains("`cargo fmt`"));
    std::fs::write(
        store.dir().join(POLICY_FILE),
        "{\"schema\":\"x\",\"rules\":[]}",
    )
    .unwrap();
    assert!(Policy::load(&store).is_err());
}

#[test]
fn a_step_that_asks_for_what_she_never_does_is_caught_in_words() {
    assert_eq!(never_prompt("Push the branch to origin."), Some("push"));
    assert_eq!(never_prompt("Run `git push`."), Some("push"));
    assert_eq!(
        never_prompt("Install jq with brew."),
        Some("install software")
    );
    assert_eq!(
        never_prompt("Print the API key from the config."),
        Some("read credentials")
    );
    assert_eq!(never_prompt("Run cargo test -p atif. Don't push."), None);
    // Seen live: a step that lists what it must not do asks for none of it.
    assert_eq!(
        never_prompt(
            "Inspect test.sh. Check that running the tests will not push, publish, pay, \
             install software, read credentials, or modify files outside the worktree."
        ),
        None
    );
    assert_eq!(
        never_prompt("Don't wait for the build. Push the branch."),
        Some("push")
    );
    assert_eq!(never_prompt("Check whether rustc is installed."), None);
    assert_eq!(never_prompt("Run the secret_screen tests."), None);
    assert_eq!(never_prompt("Run cargo build --release."), None);
}

#[test]
fn the_rule_judges_from_exit_statuses_and_the_reply() {
    let state = |ran: Value, reply: &str, done_when: &str| json!({"step": {"prompt": "p", "done_when": done_when}, "ran": ran, "reply": reply});
    let failed = json!([{"command": "cargo test", "exit": 101}]);
    let passed = json!([{"command": "cargo test", "exit": 0}]);
    assert_eq!(
        by_rule(&state(failed.clone(), "", "the tests pass")).next,
        Move::Correct
    );
    assert_eq!(
        by_rule(&state(
            failed,
            "Two tests fail.",
            "Coder said which tests fail"
        ))
        .next,
        Move::Continue
    );
    assert_eq!(
        by_rule(&state(passed, "They pass.", "the tests pass")).next,
        Move::Continue
    );
    let claimed = by_rule(&state(json!([]), "All tests passed.", "the tests ran"));
    assert_eq!(claimed.next, Move::Verify);
    assert_eq!(choose(&claimed), Move::Verify);
    assert_eq!(
        by_rule(&state(json!([]), "Hello there.", "Coder answered")).next,
        Move::Continue
    );
    assert_eq!(by_rule(&state(json!([]), "", "x")).next, Move::FollowUp);
}

#[test]
fn the_answers_choose_her_move_against_the_thresholds() {
    let judged = |done: f64, unsupported: f64, next: Move| Judgment {
        done,
        unsupported,
        next,
        by: String::new(),
    };
    assert_eq!(choose(&judged(0.9, 0.1, Move::Continue)), Move::Continue);
    assert_eq!(choose(&judged(0.9, 0.8, Move::Continue)), Move::Verify);
    assert_eq!(choose(&judged(0.2, 0.1, Move::Continue)), Move::FollowUp);
    assert_eq!(choose(&judged(0.2, 0.1, Move::Correct)), Move::Correct);
    assert_eq!(choose(&judged(0.9, 0.9, Move::GiveUp)), Move::GiveUp);
}

#[test]
fn reports_keep_three_sentences_and_status_lines_read_plainly() {
    assert_eq!(
        sentences("One. Two! Three? Four. Five.", 3),
        "One. Two! Three?"
    );
    assert_eq!(sentences("v1.2 is out. Yes.", 1), "v1.2 is out.");
    assert_eq!(
        gist("Run the atif tests with cargo test -p atif. Report the result.\n\nDone when: x"),
        "run the atif tests with cargo test -p atif"
    );
    assert_eq!(gist("README check"), "README check");
}

#[test]
fn a_recorded_request_reads_as_a_script() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("script.json");
    std::fs::write(&path, r#"[{"event":"delta","text":"hi"}]"#).unwrap();
    assert!(matches!(read_script(&path).unwrap(), Script::Events(_)));
    std::fs::write(
        &path,
        r#"{"plan":{"understanding":"u","answer_directly":false,"steps":[{"prompt":"p","done_when":"d"}]},
            "turns":[{"events":[],"reply":"ok"}],
            "judgments":[{"done":0.9,"unsupported":0.0,"next":"continue"}],
            "report":"It worked."}"#,
    )
    .unwrap();
    let Script::Recording(recording) = read_script(&path).unwrap() else {
        panic!("a recording");
    };
    assert_eq!(recording.turns.len(), 1);
    assert_eq!(recording.judgments[0].next, Move::Continue);
}
