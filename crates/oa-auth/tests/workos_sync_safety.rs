//! Backfill ownership checks against foreign Users and duplicate emails.

use oa_auth::workos::client::{UserChange, Workos};
use oa_auth::workos::fake::FakeWorkos;
use oa_auth::workos::sync::{self, Options, Report};
use oa_auth::workos::{Grant, Method, Mode};
use tenancy::accounts::identities::{GithubEmail, GithubProfile};
use tenancy::workos_links::Links;
use tenancy::{Accounts, WorkspaceKind};

const METHODS: &[Method] = &[Method::Github];
const EMAIL: &str = "same@example.com";

fn profile(id: u64, verified: bool) -> GithubProfile {
    GithubProfile {
        id,
        login: format!("person{id}"),
        email: Some(EMAIL.into()),
        emails: vec![GithubEmail {
            email: EMAIL.into(),
            verified,
            primary: true,
            visibility: None,
        }],
        ..Default::default()
    }
}

fn store() -> (tempfile::TempDir, Accounts) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    (dir, accounts)
}

async fn external(workos: &Workos, user: &str, id: &str) {
    workos
        .update_user(
            user,
            UserChange {
                external_id: Some(id.into()),
                metadata: Some([("old-product".into(), "keep".into())].into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

fn skipped(report: &Report, account: &str, users: &[&str], reason: &str) {
    assert!(!report.wrote(), "{report:?}");
    assert!(report.failed.is_empty(), "{report:?}");
    assert!(
        report.skipped.iter().any(|entry| {
            entry.contains(account)
                && entry.contains(reason)
                && users.iter().all(|id| entry.contains(id))
        }),
        "{report:?}"
    );
}

async fn backfill(dir: &std::path::Path, workos: &Workos, apply: bool) -> Report {
    sync::all(
        dir,
        workos,
        Options { apply, test: false },
        None,
        || async {},
        |_, _| {},
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn foreign_external_ids_are_unchanged_with_or_without_a_local_link() {
    for linked in [false, true] {
        for foreign in ["acct_00000000000000ff", "older-product-person"] {
            let fake = FakeWorkos::start().await;
            let workos = fake.client(Mode::Dual, METHODS);
            let (dir, accounts) = store();
            let account = accounts
                .sign_in_github(&profile(1, true), "signup")
                .unwrap()
                .account;
            let user = fake.add_user(EMAIL, None, true);
            external(&workos, &user, foreign).await;
            let links = Links::open(dir.path());
            if linked {
                links.link_user(&account.id, &user).unwrap();
            }
            let before_user = fake.user(&user);
            let before_links = links.users().unwrap();
            let before_accounts = serde_json::to_value(accounts.store().unwrap()).unwrap();
            for apply in [false, true] {
                let report = backfill(dir.path(), &workos, apply).await;
                skipped(&report, &account.id, &[&user], "foreign external_id");
                assert_eq!(fake.user(&user), before_user);
                assert_eq!(fake.user_count(), 1);
                assert_eq!(links.users().unwrap(), before_links);
                assert_eq!(
                    serde_json::to_value(accounts.store().unwrap()).unwrap(),
                    before_accounts
                );
                assert!(
                    serde_json::to_value(&report).unwrap()["skipped"][0]
                        .as_str()
                        .unwrap()
                        .contains(foreign)
                );
            }
        }
    }
}

#[tokio::test]
async fn email_requires_verification_on_both_sides() {
    for (github_verified, workos_verified, reason) in [
        (true, false, "unverified or mismatched WorkOS email"),
        (false, true, "unverified GitHub email"),
    ] {
        let fake = FakeWorkos::start().await;
        let workos = fake.client(Mode::Dual, METHODS);
        let (dir, accounts) = store();
        let account = accounts
            .sign_in_github(&profile(1, github_verified), "signup")
            .unwrap()
            .account;
        let user = fake.add_user(EMAIL, None, workos_verified);
        let before = fake.user(&user);
        for apply in [false, true] {
            let report = backfill(dir.path(), &workos, apply).await;
            skipped(&report, &account.id, &[&user], reason);
            assert_eq!(fake.user(&user), before);
            assert_eq!(fake.user_count(), 1);
            assert!(Links::open(dir.path()).users().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn duplicate_workos_emails_are_skipped_before_filtering_candidates() {
    for second_verified in [false, true] {
        let fake = FakeWorkos::start().await;
        let workos = fake.client(Mode::Dual, METHODS);
        let (dir, accounts) = store();
        let account = accounts
            .sign_in_github(&profile(1, true), "signup")
            .unwrap()
            .account;
        let first = fake.add_user(EMAIL, None, true);
        let second = fake.add_user(EMAIL, None, second_verified);
        external(&workos, &second, "acct_00000000000000ff").await;
        let before = (fake.user(&first), fake.user(&second));
        for apply in [false, true] {
            let report = backfill(dir.path(), &workos, apply).await;
            skipped(
                &report,
                &account.id,
                &[&first, &second],
                "several email matches",
            );
            assert_eq!((fake.user(&first), fake.user(&second)), before);
            assert_eq!(fake.user_count(), 2);
            assert!(Links::open(dir.path()).users().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn several_local_accounts_cannot_claim_the_same_verified_email_even_when_resuming() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, METHODS);
    let (dir, accounts) = store();
    let first = accounts
        .sign_in_github(&profile(1, true), "signup")
        .unwrap()
        .account;
    let mut other = profile(2, true);
    // A secondary verified address must also make an email match ambiguous.
    other.emails.insert(
        0,
        GithubEmail {
            email: "other@example.com".into(),
            verified: true,
            primary: true,
            visibility: None,
        },
    );
    let second = accounts.sign_in_github(&other, "signup").unwrap().account;
    let user = fake.add_user(EMAIL, None, true);
    let other_user = fake.add_user("other@example.com", None, true);
    let before = (fake.user(&user), fake.user(&other_user));
    for apply in [false, true] {
        let report = sync::account(
            dir.path(),
            &workos,
            &first.id,
            Options { apply, test: false },
        )
        .await
        .unwrap();
        skipped(
            &report,
            &first.id,
            &[&user, &second.id],
            "several accounts share",
        );
        assert_eq!((fake.user(&user), fake.user(&other_user)), before);
        assert!(Links::open(dir.path()).users().unwrap().is_empty());
    }
    // Duplicate primary addresses remain ambiguous even when one account is before the cursor.
    accounts
        .sign_in_github(&profile(2, true), "signup")
        .unwrap();
    let mut ids = [first.id, second.id];
    ids.sort();
    for apply in [false, true] {
        let report = sync::all(
            dir.path(),
            &workos,
            Options { apply, test: false },
            Some(&ids[0]),
            || async {},
            |_, _| {},
        )
        .await
        .unwrap();
        skipped(
            &report,
            &ids[1],
            &[&user, &ids[0]],
            "several accounts share",
        );
        assert_eq!((fake.user(&user), fake.user(&other_user)), before);
        assert!(Links::open(dir.path()).users().unwrap().is_empty());
    }
}

#[tokio::test]
async fn a_workos_user_linked_to_a_different_account_is_skipped() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, METHODS);
    let (dir, accounts) = store();
    let account = accounts
        .sign_in_github(&profile(1, true), "signup")
        .unwrap()
        .account;
    let other = accounts.create_account("Other", &[]).unwrap();
    let user = fake.add_user(EMAIL, None, true);
    let links = Links::open(dir.path());
    links.link_user(&other.id, &user).unwrap();
    let before = fake.user(&user);
    for apply in [false, true] {
        let report = sync::account(
            dir.path(),
            &workos,
            &account.id,
            Options { apply, test: false },
        )
        .await
        .unwrap();
        skipped(
            &report,
            &account.id,
            &[&user, &other.id],
            "linked to another account",
        );
        assert_eq!(fake.user(&user), before);
        assert_eq!(links.user_of(&other.id).unwrap(), Some(user.clone()));
        assert!(links.user_of(&account.id).unwrap().is_none());
    }
}

#[tokio::test]
async fn safe_links_and_exact_external_ids_take_priority_over_email() {
    for linked in [false, true] {
        for has_external in [false, true] {
            if !linked && !has_external {
                continue;
            }
            let fake = FakeWorkos::start().await;
            let workos = fake.client(Mode::Dual, METHODS);
            let (dir, accounts) = store();
            let account = accounts
                .sign_in_github(&profile(1, true), "signup")
                .unwrap()
                .account;
            let user = fake.add_user(
                if linked || has_external {
                    "different@example.com"
                } else {
                    EMAIL
                },
                None,
                !linked && !has_external,
            );
            let stranger = fake.add_user(EMAIL, None, false);
            external(&workos, &stranger, "foreign-product").await;
            let stranger_before = fake.user(&stranger);
            if has_external {
                external(&workos, &user, &account.id).await;
            }
            let links = Links::open(dir.path());
            if linked {
                links.link_user(&account.id, &user).unwrap();
            }
            let before = fake.user(&user);
            let dry = sync::account(
                dir.path(),
                &workos,
                &account.id,
                Options {
                    apply: false,
                    test: false,
                },
            )
            .await
            .unwrap();
            assert_eq!(dry.users_linked, u32::from(!linked));
            assert_eq!(dry.users_updated, u32::from(!has_external));
            assert!(dry.skipped.is_empty(), "{dry:?}");
            assert_eq!(fake.user(&user), before);
            let applied = sync::account(dir.path(), &workos, &account.id, Options::default())
                .await
                .unwrap();
            assert_eq!(applied, dry);
            assert_eq!(links.user_of(&account.id).unwrap(), Some(user.clone()));
            assert_eq!(fake.user(&user).unwrap()["external_id"], account.id);
            assert_eq!(fake.user(&stranger), stranger_before);
            let again = sync::account(dir.path(), &workos, &account.id, Options::default())
                .await
                .unwrap();
            assert!(!again.wrote(), "{again:?}");
        }
    }
}

#[tokio::test]
async fn a_unique_verified_email_with_an_empty_external_id_is_safe() {
    for empty_string in [false, true] {
        let fake = FakeWorkos::start().await;
        let workos = fake.client(Mode::Dual, METHODS);
        let (dir, accounts) = store();
        let account = accounts
            .sign_in_github(&profile(1, true), "signup")
            .unwrap()
            .account;
        let user = fake.add_user(EMAIL, None, true);
        if empty_string {
            external(&workos, &user, "").await;
        }
        let before = fake.user(&user);
        let dry = backfill(dir.path(), &workos, false).await;
        assert_eq!((dry.users_linked, dry.users_updated), (1, 1));
        assert!(dry.skipped.is_empty(), "{dry:?}");
        assert_eq!(fake.user(&user), before);
        assert!(Links::open(dir.path()).users().unwrap().is_empty());
        assert_eq!(backfill(dir.path(), &workos, true).await, dry);
        assert_eq!(fake.user(&user).unwrap()["external_id"], account.id);
        assert_eq!(
            Links::open(dir.path()).user_of(&account.id).unwrap(),
            Some(user)
        );
        assert!(!backfill(dir.path(), &workos, true).await.wrote());
    }
}

async fn github_user(fake: &FakeWorkos, workos: &Workos, id: u64, email: &str) -> String {
    fake.queue_sign_in("GitHubOAuth", email, &id.to_string(), None);
    let url = workos
        .authorization_url(
            Some("GitHubOAuth"),
            None,
            "http://127.0.0.1:1/auth/callback",
            "state",
            None,
        )
        .unwrap();
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();
    let redirect = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let code = redirect
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned();
    workos
        .authenticate(Grant::Code { code })
        .await
        .unwrap()
        .user
        .id
}

#[tokio::test]
async fn github_identity_precedes_email_but_never_overwrites_foreign_ownership() {
    for foreign in [false, true] {
        let fake = FakeWorkos::start().await;
        let workos = fake.client(Mode::Dual, METHODS);
        let (dir, accounts) = store();
        let account = accounts
            .sign_in_github(&profile(42, false), "signup")
            .unwrap()
            .account;
        let user = github_user(&fake, &workos, 42, "different@example.com").await;
        let email_match = fake.add_user(EMAIL, None, true);
        let email_before = fake.user(&email_match);
        if foreign {
            external(&workos, &user, "another-product").await;
        }
        let before = fake.user(&user);
        for apply in [false, true] {
            let report = backfill(dir.path(), &workos, apply).await;
            if foreign {
                skipped(&report, &account.id, &[&user], "foreign external_id");
                assert_eq!(fake.user(&user), before);
                assert!(Links::open(dir.path()).users().unwrap().is_empty());
            } else {
                assert_eq!((report.users_linked, report.users_updated), (1, 1));
                assert!(report.skipped.is_empty(), "{report:?}");
                assert_eq!(
                    Links::open(dir.path()).user_of(&account.id).unwrap(),
                    apply.then(|| user.clone())
                );
            }
            assert_eq!(fake.user(&email_match), email_before);
            assert_eq!(fake.user_count(), 2);
        }
    }
}

#[tokio::test]
async fn several_workos_users_with_the_same_github_identity_are_skipped() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, METHODS);
    let (dir, accounts) = store();
    let account = accounts
        .sign_in_github(&profile(42, true), "signup")
        .unwrap()
        .account;
    let first = github_user(&fake, &workos, 42, "first@example.com").await;
    let second = github_user(&fake, &workos, 42, "second@example.com").await;
    let before = (fake.user(&first), fake.user(&second));
    for apply in [false, true] {
        let report = backfill(dir.path(), &workos, apply).await;
        skipped(
            &report,
            &account.id,
            &[&first, &second],
            "several GitHub identity matches",
        );
        assert_eq!((fake.user(&first), fake.user(&second)), before);
        assert!(Links::open(dir.path()).users().unwrap().is_empty());
    }
}

#[tokio::test]
async fn a_missing_link_is_preserved_when_its_replacement_is_unsafe() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, METHODS);
    let (dir, accounts) = store();
    let account = accounts
        .sign_in_github(&profile(1, true), "signup")
        .unwrap()
        .account;
    let links = Links::open(dir.path());
    links.link_user(&account.id, "user_missing").unwrap();
    let user = fake.add_user(EMAIL, None, true);
    external(&workos, &user, "foreign-product").await;
    let before = fake.user(&user);
    for apply in [false, true] {
        let report = backfill(dir.path(), &workos, apply).await;
        skipped(&report, &account.id, &[&user], "foreign external_id");
        assert_eq!(fake.user(&user), before);
        assert_eq!(
            links.user_of(&account.id).unwrap().as_deref(),
            Some("user_missing")
        );
    }
}

#[tokio::test]
async fn workspace_sync_does_not_add_change_or_remove_memberships_for_an_unsafe_link() {
    for linked in [false, true] {
        for existing_role in [None, Some("member"), Some("owner")] {
            let fake = FakeWorkos::start().await;
            let workos = fake.client(Mode::Dual, METHODS);
            let (dir, accounts) = store();
            let account = accounts
                .sign_in_github(&profile(1, true), "signup")
                .unwrap()
                .account;
            let team = accounts
                .create_workspace(
                    &account.id,
                    "Team",
                    WorkspaceKind::Organization,
                    "signup",
                    None,
                )
                .unwrap();
            let user = fake.add_user(EMAIL, None, true);
            external(&workos, &user, "foreign-product").await;
            let org = workos
                .create_organization(&team.name, &team.id)
                .await
                .unwrap();
            let links = Links::open(dir.path());
            if linked {
                links.link_user(&account.id, &user).unwrap();
            }
            links.link_org(&team.id, &org.id).unwrap();
            if let Some(role) = existing_role {
                workos.add_membership(&user, &org.id, role).await.unwrap();
            }
            let before = (fake.user(&user), fake.memberships(), links.users().unwrap());
            for apply in [false, true] {
                let report = sync::workspace(
                    dir.path(),
                    &workos,
                    &team.id,
                    Options { apply, test: false },
                )
                .await
                .unwrap();
                skipped(&report, &account.id, &[&user], "foreign external_id");
                assert_eq!(
                    (fake.user(&user), fake.memberships(), links.users().unwrap()),
                    before
                );
            }
        }
    }
}

#[tokio::test]
async fn safe_creation_and_replacement_of_a_missing_user_remain_retryable() {
    for verified in [false, true] {
        for missing_link in [false, true] {
            let fake = FakeWorkos::start().await;
            let workos = fake.client(Mode::Dual, METHODS);
            let (dir, accounts) = store();
            let account = accounts
                .sign_in_github(&profile(1, verified), "signup")
                .unwrap()
                .account;
            let links = Links::open(dir.path());
            if missing_link {
                links.link_user(&account.id, "user_missing").unwrap();
            }
            let dry = backfill(dir.path(), &workos, false).await;
            assert_eq!(dry.users_created, 1);
            assert!(dry.skipped.is_empty(), "{dry:?}");
            assert_eq!(fake.user_count(), 0);
            assert_eq!(
                links.user_of(&account.id).unwrap().as_deref(),
                missing_link.then_some("user_missing")
            );
            assert_eq!(backfill(dir.path(), &workos, true).await, dry);
            let id = links.user_of(&account.id).unwrap().unwrap();
            let made = fake.user(&id).unwrap();
            let email = if verified {
                EMAIL.to_string()
            } else {
                oa_auth::workos::synthetic_email(&account.id)
            };
            assert_eq!(made["email"], email);
            assert_eq!(made["external_id"], account.id);
            assert_eq!(fake.user_count(), 1);
            assert!(!backfill(dir.path(), &workos, true).await.wrote());
        }
    }
}
