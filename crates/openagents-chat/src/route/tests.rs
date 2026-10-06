//! The shared route policy: what each kind of reply routes to, what its
//! admission says, and the journal.

use super::*;
use nostr::cj_conversation::{Engine, Plan};
use route_contract::route::{RuleChange, RunPolicy};
use serde_json::json;

fn situation() -> Situation {
    Situation {
        surface: Surface::Terminal,
        caller: "local:openagents-terminal".into(),
        request: "req-1".into(),
        thread: Some("thread-1".into()),
        computer: THIS_COMPUTER.into(),
        project: Some(WorkspaceBinding {
            project: "openagents".into(),
            path: Some("/work/openagents".into()),
        }),
        ready: true,
        bound: None,
        check: CheckScope::ExecutorExit,
    }
}

fn read<'a>(meta: Option<&'a Meta>, lane: bool, text: &'a str) -> Reading<'a> {
    Reading {
        meta,
        computer_lane: lane,
        text,
        reply: "Which project do you mean?",
    }
}

fn tree(argv: &[String]) -> Option<Effect> {
    match argv.first().map(String::as_str) {
        Some("wallet") if argv.get(1).map(String::as_str) == Some("send") => Some(Effect::Spends),
        Some("wallet" | "computer") => Some(Effect::ReadOnly),
        Some("plugin") => Some(Effect::LocalWrite),
        _ => None,
    }
}

fn route(meta: Option<&Meta>, lane: bool, situation: &Situation) -> RouteResult {
    propose(&read(meta, lane, "do the thing"), situation, &tree)
}

#[test]
fn a_run_coder_offer_is_one_run_on_the_named_engine() {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        engine: Some(Engine::ClaudeCode),
        ..Meta::default()
    };
    let RouteResult::Coder { plan } = route(Some(&meta), false, &situation()) else {
        panic!("not coder");
    };
    assert_eq!(plan.fan_out, FanOut::Single);
    assert_eq!(plan.runs.len(), 1);
    assert_eq!(plan.runs[0].engine, "claude");
    assert_eq!(plan.runs[0].chosen, Chosen::Named);
    assert_eq!(plan.runs[0].mode, RunMode::Write);
    assert_eq!(plan.runs[0].input, Digest::of_bytes(b"do the thing"));
    assert!(plan.runs[0].continuation.is_none());
    assert_eq!(plan.class, TaskClass::RepositoryChange);
}

#[test]
fn a_plan_is_a_dispatch_plan_of_one_run_per_engine() {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        plan: Some(Plan {
            runs: vec![Engine::Codex, Engine::ClaudeCode, Engine::GrokBuild],
            read_only: true,
            summarize: true,
        }),
        ..Meta::default()
    };
    let result = route(Some(&meta), false, &situation());
    result.validate().unwrap();
    let RouteResult::Coder { plan } = &result else {
        panic!("not coder");
    };
    assert_eq!(plan.fan_out, FanOut::OnePerEngine);
    assert_eq!(plan.class, TaskClass::Exploration);
    assert_eq!(plan.summary, Summary::Compose);
    let engines: Vec<&str> = plan.runs.iter().map(|run| run.engine.as_str()).collect();
    assert_eq!(engines, ["codex", "claude", "grok"]);
    assert!(plan.runs.iter().all(|run| run.mode == RunMode::ReadOnly));
    let snapshot = admit(&result, &situation(), Some(&meta), "do the thing", None);
    assert_eq!(snapshot.effects.writes, WriteScope::None);
    assert_eq!(snapshot.resources.max_parallel, Some(3));
    assert_eq!(
        snapshot.evidence.deliverables,
        [Deliverable::RunResults, Deliverable::RetainedArtifacts]
    );
    // Each engine runs on the person's own login, and its provider is a
    // named recipient.
    let executors: Vec<_> = snapshot
        .money
        .payers
        .iter()
        .filter(|entry| entry.resource == Resource::Executor)
        .collect();
    assert_eq!(executors.len(), 3);
    for provider in ["openai", "anthropic", "xai"] {
        assert!(
            snapshot
                .disclosure
                .recipients
                .iter()
                .any(|recipient| recipient.id == provider)
        );
    }
}

