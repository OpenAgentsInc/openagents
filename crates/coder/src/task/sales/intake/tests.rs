use super::*;

fn now() -> u64 {
    1000
}
fn later() -> u64 {
    1001
}
fn expired() -> u64 {
    1000 + MAX_RETENTION + 1
}
fn policy() -> Policy {
    Policy {
        schema: POLICY_SCHEMA.into(),
        id: "fixture-v1".into(),
        offer: OFFER.into(),
        origin: "http://127.0.0.1:4300".into(),
        public_owner: "Fixture operator".into(),
        support_email: "operator@example.invalid".into(),
        commercial_approval: "fixture:owner-accepted-proposed-commercial-terms".into(),
        responsibility_acceptance: "fixture:owner-accepted-standing-review-responsibility".into(),
        consent_version: "email-review-v1".into(),
        expires_at: 2000,
        retention_seconds: MAX_RETENTION,
        review_within_seconds: 86400,
        max_leads: 2,
    }
}
fn request(number: u8) -> Submission {
    Submission {
        request: format!("{number:064x}"),
        issued_at: 999,
        email: "buyer@example.invalid".into(),
        account: "Fixture buyer".into(),
        jurisdiction: "US".into(),
        workflow: "One public repository fix".into(),
        referral: Some("opaque-referrer".into()),
        consent_version: "email-review-v1".into(),
        consent: true,
    }
}
fn fixture() -> (tempfile::TempDir, Store, super::super::Access, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner_file = dir.path().join("owner");
    store.initialize("operator", &owner_file).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    let credential = dir.path().join("intake");
    store.issue_intake(&owner, policy(), &credential).unwrap();
    (dir, store, owner, credential)
}
#[test]
fn create_only_authority_preserves_canonical_ownership_consent_and_source() {
    let (_dir, mut s, owner, credential) = fixture();
    let secret = Store::read_credential(&credential).unwrap();
    assert!(s.authenticate(&secret).is_err());
    let access = s.authenticate_intake(&secret).unwrap();
    let ack = s.submit_intake(&access, &request(1)).unwrap();
    assert_eq!(ack.reference, request(1).request);
    let lead = s.list(&owner, None, 1).unwrap().remove(0);
    assert_eq!(lead.responsible_human, "operator");
    assert_eq!(
        lead.ownership_acceptance,
        policy().responsibility_acceptance
    );
    assert_eq!(lead.source_at, 999);
    assert_eq!(
        lead.source,
        "Permissioned public pilot request at http://127.0.0.1:4300/pilot"
    );
    assert_eq!(lead.details.permission.channels, ["email"]);
    assert_eq!(lead.details.permission.state, PermissionState::Granted);
    assert_eq!(lead.details.data.recipients, ["human:operator"]);
    assert_eq!(lead.details.data.permitted_use, USE);
    assert_eq!(lead.details.stage, Stage::New);
    assert_eq!(lead.details.next.unwrap().due_at, 87400);
    assert_eq!(
        lead.intake.unwrap().referral,
        Some("opaque-referrer".into())
    );
    assert!(!serde_json::to_string(&ack).unwrap().contains("lead_"));
}
#[test]
fn dropped_acknowledgment_replays_and_duplicate_forms_preserve_first_record() {
    let (dir, mut s, owner, credential) = fixture();
    let secret = Store::read_credential(&credential).unwrap();
    let access = s.authenticate_intake(&secret).unwrap();
    let first = s.submit_intake(&access, &request(1)).unwrap();
    let mut duplicate = request(2);
    duplicate.email = "BUYER@EXAMPLE.INVALID".into();
    duplicate.workflow = "Changed duplicate workflow".into();
    duplicate.referral = Some("second-referrer".into());
    s.submit_intake(&access, &duplicate).unwrap();
    let leads = s.list(&owner, None, 10).unwrap();
    assert_eq!(leads.len(), 1);
    assert_eq!(leads[0].details.workflow, request(1).workflow);
    assert_eq!(
        leads[0].intake.as_ref().unwrap().referral,
        request(1).referral
    );
    drop(s);
    let mut s = Store::open_with_clock(&dir.path().join("host"), later).unwrap();
    let access = s.authenticate_intake(&secret).unwrap();
    assert_eq!(s.submit_intake(&access, &request(1)).unwrap(), first);
    let mut changed = request(1);
    changed.workflow = "changed retry".into();
    assert!(s.submit_intake(&access, &changed).is_err());
    assert_eq!(s.state.leads.len(), 1);
    assert_eq!(s.state.intakes["fixture-v1"].admitted, 1);
}
#[test]
fn existing_manual_contact_needs_human_reconciliation_without_a_competing_lead() {
    let (_dir, mut s, owner, credential) = fixture();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "warm-manual-record".into(),
        lead: None,
        expected_revision: 0,
        operation: Operation::Create {
            input: Input {
                contact: "email:buyer@example.invalid".into(),
                source: "Private warm introduction".into(),
                source_at: 999,
                details: Details {
                    account: "Manual fixture account".into(),
                    jurisdiction: "US".into(),
                    permission: Permission {
                        state: PermissionState::Unknown,
                        reference: "private-consent-verification-pending".into(),
                        recorded_at: 999,
                        expires_at: 1500,
                        channels: vec![],
                    },
                    workflow: "Privately scoped workflow".into(),
                    baseline_reference: "No comparison supplied".into(),
                    data: DataBoundary {
                        recipients: vec!["human:operator".into()],
                        permitted_use: "Private human review".into(),
                        retain_until: 2000,
                    },
                    stage: Stage::New,
                    next: Some(NextAction {
                        description: "Human verifies scope and consent".into(),
                        due_at: 1100,
                    }),
                    customer_decision: None,
                    readers: vec![],
                },
            },
            ownership_acceptance: "fixture:manual-human-accepted".into(),
        },
    };
    let first = s
        .apply(&owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    let access = s
        .authenticate_intake(&Store::read_credential(&credential).unwrap())
        .unwrap();
    assert!(s.submit_intake(&access, &request(1)).is_err());
    let records = s.list(&owner, None, 10).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, first.lead);
    assert_eq!(records[0].source, "Private warm introduction");
    assert_eq!(
        records[0].details.permission.state,
        PermissionState::Unknown
    );
    assert!(records[0].intake.is_none());
}

