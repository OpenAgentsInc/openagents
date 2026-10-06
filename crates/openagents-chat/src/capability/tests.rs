//! Dispatching an admitted noncoding capability through the shared route
//! policy and journal, and the cases that must not run.

use super::*;
use crate::route::{Reading, Situation, THIS_COMPUTER, admit, propose};
use crate::router::{Meta, Offer};
use route_contract::RouteFamily;
use route_contract::snapshot::{
    CheckScope, Fee, PayerEntry, Quote, RecipientKind, Resource as Paid, Surface,
};
use serde_json::json;

fn situation() -> Situation {
    Situation {
        surface: Surface::Terminal,
        caller: "local:openagents-terminal".into(),
        request: "req-gym".into(),
        thread: Some("thread-gym".into()),
        computer: THIS_COMPUTER.into(),
        project: None,
        ready: true,
        bound: None,
        check: CheckScope::ExecutorExit,
    }
}

fn body() -> Value {
    json!({"offer": "start_eval", "suite": "routing-basics", "trials": 3})
}

/// The Gym's start-eval program, as the router proposes it, admitted and
/// journaled: the shared policy, not a hand-built record.
fn admitted(journal: &Journal) -> RouteRecord {
    let meta = Meta {
        route: Some("work.dispatch".into()),
        offers: vec![Offer::StartEval { body: body() }],
        ..Meta::default()
    };
    let situation = situation();
    let reading = Reading {
        meta: Some(&meta),
        computer_lane: true,
        text: "evaluate the routing basics",
        reply: "Starting the evaluation.",
    };
    let result = propose(&reading, &situation, &|_| None);
    assert_eq!(result.family(), RouteFamily::Plugin);
    let snapshot = admit(&result, &situation, Some(&meta), reading.text, None);
    let mut record =
        RouteRecord::received("req-gym", situation.thread.clone(), result, snapshot, 1).unwrap();
    record.step(Lifecycle::Proposed, "offer", 2).unwrap();
    // The person confirmed the offer.
    record
        .step(Lifecycle::Admitted, "offer_confirmed", 3)
        .unwrap();
    journal.write(&record).unwrap();
    record
}

fn pin() -> CapabilityPin {
    CapabilityPin {
        id: "gym.start_eval".into(),
        version: "nip-cj-2".into(),
        digest: route_contract::digest_of(&body()),
    }
}

fn release() -> Release {
    Release {
        pin: pin(),
        arguments: Arguments {
            required: vec![
                ("offer".into(), ArgKind::String),
                ("suite".into(), ArgKind::String),
            ],
            optional: vec![("trials".into(), ArgKind::Number)],
        },
        recipient: Recipient {
            kind: RecipientKind::OpenAgents,
            id: "openagents".into(),
        },
        fee_sats: 0,
        fee_payer: Payer::OpenAgents,
        noncoding: true,
    }
}

fn catalog() -> Catalog {
    Catalog {
        releases: vec![release()],
        excluded: Vec::new(),
        adequate_models: vec![ModelPin {
            provider: "openrouter".into(),
            model: "small".into(),
        }],
    }
}

/// A runner that counts its runs and returns a fixed artifact.
#[derive(Default)]
struct Gym {
    runs: usize,
    fail: bool,
}

impl Runner for Gym {
    fn run(&mut self, release: &Release, arguments: &Value) -> Result<Output, String> {
        self.runs += 1;
        assert_eq!(release.pin, pin());
        if self.fail {
            return Err("the eval runner refused the suite".into());
        }
        Ok(Output {
            artifact: format!("results for {}", arguments["suite"]).into_bytes(),
            check: CheckLabel::Verified,
            cost_microusd: None,
        })
    }
}

fn run(
    record: &mut RouteRecord,
    catalog: &Catalog,
    gym: &mut Gym,
    journal: &Journal,
) -> Dispatched {
    let mut kept = Vec::new();
    let dispatched = dispatch(
        record,
        catalog,
        gym,
        journal,
        &mut |digest, bytes| kept.push((digest.clone(), bytes.to_vec())),
        10,
    )
    .unwrap();
    for (digest, bytes) in kept {
        assert_eq!(Digest::of_bytes(&bytes), digest);
    }
    dispatched
}

