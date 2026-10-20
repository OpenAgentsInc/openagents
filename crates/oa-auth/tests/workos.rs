//! WorkOS sign-in, linking, and dual-write against the in-process fake.
//! Nothing here reaches WorkOS.

use oa_auth::workos::client::Refusal;
use oa_auth::workos::fake::FakeWorkos;
use oa_auth::workos::link::{LinkedBy, ResolveError, resolve};
use oa_auth::workos::sync::{self, Options};
use oa_auth::workos::{Grant, Method, Mode, synthetic_email};
use tenancy::accounts::identities::GithubProfile;
use tenancy::workos_links::Links;
use tenancy::{Accounts, Role, WorkspaceKind};

const ALL: &[Method] = &[
    Method::Github,
    Method::Google,
    Method::Password,
    Method::MagicCode,
    Method::Sso,
];

fn store() -> (tempfile::TempDir, Accounts) {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    (dir, accounts)
}

#[tokio::test]
async fn sign_up_with_a_password_needs_the_emailed_code_first() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    // Weak passwords are refused before anything is made.
    let weak = workos
        .create_user(oa_auth::workos::client::NewUser {
            email: "ada@example.com".into(),
            password: Some("short".into()),
            ..Default::default()
        })
        .await;
    assert_eq!(weak.unwrap_err(), Refusal::WeakPassword);
    let user = workos
        .create_user(oa_auth::workos::client::NewUser {
            email: "ada@example.com".into(),
            password: Some("correct horse battery".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!user.email_verified);
    let taken = workos
        .create_user(oa_auth::workos::client::NewUser {
            email: "ADA@example.com".into(),
            password: Some("correct horse battery".into()),
            ..Default::default()
        })
        .await;
    assert_eq!(taken.unwrap_err(), Refusal::EmailTaken);
    let wrong = workos
        .authenticate(Grant::Password {
            email: "ada@example.com".into(),
            password: "not it at all".into(),
        })
        .await;
    assert_eq!(wrong.unwrap_err(), Refusal::InvalidCredentials);
    let Err(Refusal::EmailVerificationRequired {
        pending,
        email_verification_id,
        ..
    }) = workos
        .authenticate(Grant::Password {
            email: "ada@example.com".into(),
            password: "correct horse battery".into(),
        })
        .await
    else {
        panic!("an unverified email must be verified first");
    };
    let code = fake
        .last_email("ada@example.com", "email_verification")
        .unwrap();
    // A staging check reads the code through the API instead of an inbox.
    let read = workos
        .email_verification_code(&email_verification_id.unwrap())
        .await
        .unwrap();
    assert_eq!(read.expose(), code);
    let signed = workos
        .authenticate(Grant::EmailVerification { code, pending })
        .await
        .unwrap();
    assert_eq!(signed.user.id, user.id);
    assert!(signed.user.email_verified);
    let claims = workos.verify(signed.access_token.expose()).await.unwrap();
    assert_eq!(claims.sub, user.id);
    assert!(!claims.sid.is_empty());
}

#[tokio::test]
async fn an_emailed_code_signs_in_and_refresh_tokens_are_single_use() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let code = workos.send_code("grace@example.com").await.unwrap();
    assert_eq!(
        fake.last_email("grace@example.com", "magic_auth")
            .as_deref(),
        Some(code.expose())
    );
    let bad = workos
        .authenticate(Grant::MagicCode {
            email: "grace@example.com".into(),
            code: "000000".into(),
        })
        .await;
    assert_eq!(bad.unwrap_err(), Refusal::InvalidCredentials);
    let signed = workos
        .authenticate(Grant::MagicCode {
            email: "grace@example.com".into(),
            code: code.expose().to_string(),
        })
        .await
        .unwrap();
    assert_eq!(signed.method.as_deref(), Some("MagicAuth"));
    let refresh = signed.refresh_token.expose().to_string();
    let renewed = workos
        .authenticate(Grant::Refresh {
            refresh_token: refresh.clone(),
        })
        .await
        .unwrap();
    assert_eq!(renewed.user.id, signed.user.id);
    assert_eq!(
        workos
            .authenticate(Grant::Refresh {
                refresh_token: refresh
            })
            .await
            .unwrap_err(),
        Refusal::InvalidCredentials
    );
    // Revoking the session ends its refresh tokens.
    let sid = workos
        .verify(renewed.access_token.expose())
        .await
        .unwrap()
        .sid;
    workos.revoke_session(&sid).await.unwrap();
    assert!(fake.revoked(&sid));
    assert!(
        workos
            .authenticate(Grant::Refresh {
                refresh_token: renewed.refresh_token.expose().to_string()
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn tokens_are_checked_against_the_key_set() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let user = fake.add_user("lin@example.com", None, true);
    let (access, _) = fake.sign_in(&user);
    assert_eq!(workos.verify(&access).await.unwrap().sub, user);
    // Cached now: the check needs no network.
    assert!(workos.keys().verify_cached(&access, false).unwrap().is_ok());
    let expired = oa_auth::workos::fake::mint(&user, "session_x", None, -10);
    assert!(workos.verify(&expired).await.is_err());
    assert!(workos.keys().verify(&expired, true).await.is_ok());
    let mut forged = access.clone();
    forged.pop();
    forged.push(if access.ends_with('A') { 'B' } else { 'A' });
    assert!(workos.verify(&forged).await.is_err());
    assert!(workos.verify("sess_0123").await.is_err());
}

#[tokio::test]
async fn password_reset_sets_a_new_password() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    fake.add_user("kay@example.com", Some("first password!"), true);
    workos.send_password_reset("kay@example.com").await.unwrap();
    // Unknown emails look the same to the person.
    workos
        .send_password_reset("nobody@example.com")
        .await
        .unwrap();
    let token = fake
        .last_email("kay@example.com", "password_reset")
        .unwrap();
    assert_eq!(
        workos.reset_password(&token, "short").await.unwrap_err(),
        Refusal::WeakPassword
    );
    workos
        .reset_password(&token, "second password!")
        .await
        .unwrap();
    assert_eq!(
        workos
            .reset_password(&token, "third password!!")
            .await
            .unwrap_err(),
        Refusal::Expired
    );
    workos
        .authenticate(Grant::Password {
            email: "kay@example.com".into(),
            password: "second password!".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_new_person_gets_one_account_and_signs_in_to_it_again() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let user = fake.add_user("new@example.com", Some("a long password"), true);
    let user = workos.user(&user).await.unwrap().unwrap();
    assert_eq!(
        resolve(dir.path(), &workos, &user, None).await.unwrap_err(),
        ResolveError::SignupClosed
    );
    let first = resolve(dir.path(), &workos, &user, Some("signup"))
        .await
        .unwrap();
    assert!(first.created);
    assert_eq!(first.by, LinkedBy::New);
    assert_eq!(first.account.label, "new");
    let workspace = first.workspace.unwrap();
    assert_eq!(workspace.kind, WorkspaceKind::Personal);
    // WorkOS now names the account.
    let stamped = workos.user(&user.id).await.unwrap().unwrap();
    assert_eq!(
        stamped.external_id.as_deref(),
        Some(first.account.id.as_str())
    );
    let again = resolve(dir.path(), &workos, &stamped, Some("signup"))
        .await
        .unwrap();
    assert_eq!(again.by, LinkedBy::Existing);
    assert_eq!(again.account.id, first.account.id);
    assert_eq!(accounts.store().unwrap().accounts.len(), 1);
}

fn github_profile(id: u64, email: &str) -> GithubProfile {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "login": "octo",
        "emails": [{"email": email, "primary": true, "verified": true}],
    }))
    .unwrap()
}

#[tokio::test]
async fn github_through_workos_finds_the_account_github_made_before() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let old = accounts
        .sign_in_github(&github_profile(583231, "octo@example.com"), "signup")
        .unwrap();
    // The person's GitHub email differs from anything WorkOS knows.
    fake.queue_sign_in(
        "GitHubOAuth",
        "octo-other@example.com",
        "583231",
        Some("Octo"),
    );
    let url = workos
        .authorization_url(
            Some("GitHubOAuth"),
            None,
            "http://127.0.0.1:1/auth/callback",
            "st",
            None,
        )
        .unwrap();
    let code = follow(&url).await;
    let signed = workos.authenticate(Grant::Code { code }).await.unwrap();
    let resolved = resolve(dir.path(), &workos, &signed.user, Some("signup"))
        .await
        .unwrap();
    assert_eq!(resolved.by, LinkedBy::Github);
    assert_eq!(resolved.account.id, old.account.id);
    assert_eq!(resolved.github, Some(583231));
    assert!(!resolved.created);
    assert_eq!(accounts.store().unwrap().accounts.len(), 1);
}

