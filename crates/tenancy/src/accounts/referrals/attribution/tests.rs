use super::*;

fn fixture() -> (tempfile::TempDir, Accounts, Account, Account, Policy) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let a = accounts.create_account("Synthetic referrer", &[]).unwrap();
    let b = accounts
        .create_account("Other synthetic account", &[])
        .unwrap();
    let policy = Policy::new("fixture-v1".into(), "Retain this agreed relationship across credentials and team ownership. Exceptions require both parties' explicit review. This fixture grants no commission.".into()).unwrap();
    accounts.publish_attribution_policy(&policy).unwrap();
    (dir, accounts, a, b, policy)
}
fn proposal(
    policy: &Policy,
    request: &str,
    introduction: Introduction,
    referrer: Option<&str>,
) -> Proposal {
    Proposal {
        request: request.into(),
        policy_digest: policy.digest.clone(),
        introduction,
        referrer: referrer.map(str::to_owned),
        evidence: if introduction == Introduction::CapturedSource {
            vec![]
        } else {
            vec![Evidence {
                reference: "private-agreement-1".into(),
                digest: format!("sha256:{}", "a".repeat(64)),
            }]
        },
        reason: "Both parties review the retained agreement.".into(),
        consent: true,
        expected_decision: None,
    }
}
fn signup(accounts: &Accounts, owner: &Account) -> (Account, Referrer, Source) {
    let r = accounts
        .create_referrer(&owner.id, Kind::Person, "Synthetic source")
        .unwrap();
    let link = accounts.issue_referral_link(&owner.id, &r.id).unwrap();
    let (customer, source) = accounts
        .create_account_acquired(
            "Consenting buyer",
            &Capture {
                request: "signup".into(),
                token: Some(link.token),
                consent: true,
                consent_version: Some(CONSENT.into()),
            },
        )
        .unwrap();
    (customer, r, source)
}

#[test]
fn accepted_signup_survives_team_owner_rotation_and_restart_without_commission() {
    let (dir, accounts, referrer_owner, next, policy) = fixture();
    let (customer, r, source) = signup(&accounts, &referrer_owner);
    let personal = accounts
        .create_workspace(
            &customer.id,
            "Personal",
            WorkspaceKind::Personal,
            "tenant",
            None,
        )
        .unwrap();
    let d = accounts
        .propose_attribution(
            &customer.id,
            &proposal(&policy, "accept", Introduction::CapturedSource, Some(&r.id)),
        )
        .unwrap();
    assert_eq!(d.status, Status::Accepted);
    assert_eq!(d.source, Some(source));
    let original = accounts.attribution(&customer.id).unwrap().unwrap();
    assert!(!original.commission_eligibility);
    let binding = original.binding.unwrap();
    assert_eq!(
        accounts
            .workspace_attribution(&customer.id, &personal.id)
            .unwrap()
            .unwrap()
            .binding,
        binding
    );
    let team = accounts
        .create_workspace(
            &customer.id,
            "Team",
            WorkspaceKind::Organization,
            "tenant",
            Some(3),
        )
        .unwrap();
    let grant = accounts
        .invite(&customer.id, &team.id, Role::Member, 60)
        .unwrap();
    accounts.accept(&next.id, &grant.token).unwrap();
    assert_eq!(
        accounts.workspace_attribution(&next.id, &team.id),
        Err(Error::Unauthorized)
    );
    accounts
        .transfer_ownership(&customer.id, &team.id, &next.id)
        .unwrap();
    accounts
        .remove_member(&next.id, &team.id, &customer.id)
        .unwrap();
    assert_eq!(
        accounts.workspace_attribution(&customer.id, &team.id),
        Err(Error::Unauthorized)
    );
    let next_view = accounts
        .workspace_attribution(&next.id, &team.id)
        .unwrap()
        .unwrap();
    assert_eq!(next_view.binding, binding);
    assert!(!next_view.commission_eligibility);
    accounts
        .update_principals(&customer.id, &["key:aaaaaaaaaaaaaaaa".into()])
        .unwrap();
    accounts
        .update_principals(&customer.id, &["key:bbbbbbbbbbbbbbbb".into()])
        .unwrap();
    let reopened = Accounts::open(dir.path()).unwrap();
    assert_eq!(
        reopened.workspace_attribution(&next.id, &team.id).unwrap(),
        Some(next_view)
    );
    assert_eq!(
        reopened.attribution(&customer.id).unwrap().unwrap().binding,
        Some(binding)
    );
}