#[test]
fn an_admitted_noncoding_capability_runs_once_and_keeps_its_artifact_and_check() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut record = admitted(&journal);
    let mut gym = Gym::default();
    let Dispatched::Ran { artifact, check } = run(&mut record, &catalog(), &mut gym, &journal)
    else {
        panic!("the capability did not run");
    };
    assert_eq!(check, CheckLabel::Verified);
    assert_eq!(gym.runs, 1);
    // The journal keeps the settled record with the artifact and check.
    let kept = journal.latest("thread-gym", "req-gym").unwrap();
    assert_eq!(kept.state, Lifecycle::Completed);
    assert_eq!(kept.runs.len(), 1);
    assert_eq!(kept.runs[0].artifacts, vec![artifact]);
    assert_eq!(kept.runs[0].projection.check, CheckLabel::Verified);
    assert_eq!(kept.runs[0].cost_microusd, None);
    assert!(kept.settled());
    // Asking again follows the record; nothing runs twice.
    let mut again = kept;
    assert_eq!(
        run(&mut again, &catalog(), &mut gym, &journal),
        Dispatched::Followed
    );
    assert_eq!(gym.runs, 1);
}

#[test]
fn a_failed_run_is_recorded_as_failed() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut record = admitted(&journal);
    let mut gym = Gym {
        fail: true,
        ..Gym::default()
    };
    assert!(matches!(
        run(&mut record, &catalog(), &mut gym, &journal),
        Dispatched::Failed { .. }
    ));
    let kept = journal.latest("thread-gym", "req-gym").unwrap();
    assert_eq!(kept.state, Lifecycle::Failed);
    assert!(kept.runs[0].artifacts.is_empty());
}

#[test]
fn excluded_unknown_and_ill_typed_capabilities_never_run() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut gym = Gym::default();
    // Excluded by ID.
    let mut excluded = catalog();
    excluded.excluded.push("gym.start_eval".into());
    let mut record = admitted(&journal);
    assert_eq!(
        run(&mut record, &excluded, &mut gym, &journal),
        Dispatched::Refused(RefusalReason::RouteNotAllowed)
    );
    assert_eq!(record.state, Lifecycle::Failed);
    // Another release of the same capability is not the admitted one.
    let mut other = catalog();
    other.releases[0].pin.version = "nip-cj-3".into();
    let mut record = admitted(&journal);
    assert!(matches!(
        run(&mut record, &other, &mut gym, &journal),
        Dispatched::Refused(_)
    ));
    // A release that codes is outside the first eligible set.
    let mut coding = catalog();
    coding.releases[0].noncoding = false;
    let mut record = admitted(&journal);
    assert!(matches!(
        run(&mut record, &coding, &mut gym, &journal),
        Dispatched::Refused(_)
    ));
    // Arguments that do not fit the release's types.
    let mut typed = catalog();
    typed.releases[0]
        .arguments
        .required
        .push(("budget".into(), ArgKind::Number));
    let mut record = admitted(&journal);
    assert!(matches!(
        run(&mut record, &typed, &mut gym, &journal),
        Dispatched::Refused(_)
    ));
    assert_eq!(gym.runs, 0);
    let kept = journal.latest("thread-gym", "req-gym").unwrap();
    assert_eq!(kept.state, Lifecycle::Failed);
}

#[test]
fn a_new_recipient_fee_or_payer_is_a_new_offer() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut gym = Gym::default();
    let changed = |change: &dyn Fn(&mut Release)| {
        let mut catalog = catalog();
        change(&mut catalog.releases[0]);
        catalog
    };
    for catalog in [
        changed(&|release| {
            release.recipient = Recipient {
                kind: RecipientKind::ToolProvider,
                id: "another-runner".into(),
            }
        }),
        changed(&|release| release.fee_sats = 50),
    ] {
        let mut record = admitted(&journal);
        assert!(matches!(
            run(&mut record, &catalog, &mut gym, &journal),
            Dispatched::NewOffer {
                action: Action::PluginRun,
                ..
            }
        ));
        // Still admitted: the person decides on the new offer.
        assert_eq!(record.state, Lifecycle::Admitted);
    }
    // A fee admitted with another payer.
    let mut record = admitted(&journal);
    record.snapshot.money.fees.push(Fee {
        plugin: "gym.start_eval".into(),
        author: "ab".repeat(32),
        sats: 50,
    });
    record.snapshot.money.payers.push(PayerEntry {
        resource: Paid::PluginFee,
        payer: Payer::OpenAgents,
    });
    let caller_pays = changed(&|release| {
        release.fee_sats = 50;
        release.fee_payer = Payer::CallerKey {
            provider: "openrouter".into(),
        };
    });
    assert!(matches!(
        run(&mut record, &caller_pays, &mut gym, &journal),
        Dispatched::NewOffer { .. }
    ));
    assert_eq!(gym.runs, 0);
}