#[tokio::test]
async fn a_verified_email_finds_the_account_with_that_github_email() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let old = accounts
        .sign_in_github(&github_profile(77, "same@example.com"), "signup")
        .unwrap();
    let code = workos.send_code("same@example.com").await.unwrap();
    let signed = workos
        .authenticate(Grant::MagicCode {
            email: "same@example.com".into(),
            code: code.expose().to_string(),
        })
        .await
        .unwrap();
    let resolved = resolve(dir.path(), &workos, &signed.user, Some("signup"))
        .await
        .unwrap();
    assert_eq!(resolved.by, LinkedBy::Email);
    assert_eq!(resolved.account.id, old.account.id);
}

#[tokio::test]
async fn a_key_only_account_moves_off_its_synthetic_user() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let key = "ab".repeat(32);
    let made = accounts.sign_in_nostr(&key, "signup").unwrap();
    // The dual-write makes its synthetic User.
    let report = sync::account(dir.path(), &workos, &made.account.id, Options::default())
        .await
        .unwrap();
    assert_eq!(report.users_created, 1);
    let synthetic = fake
        .user_by_email(&synthetic_email(&made.account.id))
        .unwrap();
    assert_eq!(synthetic["email_verified"], true);
    assert!(
        synthetic["metadata"]["npub"]
            .as_str()
            .unwrap()
            .starts_with("npub1")
    );
    // Running it again writes nothing.
    let again = sync::account(dir.path(), &workos, &made.account.id, Options::default())
        .await
        .unwrap();
    assert!(!again.wrote(), "{again:?}");
    // The person signs in with a real email whose User names the account.
    let real = fake.add_user("keyed@example.com", Some("a long password"), true);
    workos
        .update_user(
            &real,
            oa_auth::workos::client::UserChange {
                external_id: None,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // A User whose external ID is the account (set by support, say).
    let mut user = workos.user(&real).await.unwrap().unwrap();
    user.external_id = Some(made.account.id.clone());
    let resolved = resolve(dir.path(), &workos, &user, Some("signup"))
        .await
        .unwrap();
    assert_eq!(resolved.by, LinkedBy::ExternalId);
    assert_eq!(resolved.account.id, made.account.id);
    assert_eq!(
        Links::open(dir.path())
            .user_of(&made.account.id)
            .unwrap()
            .as_deref(),
        Some(real.as_str())
    );
    assert!(
        fake.user_by_email(&synthetic_email(&made.account.id))
            .is_none(),
        "the synthetic User is deleted"
    );
}

#[tokio::test]
async fn an_account_linked_to_a_real_user_is_never_taken_over() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let made = accounts.sign_in_nostr(&"cd".repeat(32), "signup").unwrap();
    let first = fake.add_user("one@example.com", None, true);
    Links::open(dir.path())
        .link_user(&made.account.id, &first)
        .unwrap();
    let second = fake.add_user("two@example.com", None, true);
    let mut user = workos.user(&second).await.unwrap().unwrap();
    user.external_id = Some(made.account.id.clone());
    assert_eq!(
        resolve(dir.path(), &workos, &user, Some("signup"))
            .await
            .unwrap_err(),
        ResolveError::Conflict
    );
}

