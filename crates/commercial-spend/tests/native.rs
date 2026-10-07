mod support;
use openagents_wallet::{LightningWallet, resident::RemoteWallet};
use pay_ledger::{
    Ledger,
    compute::{HoldRequest, credential_digest},
    shared::{Intent, Liability, Operation},
};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Barrier},
    time::Duration,
};
use support::*;
fn gateway_intent(f: &Fixture, attempt: &str, amount: u64) -> Intent {
    Intent {
        id: Intent::stable_id(&f.gateway_binding, attempt),
        binding: f.gateway_binding.clone(),
        native_attempt: attempt.into(),
        quote: "sha256:reviewed-native-price".into(),
        execution: attempt.into(),
        terms: "original-native-request-digest".into(),
        maximum_units: amount,
        fee_cap_msat: 0,
        invoice: None,
        liability: Liability::NativeService {
            resource: "openagents.gateway.systemone.v1".into(),
        },
        admitted_at: commercial_spend::now(),
    }
}
#[test]
fn native_grants_original_funding_and_persistent_standalone_fences() {
    let f = Fixture::new(None);
    assert_eq!(f.fund("original", 1)["state"], "paid");
    assert_eq!(f.fund("original", 1)["state"], "paid");
    assert_eq!(f.fake.incoming.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        f.retail
            .call(Operation::Funding {
                purchase: "original".into(),
                amount_sats: 2
            })
            .is_err()
    );
    let mut book = Ledger::open(&f.ledger).unwrap();
    assert_eq!(book.compute_balance("retail").unwrap().credited_msat, 1000);
    assert!(
        book.reserve(&HoldRequest {
            id: "raw".into(),
            account: "retail".into(),
            quote: "q".into(),
            execution: "e".into(),
            terms: "t".into(),
            amount_msat: 1,
            at: commercial_spend::now() as i64
        })
        .is_err()
    );
    let wallet = RemoteWallet::probe(&f.wallet_home).unwrap();
    assert!(wallet.pay("raw", 1, Duration::from_secs(1)).is_err());
    assert!(wallet.receive_exact(1, [1; 32], 60).is_err());
    let mut money = tenancy::money::Ledger::open(&f.native.money).unwrap();
    assert!(
        money
            .apply(tenancy::money::Mutation {
                workspace: f.native.workspace.clone(),
                source: "omitted-config-credit".into(),
                audit: "Synthetic refusal".into(),
                operation: tenancy::money::Operation::Credit {
                    amount: 1000,
                    credit_kind: tenancy::money::CreditKind::TopUp
                }
            })
            .is_err()
    );
    assert_eq!(money.balance(&f.native.workspace).unwrap().available, 0);
    assert!(
        book.resolve_principal(
            "cli:retail",
            &credential_digest("foreign"),
            pay_ledger::compute::Need::Spend
        )
        .is_err()
    );
}
#[test]
fn actual_native_controller_requests_share_last_funds_and_recheck_revocation() {
    let f = Fixture::new(None);
    f.fund("last-funds", 1);
    let gateway = gateway_intent(&f, "native-request#0", 800);
    let barrier = Arc::new(Barrier::new(3));
    let g = f.gateway.clone();
    let gate = barrier.clone();
    let intent = gateway.clone();
    let actor = pay_ledger::shared::GatewayActor {
        credential: f.native.token.clone(),
        door: "decision-a".into(),
    };
    let gw = std::thread::spawn(move || {
        gate.wait();
        g.call(Operation::Reserve {
            intent,
            projection_head: 0,
            actor,
        })
        .is_ok()
    });
    let r = f.retail.clone();
    let gate = barrier.clone();
    let retail = std::thread::spawn(move || {
        gate.wait();
        r.call(Operation::RetailReserve {
            request: HoldRequest {
                id: "compute".into(),
                account: "retail".into(),
                quote: "q".into(),
                execution: "compute-execution".into(),
                terms: "compute-terms".into(),
                amount_msat: 800,
                at: commercial_spend::now() as i64,
            },
        })
        .is_ok()
    });
    barrier.wait();
    assert_ne!(gw.join().unwrap(), retail.join().unwrap());
    let book = Ledger::open_read_only(&f.ledger).unwrap();
    let balance = book.compute_balance("retail").unwrap();
    assert_eq!(
        (
            balance.credited_msat,
            balance.available_msat,
            balance.held_msat
        ),
        (1000, 200, 800)
    );
    let registry = tenancy::Registry::open(&f.native.directory).unwrap();
    let key =
        tenancy::keys::authenticate(&f.native.directory, registry.manifest(), &f.native.token)
            .unwrap();
    tenancy::keys::revoke(&f.native.directory, &key.key_id).unwrap();
    std::fs::set_permissions(
        f.native.directory.join("keys.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(
        f.gateway
            .call(Operation::Reserve {
                intent: gateway_intent(&f, "revoked#0", 1),
                projection_head: 0,
                actor: pay_ledger::shared::GatewayActor {
                    credential: f.native.token.clone(),
                    door: "decision-a".into()
                }
            })
            .is_err()
    );
    assert!(f.plugin.call(Operation::Identity {}).is_err());
    assert_eq!(book.compute_balance("retail").unwrap(), balance);
}

#[test]
fn scoped_native_reader_cannot_borrow_another_key_spend_grant() {
    let f = Fixture::new_scoped(
        None,
        Some(tenancy::keys::Scopes {
            models: None,
            actions: Some(
                ["accounts".into(), "shared-spend".into()]
                    .into_iter()
                    .collect(),
            ),
        }),
    );
    f.fund("qualified-scope-funding", 1);
    assert!(f.plugin.call(Operation::Identity {}).is_ok());
    assert!(f.plugin.call(Operation::Binding {}).is_ok());
    drop(f);
    let mut f = Fixture::new(None);
    f.fund("scope-funding", 1);
    let original = Ledger::open_read_only(&f.ledger)
        .unwrap()
        .compute_balance("retail")
        .unwrap();
    for (actions, read) in [(vec!["accounts"], true), (vec!["inference"], false)] {
        f.stop_controller();
        let registry = tenancy::Registry::open(&f.native.directory).unwrap();
        let issued = tenancy::keys::issue_scoped(
            &f.native.directory,
            registry.manifest(),
            &f.native.tenant,
            None,
            Some(tenancy::keys::Scopes {
                models: None,
                actions: Some(actions.into_iter().map(str::to_owned).collect()),
            }),
        )
        .unwrap();
        write(
            &f.native.directory.join("keys.json"),
            &std::fs::read(f.native.directory.join("keys.json")).unwrap(),
        );
        let accounts = tenancy::Accounts::open(&f.native.directory).unwrap();
        let mut principals = accounts.store().unwrap().accounts[&f.native.account]
            .principals
            .clone();
        principals.push(format!("key:{}", issued.key.id));
        accounts
            .update_principals(&f.native.account, &principals)
            .unwrap();
        let member = accounts
            .authenticate_key(registry.manifest(), &f.native.workspace, &issued.token)
            .unwrap();
        let mut config: commercial_spend::Config =
            serde_json::from_slice(&std::fs::read(&f.config).unwrap()).unwrap();
        for grant in &mut config.grants {
            if let commercial_spend::Native::Tenancy {
                principal,
                member_epoch,
                members_epoch,
                ..
            } = &mut grant.native
            {
                *principal = format!("key:{}", issued.key.id);
                *member_epoch = member.epoch;
                *members_epoch = member.members_epoch;
                write(&grant.native_credential_file, issued.token.as_bytes());
            }
        }
        write(&f.config, &serde_json::to_vec(&config).unwrap());
        if read {
            f.restart_controller();
            assert!(f.plugin.call(Operation::Identity {}).is_ok());
            assert!(f.plugin.call(Operation::Binding {}).is_err());
        } else {
            assert!(commercial_spend::Controller::open(&f.config).is_err());
        }
        assert_eq!(
            Ledger::open_read_only(&f.ledger)
                .unwrap()
                .compute_balance("retail")
                .unwrap(),
            original
        );
        assert_eq!(f.fake.outgoing.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

#[test]
fn lost_resident_receive_reply_recovers_original_invoice_after_controller_restart() {
    use openagents_wallet::custody::{Permit, Terms};
    use serde_json::json;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let mut f = Fixture::new(None);
    let config: commercial_spend::Config =
        serde_json::from_slice(&std::fs::read(&f.config).unwrap()).unwrap();
    let writer = std::fs::read_to_string(&config.writer_file).unwrap();
    let purchase = "lost-original-receive";
    let id = format!(
        "shared-funding:{}",
        commercial_spend::digest(
            &serde_json::to_vec(&(f.retail_binding.native_identity(), purchase)).unwrap(),
        )
    );
    let created = commercial_spend::now();
    let request_hash = commercial_spend::digest(b"original bounded receive intent");
    let terms = json!({"binding":f.retail_binding,"purchase":purchase,"amount_msat":1000,
        "request_hash":request_hash,"created_at":created,"due":created+3600,
        "expiry_secs":3539,"direction":"inbound"});
    let intent = commercial_spend::digest(&serde_json::to_vec(&terms).unwrap());
    {
        let mut ledger = Ledger::open(&f.ledger).unwrap();
        ledger.admit_shared_writer(&writer).unwrap();
        assert!(
            ledger
                .shared_begin_funding(&id, &f.retail_binding, &terms)
                .unwrap()
        );
    }
    let permit = Permit::sign(
        &config.origin,
        &f.retail_binding.custodian_node,
        &intent,
        Terms::Receive {
            amount_msat: 1000,
            request_hash,
            expiry_secs: 3539,
        },
        &writer,
    )
    .unwrap();
    // Close the actual resident connection before reading its issuance response.
    // Its durable outcome is the only source from which recovery may proceed.
    let mut stream = UnixStream::connect(f.wallet_home.join("control.sock")).unwrap();
    writeln!(
        stream,
        "{}",
        serde_json::to_string(&openagents_wallet::resident::Request::CustodialReceive {
            permit,
            writer: writer.clone(),
        })
        .unwrap()
    )
    .unwrap();
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    drop(stream);
    let wallet = RemoteWallet::probe(&f.wallet_home).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let original = loop {
        if let Some(value) = wallet
            .custodial_result(&f.retail_binding.custodian_node, &intent, &writer)
            .unwrap()
        {
            break value["result"].clone();
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(f.fake.incoming.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        Ledger::open_read_only(&f.ledger)
            .unwrap()
            .shared_funding_record(&id)
            .unwrap()
            .unwrap()
            .2
            .is_none()
    );
    f.stop_controller();
    f.restart_controller();
    let recovered = f
        .retail
        .call(Operation::FundingStatus {
            purchase: purchase.into(),
        })
        .unwrap();
    assert_eq!(recovered["state"], "paid");
    assert_eq!(recovered["invoice"], original);
    assert_eq!(f.fund(purchase, 1)["invoice"], original);
    assert_eq!(f.fake.incoming.load(std::sync::atomic::Ordering::SeqCst), 1);
    let balance = Ledger::open_read_only(&f.ledger)
        .unwrap()
        .compute_balance("retail")
        .unwrap();
    assert_eq!(
        (
            balance.credited_msat,
            balance.available_msat,
            balance.held_msat
        ),
        (1000, 1000, 0)
    );
}

#[test]
fn missing_custody_manifest_and_backup_omission_never_restore_raw_wallet_effects() {
    let f = Fixture::new(None);
    f.fund("custody-marker", 1);
    let wallet = RemoteWallet::probe(&f.wallet_home).unwrap();
    let manifest_path = f.wallet_home.join(openagents_wallet::custody::FILE);
    let required_path = f
        .wallet_home
        .join(openagents_wallet::custody::REQUIRED_FILE);
    let manifest = std::fs::read(&manifest_path).unwrap();
    let required = std::fs::read(&required_path).unwrap();
    std::fs::remove_file(&manifest_path).unwrap();
    assert!(openagents_wallet::custody::read(&f.wallet_home).is_err());
    assert!(wallet.receive_exact(1000, [6; 32], 60).is_err());
    std::fs::remove_file(&required_path).unwrap();
    assert!(openagents_wallet::custody::read(&f.wallet_home).is_err());
    assert!(wallet.receive_exact(1000, [6; 32], 60).is_err());
    assert_eq!(f.fake.incoming.load(std::sync::atomic::Ordering::SeqCst), 1);
    write(&manifest_path, &manifest);
    write(&required_path, &required);
    write(&f.wallet_home.join("seed"), b"isolated backup fixture");
    let backup = f.root.path().join("backup");
    let saved = openagents_wallet::backup::write(&f.wallet_home, &backup).unwrap();
    for name in [
        openagents_wallet::custody::FILE,
        openagents_wallet::custody::REQUIRED_FILE,
        "shared-handoffs.sqlite",
    ] {
        assert!(saved.files.iter().any(|e| e.path == name));
    }
    let restored = f.root.path().join("restored");
    openagents_wallet::backup::restore(&backup, &restored).unwrap();
    assert!(openagents_wallet::custody::refuse_raw(&restored).is_err());
    let mut omission = serde_json::to_value(&saved).unwrap();
    omission["files"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["path"] != openagents_wallet::custody::REQUIRED_FILE);
    write(
        &backup.join("backup.json"),
        &serde_json::to_vec(&omission).unwrap(),
    );
    std::fs::remove_file(backup.join(openagents_wallet::custody::REQUIRED_FILE)).unwrap();
    assert!(openagents_wallet::backup::verify(&backup).is_err());
}

#[test]
fn reviewed_native_source_reversal_preserves_unknown_liability_and_blocks_fresh_funds() {
    use serde_json::json;
    let mut f = Fixture::new(None);
    let funding = f.fund("reviewed-source", 1);
    let held = f
        .retail
        .call(Operation::RetailReserve {
            request: HoldRequest {
                id: "retained-unknown".into(),
                account: "retail".into(),
                quote: "original-quote".into(),
                execution: "original-execution".into(),
                terms: "original-terms".into(),
                amount_msat: 800,
                at: commercial_spend::now() as i64,
            },
        })
        .unwrap();
    let id = held["request"]["id"].as_str().unwrap().to_string();
    f.retail
        .call(Operation::RetailHandoff { id: id.clone() })
        .unwrap();
    f.retail.call(Operation::RetailUnknown { id: id }).unwrap();
    f.stop_controller();
    let mut config: commercial_spend::Config =
        serde_json::from_slice(&std::fs::read(&f.config).unwrap()).unwrap();
    config
        .funding_reversals
        .push(commercial_spend::FundingReversalReview {
            id: "accepted-source-loss".into(),
            funding: funding["canonical_source"].as_str().unwrap().into(),
            funding_digest: commercial_spend::digest(
                &serde_json::to_vec(&funding["original_terms"]).unwrap(),
            ),
            amount_msat: 600,
            evidence: "original-confirmed-source-loss".into(),
            reviewed_at: commercial_spend::now(),
            valid_until: commercial_spend::now() + 3600,
        });
    write(&f.config, &serde_json::to_vec(&config).unwrap());
    f.restart_controller();
    assert!(
        f.gateway
            .call(Operation::ReverseFunding {
                review: "accepted-source-loss".into()
            })
            .is_err()
    );
    let receipt = f
        .retail
        .call(Operation::ReverseFunding {
            review: "accepted-source-loss".into(),
        })
        .unwrap();
    assert_eq!(receipt["recovered_msat"], json!(200));
    assert_eq!(receipt["loss_msat"], json!(400));
    assert_eq!(
        f.retail
            .call(Operation::ReverseFunding {
                review: "accepted-source-loss".into()
            })
            .unwrap(),
        receipt
    );
    f.fund("fresh-cannot-clear-loss", 1);
    let balance = Ledger::open_read_only(&f.ledger)
        .unwrap()
        .compute_balance("retail")
        .unwrap();
    assert_eq!(
        (
            balance.available_msat,
            balance.restricted_msat,
            balance.held_msat,
            balance.protected_loss_msat
        ),
        (0, 1000, 800, 400)
    );
}
