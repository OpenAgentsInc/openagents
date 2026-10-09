use super::identities::{GithubEmail, GithubProfile, github_principal};
use super::{Accounts, Refusal, WorkspaceKind};

fn profile(id: u64, login: &str) -> GithubProfile {
    GithubProfile {
        id,
        login: login.into(),
        name: Some(format!("{login} person")),
        avatar_url: Some(format!("https://avatars.example/{id}")),
        emails: vec![GithubEmail {
            email: format!("{login}@example.com"),
            verified: true,
            primary: true,
            visibility: Some("private".into()),
        }],
        bio: Some("Builds things.\nLikes Rust.".into()),
        public_repos: Some(12),
        created_at: Some("2011-01-25T18:44:36Z".into()),
        ..GithubProfile::default()
    }
}

fn store() -> (tempfile::TempDir, Accounts) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    (dir, accounts)
}

#[test]
fn first_github_sign_in_creates_the_account_and_a_personal_workspace() {
    let (_dir, accounts) = store();
    let first = accounts
        .sign_in_github(&profile(42, "octo"), "signup")
        .unwrap();
    assert!(first.created);
    assert_eq!(first.account.label, "octo person");
    assert_eq!(first.account.principals, vec![github_principal(42)]);
    let workspace = first.workspace.unwrap();
    assert_eq!(workspace.kind, WorkspaceKind::Personal);
    assert_eq!(workspace.tenant, "signup");
    accounts
        .authorize(&workspace.id, &first.account.id)
        .unwrap();
    assert_eq!(
        accounts.account_of_principal("github:42").unwrap(),
        Some(first.account.id.clone())
    );

    // A returning user finds the same account; the profile refreshes.
    let mut renamed = profile(42, "octo-renamed");
    renamed.public_repos = Some(13);
    let again = accounts.sign_in_github(&renamed, "signup").unwrap();
    assert!(!again.created && again.workspace.is_none());
    assert_eq!(again.account.id, first.account.id);
    let identity = accounts.github_identity(42).unwrap().unwrap();
    assert_eq!(identity.profile.login, "octo-renamed");
    assert_eq!(identity.profile.public_repos, Some(13));
    assert_eq!(accounts.store().unwrap().accounts.len(), 1);
}

#[test]
fn an_email_less_github_user_signs_up_under_the_login() {
    let (_dir, accounts) = store();
    let bare = GithubProfile {
        id: 7,
        login: "quiet".into(),
        ..GithubProfile::default()
    };
    let signed = accounts.sign_in_github(&bare, "signup").unwrap();
    assert!(signed.created);
    assert_eq!(signed.account.label, "quiet");
    assert_eq!(bare.verified_email(), None);
}

#[test]
fn linking_a_github_identity_owned_by_another_account_is_refused() {
    let (_dir, accounts) = store();
    let owner = accounts.sign_in_github(&profile(1, "first"), "t").unwrap();
    let nostr = accounts
        .create_account("Nostr person", &[format!("nostr:{}", "a".repeat(64))])
        .unwrap();
    let refused = accounts.link_github(&nostr.id, &profile(1, "first"));
    assert!(
        matches!(refused, Err(Refusal::PrincipalTaken { ref account, .. }) if *account == owner.account.id),
        "{refused:?}"
    );
    // Nothing moved.
    assert_eq!(
        accounts.account_of_principal("github:1").unwrap(),
        Some(owner.account.id)
    );

    // A Nostr-only account links a fresh GitHub identity, then signs in with it.
    let linked = accounts
        .link_github(&nostr.id, &profile(2, "second"))
        .unwrap();
    assert_eq!(linked.account, nostr.id);
    let signed = accounts.sign_in_github(&profile(2, "second"), "t").unwrap();
    assert!(!signed.created);
    assert_eq!(signed.account.id, nostr.id);

    // One GitHub identity per account.
    assert!(matches!(
        accounts.link_github(&nostr.id, &profile(3, "third")),
        Err(Refusal::ProviderLinked { .. })
    ));
}

#[test]
fn unlinking_the_last_way_to_sign_in_is_refused() {
    let (_dir, accounts) = store();
    let only = accounts.sign_in_github(&profile(9, "solo"), "t").unwrap();
    assert!(matches!(
        accounts.unlink_github(&only.account.id),
        Err(Refusal::LastCredential(_))
    ));
    let both = accounts
        .create_account("Both", &[format!("nostr:{}", "b".repeat(64))])
        .unwrap();
    accounts
        .link_github(&both.id, &profile(10, "both"))
        .unwrap();
    accounts.unlink_github(&both.id).unwrap();
    assert_eq!(accounts.github_identity(10).unwrap(), None);
    assert_eq!(accounts.account_of_principal("github:10").unwrap(), None);
}

#[test]
fn malformed_github_principals_and_profiles_are_refused() {
    let (_dir, accounts) = store();
    for bad in [
        "github:",
        "github:01",
        "github:abc",
        "github:99999999999999999999999",
    ] {
        assert!(
            accounts.create_account("x", &[bad.to_string()]).is_err(),
            "{bad}"
        );
    }
    // A github principal without its identity record fails validation.
    assert!(accounts.create_account("x", &["github:5".into()]).is_err());
    let mut hostile = profile(11, "ok");
    hostile.name = Some("bad\u{0}name".into());
    assert!(accounts.sign_in_github(&hostile, "t").is_err());
    let mut bad_login = profile(12, "ok");
    bad_login.login = "../etc".into();
    assert!(accounts.sign_in_github(&bad_login, "t").is_err());
}
