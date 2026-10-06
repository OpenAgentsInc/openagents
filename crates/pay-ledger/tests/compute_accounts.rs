//! #10705: every client resolves one account; revoked, rotated, and foreign
//! credentials cannot read or spend it.

use pay_ledger::{
    Error, Ledger,
    compute::{Binding, Need, PrincipalKind, Rights, credential_digest},
};

const AT: i64 = 1_791_000_000;

fn bind(ledger: &mut Ledger, principal: &str, account: &str, kind: PrincipalKind, spend: bool) {
    ledger
        .bind_principal(&Binding {
            principal: principal.into(),
            account: account.into(),
            kind,
            credential: credential_digest(&format!("secret-{principal}")),
            rights: Rights { read: true, spend },
            at: AT,
        })
        .unwrap();
}

fn secret(principal: &str) -> String {
    credential_digest(&format!("secret-{principal}"))
}

fn denied(result: pay_ledger::Result<impl std::fmt::Debug>) -> bool {
    matches!(result, Err(Error::Denied(_)))
}

#[test]
fn every_client_resolves_the_same_account() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.create_compute_account("acct-a", AT).unwrap();
    let clients = [
        ("window:host-1", PrincipalKind::Window),
        ("workshop:grid-1", PrincipalKind::Workshop),
        ("cli:laptop", PrincipalKind::Cli),
        ("key:oak_1", PrincipalKind::ApiKey),
    ];
    for (principal, kind) in clients {
        bind(&mut ledger, principal, "acct-a", kind, true);
    }
    for (principal, kind) in clients {
        let found = ledger
            .resolve_principal(principal, &secret(principal), Need::Spend)
            .unwrap();
        assert_eq!(found.account, "acct-a");
        assert_eq!(found.kind, kind);
    }
    assert_eq!(ledger.principals("acct-a").unwrap().len(), 4);
}

#[test]
fn rebinding_is_idempotent_and_never_moves_accounts() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.create_compute_account("acct-a", AT).unwrap();
    ledger.create_compute_account("acct-b", AT).unwrap();
    bind(
        &mut ledger,
        "cli:laptop",
        "acct-a",
        PrincipalKind::Cli,
        true,
    );
    bind(
        &mut ledger,
        "cli:laptop",
        "acct-a",
        PrincipalKind::Cli,
        true,
    );
    let moved = ledger.bind_principal(&Binding {
        principal: "cli:laptop".into(),
        account: "acct-b".into(),
        kind: PrincipalKind::Cli,
        credential: secret("cli:laptop"),
        rights: Rights {
            read: true,
            spend: true,
        },
        at: AT,
    });
    assert!(matches!(moved, Err(Error::Conflict(_))));
    let raw = ledger.bind_principal(&Binding {
        principal: "cli:other".into(),
        account: "acct-a".into(),
        kind: PrincipalKind::Cli,
        credential: "not-a-digest".into(),
        rights: Rights {
            read: true,
            spend: true,
        },
        at: AT,
    });
    assert!(matches!(raw, Err(Error::Invalid(_))));
}

#[test]
fn revoked_rotated_and_foreign_credentials_are_denied() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.create_compute_account("acct-a", AT).unwrap();
    ledger.create_compute_account("acct-b", AT).unwrap();
    bind(
        &mut ledger,
        "key:oak_1",
        "acct-a",
        PrincipalKind::ApiKey,
        true,
    );
    bind(
        &mut ledger,
        "phone:p1",
        "acct-a",
        PrincipalKind::Phone,
        false,
    );
    bind(
        &mut ledger,
        "key:oak_b",
        "acct-b",
        PrincipalKind::ApiKey,
        true,
    );

    // Another account's credential does not open this principal.
    assert!(denied(ledger.resolve_principal(
        "key:oak_1",
        &secret("key:oak_b"),
        Need::Read
    )));
    // A read-only phone cannot spend.
    assert!(
        ledger
            .resolve_principal("phone:p1", &secret("phone:p1"), Need::Read)
            .is_ok()
    );
    assert!(denied(ledger.resolve_principal(
        "phone:p1",
        &secret("phone:p1"),
        Need::Spend
    )));

    // Rotation: the old credential stops at once.
    let rotated = ledger
        .rotate_principal("key:oak_1", &credential_digest("new"))
        .unwrap();
    assert_eq!(rotated.generation, 2);
    assert!(denied(ledger.resolve_principal(
        "key:oak_1",
        &secret("key:oak_1"),
        Need::Read
    )));
    assert!(
        ledger
            .resolve_principal("key:oak_1", &credential_digest("new"), Need::Spend)
            .is_ok()
    );

    // Revocation is final.
    ledger.revoke_principal("key:oak_1", AT + 5).unwrap();
    assert!(denied(ledger.resolve_principal(
        "key:oak_1",
        &credential_digest("new"),
        Need::Read
    )));
    assert!(denied(
        ledger.rotate_principal("key:oak_1", &credential_digest("again"))
    ));
    assert!(denied(ledger.resolve_principal(
        "key:nobody",
        &secret("key:nobody"),
        Need::Read
    )));
}

#[test]
fn principals_survive_restart_without_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    {
        let mut ledger = Ledger::open(&path).unwrap();
        ledger.create_compute_account("acct-a", AT).unwrap();
        bind(
            &mut ledger,
            "window:host-1",
            "acct-a",
            PrincipalKind::Window,
            true,
        );
    }
    let ledger = Ledger::open(&path).unwrap();
    let found = ledger
        .resolve_principal("window:host-1", &secret("window:host-1"), Need::Read)
        .unwrap();
    assert_eq!(found.account, "acct-a");
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("secret-window:host-1"));
}
