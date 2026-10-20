use super::*;
use crate::task::sales::Role;

fn now() -> u64 {
    1_800_000_000
}

fn fixture() -> (tempfile::TempDir, Store, Access, Access) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner_file = dir.path().join("owner");
    store.initialize("operator", &owner_file).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    let reader_file = dir.path().join("reader");
    store
        .issue(&owner, "reader", Role::Reader, &reader_file)
        .unwrap();
    let reader = store
        .authenticate(&Store::read_credential(&reader_file).unwrap())
        .unwrap();
    (dir, store, owner, reader)
}

#[test]
fn an_empty_floor_reports_unknowns_not_zero_success() {
    let (_dir, mut store, owner, _) = fixture();
    let report = store.floor_report(&owner).unwrap();
    assert_eq!(report.leads, 0);
    assert_eq!(report.messages.sent, 0);
    assert!(!report.delivery.telemetry_absent);
    assert!(matches!(
        report.costs.billed_usd_millionths,
        Measure::Unknown { .. }
    ));
    assert!(matches!(
        report.costs.per_qualified_lead_usd_millionths,
        Measure::Unknown { .. }
    ));
    assert!(report.gaps.iter().any(|g| g.contains("not activated")));
    let again = store.floor_report(&owner).unwrap();
    assert_eq!(report, again);
}

#[test]
fn reads_need_the_owner() {
    let (_dir, mut store, owner, reader) = fixture();
    let e = store.floor_escalations(&owner).unwrap();
    assert!(e.immediate.is_empty() && e.review.is_empty());
    assert!(store.floor_report(&reader).is_err());
    assert!(store.floor_escalations(&reader).is_err());
    assert!(store.floor_weekly_draft(&reader).is_err());
}

#[test]
fn the_weekly_draft_is_never_published() {
    let (_dir, mut store, owner, _) = fixture();
    let draft = store.floor_weekly_draft(&owner).unwrap();
    assert!(!draft.published);
    assert_eq!(draft.period_end - draft.period_start, 7 * 24 * 3600);
    assert!(!draft.proposed_next_actions.is_empty());
}

#[test]
fn a_complaint_escalates_once_however_often_it_is_recorded() {
    let (_dir, mut store, owner, _) = fixture();
    let incident = outbox::Incident {
        id: "smtp-1".into(),
        kind: outbox::IncidentKind::Complaint,
        reference_sha256: "ab".repeat(32),
        at: now() - 5,
        resolved_at: None,
    };
    for _ in 0..3 {
        store
            .state
            .outbox
            .incidents
            .insert(incident.id.clone(), incident.clone());
    }
    store.state.outbox.paused = true;
    let report = store.project_floor(&owner, now());
    assert_eq!(report.delivery.complaint, 1);
    assert_eq!(report.unresolved_incidents, 1);
    assert!(report.outbox_paused);
    let e = store.floor_escalations(&owner).unwrap();
    let kinds: Vec<&str> = e.immediate.iter().map(|x| x.kind.as_str()).collect();
    assert_eq!(kinds, ["complaint", "outbox_paused"]);
    assert_eq!(e.immediate[0].reference, "ab".repeat(32));
    assert!(e.review.is_empty());
}