/// A follow-up on a bound local task steers it while it works and is its
/// next turn once it ended (13.3); its admission inherits the run's and
/// asks for nothing wider.
#[test]
fn a_follow_up_continues_the_bound_task_without_widening() {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        ..Meta::default()
    };
    let first = route(Some(&meta), false, &situation());
    let parent = admit(&first, &situation(), Some(&meta), "do the thing", None);
    for (working, how) in [(true, ContinueHow::Steer), (false, ContinueHow::NextTurn)] {
        let mut here = situation();
        here.request = "req-2".into();
        here.bound = Some(Bound {
            task: "task-1".into(),
            working,
            revision: Some(4),
        });
        let result = route(Some(&meta), false, &here);
        let RouteResult::Coder { plan } = &result else {
            panic!("not coder");
        };
        let continuation = plan.runs[0].continuation.as_ref().unwrap();
        assert_eq!(continuation.task, "task-1");
        assert_eq!(continuation.based_on, 4);
        assert_eq!(continuation.how, how);
        let snapshot = admit(&result, &here, Some(&meta), "more", Some(&parent));
        assert_eq!(snapshot.inherits, Some(parent.digest()));
        assert_eq!(snapshot.identity.task.as_deref(), Some("task-1"));
        assert_eq!(
            snapshot.placement.grant.as_ref().unwrap().source,
            GrantSource::Continuation
        );
        assert!(
            snapshot.widens(&parent).is_empty(),
            "{:?}",
            snapshot.widens(&parent)
        );
    }
}

/// The computer lane alone offers Coder only on `work.dispatch`, and any
/// other typed action outranks it (#10073, #10079): the same reading
/// `delegation::offered` gives every surface.
#[test]
fn the_computer_lane_offers_coder_only_for_a_dispatch_reply() {
    let dispatch = Meta {
        route: Some("work.dispatch".into()),
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&dispatch), true, &situation()).family(),
        RouteFamily::Coder
    );
    assert_eq!(
        route(Some(&dispatch), false, &situation()).family(),
        RouteFamily::Answer
    );
    let answered = Meta {
        route: Some("meta".into()),
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&answered), true, &situation()).family(),
        RouteFamily::Answer
    );
    let gym = Meta {
        route: Some("work.dispatch".into()),
        offers: vec![Offer::StartEval {
            body: json!({"offer": "start_eval"}),
        }],
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&gym), true, &situation()).family(),
        RouteFamily::Plugin
    );
    assert!(!crate::delegation::offered(Some(&gym), true));
    assert!(crate::delegation::offered(Some(&dispatch), true));
    assert_eq!(
        route(None, true, &situation()).family(),
        RouteFamily::Answer
    );
}

#[test]
fn a_proposed_command_runs_by_its_effect_in_this_computers_tree() {
    let command = |argv: &[&str]| Meta {
        command: Some(argv.iter().map(|word| (*word).to_owned()).collect()),
        ..Meta::default()
    };
    let balance = command(&["wallet", "balance"]);
    let result = route(Some(&balance), false, &situation());
    assert_eq!(
        result,
        RouteResult::LocalCommand {
            action: LocalAction::Command {
                argv: vec!["wallet".into(), "balance".into()],
                effect: Effect::ReadOnly,
            }
        }
    );
    let snapshot = admit(&result, &situation(), Some(&balance), "balance?", None);
    assert_eq!(snapshot.effects.commands, CommandScope::ReadOnlyTree);
    assert_eq!(snapshot.effects.writes, WriteScope::None);
    let RouteResult::LocalCommand {
        action: LocalAction::Command { effect, .. },
    } = route(Some(&command(&["wallet", "send"])), false, &situation())
    else {
        panic!("not a command");
    };
    assert_eq!(effect.run_policy(), RunPolicy::NeverFromChat);
    // A command this computer's tree does not know is refused, whatever
    // the worker said about it.
    assert_eq!(
        route(Some(&command(&["rm", "-rf"])), false, &situation()),
        RouteResult::Refusal {
            reason: RefusalReason::RouteNotAllowed
        }
    );
}

