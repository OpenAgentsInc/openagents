//! The floors' tests: what is floored, how each rule decides, the
//! hook on each harness, the permission table, and pins.

use super::*;
use std::sync::atomic::AtomicBool;

fn ability(tool: &str, input: Value) -> Option<&'static str> {
    classify(tool, &input).map(|floor| floor.ability)
}

fn bash(command: &str) -> Option<&'static str> {
    ability("Bash", json!({ "command": command }))
}

#[test]
fn the_four_floors_are_found_in_commands_and_reads() {
    // A production deploy.
    assert_eq!(
        bash("openagents deploy production sha256:abc"),
        Some(risk_policy::DEPLOY_PRODUCTION)
    );
    assert_eq!(
        bash("cd apps/web && npx wrangler deploy"),
        Some(risk_policy::DEPLOY_PRODUCTION)
    );
    assert_eq!(
        bash("gcloud run deploy web --source ."),
        Some(risk_policy::DEPLOY_PRODUCTION)
    );
    assert_eq!(
        bash("pnpm exec wrangler deploy"),
        Some(risk_policy::DEPLOY_PRODUCTION)
    );
    assert_eq!(bash("wrangler deploy --env staging"), None);
    assert_eq!(bash("wrangler deploy --dry-run"), None);
    assert_eq!(
        bash("openagents deploy production sha256:abc --approval A1"),
        None,
        "an approved deploy carries its approval; the CLI checks it"
    );
    // Merging a pull request.
    assert_eq!(bash("gh pr merge 42 --squash"), Some(risk_policy::PR_MERGE));
    assert_eq!(
        bash("gh api -X PUT repos/o/r/pulls/42/merge"),
        Some(risk_policy::PR_MERGE)
    );
    assert_eq!(bash("gh pr view 42"), None);
    // A secret read.
    assert_eq!(
        bash("gcloud secrets versions access latest --secret=stripe"),
        Some(risk_policy::SECRETS)
    );
    assert_eq!(
        bash("cat .secrets/probe-openai.env"),
        Some(risk_policy::SECRETS)
    );
    assert_eq!(
        bash("security find-generic-password -s gh -w"),
        Some(risk_policy::SECRETS)
    );
    assert_eq!(
        ability("Read", json!({"file_path": "/repo/.env"})),
        Some(risk_policy::SECRETS)
    );
    assert_eq!(ability("Read", json!({"path": "/repo/.env.example"})), None);
    assert_eq!(
        ability("Read", json!({"file_path": "/repo/src/env.rs"})),
        None
    );
    // A payment.
    assert_eq!(
        bash("openagents wallet send lnbc1... --amount 100"),
        Some(risk_policy::PAYMENTS)
    );
    assert_eq!(
        bash("lncli payinvoice lnbc1..."),
        Some(risk_policy::PAYMENTS)
    );
    assert_eq!(
        ability(
            "openagents_cli",
            json!({"arguments": ["wallet", "pay", "lnbc1"]})
        ),
        Some(risk_policy::PAYMENTS)
    );
    // Through wrappers and chains.
    assert_eq!(
        bash("cargo test && FOO=1 sudo gh pr merge 7"),
        Some(risk_policy::PR_MERGE)
    );
    // Everyday work is not floored, pushes to main included (this
    // repository's agents commit straight to main).
    for command in [
        "cargo test -p coder-new",
        "git push origin HEAD:main",
        "cat README.md",
        "echo 'gh pr merge 1'",
        "grep -rn deploy docs",
    ] {
        assert_eq!(bash(command), None, "{command}");
    }
}

fn checkout(policy: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(dir.path())
        .status()
        .unwrap();
    if let Some(rules) = policy {
        std::fs::create_dir_all(dir.path().join(".openagents")).unwrap();
        std::fs::write(
            dir.path().join(".openagents/approvals.json"),
            format!(
                r#"{{"schema":"{}","rules":{rules}}}"#,
                risk_policy::POLICY_SCHEMA
            ),
        )
        .unwrap();
    }
    dir
}

fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// With nobody to answer, a floored call is held; a person on the
/// approval card lets it run or says no; the project's policy decides
/// what asks, and a file never lowers a floor.
#[test]
fn the_same_rules_hold_ask_or_refuse() {
    let _lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let repo = checkout(None);
    let cancel = AtomicBool::new(false);
    let merge = json!({"command": "gh pr merge 42"});
    install_desk(None);
    let held_call = block(hold("Coder", "Run", &merge, repo.path(), &cancel)).unwrap_err();
    assert!(held_call.starts_with("Held for the person: this would merge a pull request"));
    assert!(
        block(hold(
            "Coder",
            "Run",
            &json!({"command": "cargo test"}),
            repo.path(),
            &cancel
        ))
        .is_ok()
    );

    // A person answers on the card.
    for (confirm, expect_ok) in [(true, true), (false, false)] {
        let desk = crate::approval::Desk::new();
        install_desk(Some(desk.clone()));
        let answering = desk.clone();
        let answer = std::thread::spawn(move || {
            for _ in 0..200 {
                if let Some(event) = answering
                    .drain()
                    .into_iter()
                    .find(|event| event["event"] == "approval")
                {
                    assert_eq!(event["kind"], "action");
                    assert_eq!(event["title"], "Let Coder merge a pull request?");
                    assert_eq!(event["subject"], "gh pr merge 42");
                    let word = if confirm { "confirm" } else { "reject" };
                    answering
                        .answer(&format!("{word} {}", event["id"]))
                        .unwrap();
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("no question");
        });
        let result = block(hold("Coder", "Run", &merge, repo.path(), &cancel));
        answer.join().unwrap();
        assert_eq!(result.is_ok(), expect_ok, "{result:?}");
        if !expect_ok {
            assert!(result.unwrap_err().starts_with("The person said no"));
        }
    }
    install_desk(None);

    // The project may let merges run, or refuse them outright; a file
    // cannot open a floored ability.
    let open = checkout(Some(r#"{"pr.merge":"allow","deploy.production":"allow"}"#));
    assert!(block(hold("Coder", "Run", &merge, open.path(), &cancel)).is_ok());
    let deploy = json!({"command": "gcloud run deploy web"});
    assert!(
        block(hold("Coder", "Run", &deploy, open.path(), &cancel))
            .unwrap_err()
            .starts_with("Held for the person")
    );
    let closed = checkout(Some(r#"{"pr.merge":"deny"}"#));
    assert!(
        block(hold("Coder", "Run", &merge, closed.path(), &cancel))
            .unwrap_err()
            .starts_with("Not allowed here")
    );
}

/// Codex's hook (`coder-floor-hook`): a floored call is denied with a note
/// to hand it back; anything else gets no answer and runs.
#[test]
fn the_codex_hook_holds_each_floor() {
    let repo = checkout(None);
    let cwd = repo.path().display().to_string();
    for (tool, input) in [
        (
            "Bash",
            json!({"command": "openagents deploy production sha256:abc"}),
        ),
        ("Bash", json!({"command": "gh pr merge 1"})),
        ("Bash", json!({"command": "cat .secrets/key.env"})),
        ("Bash", json!({"command": "openagents wallet send x"})),
    ] {
        let decision = hook_decision(&json!({
            "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input, "cwd": cwd,
        }))
        .expect("held");
        let output = &decision["hookSpecificOutput"];
        assert_eq!(output["permissionDecision"], "deny");
        assert!(
            output["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person")
        );
    }
    assert!(
        hook_decision(&json!({"tool_name": "Bash", "tool_input": {"command": "ls"}, "cwd": cwd}))
            .is_none()
    );
}

/// Claude Code launches as the permission table says, with the floors'
/// hook installed; the hook denies a floored call and lets others run.
#[test]
fn claude_code_launches_as_the_table_says_with_the_floors_hook() {
    let _lock = crate::approval::test_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let repo = checkout(None);
    let (options, _) = crate::claude::turn_options(repo.path(), crate::claude::Access::Full, None);
    let row = row(Harness::ClaudeCode);
    assert_eq!(row.permission, PERMISSION);
    assert!(row.native.contains(&"permissionMode=bypassPermissions"));
    assert_eq!(
        options.permission_mode,
        Some(claude_agent_sdk::PermissionMode::BypassPermissions)
    );
    assert!(row.native.contains(&"allowDangerouslySkipPermissions"));
    assert!(options.allow_dangerously_skip_permissions);
    let hooks = options
        .hooks
        .get(&claude_agent_sdk::HookEvent::PreToolUse)
        .expect("the floors' hook is installed");
    // The floors first, on every tool; the squeezer's rewrite (#11486) on Bash.
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0].matcher, None);
    assert_eq!(hooks[1].matcher.as_deref(), Some("Bash"));
    install_desk(None);
    let hook = hooks[0].hooks[0].clone();
    let cwd = repo.path().display().to_string();
    let denied = block(hook.call(
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "gh pr merge 3"}, "cwd": cwd}),
        None,
    ))
    .unwrap();
    let value = serde_json::to_value(&denied).unwrap();
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    let allowed = block(hook.call(
        json!({"tool_name": "Bash", "tool_input": {"command": "ls"}}),
        None,
    ))
    .unwrap();
    let value = serde_json::to_value(&allowed).unwrap();
    assert!(value.get("hookSpecificOutput").is_none(), "{value}");
}

/// Every harness has one row, all on the one permission.
#[test]
fn the_permission_table_names_every_harness_once() {
    for harness in [Harness::CoderLoop, Harness::ClaudeCode, Harness::Codex] {
        assert_eq!(TABLE.iter().filter(|row| row.harness == harness).count(), 1);
        assert_eq!(row(harness).permission, PERMISSION);
    }
    // The documented table says the same.
    let doc = include_str!("../../../docs/coder/permissions.md");
    for row in TABLE {
        for native in row.native {
            assert!(doc.contains(native), "{native} is documented");
        }
        assert!(doc.contains(row.floors), "{} is documented", row.floors);
    }
}

fn pin(
    harness: Harness,
    kind: Option<crate::router::Kind>,
    model: Option<&str>,
) -> crate::routing_preferences::Pin {
    crate::routing_preferences::Pin {
        harness,
        model: model.map(str::to_owned),
        repo: Some("openagents".into()),
        kind,
        said: "always use it here".into(),
        at: "2026-10-12T00:00:00.000Z".into(),
    }
}

#[test]
fn pins_are_written_read_and_shown_in_plain_words() {
    let dir = tempfile::tempdir().unwrap();
    let path = crate::routing_preferences::path(dir.path());
    let saved = crate::routing_preferences::execute_in(
        &path,
        json!({"harness": "claude-code", "pin": true, "this_repo": true, "kind": "repo_change",
            "model": "claude-opus-5-5", "said": "always use Claude Opus for code here"}),
        Some("openagents"),
    )
    .unwrap();
    assert_eq!(
        saved["pinned"],
        "Always Claude Code (claude-opus-5-5) for changes to code, in openagents"
    );
    let preferences = crate::routing_preferences::RoutingPreferences::load(&path).unwrap();
    assert_eq!(preferences.pins_in(Some("openagents")).len(), 1);
    assert!(preferences.pins_in(Some("elsewhere")).is_empty());
    assert!(preferences.summary().contains(
        &"Always Claude Code (claude-opus-5-5) for changes to code, in openagents".to_owned()
    ));
    let again =
        crate::routing_preferences::RoutingPreferences::parse(&preferences.render()).unwrap();
    assert_eq!(again, preferences);
    crate::routing_preferences::execute_in(
        &path,
        json!({"harness": "claude-code", "unpin": true, "said": "stop pinning Claude"}),
        Some("openagents"),
    )
    .unwrap();
    assert!(
        crate::routing_preferences::RoutingPreferences::load(&path)
            .unwrap()
            .pins
            .is_empty()
    );
}

fn situation(pins: Vec<crate::routing_preferences::Pin>) -> crate::router::Situation {
    crate::router::Situation {
        admitted: vec![Harness::ClaudeCode, Harness::CoderLoop],
        preferred: None,
        payers: vec![
            (
                Harness::ClaudeCode,
                crate::harness_plugin::Payer::ClaudePlan,
            ),
            (
                Harness::CoderLoop,
                crate::harness_plugin::Payer::GatewayCredits,
            ),
        ],
        current: None,
        claude_five_hour: None,
        time_weight: crate::preferences::BALANCED_TIME_WEIGHT,
        pins,
    }
}

fn judged(kind: crate::router::Kind) -> crate::router::Judgment {
    crate::router::Judgment {
        kind,
        size: crate::router::Size::Small,
        needs_repo: false,
        own_tools: false,
        risk: crate::router::Risk::Low,
        new_work: None,
    }
}

/// A pin decides ahead of the router's own steps, for its kind of work or
/// for all; a continuation still stays where it is.
#[test]
fn a_pin_decides_for_its_kind_of_work() {
    use crate::router::{Kind, Step, policy};
    let pins = situation(vec![pin(
        Harness::CoderLoop,
        Some(Kind::RepoChange),
        Some("deepseek/v4"),
    )]);
    let decision = policy(&judged(Kind::RepoChange), &pins);
    assert_eq!(
        (decision.harness, decision.step),
        (Harness::CoderLoop, Step::Pinned)
    );
    let other = policy(&judged(Kind::Answer), &pins);
    assert_ne!(
        other.step,
        Step::Pinned,
        "a pin for code leaves answers alone"
    );
    let everything = situation(vec![pin(Harness::ClaudeCode, None, None)]);
    assert_eq!(
        policy(&judged(Kind::Answer), &everything).step,
        Step::Pinned
    );
    assert_eq!(crate::router::fallback(&everything), Harness::ClaudeCode);
}

/// A first message in a repository with a pin runs where the pin says, on
/// its model, and the record keeps the pin; a never on every engine refuses
/// the message.
#[test]
fn a_pin_is_honored_and_recorded_and_never_means_never() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records");
    crate::harness_plugin::sign_in_claude(true);
    crate::routing_record::record_to(Some((
        records.clone(),
        crate::harness_plugin::Facts {
            local: true,
            claude_plugin_on: true,
            claude: crate::harness_plugin::Health {
                signed_in: true,
                at_limit: false,
            },
            codex_on: false,
            codex: crate::harness_plugin::Health {
                signed_in: true,
                at_limit: false,
            },
            openrouter_key: false,
            leaning: crate::routing_preferences::Leaning::default(),
        },
    )));
    let store = dir.path().join("store");
    let path = crate::routing_preferences::path(&store);
    let mut preferences = crate::routing_preferences::RoutingPreferences::default();
    let mut everywhere = pin(
        Harness::CoderLoop,
        None,
        Some("deepseek/deepseek-v4.1-flash"),
    );
    everywhere.repo = None;
    preferences.pin(everywhere).unwrap();
    preferences.save(&path).unwrap();
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.plugins.use_store(&store);
    app.plugins.enabled = true;
    app.plugins.key_configured = true;
    app.draft.text = "hello".into();
    app.submit_live();
    assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
    assert_eq!(
        app.live.pinned_model.as_deref(),
        Some("deepseek/deepseek-v4.1-flash")
    );
    app.live.busy = false;
    app.request = None;
    app.record_end(Some(crate::routing_record::Ended::Done), None, None);

    // Never on both chat engines: nothing may run, and the message stays.
    let mut never = crate::routing_preferences::RoutingPreferences::load(&path).unwrap();
    never.pins.clear();
    for harness in [Harness::CoderLoop, Harness::ClaudeCode] {
        never
            .change(
                crate::routing_preferences::Who::Person,
                harness,
                None,
                crate::routing_preferences::Stance::Never,
                "never",
                None,
            )
            .unwrap();
    }
    never.save(&path).unwrap();
    app.draft.text = "and this".into();
    app.submit_live();
    crate::routing_record::record_to(None);
    crate::harness_plugin::sign_in_claude(false);
    assert_eq!(
        app.live.notice.as_deref(),
        Some(crate::harness_plugin::NEVER_LEFT)
    );
    assert_eq!(app.draft.text, "and this", "the message stays in the input");
    let chose: Vec<_> = crate::routing_record::read(&records)
        .into_iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Chose(chose) => Some(chose),
            _ => None,
        })
        .collect();
    assert_eq!(chose.len(), 1);
    assert_eq!(chose[0].rule, crate::routing_record::Rule::Pin);
    assert_eq!(chose[0].harness, Harness::CoderLoop);
}
