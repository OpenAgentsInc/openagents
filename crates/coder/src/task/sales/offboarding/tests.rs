//! Synthetic data only.
use super::*;
use crate::task::sales::tests::service_fixture::{doc, retain};
use crate::task::sales::tests::{command, grant, now, service_apply, service_setup};
use crate::task::sales::{Operation, Role};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

const SALE: &str = "synthetic-sale";

/// A removal of one plan item: the operation result and a later re-read.
pub(crate) fn removed(root: &Path, item: &str, target: &str, operation: &str) -> ItemInput {
    let op = doc(
        root,
        &format!("{item}-operation.json"),
        json!({"schema":OPERATION_SCHEMA,"plan_item":item,"target":target,
            "operation":operation,"result":"revoked","at":990}),
    );
    let reread = doc(
        root,
        &format!("{item}-reread.json"),
        json!({"schema":REREAD_SCHEMA,"plan_item":item,"target":target,
            "observed":"absent","at":995}),
    );
    ItemInput {
        plan_item: item.into(),
        decision: Some(Decision::Removed),
        operation_evidence: Some(op),
        reread_evidence: Some(reread),
        ..ItemInput::default()
    }
}

pub(crate) fn kept(root: &Path, item: &str, until: u64) -> ItemInput {
    ItemInput {
        plan_item: item.into(),
        decision: Some(Decision::Kept),
        kept_until: Some(until),
        requirement: Some("synthetic invoice record kept for the agreed period".into()),
        requirement_evidence: Some(retain(
            root,
            &format!("{item}-requirement"),
            b"synthetic agreement clause requiring the record",
        )),
        ..ItemInput::default()
    }
}

pub(crate) fn pending(item: &str) -> ItemInput {
    ItemInput {
        plan_item: item.into(),
        decision: Some(Decision::Pending),
        next_action: Some("remove the synthetic local copy".into()),
        next_action_due_at: Some(1040),
        ..ItemInput::default()
    }
}

/// The admitted sale's handoff digest.
fn handoff(store: &mut Store, owner: &Access, lead: &str) -> String {
    store
        .service_show(owner, lead, SALE)
        .unwrap()
        .admission
        .sources
        .handoff
        .sha256
}

fn report(handoff: &str, items: Vec<ItemInput>) -> Operation {
    Operation::RecordOffboarding {
        sale: SALE.into(),
        report: Report {
            handoff_sha256: handoff.into(),
            items,
        },
    }
}

fn admit(
    store: &mut Store,
    owner: &Access,
    root: &Path,
    lead: &str,
    admission: receipts::service_sale::Admission,
) {
    service_apply(
        store,
        owner,
        root,
        lead,
        "admit-sale",
        Operation::RecordServiceSale { admission },
    );
}

#[test]
fn removed_kept_and_open_items_read_as_plain_states_and_survive_restart() {
    let (dir, mut store, owner, lead, admission) = service_setup();
    let root = dir.path().join("evidence");
    admit(&mut store, &owner, &root, &lead, admission);
    assert_eq!(store.offboarding_show(&owner, &lead, SALE).unwrap(), None);
    let digest = handoff(&mut store, &owner, &lead);
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "offboard",
        report(
            &digest,
            vec![
                removed(
                    &root,
                    "temporary-credentials",
                    "synthetic-temporary-key",
                    "synthetic-revoke-key",
                ),
                kept(&root, "test-data", 1800),
            ],
        ),
    );
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "offboard-open",
        report(&digest, vec![pending("local-copies")]),
    );
    let view = store
        .offboarding_show(&owner, &lead, SALE)
        .unwrap()
        .unwrap();
    let plain: Vec<String> = view.rows.iter().map(|r| r.state.plain()).collect();
    assert_eq!(
        plain,
        [
            "Removed 1970-01-01",
            "Kept until 1970-01-01 (required)",
            "Not yet removed (due 1970-01-01)",
        ]
    );
    assert_eq!(
        view.rows[0].state,
        State::Removed {
            at: 995,
            operation: "synthetic-revoke-key".into()
        }
    );
    assert_eq!(
        view.summary(),
        "1 removed, 1 kept as required, 1 not yet removed"
    );
    let record = store.show(&owner, &lead).unwrap().offboarding[SALE].clone();
    let removal = record.items["temporary-credentials"]
        .removal
        .clone()
        .unwrap();
    assert_eq!(
        (
            removal.target.as_str(),
            removal.result.as_str(),
            removal.removed_at
        ),
        ("synthetic-temporary-key", "revoked", 990)
    );
    assert!(
        store
            .audit(&owner, 0, 100)
            .unwrap()
            .iter()
            .any(|a| a.operation == "offboarding_recorded")
    );

    // A later report finishes the open item; everything left is done.
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "offboard-done",
        report(
            &digest,
            vec![removed(
                &root,
                "local-copies",
                "synthetic-local-copy",
                "synthetic-remove-copy",
            )],
        ),
    );
    drop(store);
    let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
        .unwrap();
    let view = store
        .offboarding_show(&owner, &lead, SALE)
        .unwrap()
        .unwrap();
    assert!(view.rows.iter().all(|r| r.state.done()));
    assert_eq!(
        view.summary(),
        "2 removed, 1 kept as required, nothing left to remove"
    );
}

