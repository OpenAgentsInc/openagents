use std::collections::BTreeSet;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::digest::{Digest, canonical};
use crate::eval::{self, Part, Role};
use crate::lifecycle::{
    CheckLabel, Lifecycle, TaskChecks, TaskDisposition, TaskExecution, TaskStatus, project,
};
use crate::offer::{self, Offer, Price, Terms};
use crate::route::*;
use crate::snapshot::*;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
    let text = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, value, "{text}");
}

fn d(byte: &str) -> Digest {
    Digest::of_bytes(byte.as_bytes())
}

fn plan() -> DispatchPlan {
    DispatchPlan {
        class: TaskClass::Exploration,
        fan_out: FanOut::OnePerEngine,
        runs: ["codex", "claude", "grok"]
            .into_iter()
            .map(|engine| PlannedRun {
                engine: engine.into(),
                chosen: Chosen::Default,
                mode: RunMode::ReadOnly,
                input: d("explore the repo"),
                continuation: None,
            })
            .collect(),
        summary: Summary::Compose,
    }
}

fn snapshot() -> AdmissionSnapshot {
    let result = RouteResult::Coder { plan: plan() };
    AdmissionSnapshot {
        schema: crate::SNAPSHOT_SCHEMA.into(),
        identity: Identity {
            caller: Caller {
                kind: CallerKind::AppUser,
                id: "npub1example".into(),
            },
            surface: Surface::Terminal,
            workspace: None,
            request: "req_1".into(),
            thread: Some("th_1".into()),
            task: None,
            attempt: None,
        },
        input: Input {
            request: d("do 3 readonly delegations, 1 per agent"),
            source: Some(SourcePin {
                revision: Some("f2429380a9".into()),
                snapshot: None,
            }),
            instructions: vec![d("AGENTS.md")],
            attachments: Vec::new(),
        },
        route: Route {
            family: RouteFamily::Coder,
            result: result.digest(),
            explicit: false,
            capability: None,
            adapter: Some(d("delegate-settings-v1")),
            executor_revision: None,
            model: None,
            policy: "route-policy-v1".into(),
            question_set: QuestionSet {
                id: "chat-router-v4".into(),
                digest: d("chat-router-v4"),
            },
        },
        placement: Placement {
            computer: Some("cmp_here".into()),
            workspace: Some(WorkspaceBinding {
                project: "openagents".into(),
                path: Some("/Users/example/work/openagents".into()),
            }),
            grant: Some(GrantRef {
                id: "grant_1".into(),
                epoch: 3,
                source: GrantSource::Autostart,
            }),
        },
        effects: Effects {
            reads: vec![ReadScope::Workspace, ReadScope::Toolchains],
            writes: WriteScope::None,
            network: Network::Open,
            commands: CommandScope::EngineTools,
            publication: Vec::new(),
            access: Access::Toolchains,
            os_deny: OsDenySet::macos(),
        },
        disclosure: Disclosure {
            recipients: vec![
                Recipient {
                    kind: RecipientKind::ModelProvider,
                    id: "openai".into(),
                },
                Recipient {
                    kind: RecipientKind::ModelProvider,
                    id: "anthropic".into(),
                },
            ],
            context: vec![ContentClass::Message, ContentClass::RepositorySource],
            artifacts: vec![ContentClass::RunSummary],
        },
        resources: Resources {
            wall_secs: Some(1800),
            memory_bytes: None,
            max_parallel: Some(3),
            capacity: Capacity::Available,
            caller_limits: Vec::new(),
        },
        money: Money {
            byok: ByokMode::Ours,
            payers: vec![
                PayerEntry {
                    resource: Resource::Executor,
                    payer: Payer::CallerLogin {
                        engine: "codex".into(),
                    },
                },
                PayerEntry {
                    resource: Resource::Decision,
                    payer: Payer::OpenAgents,
                },
            ],
            funding: Funding::None,
            price_book: None,
            quote: None,
            fees: Vec::new(),
            reservation: None,
            settlement: None,
            shown: false,
        },
        evidence: Evidence {
            deliverables: vec![Deliverable::RunResults],
            check: CheckScope::ExecutorExit,
            checker: None,
            retention_days: None,
        },
        defaults_applied: vec![
            DefaultApplied {
                default: DefaultKind::SignedInEngines,
                value: "codex,claude,grok".into(),
            },
            DefaultApplied {
                default: DefaultKind::ThisComputer,
                value: "cmp_here".into(),
            },
        ],
        inherits: None,
    }
}

