use super::*;
use pay_ledger::compute::{Binding, PrincipalKind, Rights};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
use tenancy::{Manifest, Tenant, WorkspaceKind};
fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn write(path: &Path, value: &[u8]) {
    std::fs::write(path, value).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
fn policy(path: &Path, entries: Vec<Entry>) {
    write(
        path,
        &serde_json::to_vec(&Policy {
            schema: SCHEMA.into(),
            operator: "fixture-operator".into(),
            entries,
        })
        .unwrap(),
    );
}
fn entry(
    accounts: &Accounts,
    customer: &str,
    workspace: &str,
    source: Source,
    principal: &str,
    credential_file: &Path,
) -> Entry {
    let owner = accounts.authorize(workspace, customer).unwrap();
    let at = now();
    Entry {
        operator: "fixture-operator".into(),
        source,
        customer: customer.into(),
        workspace: workspace.into(),
        canonical_owner: customer.into(),
        canonical_owner_epoch: owner.epoch,
        canonical_members_epoch: owner.members_epoch,
        principal: principal.into(),
        credential_file: credential_file.into(),
        generation: 1,
        native_owner: None,
        native_owner_epoch: None,
        native_members_epoch: None,
        reviewed_at: at,
        valid_until: at + 3600,
        previous_authority: None,
    }
}
fn customer(accounts: &Accounts, label: &str, principal: &str) -> (String, String) {
    let account = accounts.create_account(label, &[principal.into()]).unwrap();
    let workspace = accounts
        .create_workspace(&account.id, label, WorkspaceKind::Personal, "fixture", None)
        .unwrap();
    (account.id, workspace.id)
}
#[test]
fn actual_retail_credentials_operator_mapping_rotation_and_two_customers_preserve_native_money() {
    let dir = private_dir();
    let canonical_dir = dir.path().join("canonical");
    std::fs::create_dir(&canonical_dir).unwrap();
    std::fs::set_permissions(&canonical_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let accounts = Accounts::install(&canonical_dir).unwrap();
    let (alice, workspace) = customer(&accounts, "Alice", "key:aaaaaaaaaaaaaaaa");
    let (bob, bob_workspace) = customer(&accounts, "Bob", "key:bbbbbbbbbbbbbbbb");
    let ledger_path = dir.path().join("money.db");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    std::fs::set_permissions(&ledger_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    ledger.create_compute_account("retail-alice", 1).unwrap();
    ledger.create_compute_account("retail-bob", 1).unwrap();
    ledger
        .bind_principal(&Binding {
            principal: "cli:alice".into(),
            account: "retail-alice".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("synthetic-alice"),
            rights: Rights {
                read: true,
                spend: false,
            },
            at: 1,
        })
        .unwrap();
    ledger
        .bind_principal(&Binding {
            principal: "cli:bob".into(),
            account: "retail-bob".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("synthetic-bob"),
            rights: Rights {
                read: true,
                spend: true,
            },
            at: 1,
        })
        .unwrap();
    for (account, amount, hash) in [("retail-alice", 21_000, 'a'), ("retail-bob", 43_000, 'b')] {
        let payment_hash = hash.to_string().repeat(64);
        ledger
            .open_top_up(&pay_ledger::compute::TopUp {
                id: format!("synthetic-{account}"),
                account: account.into(),
                amount_msat: amount,
                payment_hash: payment_hash.clone(),
                invoice: "synthetic-invoice".into(),
                created_at: 1,
                expires_at: 31,
            })
            .unwrap();
        ledger
            .observe_top_up(
                &payment_hash,
                &pay_ledger::compute::Receipt::Paid {
                    received_msat: amount,
                    at: 2,
                },
            )
            .unwrap();
    }
    let credential = dir.path().join("alice.credential");
    write(&credential, b"synthetic-alice\n");
    let policy_path = dir.path().join("policy.json");
    let source = Source {
        product: Product::Retail,
        issuer: "retail-fixture".into(),
        account: "retail-alice".into(),
        workspace: None,
    };
    let mut mapping = entry(
        &accounts,
        &alice,
        &workspace,
        source.clone(),
        "cli:alice",
        &credential,
    );
    policy(&policy_path, vec![mapping.clone()]);
    let config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Retail {
            issuer: source.issuer.clone(),
            ledger: ledger_path.clone(),
        }],
    };
    let adapter = NativeSources::open(&canonical_dir, &config).unwrap();
    let alice_before = ledger.compute_balance("retail-alice").unwrap();
    let bob_before = ledger.compute_balance("retail-bob").unwrap();
    assert_eq!(alice_before.available_msat, 21_000);
    assert_eq!(bob_before.available_msat, 43_000);
    let original = accounts
        .review_commercial(
            "commercial-alice",
            &alice,
            &workspace,
            &alice,
            None,
            std::slice::from_ref(&source),
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&original, &original.digest, &adapter)
        .unwrap();
    assert_eq!(adapter.selection(&source).unwrap(), Some(original.clone()));
    assert!(
        accounts
            .review_commercial(
                "commercial-bob",
                &bob,
                &bob_workspace,
                &bob,
                None,
                std::slice::from_ref(&source),
                &adapter
            )
            .is_err()
    );
    // Renaming the configured issuer, even across a new adapter, cannot
    // reattribute these same native records to the second customer.
    let alias_source = Source {
        issuer: "renamed-retail".into(),
        ..source.clone()
    };
    let mut alias_entry = entry(
        &accounts,
        &bob,
        &bob_workspace,
        alias_source.clone(),
        "cli:alice",
        &credential,
    );
    policy(&policy_path, vec![alias_entry.clone()]);
    let alias_config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Retail {
            issuer: alias_source.issuer.clone(),
            ledger: ledger_path.clone(),
        }],
    };
    let alias_adapter = NativeSources::open(&canonical_dir, &alias_config).unwrap();
    let alias_proof = alias_adapter
        .authorize(&alias_source, &bob, &bob_workspace)
        .unwrap();
    assert_eq!(
        alias_proof.native_identity,
        original.sources[0].native_identity
    );
    assert!(
        accounts
            .review_commercial(
                "commercial-bob",
                &bob,
                &bob_workspace,
                &bob,
                None,
                &[alias_source.clone()],
                &alias_adapter
            )
            .is_err()
    );
    // A backup retains the origin. Copying paths or changing an issuer cannot
    // manufacture new ownership for the retained ledger records either.
    let copied_path = dir.path().join("copied.db");
    std::fs::copy(&ledger_path, &copied_path).unwrap();
    let copied_config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Retail {
            issuer: alias_source.issuer.clone(),
            ledger: copied_path,
        }],
    };
    let copied_adapter = NativeSources::open(&canonical_dir, &copied_config).unwrap();
    assert!(
        accounts
            .review_commercial(
                "commercial-bob",
                &bob,
                &bob_workspace,
                &bob,
                None,
                &[alias_source.clone()],
                &copied_adapter
            )
            .is_err()
    );
    // Independent native books may have the same account label. Their origin,
    // not the label, determines whether these are distinct records.
    let independent_path = dir.path().join("independent.db");
    let mut independent = Ledger::open(&independent_path).unwrap();
    std::fs::set_permissions(&independent_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    independent
        .create_compute_account("retail-alice", 1)
        .unwrap();
    independent
        .bind_principal(&Binding {
            principal: "cli:alice".into(),
            account: "retail-alice".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("synthetic-independent"),
            rights: Rights {
                read: true,
                spend: false,
            },
            at: 1,
        })
        .unwrap();
    let independent_credential = dir.path().join("independent.credential");
    write(&independent_credential, b"synthetic-independent");
    alias_entry.credential_file = independent_credential;
    policy(&policy_path, vec![alias_entry]);
    let independent_config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Retail {
            issuer: alias_source.issuer.clone(),
            ledger: independent_path,
        }],
    };
    let independent_adapter = NativeSources::open(&canonical_dir, &independent_config).unwrap();
    assert_ne!(
        independent_adapter
            .authorize(&alias_source, &bob, &bob_workspace)
            .unwrap()
            .native_identity,
        original.sources[0].native_identity
    );
    let independent_review = accounts
        .review_commercial(
            "commercial-bob",
            &bob,
            &bob_workspace,
            &bob,
            None,
            &[alias_source],
            &independent_adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(
            &independent_review,
            &independent_review.digest,
            &independent_adapter,
        )
        .unwrap();
    policy(&policy_path, vec![mapping.clone()]);
    assert!(
        ledger
            .resolve_principal(
                "cli:alice",
                &credential_digest("synthetic-alice"),
                Need::Spend
            )
            .is_err()
    );
    // Neither a real read grant nor a wallet/label substitutes for the explicit
    // operator mapping; an unadmitted native account has no commercial alias.
    let other = Source {
        account: "retail-bob".into(),
        ..source.clone()
    };
    assert!(adapter.authorize(&other, &alice, &workspace).is_err());
    assert_eq!(adapter.selection(&other).unwrap(), None);
    ledger
        .rotate_principal("cli:alice", &credential_digest("synthetic-rotated"))
        .unwrap();
    assert!(adapter.selection(&source).is_err());
    write(&credential, b"synthetic-rotated");
    mapping.generation = 2;
    policy(&policy_path, vec![mapping.clone()]);
    assert!(
        accounts
            .review_commercial(
                "commercial-alice",
                &alice,
                &workspace,
                &alice,
                None,
                std::slice::from_ref(&source),
                &adapter
            )
            .is_err()
    );
    mapping.previous_authority = Some(original.sources[0].digest());
    policy(&policy_path, vec![mapping]);
    let next = accounts
        .review_commercial(
            "commercial-alice",
            &alice,
            &workspace,
            &alice,
            None,
            std::slice::from_ref(&source),
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&next, &next.digest, &adapter)
        .unwrap();
    assert_eq!(adapter.selection(&source).unwrap(), Some(next));
    assert_eq!(
        accounts.store().unwrap().commercial.bindings["commercial-alice"][0],
        original
    );
    ledger.revoke_principal("cli:alice", 2).unwrap();
    assert!(adapter.selection(&source).is_err());
    assert_eq!(
        ledger.compute_balance("retail-alice").unwrap(),
        alice_before
    );
    assert_eq!(ledger.compute_balance("retail-bob").unwrap(), bob_before);
}
#[test]
fn actual_tenancy_key_owner_and_product_family_are_checked_without_customer_inference() {
    let canonical = private_dir();
    let accounts = Accounts::install(canonical.path()).unwrap();
    let (alice, workspace) = customer(&accounts, "Alice", "key:aaaaaaaaaaaaaaaa");
    let (bob, bob_workspace) = customer(&accounts, "Bob", "key:bbbbbbbbbbbbbbbb");
    let native = private_dir();
    let manifest = Manifest {
        v: tenancy::SCHEMA.into(),
        sequence: 0,
        supersedes: None,
        shared: BTreeMap::new(),
        tenants: BTreeMap::from([(
            "fixture".into(),
            Tenant {
                credential: "key-ref:fixture".into(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        )]),
        digest: String::new(),
    };
    let registry = Registry::install(native.path(), manifest).unwrap();
    let key = tenancy::keys::issue(native.path(), registry.manifest(), "fixture").unwrap();
    std::fs::set_permissions(
        native.path().join("keys.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let native_accounts = Accounts::install(native.path()).unwrap();
    let principal = format!("key:{}", key.key.id);
    let (native_account, native_workspace) =
        customer(&native_accounts, "Different display label", &principal);
    let credential = canonical.path().join("native.credential");
    write(&credential, key.token.as_bytes());
    let source = Source {
        product: Product::Gateway,
        issuer: "native-fixture".into(),
        account: native_account.clone(),
        workspace: Some(native_workspace.clone()),
    };
    let mut mapping = entry(
        &accounts,
        &alice,
        &workspace,
        source.clone(),
        &principal,
        &credential,
    );
    let native_owner = native_accounts
        .authorize(&native_workspace, &native_account)
        .unwrap();
    mapping.native_owner = Some(native_account.clone());
    mapping.native_owner_epoch = Some(native_owner.epoch);
    mapping.native_members_epoch = Some(native_owner.members_epoch);
    let gateway_mapping = mapping.clone();
    let policy_path = canonical.path().join("policy.json");
    policy(&policy_path, vec![mapping.clone()]);
    let config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Tenancy {
            issuer: source.issuer.clone(),
            directory: native.path().into(),
        }],
    };
    let adapter = NativeSources::open(canonical.path(), &config).unwrap();
    let original = accounts
        .review_commercial(
            "commercial-alice",
            &alice,
            &workspace,
            &alice,
            None,
            std::slice::from_ref(&source),
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&original, &original.digest, &adapter)
        .unwrap();
    assert_eq!(adapter.selection(&source).unwrap(), Some(original));
    let plugin_source = Source {
        product: Product::Plugin,
        ..source.clone()
    };
    mapping.source = plugin_source.clone();
    mapping.customer = bob.clone();
    mapping.workspace = bob_workspace.clone();
    let bob_owner = accounts.authorize(&bob_workspace, &bob).unwrap();
    mapping.canonical_owner = bob.clone();
    mapping.canonical_owner_epoch = bob_owner.epoch;
    mapping.canonical_members_epoch = bob_owner.members_epoch;
    policy(&policy_path, vec![mapping]);
    assert!(
        accounts
            .review_commercial(
                "commercial-bob",
                &bob,
                &bob_workspace,
                &bob,
                None,
                &[plugin_source],
                &adapter
            )
            .is_err()
    );
    policy(&policy_path, vec![gateway_mapping]);
    assert!(adapter.authorize(&source, &alice, &workspace).is_ok());
    tenancy::keys::revoke(native.path(), &key.key.id).unwrap();
    std::fs::set_permissions(
        native.path().join("keys.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let adapter = NativeSources::open(canonical.path(), &config).unwrap();
    assert!(adapter.authorize(&source, &alice, &workspace).is_err());
}
#[test]
fn private_policy_replacement_shared_credentials_and_foreign_issuer_refuse() {
    let dir = private_dir();
    let accounts = Accounts::install(dir.path()).unwrap();
    let (alice, workspace) = customer(&accounts, "Alice", "key:aaaaaaaaaaaaaaaa");
    let ledger_path = dir.path().join("money.db");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    std::fs::set_permissions(&ledger_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    ledger.create_compute_account("retail-alice", 1).unwrap();
    ledger
        .bind_principal(&Binding {
            principal: "cli:alice".into(),
            account: "retail-alice".into(),
            kind: PrincipalKind::Cli,
            credential: credential_digest("synthetic"),
            rights: Rights {
                read: true,
                spend: false,
            },
            at: 1,
        })
        .unwrap();
    let credential = dir.path().join("credential");
    write(&credential, b"synthetic");
    let source = Source {
        product: Product::Retail,
        issuer: "fixture".into(),
        account: "retail-alice".into(),
        workspace: None,
    };
    let mapping = entry(
        &accounts,
        &alice,
        &workspace,
        source.clone(),
        "cli:alice",
        &credential,
    );
    let path = dir.path().join("policy.json");
    policy(&path, vec![mapping.clone()]);
    let config = Config {
        policy: path.clone(),
        stores: vec![NativeStore::Retail {
            issuer: "fixture".into(),
            ledger: ledger_path,
        }],
    };
    let mut duplicate = config.clone();
    let NativeStore::Retail { ledger, .. } = &config.stores[0] else {
        unreachable!()
    };
    duplicate.stores.push(NativeStore::Retail {
        issuer: "same-book-alias".into(),
        ledger: ledger.clone(),
    });
    assert!(NativeSources::open(dir.path(), &duplicate).is_err());
    let adapter = NativeSources::open(dir.path(), &config).unwrap();
    assert!(adapter.authorize(&source, &alice, &workspace).is_ok());
    let foreign = Source {
        issuer: "foreign".into(),
        ..source.clone()
    };
    assert!(adapter.authorize(&foreign, &alice, &workspace).is_err());
    std::fs::hard_link(&credential, dir.path().join("shared")).unwrap();
    assert!(adapter.authorize(&source, &alice, &workspace).is_err());
    std::fs::remove_file(dir.path().join("shared")).unwrap();
    std::fs::rename(&path, dir.path().join("prior-policy")).unwrap();
    policy(&path, vec![mapping]);
    assert!(adapter.authorize(&source, &alice, &workspace).is_err());
}

#[test]
fn native_team_member_selection_requires_current_owner_approval_and_reviewed_epochs() {
    let canonical = private_dir();
    let accounts = Accounts::install(canonical.path()).unwrap();
    let (customer, workspace) = customer(&accounts, "Member", "key:dddddddddddddddd");
    let native = private_dir();
    let manifest = Manifest {
        v: tenancy::SCHEMA.into(),
        sequence: 0,
        supersedes: None,
        shared: BTreeMap::new(),
        tenants: BTreeMap::from([(
            "fixture".into(),
            Tenant {
                credential: "key-ref:fixture".into(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        )]),
        digest: String::new(),
    };
    let registry = Registry::install(native.path(), manifest).unwrap();
    let owner_key = tenancy::keys::issue(native.path(), registry.manifest(), "fixture").unwrap();
    let member_key = tenancy::keys::issue(native.path(), registry.manifest(), "fixture").unwrap();
    std::fs::set_permissions(
        native.path().join("keys.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let native_accounts = Accounts::install(native.path()).unwrap();
    let owner_principal = format!("key:{}", owner_key.key.id);
    let member_principal = format!("key:{}", member_key.key.id);
    let owner = native_accounts
        .create_account("Owner", &[owner_principal.clone()])
        .unwrap();
    let member = native_accounts
        .create_account("Member", &[member_principal.clone()])
        .unwrap();
    let team = native_accounts
        .create_workspace(
            &owner.id,
            "Team",
            WorkspaceKind::Organization,
            "fixture",
            Some(4),
        )
        .unwrap();
    let invitation = native_accounts
        .invite(&owner.id, &team.id, Role::Member, 3600)
        .unwrap();
    native_accounts
        .accept_reviewed(&member.id, &invitation.token, &team.id, Role::Member)
        .unwrap();
    let source = Source {
        product: Product::Gateway,
        issuer: "native-team".into(),
        account: member.id.clone(),
        workspace: Some(team.id.clone()),
    };
    let credential = canonical.path().join("owner.credential");
    let member_credential = canonical.path().join("member.credential");
    write(&credential, owner_key.token.as_bytes());
    write(&member_credential, member_key.token.as_bytes());
    let mut mapping = entry(
        &accounts,
        &customer,
        &workspace,
        source.clone(),
        &owner_principal,
        &credential,
    );
    let authority = native_accounts.authorize(&team.id, &owner.id).unwrap();
    mapping.native_owner = Some(owner.id.clone());
    mapping.native_owner_epoch = Some(authority.epoch);
    mapping.native_members_epoch = Some(authority.members_epoch);
    let policy_path = canonical.path().join("policy.json");
    policy(&policy_path, vec![mapping.clone()]);
    let config = Config {
        policy: policy_path.clone(),
        stores: vec![NativeStore::Tenancy {
            issuer: source.issuer.clone(),
            directory: native.path().into(),
        }],
    };
    let adapter = NativeSources::open(canonical.path(), &config).unwrap();
    // The actual caller is the member; attribution approval comes separately
    // from the native owner credential in the private operator policy.
    let caller = native_accounts
        .authenticate_key(registry.manifest(), &team.id, &member_key.token)
        .unwrap();
    assert_eq!(caller.account, source.account);
    assert_eq!(caller.role, Role::Member);
    let original = accounts
        .review_commercial(
            "member-binding",
            &customer,
            &workspace,
            &customer,
            None,
            &[source.clone()],
            &adapter,
        )
        .unwrap();
    accounts
        .admit_commercial(&original, &original.digest, &adapter)
        .unwrap();
    assert_eq!(adapter.selection(&source).unwrap(), Some(original.clone()));
    let mut latest = original.clone();
    let refresh = |mapping: &mut Entry, latest: &tenancy::accounts::commercial::Revision| {
        let current_owner = native_accounts
            .authorize(&team.id, mapping.native_owner.as_deref().unwrap())
            .unwrap();
        mapping.native_owner_epoch = Some(current_owner.epoch);
        mapping.native_members_epoch = Some(current_owner.members_epoch);
        mapping.generation += 1;
        mapping.previous_authority = Some(latest.sources[0].digest());
        policy(&policy_path, vec![mapping.clone()]);
        let review = accounts
            .review_commercial(
                "member-binding",
                &customer,
                &workspace,
                &customer,
                None,
                &[source.clone()],
                &adapter,
            )
            .unwrap();
        accounts
            .admit_commercial(&review, &review.digest, &adapter)
            .unwrap();
        review
    };
    native_accounts
        .set_role(&owner.id, &team.id, &member.id, Role::Admin)
        .unwrap();
    assert!(adapter.selection(&source).is_err());
    latest = refresh(&mut mapping, &latest);
    assert_eq!(adapter.selection(&source).unwrap(), Some(latest.clone()));
    native_accounts
        .remove_member(&owner.id, &team.id, &member.id)
        .unwrap();
    assert!(adapter.selection(&source).is_err());
    let invitation = native_accounts
        .invite(&owner.id, &team.id, Role::Member, 3600)
        .unwrap();
    native_accounts
        .accept_reviewed(&member.id, &invitation.token, &team.id, Role::Member)
        .unwrap();
    assert!(adapter.selection(&source).is_err());
    latest = refresh(&mut mapping, &latest);
    native_accounts
        .transfer_ownership(&owner.id, &team.id, &member.id)
        .unwrap();
    assert!(adapter.selection(&source).is_err());
    mapping.native_owner = Some(member.id.clone());
    mapping.principal = member_principal;
    mapping.credential_file = member_credential;
    latest = refresh(&mut mapping, &latest);
    assert_eq!(adapter.selection(&source).unwrap(), Some(latest));
    assert_eq!(
        accounts.store().unwrap().commercial.bindings["member-binding"][0],
        original
    );
}

#[test]
fn replaced_canonical_directory_cannot_restore_old_commercial_selection() {
    let canonical = private_dir();
    let operator = private_dir();
    let accounts = Accounts::install(canonical.path()).unwrap();
    let original_accounts = std::fs::read(canonical.path().join("accounts.json")).unwrap();
    let ledger = operator.path().join("money.db");
    Ledger::open(&ledger).unwrap();
    std::fs::set_permissions(&ledger, std::fs::Permissions::from_mode(0o600)).unwrap();
    let policy_path = operator.path().join("policy.json");
    policy(&policy_path, vec![]);
    let config = Config {
        policy: policy_path,
        stores: vec![NativeStore::Retail {
            issuer: "retail".into(),
            ledger,
        }],
    };
    let adapter = NativeSources::open(canonical.path(), &config).unwrap();
    let source = Source {
        product: Product::Retail,
        issuer: "retail".into(),
        account: "native-alice".into(),
        workspace: None,
    };
    assert_eq!(adapter.selection(&source).unwrap(), None);
    // An ordinary atomic accounts revision keeps its enclosing directory.
    customer(&accounts, "Alice", "key:eeeeeeeeeeeeeeee");
    assert_eq!(adapter.selection(&source).unwrap(), None);
    let old = operator.path().join("original-canonical");
    std::fs::rename(canonical.path(), &old).unwrap();
    std::fs::create_dir(canonical.path()).unwrap();
    std::fs::set_permissions(canonical.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    write(&canonical.path().join("accounts.json"), &original_accounts);
    assert!(Accounts::open(canonical.path()).is_ok());
    assert!(adapter.selection(&source).is_err());
}