#[test]
fn nothing_is_marked_done_without_matching_evidence() {
    let (dir, mut store, owner, lead, admission) = service_setup();
    let root = dir.path().join("evidence");
    admit(&mut store, &owner, &root, &lead, admission);
    let digest = handoff(&mut store, &owner, &lead);
    let good = || {
        removed(
            &root,
            "temporary-credentials",
            "synthetic-temporary-key",
            "synthetic-revoke-key",
        )
    };
    let reread = |name: &str, value: serde_json::Value| ItemInput {
        reread_evidence: Some(doc(&root, name, value)),
        ..good()
    };
    let mut cases: Vec<(&str, Report, Option<&Path>)> = Vec::new();
    let one = |item: ItemInput| Report {
        handoff_sha256: digest.clone(),
        items: vec![item],
    };
    cases.push((
        "re-read",
        one(ItemInput {
            reread_evidence: None,
            ..good()
        }),
        Some(&root),
    ));
    cases.push((
        "gone",
        one(reread(
            "present.json",
            json!({"schema":REREAD_SCHEMA,"plan_item":"temporary-credentials",
            "target":"synthetic-temporary-key","observed":"present","at":995}),
        )),
        Some(&root),
    ));
    cases.push((
        "different",
        one(reread(
            "another.json",
            json!({"schema":REREAD_SCHEMA,"plan_item":"temporary-credentials",
            "target":"another-key","observed":"absent","at":995}),
        )),
        Some(&root),
    ));
    cases.push((
        "follow",
        one(reread(
            "early.json",
            json!({"schema":REREAD_SCHEMA,"plan_item":"temporary-credentials",
            "target":"synthetic-temporary-key","observed":"absent","at":980}),
        )),
        Some(&root),
    ));
    cases.push((
        "different",
        one(ItemInput {
            operation_evidence: Some(doc(
                &root,
                "other-operation.json",
                json!({"schema":OPERATION_SCHEMA,"plan_item":"temporary-credentials",
                    "target":"synthetic-temporary-key","operation":"some-other-operation",
                    "result":"revoked","at":990}),
            )),
            ..good()
        }),
        Some(&root),
    ));
    let requested = doc(
        &root,
        "requested-operation.json",
        json!({"schema":OPERATION_SCHEMA,"plan_item":"temporary-credentials",
            "target":"synthetic-temporary-key","operation":"synthetic-revoke-key",
            "result":"requested","at":990}),
    );
    cases.push((
        "did not report",
        one(ItemInput {
            operation_evidence: Some(requested),
            ..good()
        }),
        Some(&root),
    ));
    let mut stale = good();
    stale.reread_evidence.as_mut().unwrap().sha256 = "0".repeat(64);
    cases.push(("changed", one(stale), Some(&root)));
    cases.push(("private evidence root", one(good()), None));
    cases.push((
        "future end date",
        one(kept(&root, "test-data", 1000)),
        Some(&root),
    ));
    cases.push((
        "requirement",
        one(ItemInput {
            requirement_evidence: None,
            ..kept(&root, "test-data", 1800)
        }),
        Some(&root),
    ));
    cases.push((
        "explicit reason",
        one(ItemInput {
            plan_item: "test-data".into(),
            decision: Some(Decision::NotRequired),
            ..ItemInput::default()
        }),
        Some(&root),
    ));
    cases.push((
        "attempt",
        one(ItemInput {
            decision: Some(Decision::Unknown),
            ..pending("local-copies")
        }),
        Some(&root),
    ));
    cases.push((
        "outside the plan",
        one(pending("someone-elses-item")),
        Some(&root),
    ));
    cases.push((
        "different handoff",
        Report {
            handoff_sha256: "f".repeat(64),
            items: vec![good()],
        },
        Some(&root),
    ));
    cases.push((
        "repeats",
        Report {
            handoff_sha256: digest.clone(),
            items: vec![pending("local-copies"), pending("local-copies")],
        },
        Some(&root),
    ));
    for (n, (expected, report, evidence)) in cases.into_iter().enumerate() {
        let revision = store.show(&owner, &lead).unwrap().revision;
        let error = store
            .apply_with_evidence_root(
                &owner,
                &command(
                    &format!("refused-{n}"),
                    Some(&lead),
                    revision,
                    Operation::RecordOffboarding {
                        sale: SALE.into(),
                        report,
                    },
                ),
                evidence,
            )
            .unwrap_err();
        assert!(error.contains(expected), "case {n}: {error}");
    }

    // Only the sales owner records offboarding.
    let writer = grant(&dir, &mut store, &owner, "writer-a", Role::Writer);
    let revision = store.show(&owner, &lead).unwrap().revision;
    assert!(
        store
            .apply_with_evidence_root(
                &writer,
                &command(
                    "writer-offboard",
                    Some(&lead),
                    revision,
                    Operation::RecordOffboarding {
                        sale: SALE.into(),
                        report: Report {
                            handoff_sha256: digest.clone(),
                            items: vec![pending("local-copies")],
                        },
                    },
                ),
                Some(&root),
            )
            .is_err()
    );
    assert_eq!(store.offboarding_show(&owner, &lead, SALE).unwrap(), None);
}

