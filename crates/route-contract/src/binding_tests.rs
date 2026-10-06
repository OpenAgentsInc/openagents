//! Workbench bindings (#10669): continuation on the same task and engine
//! session without widening, refusals before any execution, and offers
//! bound to exact terms and expiry.

use workbench::{Host, Kind, ResourceRef};

use crate::binding::{Current, GrantNow, HostPlacement, Refusal, RunBinding, WorkbenchBinding};
use crate::offer::{self, Action, Offer, Price, Terms};
use crate::snapshot::*;
use crate::tests::{snapshot, terms};

const HOST: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const GENERATION: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const RESTARTED: &str = "b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3";
const TERMINAL: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";

fn terminal(generation: &str) -> ResourceRef {
    ResourceRef::terminal(Host::Paired { key: HOST.into() }, generation, TERMINAL)
}

fn run() -> RunBinding {
    RunBinding {
        task: "task_1".into(),
        engine: "codex".into(),
        session: Some("sess_1".into()),
    }
}

/// The first route: admitted, dispatched as `task_1` in engine session
/// `sess_1`, from a terminal on the host.
fn parent() -> (AdmissionSnapshot, WorkbenchBinding) {
    let snapshot = snapshot();
    let binding = WorkbenchBinding {
        schema: crate::BINDING_SCHEMA.into(),
        snapshot: snapshot.digest(),
        parent: None,
        placement: HostPlacement {
            computer: "cmp_here".into(),
            generation: GENERATION.into(),
            recipient: HOST.into(),
        },
        run: Some(run()),
        terminal: Some(terminal(GENERATION)),
        resources: vec![ResourceRef::new(
            Kind::Thread,
            Host::Paired { key: HOST.into() },
            "th_1",
        )],
    };
    (snapshot, binding)
}

/// A follow-up that continues the same run under the parent's admission.
fn child(parent: &(AdmissionSnapshot, WorkbenchBinding)) -> (AdmissionSnapshot, WorkbenchBinding) {
    let mut snapshot = parent.0.clone();
    snapshot.inherits = Some(parent.0.digest());
    snapshot.identity.task = Some("task_1".into());
    snapshot.identity.request = "req_2".into();
    snapshot.placement.grant.as_mut().unwrap().source = GrantSource::Continuation;
    let mut binding = parent.1.clone();
    binding.parent = Some(parent.1.digest());
    binding.snapshot = snapshot.digest();
    (snapshot, binding)
}

/// Rebinds `binding` to `snapshot` after a test edits the snapshot.
fn rebind(pair: &mut (AdmissionSnapshot, WorkbenchBinding)) {
    pair.1.snapshot = pair.0.digest();
}

fn current() -> Current {
    Current {
        computer: "cmp_here".into(),
        generation: GENERATION.into(),
        recipient: HOST.into(),
        grant: Some(GrantNow {
            id: "grant_1".into(),
            epoch: 3,
            revoked: false,
        }),
        terminal_generation: Some(GENERATION.into()),
    }
}

/// A stand-in executor: it runs only after the recheck admits.
fn dispatch(
    pair: &(AdmissionSnapshot, WorkbenchBinding),
    now: &Current,
    runs: &mut u32,
) -> Result<(), Refusal> {
    pair.1.recheck(&pair.0, now)?;
    *runs += 1;
    Ok(())
}

#[test]
fn a_binding_round_trips_and_names_its_snapshot() {
    let (snapshot, binding) = parent();
    binding.check(&snapshot).unwrap();
    let text = serde_json::to_string(&binding).unwrap();
    let back: WorkbenchBinding = serde_json::from_str(&text).unwrap();
    assert_eq!(back, binding);
    assert_eq!(back.digest(), binding.digest());
    for field in [
        "\"schema\"",
        "\"placement\"",
        "\"generation\"",
        "\"recipient\"",
        "\"session\"",
        "\"terminal\"",
    ] {
        assert!(text.contains(field), "{field} in {text}");
    }
    let mut extra: serde_json::Value = serde_json::from_str(&text).unwrap();
    extra["price"] = serde_json::json!(1);
    assert!(serde_json::from_value::<WorkbenchBinding>(extra).is_err());

    let mut other = binding.clone();
    other.snapshot = crate::Digest::of_bytes(b"another snapshot");
    assert_eq!(other.check(&snapshot), Err(Refusal::SnapshotMismatch));
    let mut elsewhere = binding;
    elsewhere.placement.computer = "cmp_other".into();
    assert_eq!(elsewhere.check(&snapshot), Err(Refusal::SnapshotMismatch));
}