#[tokio::test]
async fn shared_workspaces_become_organizations_with_their_members() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let owner = accounts
        .sign_in_nostr(&"01".repeat(32), "signup")
        .unwrap()
        .account;
    let member = accounts
        .sign_in_nostr(&"02".repeat(32), "signup")
        .unwrap()
        .account;
    let team = accounts
        .create_workspace(
            &owner.id,
            "Team",
            WorkspaceKind::Organization,
            "signup",
            None,
        )
        .unwrap();
    let invite = accounts
        .invite(&owner.id, &team.id, Role::Member, 3600)
        .unwrap();
    accounts.accept(&member.id, &invite.token).unwrap();
    // A dry run writes nothing.
    let dry = sync::account(
        dir.path(),
        &workos,
        &owner.id,
        Options {
            apply: false,
            test: false,
        },
    )
    .await
    .unwrap();
    assert!(dry.orgs_created == 1 && dry.users_created >= 2, "{dry:?}");
    assert_eq!(fake.user_count(), 0);
    assert!(fake.organizations().is_empty());
    let report = sync::account(dir.path(), &workos, &owner.id, Options::default())
        .await
        .unwrap();
    assert_eq!(report.orgs_created, 1);
    assert_eq!(report.memberships_added, 2);
    let orgs = fake.organizations();
    assert_eq!(orgs.len(), 1, "personal workspaces aren't organizations");
    assert_eq!(orgs[0]["external_id"], team.id.as_str());
    let mut roles: Vec<String> = fake
        .memberships()
        .into_iter()
        .map(|(_, _, role)| role)
        .collect();
    roles.sort();
    assert_eq!(roles, ["member", "owner"]);
    // A role change and a removal follow.
    accounts
        .set_role(&owner.id, &team.id, &member.id, Role::Admin)
        .unwrap();
    let changed = sync::workspace(dir.path(), &workos, &team.id, Options::default())
        .await
        .unwrap();
    assert_eq!(changed.memberships_changed, 1);
    accounts
        .remove_member(&owner.id, &team.id, &member.id)
        .unwrap();
    let removed = sync::workspace(dir.path(), &workos, &team.id, Options::default())
        .await
        .unwrap();
    assert_eq!(removed.memberships_removed, 1);
    assert_eq!(fake.memberships().len(), 1);
}

