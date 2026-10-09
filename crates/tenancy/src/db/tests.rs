//! Tests that need a database: they run with `TENANCY_TEST_DATABASE_URL`
//! set and are skipped (passing) without it.

use std::collections::BTreeMap;

use serde_json::Value;

use super::testing::{fresh_database, keep_on_files};
use super::{Database, attach, docs, import};
use crate::accounts::{Accounts, Role, WorkspaceKind};
use crate::manifest::{Manifest, Tenant};
use crate::{Registry, SessionBook, Sessions, keys};

fn database() -> Option<Database> {
    let database = fresh_database();
    if database.is_none() {
        eprintln!("skipped: TENANCY_TEST_DATABASE_URL is not set");
    }
    database
}

fn manifest() -> Manifest {
    let mut tenants = BTreeMap::new();
    tenants.insert(
        "acme".to_string(),
        Tenant {
            credential: "key-ref:acme".to_string(),
            principals: vec![],
            doors: BTreeMap::new(),
            quota: None,
        },
    );
    Manifest {
        v: crate::SCHEMA.to_string(),
        sequence: 0,
        supersedes: None,
        shared: BTreeMap::new(),
        tenants,
        digest: String::new(),
    }
}

/// Everything the database holds, as text, to look for secrets in.
fn dump(database: &Database) -> String {
    let mut text = String::new();
    for table in [
        "identity.stores",
        "identity.accounts",
        "identity.principals",
        "identity.bearer_keys",
        "identity.account_sessions",
        "workspace.workspaces",
        "workspace.memberships",
        "workspace.invitations",
        "audit.revisions",
    ] {
        for row in database
            .query(&format!("SELECT row_to_json(t)::text FROM {table} t"), &[])
            .unwrap()
        {
            text.push_str(&row.get::<_, String>(0));
            text.push('\n');
        }
    }
    text
}

#[test]
fn migrations_apply_once() {
    let Some(database) = database() else { return };
    // The template already ran them; running again applies nothing.
    assert!(database.migrate().unwrap().is_empty());
    let applied = database
        .query(
            "SELECT version FROM public.schema_migrations ORDER BY version",
            &[],
        )
        .unwrap();
    let versions: Vec<i32> = applied.iter().map(|row| row.get(0)).collect();
    assert_eq!(versions, vec![1]);
    // Two processes starting at once both come up.
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| database.migrate().unwrap());
        }
    });
}

#[test]
fn the_audit_log_refuses_changes() {
    let Some(database) = database() else { return };
    let dir = tempfile::tempdir().unwrap();
    attach(dir.path(), database.clone());
    Accounts::install(dir.path()).unwrap();
    assert!(
        database
            .execute("UPDATE audit.revisions SET sequence = 9", &[])
            .is_err()
    );
    assert!(
        database
            .execute("DELETE FROM audit.revisions", &[])
            .is_err()
    );
}

#[test]
fn secrets_never_reach_the_database_and_rows_have_their_columns() {
    let Some(database) = database() else { return };
    let dir = tempfile::tempdir().unwrap();
    attach(dir.path(), database.clone());
    let registry = Registry::install(dir.path(), manifest()).unwrap();
    let issued = keys::issue(dir.path(), registry.manifest(), "acme").unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let owner = accounts
        .create_account("Owner", &[format!("key:{}", issued.key.id)])
        .unwrap();
    let ws = accounts
        .create_workspace(&owner.id, "Team", WorkspaceKind::Organization, "acme", None)
        .unwrap();
    let invited = accounts
        .invite(&owner.id, &ws.id, Role::Member, 3600)
        .unwrap();
    let text = dump(&database);
    let key_secret = issued.token.split_once('.').unwrap().1;
    let invite_secret = invited.token.split_once('.').unwrap().1;
    assert!(!text.contains(key_secret));
    assert!(!text.contains(invite_secret));
    // The generated columns and the principal index are filled in.
    let rows = database
        .query(
            "SELECT p.principal, a.label FROM identity.principals p JOIN identity.accounts a ON a.id = p.account_id",
            &[],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get::<_, String>(0),
        format!("key:{}", issued.key.id)
    );
    let rows = database
        .query(
            "SELECT role, status FROM workspace.memberships WHERE workspace_id = $1",
            &[&ws.id],
        )
        .unwrap();
    assert_eq!(rows[0].get::<_, String>(0), "owner");
    let rows = database
        .query("SELECT tenant, status FROM identity.bearer_keys", &[])
        .unwrap();
    assert_eq!(rows[0].get::<_, String>(0), "acme");
}