#[test]
fn a_missing_capability_offers_the_build_flow_and_installs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut record = admitted(&journal);
    record.result = RouteResult::MissingCapability {
        need: "convert invoices to CSV".into(),
        remedy: Remedy::Build,
    };
    let mut gym = Gym::default();
    assert!(matches!(
        run(&mut record, &catalog(), &mut gym, &journal),
        Dispatched::NewOffer {
            action: Action::PluginCreate,
            ..
        }
    ));
    record.result = RouteResult::MissingCapability {
        need: "convert invoices to CSV".into(),
        remedy: Remedy::Install {
            plugin: "invoices".into(),
        },
    };
    assert!(matches!(
        run(&mut record, &catalog(), &mut gym, &journal),
        Dispatched::NewOffer {
            action: Action::PluginInstall,
            ..
        }
    ));
    assert_eq!(gym.runs, 0);
    assert!(record.runs.is_empty());
}

#[test]
fn a_model_fallback_stays_within_the_admission_or_is_a_new_offer() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let mut admitted_snapshot = admitted(&journal).snapshot;
    admitted_snapshot.route.model = Some(ModelPin {
        provider: "openrouter".into(),
        model: "large".into(),
    });
    admitted_snapshot.money.quote = Some(Quote {
        id: "q-1".into(),
        max_sats: 100,
        basis: "fixed".into(),
    });
    let candidate = |change: &dyn Fn(&mut AdmissionSnapshot)| {
        let mut snapshot = admitted_snapshot.clone();
        snapshot.route.model = Some(ModelPin {
            provider: "openrouter".into(),
            model: "small".into(),
        });
        change(&mut snapshot);
        snapshot
    };
    assert_eq!(
        fallback(&admitted_snapshot, &candidate(&|_| {}), &catalog()),
        Fallback::Allowed
    );
    // A new recipient.
    let recipient = candidate(&|snapshot| {
        snapshot.disclosure.recipients.push(Recipient {
            kind: RecipientKind::ModelProvider,
            id: "another-provider".into(),
        })
    });
    assert!(matches!(
        fallback(&admitted_snapshot, &recipient, &catalog()),
        Fallback::NewOffer { widenings, .. } if !widenings.is_empty()
    ));
    // A new payer.
    let payer = candidate(&|snapshot| {
        snapshot.money.payers.push(PayerEntry {
            resource: Paid::ChatModel,
            payer: Payer::CallerKey {
                provider: "openrouter".into(),
            },
        })
    });
    assert!(matches!(
        fallback(&admitted_snapshot, &payer, &catalog()),
        Fallback::NewOffer { .. }
    ));
    // A higher price, and a model not adequate for the task.
    let dearer = candidate(&|snapshot| {
        if let Some(quote) = &mut snapshot.money.quote {
            quote.max_sats = 150;
        }
    });
    assert!(matches!(
        fallback(&admitted_snapshot, &dearer, &catalog()),
        Fallback::NewOffer { .. }
    ));
    let weak = candidate(&|snapshot| {
        snapshot.route.model = Some(ModelPin {
            provider: "openrouter".into(),
            model: "tiny".into(),
        })
    });
    assert!(matches!(
        fallback(&admitted_snapshot, &weak, &catalog()),
        Fallback::NewOffer { .. }
    ));
}

fn plugin_pin(version: &str, bytes: &[u8]) -> CapabilityPin {
    CapabilityPin {
        id: "local:meeting-notes".into(),
        version: version.into(),
        digest: Digest::of_bytes(bytes),
    }
}