#[test]
fn a_continuation_runs_on_the_same_task_and_engine_session_without_widening() {
    let first = parent();
    let next = child(&first);
    next.1.continues(&next.0, &first.1, &first.0).unwrap();
    assert!(next.0.widens(&first.0).is_empty());
    let mut runs = 0;
    dispatch(&next, &current(), &mut runs).unwrap();
    assert_eq!(runs, 1);
    // A narrower continuation still continues.
    let mut narrower = child(&first);
    narrower.0.disclosure.recipients.truncate(1);
    rebind(&mut narrower);
    narrower
        .1
        .continues(&narrower.0, &first.1, &first.0)
        .unwrap();
}

#[test]
fn a_mismatched_or_widened_continuation_refuses_before_any_execution() {
    let first = parent();
    let mut cases: Vec<(&str, (AdmissionSnapshot, WorkbenchBinding), Refusal)> = Vec::new();

    let mut widened = child(&first);
    widened.0.disclosure.recipients.push(Recipient {
        kind: RecipientKind::ModelProvider,
        id: "xai".into(),
    });
    widened.0.effects.writes = WriteScope::Workspace;
    rebind(&mut widened);
    cases.push((
        "a new recipient and wider writes",
        widened,
        Refusal::Widened {
            widenings: vec![Widening::Writes, Widening::Disclosure],
        },
    ));

    let mut payer = child(&first);
    payer.0.money.payers[0].payer = Payer::OpenAgents;
    rebind(&mut payer);
    cases.push((
        "a payer switched to OpenAgents",
        payer,
        Refusal::Widened {
            widenings: vec![Widening::Payer],
        },
    ));

    let mut computer = child(&first);
    computer.0.placement.computer = Some("cmp_other".into());
    computer.1.placement.computer = "cmp_other".into();
    rebind(&mut computer);
    cases.push((
        "another computer",
        computer,
        Refusal::Widened {
            widenings: vec![Widening::Computer],
        },
    ));

    let mut recipient = child(&first);
    recipient.1.placement.recipient = "f".repeat(64);
    cases.push((
        "another dispatch recipient",
        recipient,
        Refusal::PlacementChanged,
    ));

    let mut restarted = child(&first);
    restarted.1.placement.generation = RESTARTED.into();
    cases.push(("a restarted host", restarted, Refusal::PlacementChanged));

    let mut session = child(&first);
    session.1.run.as_mut().unwrap().session = Some("sess_2".into());
    cases.push(("another engine session", session, Refusal::RunChanged));

    let mut engine = child(&first);
    engine.1.run.as_mut().unwrap().engine = "claude".into();
    cases.push(("another engine", engine, Refusal::RunChanged));

    let mut no_session = child(&first);
    no_session.1.run.as_mut().unwrap().session = None;
    cases.push((
        "no engine session to resume",
        no_session,
        Refusal::RunChanged,
    ));

    let mut terminal_gen = child(&first);
    terminal_gen.1.terminal = Some(terminal(RESTARTED));
    cases.push((
        "another terminal generation",
        terminal_gen,
        Refusal::TerminalChanged,
    ));

    let mut orphan = child(&first);
    orphan.0.inherits = Some(crate::Digest::of_bytes(b"another parent"));
    rebind(&mut orphan);
    cases.push((
        "a snapshot that inherits from another parent",
        orphan,
        Refusal::NotAContinuation,
    ));

    let mut unbound = child(&first);
    unbound.1.parent = None;
    cases.push((
        "a binding without its parent",
        unbound,
        Refusal::NotAContinuation,
    ));

    let mut task = child(&first);
    task.0.identity.task = Some("task_2".into());
    rebind(&mut task);
    cases.push((
        "a snapshot for another task",
        task,
        Refusal::SnapshotMismatch,
    ));

    for (why, pair, want) in cases {
        let mut runs = 0;
        let admitted = pair
            .1
            .continues(&pair.0, &first.1, &first.0)
            .and_then(|()| dispatch(&pair, &current(), &mut runs));
        assert_eq!(admitted, Err(want), "{why}");
        assert_eq!(runs, 0, "{why}: nothing ran");
    }
}