#[test]
fn invalid_permission_and_payloads_never_create_records() {
    let (_dir, mut s, _owner, credential) = fixture();
    let access = s
        .authenticate_intake(&Store::read_credential(&credential).unwrap())
        .unwrap();
    for kind in 0..7 {
        let mut submission = request(kind + 1);
        match kind {
            0 => submission.consent = false,
            1 => submission.consent_version = "other-terms".into(),
            2 => submission.issued_at = 1001,
            3 => submission.email = "not an email".into(),
            4 => submission.referral = Some("person@example.invalid".into()),
            5 => submission.workflow = "x".repeat(2049),
            6 => submission.account.clear(),
            _ => unreachable!(),
        }
        assert!(s.submit_intake(&access, &submission).is_err());
    }
    assert!(s.state.leads.is_empty());
    assert!(s.state.intake_submissions.is_empty());
}
#[test]
fn owner_acceptance_is_required_and_caps_revocation_and_suppression_survive() {
    let (dir, mut s, owner, credential) = fixture();
    let owner_secret = Store::read_credential(&dir.path().join("owner")).unwrap();
    assert!(
        s.issue_intake(&owner, policy(), &dir.path().join("owner"))
            .is_err()
    );
    assert!(s.authenticate_intake(&owner_secret).is_err());
    let reader_file = dir.path().join("reader");
    s.issue(&owner, "reader", Role::Reader, &reader_file)
        .unwrap();
    let reader = s
        .authenticate(&Store::read_credential(&reader_file).unwrap())
        .unwrap();
    let mut other = policy();
    other.id = "reader-policy".into();
    assert!(
        s.issue_intake(&reader, other, &dir.path().join("forbidden"))
            .is_err()
    );
    let access = s
        .authenticate_intake(&Store::read_credential(&credential).unwrap())
        .unwrap();
    s.submit_intake(&access, &request(1)).unwrap();
    let mut second = request(2);
    second.email = "second@example.invalid".into();
    s.submit_intake(&access, &second).unwrap();
    let mut third = request(3);
    third.email = "third@example.invalid".into();
    assert!(s.submit_intake(&access, &third).is_err());
    let lead = s
        .list(&owner, None, 10)
        .unwrap()
        .into_iter()
        .find(|l| l.contact == "email:buyer@example.invalid")
        .unwrap();
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        id: "delete-first".into(),
        lead: Some(lead.id),
        expected_revision: 1,
        operation: Operation::Delete {
            reference: "fixture:withdrawal".into(),
        },
    };
    s.apply(&owner, &serde_json::to_vec(&command).unwrap())
        .unwrap();
    assert!(s.submit_intake(&access, &request(4)).is_err());
    assert!(s.submit_intake(&access, &third).is_err());
    let persisted = std::fs::read_to_string(dir.path().join("host/sales/state.json")).unwrap();
    assert!(!persisted.contains("buyer@example.invalid"));
    s.revoke_intake(&owner, "fixture-v1").unwrap();
    assert!(s.submit_intake(&access, &second).is_err());
}
#[test]
fn retention_removes_intake_content_and_old_pipeline_schema_still_reads() {
    let (dir, mut s, _owner, credential) = fixture();
    let access = s
        .authenticate_intake(&Store::read_credential(&credential).unwrap())
        .unwrap();
    let mut submission = request(1);
    submission.workflow = "Private fixture text to erase".into();
    s.submit_intake(&access, &submission).unwrap();
    drop(s);
    let s = Store::open_with_clock(&dir.path().join("host"), expired).unwrap();
    assert!(s.state.leads.is_empty());
    let persisted = std::fs::read_to_string(dir.path().join("host/sales/state.json")).unwrap();
    assert!(!persisted.contains("buyer@example.invalid"));
    assert!(!persisted.contains("Private fixture text to erase"));
    assert!(!persisted.contains("opaque-referrer"));
    drop(s);
    let mut old: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    old.as_object_mut().unwrap().remove("intakes");
    old.as_object_mut().unwrap().remove("intake_submissions");
    std::fs::write(
        dir.path().join("host/sales/state.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    assert!(Store::open_with_clock(&dir.path().join("host"), now).is_ok());
}
