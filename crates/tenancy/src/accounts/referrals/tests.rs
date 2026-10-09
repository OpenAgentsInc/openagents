use super::*;

fn capture(request: &str, token: Option<&str>, consent: bool) -> Capture {
    Capture {
        request: request.into(),
        token: token.map(str::to_owned),
        consent,
        consent_version: consent.then(|| CONSENT.into()),
    }
}
fn setup() -> (tempfile::TempDir, Accounts, Account, Account) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let alice = accounts
        .create_account("Alice private contact", &[])
        .unwrap();
    let bob = accounts.create_account("Bob private contact", &[]).unwrap();
    (dir, accounts, alice, bob)
}

#[test]
fn signup_capture_is_stable_private_and_replay_cannot_rewrite_it() {
    crate::files_only!();
    let (dir, accounts, alice, bob) = setup();
    let record = accounts
        .create_referrer(&alice.id, Kind::Person, "Private lead notes")
        .unwrap();
    let link = accounts.issue_referral_link(&alice.id, &record.id).unwrap();
    assert_eq!(link.path, format!("/join?ref={}", link.token));
    for private in [&alice.id, &record.id, &alice.label, &record.label] {
        assert!(!link.path.contains(private));
    }
    let input = capture("first", Some(&link.token), true);
    let (buyer, source) = accounts.create_account_acquired("Buyer", &input).unwrap();
    assert_eq!(source.referrer.as_ref().unwrap().id, record.id);
    let sequence = accounts.store().unwrap().sequence;
    assert_eq!(
        accounts.capture_acquisition(&buyer.id, &input).unwrap(),
        source
    );
    assert_eq!(accounts.store().unwrap().sequence, sequence);
    assert_eq!(
        accounts
            .create_account_acquired("Replay", &input)
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(
        accounts
            .capture_acquisition(&buyer.id, &capture("other", None, false))
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(
        accounts.referrer(&bob.id, &record.id).unwrap_err(),
        Error::Unauthorized
    );
    assert_eq!(
        accounts
            .issue_referral_link(&bob.id, &record.id)
            .unwrap_err(),
        Error::Unauthorized
    );
    assert_eq!(
        accounts
            .disable_referral_links(&bob.id, &record.id)
            .unwrap_err(),
        Error::Unauthorized
    );
    drop(accounts);
    let reopened = Accounts::open(dir.path()).unwrap();
    assert_eq!(reopened.acquisition(&buyer.id).unwrap(), Some(source));
    let stored = std::fs::read_to_string(dir.path().join(ACCOUNTS)).unwrap();
    assert!(!stored.contains(&link.token));
    assert!(
        !serde_json::to_string(&reopened.acquisition(&buyer.id).unwrap())
            .unwrap()
            .contains("commission")
    );
}

#[test]
fn refusal_consent_disable_rotation_migration_and_source_only_survive() {
    let (_dir, accounts, alice, bob) = setup();
    let record = accounts
        .create_sales_referrer(&alice.id, "OpenAgents sales agent")
        .unwrap();
    let old = accounts.issue_referral_link(&alice.id, &record.id).unwrap();
    let (_, retained) = accounts
        .create_account_acquired("One", &capture("one", Some(&old.token), true))
        .unwrap();
    assert!(retained.referrer.as_ref().unwrap().source_only);
    let new = accounts.issue_referral_link(&alice.id, &record.id).unwrap();
    let unknown = format!("rfr_{}", "0".repeat(64));
    for (request, value, consent, expected) in [
        ("missing", None, false, Outcome::Missing),
        (
            "declined",
            Some(new.token.as_str()),
            false,
            Outcome::Declined,
        ),
        (
            "malformed",
            Some("../../customer-secret"),
            true,
            Outcome::Malformed,
        ),
        ("unknown", Some(unknown.as_str()), true, Outcome::Unknown),
        ("rotated", Some(old.token.as_str()), true, Outcome::Disabled),
    ] {
        let (_, source) = accounts
            .create_account_acquired("Synthetic", &capture(request, value, consent))
            .unwrap();
        assert_eq!(source.outcome, expected);
        assert!(source.referrer.is_none());
    }
    let mut wrong = capture("wrong", Some(&new.token), true);
    wrong.consent_version = Some("unreviewed".into());
    assert_eq!(
        accounts.create_account_acquired("Bad", &wrong).unwrap_err(),
        Error::Invalid
    );
    let too_big = "x".repeat(257);
    assert_eq!(
        accounts
            .create_account_acquired("Bad", &capture("large", Some(&too_big), true))
            .unwrap_err(),
        Error::Invalid
    );
    accounts
        .disable_referral_links(&alice.id, &record.id)
        .unwrap();
    assert_eq!(
        accounts
            .create_account_acquired("Disabled", &capture("disabled", Some(&new.token), true))
            .unwrap()
            .1
            .outcome,
        Outcome::Disabled
    );
    assert_eq!(
        accounts
            .accept_referrer_migration(&bob.id, &record.id)
            .unwrap_err(),
        Error::Unauthorized
    );
    let before_migration = accounts.issue_referral_link(&alice.id, &record.id).unwrap();
    accounts
        .offer_referrer_migration(&alice.id, &record.id, &bob.id)
        .unwrap();
    let migrated = accounts
        .accept_referrer_migration(&bob.id, &record.id)
        .unwrap();
    assert_eq!(
        accounts
            .create_account_acquired(
                "Old manager link",
                &capture("prior-manager", Some(&before_migration.token), true)
            )
            .unwrap()
            .1
            .outcome,
        Outcome::Disabled
    );
    assert_eq!(migrated.id, record.id);
    assert!(migrated.source_only);
    assert_eq!(
        accounts
            .issue_referral_link(&alice.id, &record.id)
            .unwrap_err(),
        Error::Unauthorized
    );
    let latest = accounts.issue_referral_link(&bob.id, &record.id).unwrap();
    let (_, source) = accounts
        .create_account_acquired("After", &capture("after", Some(&latest.token), true))
        .unwrap();
    assert_eq!(
        source.referrer.as_ref().unwrap().id,
        retained.referrer.as_ref().unwrap().id
    );
    assert_ne!(
        source.referrer.as_ref().unwrap().version,
        retained.referrer.as_ref().unwrap().version
    );
}

#[test]
fn link_retirement_does_not_reset_lifetime_bounds() {
    let (_dir, accounts, alice, _) = setup();
    let referrer = accounts
        .create_referrer(&alice.id, Kind::Person, "Source")
        .unwrap();
    for _ in 0..LINKS {
        accounts
            .issue_referral_link(&alice.id, &referrer.id)
            .unwrap();
        accounts
            .disable_referral_links(&alice.id, &referrer.id)
            .unwrap();
    }
    assert_eq!(
        accounts
            .issue_referral_link(&alice.id, &referrer.id)
            .unwrap_err(),
        Error::Bound
    );
}

#[test]
fn current_principal_rotation_keeps_manager_and_source_identity() {
    let (dir, accounts, alice, _) = setup();
    let tenant = accounts
        .create_workspace(
            &alice.id,
            "Private",
            WorkspaceKind::Personal,
            "tenant",
            None,
        )
        .unwrap();
    accounts
        .update_principals(&alice.id, &["key:aaaaaaaaaaaaaaaa".into()])
        .unwrap();
    let record = accounts
        .create_referrer(&alice.id, Kind::Author, "Author")
        .unwrap();
    let link = accounts.issue_referral_link(&alice.id, &record.id).unwrap();
    accounts
        .update_principals(&alice.id, &["key:bbbbbbbbbbbbbbbb".into()])
        .unwrap();
    assert!(
        accounts
            .authorize_principal(&tenant.id, "key:aaaaaaaaaaaaaaaa")
            .is_err()
    );
    let actual = accounts
        .authorize_principal(&tenant.id, "key:bbbbbbbbbbbbbbbb")
        .unwrap();
    let source = accounts
        .capture_acquisition(&alice.id, &capture("own-source", Some(&link.token), true))
        .unwrap();
    assert_eq!(
        accounts.referrer(&actual.account, &record.id).unwrap().id,
        record.id
    );
    assert_eq!(
        Accounts::open(dir.path())
            .unwrap()
            .acquisition(&actual.account)
            .unwrap(),
        Some(source)
    );
    // Introduction capture remains inert, including self-introductions. Abuse
    // and commercial eligibility belong to the later commission contract.
}