#[tokio::test]
async fn the_backfill_is_resumable_and_the_reconcile_report_ends_empty() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    for byte in ["0a", "0b", "0c"] {
        accounts.sign_in_nostr(&byte.repeat(32), "signup").unwrap();
    }
    accounts
        .sign_in_github(&github_profile(9, "gh@example.com"), "signup")
        .unwrap();
    let before = sync::reconcile(dir.path()).unwrap();
    assert_eq!(before.accounts_without_user.len(), 4);
    let total = sync::all(
        dir.path(),
        &workos,
        Options::default(),
        None,
        || async {},
        |_, _| {},
    )
    .await
    .unwrap();
    assert_eq!(total.users_created, 4);
    // The GitHub account's User has its verified email, not a synthetic one.
    assert!(fake.user_by_email("gh@example.com").is_some());
    let after = sync::reconcile(dir.path()).unwrap();
    assert!(after.accounts_without_user.is_empty(), "{after:?}");
    assert_eq!(after.linked_accounts, 4);
    let rerun = sync::all(
        dir.path(),
        &workos,
        Options::default(),
        None,
        || async {},
        |_, _| {},
    )
    .await
    .unwrap();
    assert!(!rerun.wrote(), "{rerun:?}");
}

#[tokio::test]
async fn deleting_an_account_deletes_its_user() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let dir = tempfile::tempdir().unwrap();
    Accounts::install(dir.path()).unwrap();
    // A link to an account the store doesn't hold: the account was deleted.
    let gone = "acct_00000000000000aa";
    let user = fake.add_user(&synthetic_email(gone), None, true);
    workos
        .update_user(
            &user,
            oa_auth::workos::client::UserChange {
                external_id: Some(gone.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    Links::open(dir.path()).link_user(gone, &user).unwrap();
    let report = sync::account(dir.path(), &workos, gone, Options::default())
        .await
        .unwrap();
    assert_eq!(report.users_deleted, 1);
    assert!(fake.user(&user).is_none());
    assert_eq!(Links::open(dir.path()).user_of(gone).unwrap(), None);
}

#[tokio::test]
async fn tombstone_sync_deletes_and_unlinks_without_recreating_users() {
    for linked in [false, true] {
        for externally_deleted in [false, true] {
            let fake = FakeWorkos::start().await;
            let workos = fake.client(Mode::Dual, ALL);
            let (dir, accounts) = store();
            let account = accounts.create_account("Ada", &[]).unwrap();
            sync::account(dir.path(), &workos, &account.id, Options::default())
                .await
                .unwrap();
            let links = Links::open(dir.path());
            let user = links.user_of(&account.id).unwrap().unwrap();
            if !linked {
                links.unlink_account(&account.id).unwrap();
            }
            if externally_deleted {
                workos.delete_user(&user).await.unwrap();
            }
            accounts.delete_account(&account.id).unwrap();
            let tombstone = accounts.store().unwrap().accounts[&account.id].clone();
            assert_eq!(tombstone.label, tenancy::accounts::deletion::DELETED_LABEL);
            assert!(tombstone.principals.is_empty());

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
            assert_eq!(dry.users_deleted, u32::from(!externally_deleted));
            assert_eq!(dry.users_created, 0);
            assert_eq!(fake.user_count(), usize::from(!externally_deleted));
            assert_eq!(links.user_of(&account.id).unwrap().is_some(), linked);

            let applied = sync::account(dir.path(), &workos, &account.id, Options::default())
                .await
                .unwrap();
            assert_eq!(applied.users_deleted, dry.users_deleted);
            assert_eq!(applied.users_created, 0);
            assert_eq!(fake.user_count(), 0);
            assert!(links.user_of(&account.id).unwrap().is_none());
            let again = sync::account(dir.path(), &workos, &account.id, Options::default())
                .await
                .unwrap();
            assert!(!again.wrote(), "{again:?}");
            assert_eq!(accounts.store().unwrap().accounts[&account.id], tombstone);
        }
    }
}

#[tokio::test]
async fn backfill_and_reconcile_exclude_tombstones_and_preserve_new_accounts() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    // Neither a new account without principals nor the label alone means deleted.
    let new = accounts.create_account("New", &[]).unwrap();
    let named = accounts
        .create_account(
            tenancy::accounts::deletion::DELETED_LABEL,
            &["key:aaaaaaaaaaaaaaaa".into()],
        )
        .unwrap();
    let agent = accounts
        .register_agent("Bot", "signup", "bbbbbbbbbbbbbbbb", &"c".repeat(64), 1000)
        .unwrap()
        .account;
    let gone = accounts.create_account("Gone", &[]).unwrap();
    let unlinked = accounts.create_account("Also gone", &[]).unwrap();
    sync::account(dir.path(), &workos, &gone.id, Options::default())
        .await
        .unwrap();
    let links = Links::open(dir.path());
    let user = links.user_of(&gone.id).unwrap().unwrap();
    workos.delete_user(&user).await.unwrap();
    accounts.delete_account(&gone.id).unwrap();
    accounts.delete_account(&unlinked.id).unwrap();
    let before_store = accounts.store().unwrap();
    let before = sync::reconcile(dir.path()).unwrap();
    assert_eq!(before.accounts, 3);
    assert_eq!(before.agent_accounts, 1);
    assert_eq!(before.linked_accounts, 0);
    let mut missing = vec![new.id.clone(), named.id.clone()];
    missing.sort();
    assert_eq!(before.accounts_without_user, missing);
    assert_eq!(before.links_without_account, vec![gone.id.clone()]);

    for apply in [false, true] {
        let mut visited = Vec::new();
        let report = sync::all(
            dir.path(),
            &workos,
            Options { apply, test: false },
            None,
            || async {},
            |id, _| visited.push(id.to_string()),
        )
        .await
        .unwrap();
        assert_eq!(report.users_created, 2, "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(visited.contains(&new.id) && visited.contains(&named.id));
        assert!(visited.contains(&agent.id));
        assert!(!visited.contains(&gone.id) && !visited.contains(&unlinked.id));
        assert_eq!(fake.user_count(), if apply { 2 } else { 0 });
    }
    let after = sync::reconcile(dir.path()).unwrap();
    assert_eq!(after.accounts, 3);
    assert_eq!(after.agent_accounts, 1);
    assert_eq!(after.linked_accounts, 2);
    assert!(after.accounts_without_user.is_empty());
    assert_eq!(after.links_without_account, vec![gone.id]);
    assert_eq!(
        serde_json::to_value(accounts.store().unwrap()).unwrap(),
        serde_json::to_value(before_store).unwrap()
    );
}

#[tokio::test]
async fn workspace_sync_removes_a_deleted_members_membership_in_dry_run_and_apply() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let owner = accounts.create_account("Owner", &[]).unwrap();
    let member = accounts.create_account("Member", &[]).unwrap();
    let team = accounts
        .create_workspace(
            &owner.id,
            "Team",
            WorkspaceKind::Organization,
            "signup",
            None,
        )
        .unwrap();
    let invite = accounts
        .invite(&owner.id, &team.id, Role::Member, 3600)
        .unwrap();
    accounts.accept(&member.id, &invite.token).unwrap();
    sync::workspace(dir.path(), &workos, &team.id, Options::default())
        .await
        .unwrap();
    let user = Links::open(dir.path())
        .user_of(&member.id)
        .unwrap()
        .unwrap();
    accounts.delete_account(&member.id).unwrap();
    let dry = sync::workspace(
        dir.path(),
        &workos,
        &team.id,
        Options {
            apply: false,
            test: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(dry.memberships_removed, 1);
    assert_eq!(dry.memberships_added, 0);
    assert_eq!(fake.memberships().len(), 2);
    let applied = sync::workspace(dir.path(), &workos, &team.id, Options::default())
        .await
        .unwrap();
    assert_eq!(applied.memberships_removed, 1);
    assert_eq!(fake.memberships().len(), 1);
    assert!(fake.memberships().iter().all(|(id, _, _)| id != &user));
    sync::account(dir.path(), &workos, &member.id, Options::default())
        .await
        .unwrap();
    assert!(fake.user(&user).is_none());
    assert_eq!(fake.organizations().len(), 1);
}

#[tokio::test]
async fn a_tombstone_owner_does_not_keep_a_workos_organization_or_membership() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let owner = accounts.create_account("Owner", &[]).unwrap();
    let team = accounts
        .create_workspace(
            &owner.id,
            "Team",
            WorkspaceKind::Organization,
            "signup",
            None,
        )
        .unwrap();
    sync::account(dir.path(), &workos, &owner.id, Options::default())
        .await
        .unwrap();
    assert_eq!(fake.memberships().len(), 1);
    accounts.delete_account(&owner.id).unwrap();
    let dry = sync::workspace(
        dir.path(),
        &workos,
        &team.id,
        Options {
            apply: false,
            test: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(dry.orgs_deleted, 1);
    assert_eq!(fake.memberships().len(), 1);
    let counts = sync::reconcile(dir.path()).unwrap();
    assert_eq!(counts.accounts, 0);
    assert_eq!(counts.organization_workspaces, 0);
    assert_eq!(counts.linked_workspaces, 0);
    let report = sync::account(dir.path(), &workos, &owner.id, Options::default())
        .await
        .unwrap();
    assert_eq!(report.users_deleted, 1);
    assert_eq!(report.orgs_deleted, 1);
    assert!(fake.organizations().is_empty());
    assert!(fake.memberships().is_empty());
    assert!(Links::open(dir.path()).org_of(&team.id).unwrap().is_none());
    let rerun = sync::all(
        dir.path(),
        &workos,
        Options::default(),
        None,
        || async {},
        |_, _| {},
    )
    .await
    .unwrap();
    assert!(!rerun.wrote(), "{rerun:?}");
    assert_eq!(fake.user_count(), 0);
    assert!(fake.organizations().is_empty());
}

#[tokio::test]
async fn sso_starts_from_the_organization_for_an_email_domain() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    let org = fake.add_sso_organization("Acme", "acme.test");
    assert_eq!(
        workos
            .organization_for_domain("acme.test")
            .await
            .unwrap()
            .unwrap()
            .id,
        org
    );
    assert!(
        workos
            .organization_for_domain("other.test")
            .await
            .unwrap()
            .is_none()
    );
    fake.queue_sign_in(&org, "wile@acme.test", "sso-wile", None);
    let url = workos
        .authorization_url(
            None,
            Some(&org),
            "http://127.0.0.1:1/auth/callback",
            "st",
            None,
        )
        .unwrap();
    let code = follow(&url).await;
    let signed = workos.authenticate(Grant::Code { code }).await.unwrap();
    assert_eq!(signed.user.email, "wile@acme.test");
}

#[tokio::test]
async fn an_outage_is_unavailable_not_a_wrong_password() {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(Mode::Dual, ALL);
    fake.set_down(true);
    assert_eq!(
        workos
            .authenticate(Grant::Password {
                email: "a@example.com".into(),
                password: "whatever it is".into()
            })
            .await
            .unwrap_err(),
        Refusal::Unavailable
    );
}

/// Follow an authorization URL to the fake and read the code it returns.
async fn follow(url: &str) -> String {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let response = client.get(url).send().await.unwrap();
    assert_eq!(
        response.status(),
        303,
        "{}",
        response.text().await.unwrap_or_default()
    );
    let location = response.headers()["location"].to_str().unwrap().to_string();
    let parsed = url::Url::parse(&location).unwrap();
    let code = parsed
        .query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    assert_eq!(
        parsed.query_pairs().find(|(n, _)| n == "state").unwrap().1,
        "st"
    );
    code
}

#[tokio::test]
async fn an_owner_is_an_admin_where_the_environment_has_no_owner_role() {
    let fake = FakeWorkos::start().await;
    fake.set_roles(&["member", "admin"]);
    let workos = fake.client(Mode::Dual, ALL);
    let (dir, accounts) = store();
    let owner = accounts
        .sign_in_nostr(&"03".repeat(32), "signup")
        .unwrap()
        .account;
    accounts
        .create_workspace(
            &owner.id,
            "Crew",
            WorkspaceKind::Organization,
            "signup",
            None,
        )
        .unwrap();
    let first = sync::account(dir.path(), &workos, &owner.id, Options::default())
        .await
        .unwrap();
    assert_eq!(first.memberships_added, 1);
    assert_eq!(fake.memberships()[0].2, "admin");
    let again = sync::account(dir.path(), &workos, &owner.id, Options::default())
        .await
        .unwrap();
    assert!(!again.wrote(), "no drift: {again:?}");
}

/// A read-only check against a real WorkOS environment: the key set, a
/// user search, and an organization search decode with the SDK's models.
/// Run by hand with `WORKOS_API_KEY` and `WORKOS_CLIENT_ID` set; it never
/// writes. `cargo test -p oa-auth --test workos live_read_only -- --ignored`
#[tokio::test]
#[ignore = "reaches a real WorkOS environment; run by hand"]
async fn live_read_only() {
    let (Ok(key), Ok(client)) = (
        std::env::var("WORKOS_API_KEY"),
        std::env::var("WORKOS_CLIENT_ID"),
    ) else {
        panic!("set WORKOS_API_KEY and WORKOS_CLIENT_ID");
    };
    let settings: oa_auth::workos::Settings = serde_json::from_value(
        serde_json::json!({"client_id": client, "api_key_env": "WORKOS_API_KEY"}),
    )
    .unwrap();
    let workos = oa_auth::workos::Workos::with_key(settings, &key);
    workos.keys().refresh().await.expect("the key set decodes");
    let nobody = workos
        .users_by_email("nobody-oa-probe@example.invalid")
        .await
        .expect("a user search decodes");
    assert!(nobody.is_empty());
    assert!(
        workos
            .organization_for_domain("example.invalid")
            .await
            .expect("an organization search decodes")
            .is_none()
    );
    assert!(
        workos
            .user("user_01OAPROBENOTREAL")
            .await
            .expect("a missing user is None")
            .is_none()
    );
    let forged = oa_auth::workos::fake::mint("user_01X", "session_x", None, 60);
    assert!(
        workos.verify(&forged).await.is_err(),
        "a token from another key is refused"
    );
    let role = workos.role_slug(tenancy::Role::Owner).await;
    eprintln!("owner maps to {role}");
}

#[test]
fn retirement_preserves_keys_anonymous_and_proved_device_sessions_and_restores_safely() {
    use tenancy::sessions::{AppLabel, KeyLabel, SessionBook, Sessions};
    let (dir, accounts) = store();
    let person = accounts.create_account("Person", &[]).unwrap();
    let unlinked = accounts.create_account("Unlinked", &[]).unwrap();
    let sessions = Sessions::install(dir.path(), SessionBook::new(3600, 3600)).unwrap();
    Links::open(dir.path())
        .link_user(&person.id, "user_person")
        .unwrap();
    let old = sessions
        .mutate(|book, _, at| {
            book.set_credential(person.id.as_str().into(), "d".repeat(64), at)?;
            book.issue_recovery(&person.id.as_str().into(), at)?;
            book.burn_nostr_proofs(&[&"e".repeat(64)], at + 300, at)?;
            let legacy = book.issue(person.id.as_str().into(), at)?;
            book.issue_app(
                person.id.as_str().into(),
                AppLabel::new("Coder", "device_code")?,
                at,
            )?;
            book.issue_key(
                person.id.as_str().into(),
                KeyLabel::new("automation")?,
                None,
                at,
            )?;
            book.issue_anonymous(at)?;
            let device = book.start_device(AppLabel::new("Coder", "fixture")?, at)?;
            book.decide_device(&device.user_code, &person.id.as_str().into(), true, at)?;
            book.poll_device(&device.device_code, at + 10)?;
            let unlinked_device = book.start_device(AppLabel::new("Coder", "unlinked")?, at)?;
            book.decide_device(
                &unlinked_device.user_code,
                &unlinked.id.as_str().into(),
                true,
                at,
            )?;
            book.poll_device(&unlinked_device.device_code, at + 10)?;
            Ok(legacy.once)
        })
        .unwrap();
    let before = sessions.store().unwrap();
    let dry = oa_auth::workos::retire::retire(dir.path(), false).unwrap();
    assert_eq!(dry.retired, 1);
    assert_eq!(dry.preserved, 5);
    assert_eq!(dry.app_sessions, 3);
    assert!(dry.retire_apps_after_release);
    assert!(dry.snapshot.is_none());
    assert_eq!(before.digest, sessions.store().unwrap().digest);
    let applied = oa_auth::workos::retire::retire(dir.path(), true).unwrap();
    let snapshot = applied.snapshot.unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join(&snapshot)).unwrap()).unwrap();
    assert_eq!(saved["preserved_apps"].as_array().unwrap().len(), 3);
    let after = sessions.store().unwrap();
    after.validate("retired test").unwrap();
    assert_eq!(after.book.sessions.len(), 5);
    assert!(after.book.session_of_token(&old).is_none());
    assert_eq!(
        serde_json::to_value(&before.book.recoveries).unwrap(),
        serde_json::to_value(&after.book.recoveries).unwrap()
    );
    assert_eq!(before.book.nostr_proofs, after.book.nostr_proofs);
    assert_eq!(
        oa_auth::workos::retire::restore(dir.path(), &snapshot, false)
            .unwrap()
            .restored,
        1
    );
    assert_eq!(sessions.store().unwrap().book.sessions.len(), 5);
    oa_auth::workos::retire::restore(dir.path(), &snapshot, true).unwrap();
    let restored = sessions.store().unwrap();
    restored.validate("restored test").unwrap();
    assert_eq!(restored.book.sessions, before.book.sessions);
    assert_eq!(
        oa_auth::workos::retire::restore(dir.path(), &snapshot, true)
            .unwrap()
            .restored,
        0
    );
    assert!(oa_auth::workos::retire::restore(dir.path(), "../sessions.json", true).is_err());
}