#[test]
fn answers_screens_plugins_and_refusals_route_by_typed_words() {
    let canned = Meta {
        tier: Some("canned".into()),
        answer: Some("pricing.free@3".into()),
        route: Some("product.kb".into()),
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&canned), false, &situation()),
        RouteResult::Answer {
            source: AnswerSource::Prepared {
                entry: "pricing.free@3".into()
            }
        }
    );
    let grounded = Meta {
        tier: Some("grounded".into()),
        route: Some("codebase.kb".into()),
        ..Meta::default()
    };
    assert!(matches!(
        route(Some(&grounded), false, &situation()),
        RouteResult::Answer {
            source: AnswerSource::Knowledge { corpus, .. }
        } if corpus == "codebase.kb"
    ));
    let wallet = Meta {
        offers: vec![Offer::OpenScreen {
            screen: Screen::Wallet,
        }],
        ..Meta::default()
    };
    assert!(matches!(
        route(Some(&wallet), false, &situation()),
        RouteResult::LocalCommand { action: LocalAction::Screen { screen, .. } } if screen == "wallet"
    ));
    let author = Meta {
        route: Some("eval.author".into()),
        tier: Some("author".into()),
        ..Meta::default()
    };
    let result = route(Some(&author), false, &situation());
    assert!(matches!(
        &result,
        RouteResult::Plugin { plugin: PluginRoute::Create { brief } } if brief == "do the thing"
    ));
    let snapshot = admit(&result, &situation(), Some(&author), "do the thing", None);
    assert_eq!(snapshot.evidence.deliverables, [Deliverable::PluginPackage]);
    let missing = Meta {
        route: Some("capability.missing".into()),
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&missing), false, &situation()).family(),
        RouteFamily::MissingCapability
    );
    let clarify = Meta {
        route: Some("clarify".into()),
        ..Meta::default()
    };
    assert!(matches!(
        route(Some(&clarify), false, &situation()),
        RouteResult::Clarification { question, .. } if question == "Which project do you mean?"
    ));
    let refuse = Meta {
        tier: Some("refuse".into()),
        ..Meta::default()
    };
    assert_eq!(
        route(Some(&refuse), false, &situation()).family(),
        RouteFamily::Refusal
    );
}

/// Every admission in our own apps records its cost without showing it,
/// names the policy and the question set, and keeps the macOS deny set.
#[test]
fn every_admission_names_its_policy_and_records_cost_unshown() {
    let judged = Meta {
        judgment: Some(json!({"set": "chat-router-v4@0123456789ab"}).to_string()),
        ..Meta::default()
    };
    let rule = RouteResult::StandingRule {
        rule: route_contract::route::RuleProposal {
            change: RuleChange::Define,
            rule_schema: "openagents.background.rule.v1".into(),
            rule: json!({"trigger": "disk"}),
            rule_digest: digest_of(&json!({"trigger": "disk"})),
        },
    };
    for result in [
        route(Some(&judged), false, &situation()),
        RouteResult::Coder {
            plan: dispatch_plan(None, &situation(), "x"),
        },
        rule,
    ] {
        let snapshot = admit(&result, &situation(), Some(&judged), "x", None);
        assert_eq!(snapshot.route.policy, POLICY);
        assert_eq!(snapshot.route.family, result.family());
        assert_eq!(snapshot.route.result, result.digest());
        assert_eq!(snapshot.route.question_set.id, "chat-router-v4");
        assert!(!snapshot.money.shown);
        assert_eq!(snapshot.effects.os_deny, OsDenySet::macos());
        assert_eq!(snapshot.identity.surface, Surface::Terminal);
        if let RouteResult::Coder { plan } = &result {
            // The adapter is the delegate recipe's rows for the runs'
            // engines, and the defaults name its version (#10208).
            let engines: Vec<&str> = plan.runs.iter().map(|run| run.engine.as_str()).collect();
            assert_eq!(
                snapshot.route.adapter,
                Some(route_contract::recipe::adapter_digest(&engines))
            );
            assert!(snapshot.defaults_applied.iter().any(|applied| {
                applied.default == DefaultKind::DelegateSettings && applied.value == RECIPE_VERSION
            }));
        }
        // The record binds it.
        RouteRecord::received("req-1", Some("thread-1".into()), result, snapshot, 0).unwrap();
    }
}