static CLOCK: AtomicU64 = AtomicU64::new(1000);
fn moving() -> u64 {
    CLOCK.load(Ordering::SeqCst)
}

#[test]
fn a_keep_ends_on_its_date_and_the_record_leaves_with_its_sale() {
    let (dir, mut store, owner, lead, admission) = service_setup();
    let root = dir.path().join("evidence");
    admit(&mut store, &owner, &root, &lead, admission);
    let digest = handoff(&mut store, &owner, &lead);
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "keep",
        report(&digest, vec![kept(&root, "test-data", 1500)]),
    );
    // A reader outside the sale's recipients sees no offboarding.
    let reader = grant(&dir, &mut store, &owner, "outside-reader", Role::Reader);
    let _ = store
        .show(&reader, &lead)
        .map(|l| assert!(l.offboarding.is_empty()));
    drop(store);

    CLOCK.store(1600, Ordering::SeqCst);
    let mut store = Store::open_with_clock(&dir.path().join("host"), moving).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
        .unwrap();
    let view = store
        .offboarding_show(&owner, &lead, SALE)
        .unwrap()
        .unwrap();
    let row = view.rows.iter().find(|r| r.id == "test-data").unwrap();
    assert_eq!(row.state, State::KeepEnded { until: 1500 });
    assert_eq!(
        row.state.plain(),
        "Not yet removed (was kept until 1970-01-01)"
    );
    assert!(!row.state.done());
    drop(store);

    // Past the sale's retention the sale and its offboarding record go.
    CLOCK.store(2001, Ordering::SeqCst);
    let mut store = Store::open_with_clock(&dir.path().join("host"), moving).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
        .unwrap();
    assert!(store.offboarding_show(&owner, &lead, SALE).is_err());
    let state = std::fs::read_to_string(dir.path().join("host/sales/state.json")).unwrap();
    assert!(!state.contains("synthetic-revoke-key"));
}

#[test]
fn plain_dates_render_as_calendar_days() {
    assert_eq!(date(0), "1970-01-01");
    assert_eq!(date(1_791_000_000), "2026-10-03");
    assert_eq!(
        State::Kept {
            until: 1_791_000_000,
            requirement: "synthetic".into()
        }
        .plain(),
        "Kept until 2026-10-03 (required)"
    );
}
