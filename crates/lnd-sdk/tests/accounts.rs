#![cfg(all(feature = "supervisor", unix))]
use lnd_sdk::{Client, Error, service::LightningRail, supervisor::Regtest};
use std::{path::PathBuf, time::Duration};
#[test]
#[ignore = "isolated Docker regtest; no real-money orders"]
fn litd_account_caps_payouts_and_fails_closed_when_middleware_stops() {
    let root =
        PathBuf::from(std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root"))
            .join("accounts");
    let nodes = Regtest::start(&root).unwrap();
    let address = nodes.lncli("alice", &["newaddress", "p2wkh"]).unwrap()["address"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes.bitcoin(&["sendtoaddress", &address, "0.02"]).unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            if nodes.lncli("alice", &["walletbalance"])?["confirmed_balance"]
                .as_str()
                .unwrap_or("0")
                != "0"
            {
                Ok(())
            } else {
                Err(Error::NotSynced)
            }
        })
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (config, mac) = nodes.config("alice").unwrap();
        let alice = Client::connect(&config, mac).await.unwrap();
        let (config, mac) = nodes.config("bob").unwrap();
        let bob = Client::connect(&config, mac).await.unwrap();
        let point = alice
            .open_peer_channel_at_rate(&bob.node_id(), "bob:9735", 1_000_000, false, 2)
            .await
            .unwrap();
        nodes.mine(6).unwrap();
        nodes
            .wait(|| {
                if nodes.lncli("alice", &["listchannels"])?["channels"][0]["active"] == true {
                    Ok(())
                } else {
                    Err(Error::NotSynced)
                }
            })
            .unwrap();
        assert_eq!(
            nodes.lncli("alice", &["listchannels"]).unwrap()["channels"][0]["commitment_type"],
            "ANCHORS"
        );
        assert!(alice.channel_opening_fee(&point).await.unwrap() > 0);
        assert!(alice.balance().await.unwrap().onchain_spendable_sats >= 100_000);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let hints = loop {
            match bob.private_route_hints().await {
                Ok(h) => break h,
                Err(Error::NotSynced) if std::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(200)).await
                }
                Err(e) => panic!("{e}"),
            }
        };

        let (lit_config, admin) = nodes.start_accounts().unwrap();
        let accounts = lnd_sdk::payout::Accounts::connect(&lit_config, admin)
            .await
            .unwrap();
        let expiration = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 3600;
        let access = accounts
            .create("payout-worker-test".into(), 50_000, expiration)
            .await
            .unwrap();
        let (lnd_config, _) = nodes.config("alice").unwrap();
        let limited = Client::connect(&lnd_config, access.macaroon).await.unwrap();
        assert_eq!(
            limited.balance().await.unwrap().lightning_total_sats,
            50_000
        );
        for n in 0..2 {
            let invoice = bob
                .create_invoice_with_hints(20_000_000, [60 + n; 32], 600, hints.clone())
                .await
                .unwrap();
            limited
                .pay(&invoice.bolt11, 10_000, Duration::from_secs(15))
                .await
                .unwrap();
        }
        assert_eq!(
            limited.balance().await.unwrap().lightning_total_sats,
            10_000
        );
        let over = bob
            .create_invoice_with_hints(20_000_000, [62; 32], 600, hints.clone())
            .await
            .unwrap();
        assert!(
            limited
                .pay(&over.bolt11, 10_000, Duration::from_secs(5))
                .await
                .is_err()
        );
        let worker = lnd_sdk::service::Rails {
            wallet: lnd_sdk::service::Wallet::new(std::sync::Arc::new(limited.clone())).unwrap(),
            delegate: NoDelegate,
            payment_timeout: Duration::from_secs(5),
            funding_journal: root.join("funding.json"),
        };
        use pay_ledger::payout::Rails as _;
        let invoice = pay_ledger::payout::Invoice {
            bolt11: over.bolt11.clone(),
            payment_hash: over.payment_hash.clone(),
            amount_msat: 20_000_000,
        };
        assert!(matches!(
            worker.pay_lightning(&invoice, 10_000),
            pay_ledger::payout::Outcome::Failed(_)
        ));
        assert_eq!(
            bob.received_msat(openagents_wallet::parse_hash32(&over.payment_hash).unwrap())
                .await
                .unwrap(),
            None
        );
        nodes.command(&["stop", "litd"]).unwrap();
        let offline = bob
            .create_invoice_with_hints(1_000, [63; 32], 600, hints)
            .await
            .unwrap();
        assert!(
            limited
                .pay(&offline.bolt11, 10_000, Duration::from_secs(5))
                .await
                .is_err()
        );
        assert_eq!(
            bob.received_msat(openagents_wallet::parse_hash32(&offline.payment_hash).unwrap())
                .await
                .unwrap(),
            None
        );
    });
}

struct NoDelegate;
impl pay_ledger::payout::Rails for NoDelegate {
    fn lightning_invoice(&self, _: &str, _: i64) -> Result<pay_ledger::payout::Invoice, String> {
        panic!("unexpected address lookup")
    }
    fn pay_lightning(
        &self,
        _: &pay_ledger::payout::Invoice,
        _: i64,
    ) -> pay_ledger::payout::Outcome {
        panic!("payout used the old rail")
    }
    fn lookup_lightning(&self, _: &str) -> Result<pay_ledger::payout::Lookup, String> {
        panic!("lookup used the old rail")
    }
    fn fund_spark(&self, _: u64) -> Result<(), String> {
        panic!("refill used the old rail")
    }
    fn pay_spark(&self, _: &str, _: u64, _: &str) -> pay_ledger::payout::Outcome {
        panic!("unexpected Spark transfer")
    }
    fn lookup_spark(&self, _: &str) -> Result<pay_ledger::payout::Lookup, String> {
        panic!("unexpected Spark lookup")
    }
}