#[test]
fn issue_work_carries_the_issue_flows_effects_and_gate() {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        ..Meta::default()
    };
    let result = route(Some(&meta), false, &situation());
    let snapshot = admit(&result, &situation(), Some(&meta), "pick an issue", None);
    let mut record =
        RouteRecord::received("req-1", Some("thread-1".into()), result, snapshot, 5).unwrap();
    record.step(Lifecycle::Admitted, "autostart", 6).unwrap();
    let issue = reissue(&record, 10178, 7).unwrap();
    assert_eq!(issue.request, "req-1");
    assert_eq!(issue.state, Lifecycle::Admitted);
    let RouteResult::Coder { plan } = &issue.result else {
        panic!("not coder");
    };
    assert_eq!(plan.class, TaskClass::IssueWork { issue: Some(10178) });
    assert_eq!(issue.snapshot.evidence.check, CheckScope::IssueGate);
    assert!(
        issue
            .snapshot
            .effects
            .publication
            .contains(&Publication::Push)
    );
}

#[test]
fn the_journal_keeps_the_latest_record_of_each_request() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::beside(&dir.path().join("tasks"));
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        ..Meta::default()
    };
    let result = route(Some(&meta), false, &situation());
    let snapshot = admit(&result, &situation(), Some(&meta), "x", None);
    let mut record =
        RouteRecord::received("req-1", Some("thread-1".into()), result, snapshot, 1).unwrap();
    journal.write(&record).unwrap();
    record.step(Lifecycle::Admitted, "autostart", 2).unwrap();
    record.dispatched("task-1", Some("codex")).unwrap();
    journal.write(&record).unwrap();
    assert!(dir.path().join("routes/thread-1.jsonl").is_file());
    assert_eq!(journal.records("thread-1"), [record.clone()]);
    assert_eq!(journal.latest("thread-1", "req-1"), Some(record.clone()));
    assert_eq!(journal.of_task("thread-1", "task-1"), Some(record.clone()));
    assert_eq!(journal.latest("thread-1", "req-2"), None);
    // A pane that knows only the task finds the same record in any thread,
    // and reading it twice writes nothing.
    let bytes = std::fs::read(dir.path().join("routes/thread-1.jsonl")).unwrap();
    assert_eq!(journal.find_task("task-1"), Some(record.clone()));
    assert_eq!(journal.find_task("task-1"), Some(record.clone()));
    assert_eq!(journal.find_task("task-2"), None);
    assert_eq!(
        std::fs::read(dir.path().join("routes/thread-1.jsonl")).unwrap(),
        bytes
    );
    assert_eq!(
        Journal::at(dir.path().join("none")).find_task("task-1"),
        None
    );
    // A thread name that is not a plain id is never a path.
    record.thread = Some("../escape".into());
    assert!(journal.write(&record).is_err());
    assert!(journal.records("../escape").is_empty());
}

/// BYOK (#10176): a turn that went with the person's keys admits with
/// `mine` and names their providers as the payers of routing, decisions,
/// and the model; one on ours names OpenAgents.
#[test]
fn a_turn_on_the_persons_keys_names_their_providers_as_payers() {
    let result = RouteResult::Answer {
        source: AnswerSource::Model,
    };
    let ours = admit(&result, &situation(), Some(&Meta::default()), "hi", None);
    assert_eq!(ours.money.byok, ByokMode::Ours);
    assert!(
        ours.money
            .payers
            .iter()
            .all(|entry| entry.payer == Payer::OpenAgents)
    );
    let meta = Meta {
        payer_keys: vec![
            model_access::KeyPrint {
                provider: model_access::Provider::OpenRouter,
                fingerprint: "0a1b2c3d".into(),
            },
            model_access::KeyPrint {
                provider: model_access::Provider::TypeSafe,
                fingerprint: "4e5f6a7b".into(),
            },
        ],
        ..Meta::default()
    };
    let theirs = admit(&result, &situation(), Some(&meta), "hi", None);
    assert_eq!(theirs.money.byok, ByokMode::Mine);
    let payer = |resource| {
        theirs
            .money
            .payers
            .iter()
            .find(|entry| entry.resource == resource)
            .map(|entry| entry.payer.clone())
            .unwrap()
    };
    let key = |provider: &str| Payer::CallerKey {
        provider: provider.into(),
    };
    assert_eq!(payer(Resource::Decision), key("typesafe"));
    assert_eq!(payer(Resource::Routing), key("typesafe"));
    assert_eq!(payer(Resource::ChatModel), key("openrouter"));
    assert!(!theirs.money.switches_payer_from(&theirs.money));
    assert!(ours.money.switches_payer_from(&theirs.money));
}