#[test]
fn snapshot_round_trips_with_stable_field_names() {
    let snapshot = snapshot();
    round_trip(&snapshot);
    let value = serde_json::to_value(&snapshot).unwrap();
    let keys: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "schema",
            "identity",
            "input",
            "route",
            "placement",
            "effects",
            "disclosure",
            "resources",
            "money",
            "evidence",
            "defaults_applied",
        ])
    );
    assert_eq!(value["effects"]["os_deny"]["platform"], "macos");
    assert_eq!(value["effects"]["os_deny"]["locations"][0], "music");
    assert_eq!(value["money"]["payers"][0]["payer"]["kind"], "caller_login");
    assert_eq!(value["defaults_applied"][0]["default"], "signed_in_engines");
}

#[test]
fn snapshot_refuses_unknown_fields() {
    let mut value = serde_json::to_value(snapshot()).unwrap();
    value["surprise"] = json!(true);
    assert!(serde_json::from_value::<AdmissionSnapshot>(value).is_err());
}

/// The frozen v1 digests. A change here is a contract change: bump the
/// schema version instead of updating these.
#[test]
fn digests_are_stable() {
    assert_eq!(
        Digest::of_bytes(b"").as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let snapshot = snapshot();
    assert_eq!(snapshot.digest(), snapshot.clone().digest());
    let route = RouteResult::Coder { plan: plan() };
    assert_eq!(snapshot.route.result, route.digest());
    golden(
        "route",
        route.digest().as_str(),
        "sha256:83a9700c49263dc64a5391c225e18e6a3b5cd820d02c7a6b810ed88b5450667f",
    );
    golden(
        "snapshot",
        snapshot.digest().as_str(),
        "sha256:09700d1075fdbb098e833b3e6c9fb66265f82d1c4c0a451248ca2556c4f599cb",
    );
}

fn golden(name: &str, got: &str, want: &str) {
    assert_eq!(got, want, "{name} digest moved");
}

#[test]
fn canonical_json_sorts_keys_at_every_depth() {
    let a = json!({"b": 1, "a": {"z": [1, {"y": 2, "x": 3}], "c": "s"}});
    assert_eq!(
        String::from_utf8(canonical(&a)).unwrap(),
        r#"{"a":{"c":"s","z":[1,{"x":3,"y":2}]},"b":1}"#
    );
    assert!(Digest::try_from("sha256:ABC".to_owned()).is_err());
    assert!(Digest::try_from("md5:00".to_owned()).is_err());
    round_trip(&d("x"));
}

#[test]
fn every_route_result_round_trips_and_names_its_family() {
    let pin = CapabilityPin {
        id: "pl_repo-map".into(),
        version: "1.2.0".into(),
        digest: d("repo-map"),
    };
    let results = [
        RouteResult::Answer {
            source: AnswerSource::Knowledge {
                corpus: "product".into(),
                citations: vec!["kb_1".into()],
            },
        },
        RouteResult::LocalCommand {
            action: LocalAction::Command {
                argv: vec!["wallet".into(), "balance".into()],
                effect: Effect::ReadOnly,
            },
        },
        RouteResult::Plugin {
            plugin: PluginRoute::Run {
                capability: pin,
                arguments: json!({"depth": 2}),
            },
        },
        RouteResult::Coder { plan: plan() },
        RouteResult::StandingRule {
            rule: RuleProposal {
                change: RuleChange::Define,
                rule_schema: "openagents.background.rule.v1".into(),
                rule: json!({"id": "disk"}),
                rule_digest: d("disk"),
            },
        },
        RouteResult::MissingCapability {
            need: "book a flight".into(),
            remedy: Remedy::Build,
        },
        RouteResult::Clarification {
            question: "Which repository?".into(),
            reason: ClarifyReason::NoDefault,
        },
        RouteResult::Refusal {
            reason: RefusalReason::MissingGrant,
        },
    ];
    let families: Vec<RouteFamily> = results.iter().map(RouteResult::family).collect();
    assert_eq!(families, RouteFamily::ALL);
    for result in &results {
        round_trip(result);
        assert!(result.validate().is_ok());
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(
            value["route"],
            serde_json::to_value(result.family()).unwrap()
        );
    }
}

#[test]
fn dispatch_plans_are_bounded() {
    assert!(plan().validate().is_ok());
    let mut empty = plan();
    empty.runs.clear();
    assert_eq!(empty.validate(), Err(Invalid::NoRuns));
    let mut repeated = plan();
    repeated.runs[1].engine = "codex".into();
    assert_eq!(
        repeated.validate(),
        Err(Invalid::RepeatedEngine("codex".into()))
    );
    let mut single = plan();
    single.fan_out = FanOut::Single;
    assert_eq!(single.validate(), Err(Invalid::SingleWithMany));
    let mut many = plan();
    many.fan_out = FanOut::Named;
    many.runs = vec![many.runs[0].clone(); MAX_RUNS + 1];
    assert_eq!(many.validate(), Err(Invalid::TooManyRuns(MAX_RUNS + 1)));
    let command = RouteResult::LocalCommand {
        action: LocalAction::Command {
            argv: Vec::new(),
            effect: Effect::ReadOnly,
        },
    };
    assert_eq!(command.validate(), Err(Invalid::EmptyCommand));
}

#[test]
fn commands_run_by_effect() {
    assert_eq!(Effect::ReadOnly.run_policy(), RunPolicy::AtOnce);
    assert_eq!(Effect::LocalWrite.run_policy(), RunPolicy::Confirm);
    assert_eq!(Effect::Spends.run_policy(), RunPolicy::NeverFromChat);
    assert_eq!(Effect::Secret.run_policy(), RunPolicy::NeverFromChat);
}

fn terms() -> Terms {
    let snapshot = snapshot();
    Terms {
        route: snapshot.route.result.clone(),
        snapshot: snapshot.digest(),
        computer: snapshot.placement.computer.clone(),
        effects: snapshot.effects.clone(),
        recipients: snapshot.disclosure.recipients.clone(),
        price: Some(Price {
            max_sats: 210,
            fees: Vec::new(),
        }),
        source: snapshot.input.source.clone(),
    }
}

#[test]
fn an_offer_confirms_only_its_unchanged_terms_before_expiry() {
    let offer = Offer::new(
        "cf_1".into(),
        offer::Action::RunStart,
        "Run Coder on this computer".into(),
        100,
        700,
        terms(),
    );
    round_trip(&offer);
    assert_eq!(
        serde_json::to_value(&offer).unwrap()["action"],
        json!("run.start")
    );
    assert!(offer.intact());
    assert_eq!(offer.confirm(&offer.digest, 699, &terms()), Ok(()));
    assert_eq!(
        offer.confirm(&offer.digest, 700, &terms()),
        Err(offer::Refusal::Expired)
    );
    assert_eq!(
        offer.confirm(&d("other"), 200, &terms()),
        Err(offer::Refusal::Mismatch)
    );
    let mut wider = terms();
    wider.effects.publication.push(Publication::Push);
    assert_eq!(
        offer.confirm(&offer.digest, 200, &wider),
        Err(offer::Refusal::Changed)
    );
    let mut tampered = offer.clone();
    tampered.terms.price = Some(Price {
        max_sats: 1,
        fees: Vec::new(),
    });
    assert!(!tampered.intact());
    // The label is not a term: relabeling keeps the digest.
    let mut relabeled = offer.clone();
    relabeled.label = "Start".into();
    assert!(relabeled.intact());
}

#[test]
fn a_continuation_or_fallback_may_not_widen_its_parent() {
    let parent = snapshot();
    let mut same = parent.clone();
    same.inherits = Some(parent.digest());
    same.placement.grant.as_mut().unwrap().source = GrantSource::Continuation;
    assert!(same.widens(&parent).is_empty());

    let mut narrower = parent.clone();
    narrower.disclosure.recipients.truncate(1);
    narrower.effects.network = Network::Localhost;
    assert!(narrower.widens(&parent).is_empty());

    let mut other = parent.clone();
    other.disclosure.recipients.push(Recipient {
        kind: RecipientKind::ModelProvider,
        id: "xai".into(),
    });
    other.effects.publication.push(Publication::Push);
    other.effects.writes = WriteScope::Workspace;
    other
        .effects
        .os_deny
        .locations
        .retain(|l| *l != Protected::Music);
    other.placement.computer = Some("cmp_other".into());
    other.money.fees.push(Fee {
        plugin: "pl_x".into(),
        author: "npub1author".into(),
        sats: 5,
    });
    assert_eq!(
        other.widens(&parent),
        vec![
            Widening::Computer,
            Widening::Writes,
            Widening::Publication,
            Widening::OsDenySet,
            Widening::Disclosure,
            Widening::PluginFee,
        ]
    );

    // 13.6: a fallback never switches the caller's own payer to us.
    let mut mine = parent.clone();
    mine.money.byok = ByokMode::Mine;
    let mut ours = mine.clone();
    ours.money.byok = ByokMode::Ours;
    assert_eq!(ours.widens(&mine), vec![Widening::Payer]);
    let mut switched = parent.clone();
    switched.money.payers[0].payer = Payer::OpenAgents;
    assert_eq!(switched.widens(&parent), vec![Widening::Payer]);
}

#[test]
fn the_macos_deny_set_covers_every_protected_location() {
    let set = OsDenySet::macos();
    assert_eq!(set.locations.len(), Protected::ALL.len());
    assert!(set.app_control);
    let mut weaker = set.clone();
    weaker.app_control = false;
    assert!(!weaker.covers(&set));
    assert!(set.covers(&weaker));
}

#[test]
fn every_task_disposition_projects_onto_one_lifecycle_state() {
    let mut reached = BTreeSet::new();
    for status in TaskStatus::ALL {
        for execution in TaskExecution::ALL {
            for checks in TaskChecks::ALL {
                let disposition = TaskDisposition {
                    status,
                    execution,
                    checks,
                };
                round_trip(&disposition);
                let projection = project(disposition);
                round_trip(&projection);
                assert!(!projection.state.router_owned(), "{disposition:?}");
                // Verified only when independent checks passed; never a
                // completed state with a failed check.
                assert_eq!(
                    projection.check == CheckLabel::Verified,
                    status == TaskStatus::Finished
                        && execution == TaskExecution::Finished
                        && checks == TaskChecks::Passed,
                    "{disposition:?}"
                );
                if projection.state == Lifecycle::Completed {
                    assert_ne!(projection.check, CheckLabel::CheckFailed);
                    assert_ne!(projection.check, CheckLabel::Pending);
                }
                if execution == TaskExecution::Unknown || status == TaskStatus::Unknown {
                    assert_eq!(projection.state, Lifecycle::NeedsReconciliation);
                }
                assert_eq!(
                    projection.cancel_requested,
                    status == TaskStatus::CancelRequested
                        && projection.state != Lifecycle::Completed
                );
                reached.insert(format!("{:?}", projection.state));
            }
        }
    }
    // Every task-owned state is reachable from some disposition.
    for state in Lifecycle::ALL.into_iter().filter(|s| !s.router_owned()) {
        assert!(reached.contains(&format!("{state:?}")), "{state:?}");
    }
}

#[test]
fn the_mapping_table_rows() {
    use Lifecycle as L;
    use TaskChecks as C;
    use TaskExecution as E;
    use TaskStatus as S;
    let rows = [
        (
            S::Queued,
            E::NotStarted,
            C::NotRun,
            L::DispatchPending,
            CheckLabel::Pending,
        ),
        (
            S::Running,
            E::NotStarted,
            C::NotRun,
            L::DispatchPending,
            CheckLabel::Pending,
        ),
        (
            S::Running,
            E::Running,
            C::NotRun,
            L::Running,
            CheckLabel::Pending,
        ),
        (
            S::CancelRequested,
            E::Running,
            C::NotRun,
            L::Running,
            CheckLabel::Pending,
        ),
        (
            S::Finished,
            E::Finished,
            C::Running,
            L::Checking,
            CheckLabel::Pending,
        ),
        (
            S::Finished,
            E::Finished,
            C::Passed,
            L::Completed,
            CheckLabel::Verified,
        ),
        (
            S::Finished,
            E::Finished,
            C::NotRun,
            L::Completed,
            CheckLabel::Unchecked,
        ),
        (
            S::Finished,
            E::Finished,
            C::Unavailable,
            L::Completed,
            CheckLabel::Unverifiable,
        ),
        (
            S::Finished,
            E::Finished,
            C::Disputed,
            L::Completed,
            CheckLabel::Disputed,
        ),
        (
            S::Finished,
            E::Finished,
            C::Failed,
            L::Failed,
            CheckLabel::CheckFailed,
        ),
        (
            S::Finished,
            E::Failed,
            C::Unavailable,
            L::Failed,
            CheckLabel::Pending,
        ),
        (
            S::Finished,
            E::Stopped,
            C::NotRun,
            L::Cancelled,
            CheckLabel::Pending,
        ),
        (
            S::Cancelled,
            E::NotStarted,
            C::NotRun,
            L::Cancelled,
            CheckLabel::Pending,
        ),
        (
            S::Unknown,
            E::Unknown,
            C::NotRun,
            L::NeedsReconciliation,
            CheckLabel::Pending,
        ),
        (
            S::Running,
            E::Unknown,
            C::NotRun,
            L::NeedsReconciliation,
            CheckLabel::Pending,
        ),
    ];
    for (status, execution, checks, state, check) in rows {
        let projection = project(TaskDisposition {
            status,
            execution,
            checks,
        });
        assert_eq!((projection.state, projection.check), (state, check));
    }
}

#[test]
fn the_router_owns_only_the_states_before_a_task() {
    use Lifecycle as L;
    assert!(L::Received.router_step(L::Proposed));
    assert!(L::Received.router_step(L::Completed));
    assert!(L::Proposed.router_step(L::AwaitingAuthorityOrPayment));
    assert!(L::AwaitingAuthorityOrPayment.router_step(L::Admitted));
    assert!(L::Admitted.router_step(L::DispatchPending));
    assert!(!L::Received.router_step(L::Running));
    assert!(!L::Proposed.router_step(L::Running));
    // Nothing after dispatch moves by the router's hand.
    for from in Lifecycle::ALL.into_iter().filter(|s| !s.router_owned()) {
        for to in Lifecycle::ALL {
            assert!(!from.router_step(to), "{from:?} -> {to:?}");
        }
    }
}

#[test]
fn the_eval_split_is_labeled_and_traceable() {
    let split = eval::route_families_v1();
    assert_eq!(split.schema, crate::EVAL_SCHEMA);
    let ids: BTreeSet<&str> = split.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids.len(), split.rows.len(), "row ids are unique");
    for family in RouteFamily::ALL {
        for part in [Part::Tune, Part::Test] {
            assert!(
                split
                    .rows
                    .iter()
                    .any(|row| row.family == family && row.split == part),
                "{family:?} has {part:?} rows"
            );
        }
    }
    let routes: serde_json::Value = serde_json::from_str(include_str!(
        "../../coder/fixtures/chat-router/routes-v4.json"
    ))
    .unwrap();
    let wallet: serde_json::Value = serde_json::from_str(include_str!(
        "../../coder/fixtures/chat-router/wallet-v1.json"
    ))
    .unwrap();
    let mut followups = 0;
    let mut wallets = 0;
    for row in &split.rows {
        assert_eq!(row.messages.last().unwrap().role, Role::User, "{}", row.id);
        let last = &row.messages.last().unwrap().text;
        if let Some(id) = row.source.strip_prefix("routes-v4:") {
            let source = routes["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap_or_else(|| panic!("{id} in routes-v4"));
            assert_eq!(
                source["messages"].as_array().unwrap().last().unwrap()["text"],
                json!(last)
            );
            assert_eq!(source["route"], json!(row.chat_route.clone().unwrap()));
            let part = if source["split"] == "held_out" {
                Part::Test
            } else {
                Part::Tune
            };
            assert_eq!(row.split, part, "{} keeps its held-out split", row.id);
            if source["tags"]
                .as_array()
                .unwrap()
                .contains(&json!("coder_followup"))
            {
                followups += 1;
            }
        } else if let Some(id) = row.source.strip_prefix("wallet-v1:") {
            let source = wallet["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap_or_else(|| panic!("{id} in wallet-v1"));
            assert_eq!(source["message"], json!(last));
            wallets += 1;
        } else {
            assert_eq!(row.source, "new", "{}", row.id);
        }
    }
    assert_eq!(wallets, wallet["rows"].as_array().unwrap().len());
    assert_eq!(followups, 46);
}