#[test]
fn a_principal_names_one_account_in_the_database_too() {
    let Some(database) = database() else { return };
    let dir = tempfile::tempdir().unwrap();
    attach(dir.path(), database.clone());
    Accounts::install(dir.path()).unwrap();
    let doc = docs::load(&database, &docs::ACCOUNTS).unwrap().unwrap().1;
    let mut doc = (*doc).clone();
    // Two accounts claiming one principal, written past the store's own
    // validation: the database's constraint refuses the commit.
    let principal = format!("key:{}", "a".repeat(16));
    for id in ["acct_one", "acct_two"] {
        doc["accounts"][id] = serde_json::json!({
            "id": id, "label": id, "principals": [principal], "created": "2026-10-09T00:00:00Z"
        });
    }
    let refused = super::in_store_tx(&database, "accounts", |tx| {
        docs::write(tx, &docs::ACCOUNTS, &doc)
    });
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn another_process_sees_a_write_at_once() {
    let Some(first) = database() else { return };
    let dir = tempfile::tempdir().unwrap();
    attach(dir.path(), first.clone());
    let accounts = Accounts::install(dir.path()).unwrap();
    accounts.store().unwrap();
    // A second pool on the same database, as a second instance would be.
    let other_dir = tempfile::tempdir().unwrap();
    let second = Database::open(&first_url(&first)).unwrap();
    attach(other_dir.path(), second);
    let theirs = Accounts::open(other_dir.path()).unwrap();
    assert!(theirs.store().unwrap().accounts.is_empty());
    let made = accounts.create_account("Ada", &[]).unwrap();
    assert!(theirs.store().unwrap().accounts.contains_key(&made.id));
}

/// The connection string of a test database, from its own name.
fn first_url(database: &Database) -> String {
    let name: String = database.query("SELECT current_database()", &[]).unwrap()[0].get(0);
    format!("{} dbname={name}", super::testing::url().unwrap())
}

#[test]
fn files_import_idempotently_and_verify() {
    let Some(database) = database() else { return };
    // A registry on files, as the NFS share holds it.
    let files = tempfile::tempdir().unwrap();
    keep_on_files(files.path());
    let registry = Registry::install(files.path(), manifest()).unwrap();
    let issued = keys::issue(files.path(), registry.manifest(), "acme").unwrap();
    let accounts = Accounts::install(files.path()).unwrap();
    let owner = accounts
        .create_account("Owner", &[format!("key:{}", issued.key.id)])
        .unwrap();
    accounts
        .create_workspace(&owner.id, "Team", WorkspaceKind::Organization, "acme", None)
        .unwrap();
    let sessions = Sessions::install(files.path(), SessionBook::new(3600, 600)).unwrap();
    sessions
        .mutate(|book, _, now| {
            book.issue(crate::workspaces::UserId::from(owner.id.as_str()), now)
                .map(|_| ())
        })
        .unwrap();
    std::fs::create_dir_all(files.path().join("github-access")).unwrap();
    let account = owner.id.clone();
    let digest: String = {
        use sha2::Digest as _;
        sha2::Sha256::digest(account.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    std::fs::write(
        files
            .path()
            .join("github-access")
            .join(format!("{digest}.json")),
        serde_json::json!({"v": "openagents.github-access.v1", "account": account, "projects": []})
            .to_string(),
    )
    .unwrap();
    std::fs::write(
        files.path().join("inference-provider-keys.json"),
        serde_json::json!({"keys": {"acme": {"openrouter": {"sealed": "v1.x", "fingerprint": "f", "added_at": 1}}}})
            .to_string(),
    )
    .unwrap();

    let first = import::import(&database, files.path(), false).unwrap();
    assert_eq!(first.accounts, 1);
    assert_eq!(first.workspaces, 1);
    assert_eq!(first.memberships, 1);
    assert_eq!(first.sessions, 1);
    assert_eq!(first.bearer_keys, 1);
    assert_eq!(first.github_access, 1);
    assert_eq!(first.provider_keys, 1);
    assert!(first.revisions >= 3);
    assert!(first.stores.iter().all(|(_, state)| state == "imported"));
    let again = import::import(&database, files.path(), false).unwrap();
    assert!(again.stores.iter().all(|(_, state)| state == "unchanged"));
    let verified = import::verify(&database, files.path()).unwrap();
    assert!(verified.mismatches.is_empty(), "{:?}", verified.mismatches);

    // The files move on; a plain import refuses, a forced one follows.
    accounts.create_account("Later", &[]).unwrap();
    assert!(import::import(&database, files.path(), false).is_err());
    let verified = import::verify(&database, files.path()).unwrap();
    assert!(!verified.mismatches.is_empty());
    import::import(&database, files.path(), true).unwrap();
    assert!(
        import::verify(&database, files.path())
            .unwrap()
            .mismatches
            .is_empty()
    );

    // The imported stores serve through the ordinary API.
    let served = tempfile::tempdir().unwrap();
    attach(served.path(), database.clone());
    let reopened = Accounts::open(served.path()).unwrap();
    assert_eq!(reopened.store().unwrap().accounts.len(), 2);
    let auth = keys::authenticate(served.path(), registry.manifest(), &issued.token);
    assert!(auth.is_ok());
    let held: Value = database
        .query("SELECT record FROM identity.provider_keys", &[])
        .unwrap()[0]
        .get(0);
    assert_eq!(held["fingerprint"], "f");
}