#[test]
fn early_agreement_import_is_idempotent_and_requires_current_referrer_confirmation() {
    let (dir, accounts, owner, buyer, policy) = fixture();
    let r = accounts
        .create_referrer(&owner.id, Kind::Partner, "Partner")
        .unwrap();
    let mut input = proposal(
        &policy,
        "import-1",
        Introduction::EarlyAgreement,
        Some(&r.id),
    );
    let pending = accounts.propose_attribution(&buyer.id, &input).unwrap();
    assert_eq!(pending.review, Some(Review::AwaitingConfirmation));
    let sequence = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts.propose_attribution(&buyer.id, &input).unwrap(),
        pending
    );
    input.request = "imported-again".into();
    assert_eq!(
        accounts.propose_attribution(&buyer.id, &input).unwrap(),
        pending
    );
    assert_eq!(accounts.store().unwrap().sequence, sequence);
    assert_eq!(
        accounts.confirm_attribution(&buyer.id, &buyer.id, &pending.digest),
        Err(Error::Unauthorized)
    );
    let accepted = accounts
        .confirm_attribution(&owner.id, &buyer.id, &pending.digest)
        .unwrap();
    assert_eq!(accepted.status, Status::Accepted);
    assert_eq!(accepted.confirmed, Some(owner.id.clone()));
    let sequence = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts
            .confirm_attribution(&owner.id, &buyer.id, &pending.digest)
            .unwrap(),
        accepted
    );
    assert_eq!(accounts.store().unwrap().sequence, sequence);
    let reopened = Accounts::open(dir.path()).unwrap();
    let view = reopened.attribution(&buyer.id).unwrap().unwrap();
    assert_eq!(view.decisions, vec![pending, accepted]);
    assert_eq!(view.binding.unwrap().referrer.id, r.id);
}

#[test]
fn competing_introductions_suspend_review_and_explicit_correction_keeps_old_policy() {
    let (_dir, accounts, owner, other, policy) = fixture();
    let (buyer, first, _) = signup(&accounts, &owner);
    let initial = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(
                &policy,
                "initial",
                Introduction::CapturedSource,
                Some(&first.id),
            ),
        )
        .unwrap();
    let old_binding = accounts
        .attribution(&buyer.id)
        .unwrap()
        .unwrap()
        .binding
        .unwrap();
    let r = accounts
        .create_referrer(&other.id, Kind::Author, "Other source")
        .unwrap();
    let conflict = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(
                &policy,
                "conflict",
                Introduction::EarlyAgreement,
                Some(&r.id),
            ),
        )
        .unwrap();
    assert_eq!(conflict.review, Some(Review::CompetingIntroduction));
    assert_eq!(
        accounts.attribution(&buyer.id).unwrap().unwrap().binding,
        Some(old_binding.clone())
    );
    assert_eq!(
        accounts.confirm_attribution(&other.id, &buyer.id, &conflict.digest),
        Err(Error::Conflict)
    );
    let new_policy = Policy::new(
        "fixture-v2".into(),
        "Revised agreed wording, no commission right.".into(),
    )
    .unwrap();
    accounts.publish_attribution_policy(&new_policy).unwrap();
    let mut correction = proposal(
        &new_policy,
        "correction",
        Introduction::Correction,
        Some(&r.id),
    );
    correction.expected_decision = Some(initial.digest.clone());
    assert_eq!(
        accounts.propose_attribution(&buyer.id, &correction),
        Err(Error::Conflict)
    );
    correction.expected_decision = Some(conflict.digest.clone());
    let pending = accounts
        .propose_attribution(&buyer.id, &correction)
        .unwrap();
    let accepted = accounts
        .confirm_attribution(&other.id, &buyer.id, &pending.digest)
        .unwrap();
    let view = accounts.attribution(&buyer.id).unwrap().unwrap();
    assert_eq!(view.decisions.len(), 4);
    assert_eq!(view.decisions[0], initial);
    assert_eq!(view.decisions[0].policy_digest, policy.digest);
    assert_eq!(view.binding.as_ref().unwrap().id, old_binding.id);
    assert_eq!(view.binding.as_ref().unwrap().referrer.id, r.id);
    assert_eq!(
        view.binding.as_ref().unwrap().accepted_decision,
        accepted.digest
    );
    assert_eq!(
        accounts
            .attribution_policy_version(&buyer.id, Some(&policy.digest))
            .unwrap(),
        Some(policy)
    );
}

