use super::apple::{AppleIdentity, AppleProfile, apple_principal};
use super::{Account, Accounts};

/// An account linked to Apple before Sign in with Apple was removed
/// (#12005) keeps its identity record, and the store still loads.
#[test]
fn accounts_linked_to_apple_keep_their_identity() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let sub = "001234.0a1b2c3d4e5f.1234";
    accounts
        .mutate(|store, _| {
            let stamp = crate::registry::now_utc();
            store.accounts.insert(
                "acct_0123456789abcdef".into(),
                Account {
                    id: "acct_0123456789abcdef".into(),
                    label: "Pat Doe".into(),
                    principals: vec![apple_principal(sub)],
                    created: stamp.clone(),
                    agent: None,
                },
            );
            store.identities.apple.insert(
                sub.into(),
                AppleIdentity {
                    account: "acct_0123456789abcdef".into(),
                    profile: AppleProfile {
                        sub: sub.into(),
                        email: Some("pat@example.com".into()),
                        email_verified: true,
                        private_email: false,
                    },
                    linked: stamp.clone(),
                    refreshed: stamp,
                },
            );
            Ok::<_, super::Refusal>(())
        })
        .unwrap();
    let reopened = Accounts::open(dir.path()).unwrap();
    let identity = reopened.apple_identity(sub).unwrap().unwrap();
    assert_eq!(identity.account, "acct_0123456789abcdef");
    assert!(super::valid_principal(&apple_principal(sub)));
}