/// An installed plugin's route, admitted under the shared policy for the
/// exact pin the person confirmed.
fn admitted_plugin(journal: &Journal, pin: &CapabilityPin, request: &str) -> RouteRecord {
    let situation = Situation {
        request: request.into(),
        thread: Some("thread-use".into()),
        ..situation()
    };
    let result = RouteResult::Plugin {
        plugin: PluginRoute::Run {
            capability: pin.clone(),
            arguments: json!({"request": "turn these notes into action items"}),
        },
    };
    let snapshot = admit(
        &result,
        &situation,
        None,
        "turn these notes into action items",
        None,
    );
    let mut record =
        RouteRecord::received(request, situation.thread.clone(), result, snapshot, 1).unwrap();
    record.step(Lifecycle::Proposed, "offer", 2).unwrap();
    record
        .step(Lifecycle::Admitted, "offer_confirmed", 3)
        .unwrap();
    journal.write(&record).unwrap();
    record
}

/// Runs the installed plugin: records each run and returns its reply.
#[derive(Default)]
struct Installed {
    runs: Vec<CapabilityPin>,
}

impl Runner for Installed {
    fn run(&mut self, release: &Release, arguments: &Value) -> Result<Output, String> {
        self.runs.push(release.pin.clone());
        Ok(Output {
            artifact: format!("action items for {}", arguments["request"]).into_bytes(),
            check: CheckLabel::Pending,
            cost_microusd: None,
        })
    }
}

#[test]
fn an_installed_release_is_reused_only_while_exactly_it_is_held() {
    let home = tempfile::tempdir().unwrap();
    let journal = Journal::at(home.path());
    let admitted_pin = plugin_pin("0.1.0", b"package 0.1.0");
    let held = |pin: CapabilityPin, enabled: bool, revoked: Option<bool>| Held {
        pin,
        enabled,
        revoked,
    };
    let dispatch_with = |held: &[Held], request: &str, runner: &mut Installed| {
        let mut record = admitted_plugin(&journal, &admitted_pin, request);
        let catalog = installed_catalog(held);
        dispatch(&mut record, &catalog, runner, &journal, &mut |_, _| {}, 10).unwrap()
    };
    let mut runner = Installed::default();

    // The exact release, on: it runs once under the shared route, with
    // the plugin named as the request's recipient.
    let exact = [held(admitted_pin.clone(), true, Some(false))];
    assert_eq!(reuse(&admitted_pin, &exact), Reuse::Admitted);
    assert!(matches!(
        dispatch_with(&exact, "req-1", &mut runner),
        Dispatched::Ran { .. }
    ));
    assert_eq!(runner.runs, vec![admitted_pin.clone()]);
    let kept = journal.latest("thread-use", "req-1").unwrap();
    assert!(kept.snapshot.disclosure.recipients.contains(&Recipient {
        kind: RecipientKind::Plugin,
        id: "local:meeting-notes".into(),
    }));
    assert_eq!(kept.snapshot.route.capability.as_ref(), Some(&admitted_pin));

    // A newer version, or rebuilt bytes under the same version, is a
    // change: the admitted pin is not resolved to it, and nothing runs.
    for now in [
        plugin_pin("0.2.0", b"package 0.2.0"),
        plugin_pin("0.1.0", b"package 0.1.0 rebuilt"),
    ] {
        let changed = [held(now.clone(), true, Some(false))];
        assert_eq!(
            reuse(&admitted_pin, &changed),
            Reuse::Changed { held: now.clone() }
        );
        assert_eq!(
            dispatch_with(&changed, "req-2", &mut runner),
            Dispatched::Refused(RefusalReason::RouteNotAllowed)
        );
    }
    // Revoked, off, or gone: not reused.
    for (state, word) in [
        (
            vec![held(admitted_pin.clone(), true, Some(true))],
            "revoked",
        ),
        (
            vec![held(admitted_pin.clone(), false, Some(false))],
            "disabled",
        ),
        (Vec::new(), "missing"),
    ] {
        assert_eq!(reuse(&admitted_pin, &state).word(), word);
        assert_eq!(
            dispatch_with(&state, "req-3", &mut runner),
            Dispatched::Refused(RefusalReason::RouteNotAllowed)
        );
    }
    assert_eq!(runner.runs.len(), 1);
}