#[test]
fn missing_preexisting_self_and_source_only_have_visible_review_outcomes() {
    let (_dir, accounts, owner, buyer, policy) = fixture();
    let missing = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(&policy, "missing", Introduction::MissingEvidence, None),
        )
        .unwrap();
    assert_eq!(missing.review, Some(Review::MissingEvidence));
    let self_referrer = accounts
        .create_referrer(&buyer.id, Kind::Person, "Self")
        .unwrap();
    let d = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(
                &policy,
                "self",
                Introduction::EarlyAgreement,
                Some(&self_referrer.id),
            ),
        )
        .unwrap();
    assert_eq!(d.review, Some(Review::SelfReferral));
    let source_only = accounts
        .create_sales_referrer(&owner.id, "Source only")
        .unwrap();
    let d = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(
                &policy,
                "agent",
                Introduction::EarlyAgreement,
                Some(&source_only.id),
            ),
        )
        .unwrap();
    assert_eq!(d.review, Some(Review::SourceOnly));
    assert!(
        accounts
            .confirm_attribution(&owner.id, &buyer.id, &d.digest)
            .is_err()
    );
    let existing = accounts.create_account("Existing", &[]).unwrap();
    let r = accounts
        .create_referrer(&owner.id, Kind::Person, "Source")
        .unwrap();
    let link = accounts.issue_referral_link(&owner.id, &r.id).unwrap();
    accounts
        .capture_acquisition(
            &existing.id,
            &Capture {
                request: "later-capture".into(),
                token: Some(link.token),
                consent: true,
                consent_version: Some(CONSENT.into()),
            },
        )
        .unwrap();
    let d = accounts
        .propose_attribution(
            &existing.id,
            &proposal(
                &policy,
                "existing",
                Introduction::CapturedSource,
                Some(&r.id),
            ),
        )
        .unwrap();
    assert_eq!(d.review, Some(Review::PreexistingCustomer));
}

#[test]
fn legacy_source_stays_unknown_without_rewriting_its_original_signup_evidence() {
    crate::files_only!();
    let (dir, accounts, owner, _, policy) = fixture();
    let (buyer, r, source) = signup(&accounts, &owner);
    accounts
        .referral_write(|store, _| {
            store
                .referrals
                .sources
                .get_mut(&buyer.id)
                .unwrap()
                .at_signup = None;
            Ok(((), true))
        })
        .unwrap();
    let persisted = std::fs::read_to_string(dir.path().join(ACCOUNTS)).unwrap();
    assert!(!persisted.contains("at_signup"));
    let reopened = Accounts::open(dir.path()).unwrap();
    let d = reopened
        .propose_attribution(
            &buyer.id,
            &proposal(&policy, "legacy", Introduction::CapturedSource, Some(&r.id)),
        )
        .unwrap();
    assert_eq!(d.review, Some(Review::UnknownSignup));
    assert_eq!(d.source, Some(source));
    assert!(
        reopened
            .attribution(&buyer.id)
            .unwrap()
            .unwrap()
            .binding
            .is_none()
    );
}

#[test]
fn management_successor_is_two_party_and_does_not_relabel_accepted_decisions() {
    let (dir, accounts, owner, next, policy) = fixture();
    let (buyer, r, _) = signup(&accounts, &owner);
    let accepted = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(&policy, "accept", Introduction::CapturedSource, Some(&r.id)),
        )
        .unwrap();
    let offered = accounts
        .offer_referrer_migration(&owner.id, &r.id, &next.id)
        .unwrap();
    let sequence = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts
            .offer_referrer_migration(&owner.id, &r.id, &next.id)
            .unwrap(),
        offered
    );
    assert_eq!(accounts.store().unwrap().sequence, sequence);
    assert_eq!(
        accounts.accept_referrer_migration(&buyer.id, &r.id),
        Err(Error::Unauthorized)
    );
    let migrated = accounts.accept_referrer_migration(&next.id, &r.id).unwrap();
    let sequence = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts.accept_referrer_migration(&next.id, &r.id).unwrap(),
        migrated
    );
    assert_eq!(accounts.store().unwrap().sequence, sequence);
    assert_eq!(
        accounts.referrer_successors(&owner.id, &r.id),
        Err(Error::Unauthorized)
    );
    let lineage = accounts.referrer_successors(&next.id, &r.id).unwrap();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].from, owner.id);
    assert_eq!(lineage[0].to, next.id);
    assert!(lineage[0].management_only);
    let reopened = Accounts::open(dir.path()).unwrap();
    assert_eq!(
        reopened.accept_referrer_migration(&next.id, &r.id).unwrap(),
        migrated
    );
    assert_eq!(reopened.store().unwrap().sequence, sequence);
    assert_eq!(
        reopened.attribution(&buyer.id).unwrap().unwrap().decisions[0],
        accepted
    );
    assert_eq!(
        reopened.referrer_successors(&next.id, &r.id).unwrap(),
        lineage
    );
}

