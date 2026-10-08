use super::*;
use std::collections::VecDeque;
struct Fixture(VecDeque<Read>);
impl Source for Fixture {
    fn read(&mut self) -> Read {
        self.0.pop_front().unwrap_or(Read::Pending)
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        pipeline: [1, 2, 0, 0, 0],
        pending_drafts: 2,
        certificate_records: [1, 1, 0],
        practice_records: 10,
        proposals: vec!["a".repeat(64)],
        outbox_live: Vec::new(),
        outbox_fixture: Vec::new(),
        outbox_unknown: 0,
        idle: false,
        model_available: false,
    }
}
#[test]
fn missing_shared_and_failed_sources_clear_private_state_then_reconnect() {
    let mut floor = Floor::default();
    assert!(floor.snapshot().is_none());
    assert!(floor.lines().iter().any(|l| l.contains("UNAVAILABLE")));
    floor.set_source(Some(Box::new(Fixture(VecDeque::from([
        Read::Ready(snapshot(), 0.0),
        Read::Unavailable,
        Read::Ready(snapshot(), 0.0),
    ])))));
    floor.poll(false, 0.1);
    assert!(floor.snapshot().is_none());
    floor.poll(true, 0.1);
    assert_eq!(floor.snapshot().unwrap().pending_drafts, 2);
    floor.poll(true, 0.1);
    assert!(floor.snapshot().is_none());
    floor.poll(true, 0.1);
    assert_eq!(floor.snapshot().unwrap().pipeline, [1, 2, 0, 0, 0]);
    floor.poll(false, 0.1);
    assert!(floor.snapshot().is_none());
}
#[test]
fn stalled_reads_expire_and_invalid_private_text_never_reaches_a_mesh() {
    let mut bad = snapshot();
    bad.proposals = vec!["private-prospect-company-message-secret".into()];
    let mut floor = Floor::default();
    floor.set_source(Some(Box::new(Fixture(VecDeque::from([
        Read::Ready(snapshot(), 0.0),
        Read::Pending,
        Read::Ready(bad, 0.0),
    ])))));
    floor.poll(true, 0.1);
    assert!(floor.mesh().faces.len() > Floor::default().mesh().faces.len());
    floor.poll(true, 3.1);
    assert!(floor.snapshot().is_none());
    floor.poll(true, 0.1);
    assert!(floor.snapshot().is_none());
    assert!(!floor.lines().join(" ").contains("private-prospect"));
    assert!(!floor.lines().join(" ").contains("NEW 0"));
}
#[test]
fn measured_records_and_written_props_do_not_claim_outbound_authority() {
    let mut floor = Floor::default();
    floor.set_source(Some(Box::new(Fixture(VecDeque::from([Read::Ready(
        snapshot(),
        0.0,
    )])))));
    floor.poll(true, 0.1);
    let text = floor.lines().join(" ");
    assert!(text.contains("PHONES ARE PROPS"));
    assert!(text.contains("MODEL UNAVAILABLE"));
    assert!(text.contains("NO SEND OR PAYMENT AUTHORITY"));
    assert!(text.contains("SYNTHETIC") || text.contains("PRACTICE"));
    floor.set_source(None);
    assert!(floor.snapshot().is_none());
}