#[test]
fn stale_or_revoked_admissions_refuse_at_dispatch_and_control() {
    let first = parent();
    let next = child(&first);
    let mut cases: Vec<(&str, Current, Refusal)> = Vec::new();
    let mut restarted = current();
    restarted.generation = RESTARTED.into();
    cases.push(("the host restarted", restarted, Refusal::Stale));
    let mut epoch = current();
    epoch.grant.as_mut().unwrap().epoch = 4;
    cases.push(("the grant's revocation epoch moved", epoch, Refusal::Stale));
    let mut revoked = current();
    revoked.grant.as_mut().unwrap().revoked = true;
    cases.push(("the grant was revoked", revoked, Refusal::Revoked));
    let mut gone = current();
    gone.grant = None;
    cases.push(("the grant is gone", gone, Refusal::Revoked));
    let mut swapped = current();
    swapped.grant.as_mut().unwrap().id = "grant_2".into();
    cases.push(("another grant", swapped, Refusal::Revoked));
    let mut terminal_ended = current();
    terminal_ended.terminal_generation = None;
    cases.push((
        "the terminal's generation ended",
        terminal_ended,
        Refusal::Stale,
    ));
    let mut moved = current();
    moved.computer = "cmp_other".into();
    cases.push(("another computer answers", moved, Refusal::PlacementChanged));
    let mut recipient = current();
    recipient.recipient = "f".repeat(64);
    cases.push(("another recipient", recipient, Refusal::PlacementChanged));

    for (why, now, want) in cases {
        for pair in [&first, &next] {
            let mut runs = 0;
            assert_eq!(dispatch(pair, &now, &mut runs), Err(want.clone()), "{why}");
            assert_eq!(runs, 0, "{why}: nothing ran");
            // A control operation (steer, cancel) rechecks the same way.
            assert_eq!(pair.1.recheck(&pair.0, &now), Err(want.clone()), "{why}");
        }
    }
}

#[test]
fn every_material_change_is_a_new_offer_and_never_approves_the_old_digest() {
    let base = terms();
    let offer = Offer::new(
        "cf_1".into(),
        Action::RunStart,
        "Run".into(),
        100,
        400,
        base.clone(),
    );
    offer.confirm(&offer.digest, 200, &base).unwrap();

    let mut changes: Vec<(&str, Terms)> = Vec::new();
    let mut computer = base.clone();
    computer.computer = Some("cmp_other".into());
    changes.push(("computer", computer));
    let mut recipient = base.clone();
    recipient.recipients.push(Recipient {
        kind: RecipientKind::ModelProvider,
        id: "xai".into(),
    });
    changes.push(("recipient", recipient));
    let mut effect = base.clone();
    effect.effects.publication.push(Publication::Push);
    changes.push(("effect", effect));
    let mut price = base.clone();
    price.price = Some(Price {
        max_sats: 211,
        fees: Vec::new(),
    });
    changes.push(("price", price));
    let mut fee = base.clone();
    fee.price.as_mut().unwrap().fees.push(Fee {
        plugin: "pl_x".into(),
        author: "npub1author".into(),
        sats: 5,
    });
    changes.push(("fee", fee));
    // The payer lives in the snapshot, so it moves the snapshot digest.
    let mut paid = snapshot();
    paid.money.payers[0].payer = Payer::OpenAgents;
    let mut payer = base.clone();
    payer.snapshot = paid.digest();
    changes.push(("payer", payer));

    for (what, changed) in changes {
        // Confirming the old offer against changed terms refuses.
        assert_eq!(
            offer.confirm(&offer.digest, 200, &changed),
            Err(offer::Refusal::Changed),
            "{what}"
        );
        // The changed terms are a new offer with a new digest, which the
        // old confirmation cannot approve.
        let renewed = Offer::new(
            "cf_2".into(),
            Action::RunStart,
            "Run".into(),
            100,
            400,
            changed.clone(),
        );
        assert_ne!(renewed.digest, offer.digest, "{what}");
        assert_eq!(
            renewed.confirm(&offer.digest, 200, &changed),
            Err(offer::Refusal::Mismatch),
            "{what}"
        );
        renewed.confirm(&renewed.digest, 200, &changed).unwrap();
    }
}

#[test]
fn a_timeout_cannot_authorize_a_substitute() {
    let base = terms();
    let offer = Offer::new(
        "cf_1".into(),
        Action::RunStart,
        "Run".into(),
        100,
        400,
        base.clone(),
    );
    // At expiry the confirmation is refused, even with unchanged terms.
    assert_eq!(
        offer.confirm(&offer.digest, 400, &base),
        Err(offer::Refusal::Expired)
    );
    // A fallback after the timeout, to another computer, is a new offer
    // the old confirmation never approves.
    let mut fallback = base;
    fallback.computer = Some("cmp_cloud".into());
    let substitute = Offer::new(
        "cf_3".into(),
        Action::RunStart,
        "Run".into(),
        400,
        700,
        fallback.clone(),
    );
    assert_eq!(
        substitute.confirm(&offer.digest, 401, &fallback),
        Err(offer::Refusal::Mismatch)
    );
    // Relabeling or re-timing is not an edit that keeps the digest.
    let retimed = Offer::new(
        "cf_1".into(),
        Action::RunStart,
        "Run".into(),
        100,
        900,
        terms(),
    );
    assert_ne!(retimed.digest, offer.digest);
}