#[test]
fn under_lock_credential_guard_and_policy_conflicts_change_no_revision() {
    let (_dir, accounts, owner, buyer, policy) = fixture();
    let r = accounts
        .create_referrer(&owner.id, Kind::Person, "Source")
        .unwrap();
    let input = proposal(&policy, "early", Introduction::EarlyAgreement, Some(&r.id));
    let seq = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts.propose_attribution_guarded(&buyer.id, &input, || false),
        Err(Error::Unauthorized)
    );
    assert_eq!(accounts.store().unwrap().sequence, seq);
    let d = accounts.propose_attribution(&buyer.id, &input).unwrap();
    let seq = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts.confirm_attribution_guarded(&owner.id, &buyer.id, &d.digest, || false),
        Err(Error::Unauthorized)
    );
    let changed = Policy::new(policy.version.clone(), "Changed terms.".into()).unwrap();
    assert_eq!(
        accounts.publish_attribution_policy(&changed),
        Err(Error::Conflict)
    );
    assert_eq!(accounts.store().unwrap().sequence, seq);
}

#[test]
fn a_credential_revoked_while_waiting_for_account_custody_cannot_confirm() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_dir, accounts, owner, buyer, policy) = fixture();
    let r = accounts
        .create_referrer(&owner.id, Kind::Person, "Source")
        .unwrap();
    let input = proposal(
        &policy,
        "waiting",
        Introduction::EarlyAgreement,
        Some(&r.id),
    );
    let d = accounts.propose_attribution(&buyer.id, &input).unwrap();
    let before = accounts.store().unwrap().sequence;
    let lock = Lock::acquire(&accounts.dir).unwrap();
    let current = Arc::new(AtomicBool::new(true));
    let copied = current.clone();
    let directory = accounts.dir.clone();
    let customer = buyer.id.clone();
    let thread = std::thread::spawn(move || {
        let store = Accounts::open(&directory).unwrap();
        store.confirm_attribution_guarded(&owner.id, &customer, &d.digest, || {
            copied.load(Ordering::SeqCst)
        })
    });
    current.store(false, Ordering::SeqCst);
    drop(lock);
    assert_eq!(thread.join().unwrap(), Err(Error::Unauthorized));
    assert_eq!(accounts.store().unwrap().sequence, before);
}

#[test]
fn immutable_decision_validation_and_workspace_adoption_refuse_substitution() {
    let (_dir, accounts, owner, buyer, policy) = fixture();
    let r = accounts
        .create_referrer(&owner.id, Kind::Person, "Source")
        .unwrap();
    let team = accounts
        .create_workspace(
            &buyer.id,
            "Existing team",
            WorkspaceKind::Organization,
            "tenant",
            None,
        )
        .unwrap();
    let pending = accounts
        .propose_attribution(
            &buyer.id,
            &proposal(&policy, "early", Introduction::EarlyAgreement, Some(&r.id)),
        )
        .unwrap();
    let d = accounts
        .confirm_attribution(&owner.id, &buyer.id, &pending.digest)
        .unwrap();
    let view = accounts
        .adopt_workspace_attribution_guarded(&buyer.id, &team.id, &d.digest, || true)
        .unwrap();
    let seq = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts
            .adopt_workspace_attribution_guarded(&buyer.id, &team.id, &d.digest, || true)
            .unwrap(),
        view
    );
    assert_eq!(accounts.store().unwrap().sequence, seq);
    assert_eq!(
        accounts.adopt_workspace_attribution_guarded(&owner.id, &team.id, &d.digest, || true),
        Err(Error::Unauthorized)
    );
    let mut store = accounts.store().unwrap();
    store
        .referrals
        .attribution
        .customers
        .get_mut(&buyer.id)
        .unwrap()
        .decisions[0]
        .reason = "Tampered reason".into();
    store.seal();
    assert!(store.validate(ACCOUNTS).is_err());
}
