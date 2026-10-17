use super::apple::{AppleProfile, apple_principal};
use super::{Accounts, WorkspaceKind};

fn profile(sub: &str) -> AppleProfile {
    AppleProfile {
        sub: sub.into(),
        email: Some("pat@example.com".into()),
        email_verified: true,
        private_email: false,
    }
}

#[test]
fn first_apple_sign_in_creates_the_account_and_the_second_finds_it() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let sub = "001234.0a1b2c3d4e5f.1234";
    let first = accounts
        .sign_in_apple(&profile(sub), Some("Pat Doe"), "signup")
        .unwrap();
    assert!(first.created);
    assert_eq!(first.account.label, "Pat Doe");
    assert_eq!(first.account.principals, vec![apple_principal(sub)]);
    assert_eq!(first.workspace.unwrap().kind, WorkspaceKind::Personal);

    let mut hidden = profile(sub);
    hidden.email = None;
    let second = accounts.sign_in_apple(&hidden, None, "signup").unwrap();
    assert!(!second.created);
    assert_eq!(second.account.id, first.account.id);
    let identity = accounts.apple_identity(sub).unwrap().unwrap();
    assert_eq!(identity.profile.email.as_deref(), Some("pat@example.com"));

    // A store written before Apple identities still loads, and one with
    // them reloads.
    let reopened = Accounts::open(dir.path()).unwrap();
    assert!(reopened.apple_identity(sub).unwrap().is_some());
}

#[test]
fn labels_fall_back_to_the_address_then_a_default() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let shared = accounts
        .sign_in_apple(&profile("001.a.1"), None, "signup")
        .unwrap();
    assert_eq!(shared.account.label, "pat");
    let mut relay = profile("001.b.2");
    relay.email = Some("xyz@privaterelay.appleid.com".into());
    relay.private_email = true;
    let hidden = accounts.sign_in_apple(&relay, None, "signup").unwrap();
    assert_eq!(hidden.account.label, "OpenAgents user");
}

#[test]
fn malformed_subjects_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    assert!(
        accounts
            .sign_in_apple(&profile("bad sub!"), None, "signup")
            .is_err()
    );
    assert!(
        accounts
            .sign_in_apple(&profile(""), None, "signup")
            .is_err()
    );
}
