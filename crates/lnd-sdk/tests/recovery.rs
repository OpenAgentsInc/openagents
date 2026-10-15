#![cfg(all(feature = "supervisor", unix))]
use lnd_sdk::{Client, Error, service::LightningRail, supervisor::Regtest};
use std::{path::PathBuf, time::Duration};

#[test]
#[ignore = "fresh seeded Docker wallets; scripts/lnd-sdk-regtest.sh"]
fn seed_and_scb_recover_peer_force_close() {
    let root =
        PathBuf::from(std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root"))
            .join("recovery");
    let (nodes, seed) = Regtest::recoverable(&root).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (config, credential) = nodes.config("alice").unwrap();
    let alice = runtime
        .block_on(Client::connect(&config, credential))
        .unwrap();
    let identity = alice.node_id();
    for tower in ["bob", "tower"] {
        let info = nodes.lncli(tower, &["tower", "info"]).unwrap();
        let key = hex::decode(info["pubkey"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        runtime
            .block_on(alice.add_watchtower(key, format!("{tower}:9911")))
            .unwrap();
    }
    let address = nodes.lncli("alice", &["newaddress", "p2wkh"]).unwrap()["address"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes.bitcoin(&["sendtoaddress", &address, "1"]).unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            let b = nodes.lncli("alice", &["walletbalance"])?;
            if b["confirmed_balance"].as_str().unwrap_or("0") != "0" {
                Ok(())
            } else {
                Err(Error::NotSynced)
            }
        })
        .unwrap();
    let bob_key = nodes.lncli("bob", &["getinfo"]).unwrap()["identity_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes
        .lncli("alice", &["connect", &format!("{bob_key}@bob:9735")])
        .unwrap();
    // Begin delivery before opening the channel. The event must be captured
    // within a minute, including the RPC's delayed subscription headers.
    struct Sink(tokio::sync::mpsc::UnboundedSender<usize>);
    #[async_trait::async_trait]
    impl lnd_sdk::backups::BackupSink for Sink {
        async fn save(&mut self, b: lnd_sdk::backups::Backup) -> lnd_sdk::Result<()> {
            self.0.send(b.channels).map_err(|_| Error::Checkpoint)
        }
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let stream_client = alice.clone();
    let consumer = runtime.spawn(async move { stream_client.channel_backups(&mut Sink(tx)).await });
    runtime.block_on(async {
        assert_eq!(rx.recv().await, Some(0));
    });
    nodes
        .lncli(
            "alice",
            &[
                "openchannel",
                "--node_key",
                &bob_key,
                "--local_amt",
                "500000",
                "--push_amt",
                "100000",
                "--private",
            ],
        )
        .unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            let info = nodes.lncli("alice", &["getinfo"])?;
            if info["num_active_channels"].as_u64().unwrap_or(0) > 0 {
                Ok(())
            } else {
                Err(Error::NotSynced)
            }
        })
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            while rx.recv().await != Some(1) {}
        })
        .await
        .unwrap();
    });
    consumer.abort();
    let (bob_config, bob_credential) = nodes.config("bob").unwrap();
    let bob = runtime
        .block_on(Client::connect(&bob_config, bob_credential))
        .unwrap();
    runtime.block_on(async {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let hints = loop {
            match bob.private_route_hints().await {
                Ok(h) => break h,
                Err(Error::NotSynced) if std::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(200)).await
                }
                Err(e) => panic!("channel policy: {e}"),
            }
        };
        let invoice = bob
            .create_invoice_with_hints(21000, [6; 32], 600, hints)
            .await
            .unwrap();
        alice
            .pay(&invoice.bolt11, 1000, Duration::from_secs(30))
            .await
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let towers = alice.watchtowers().await.unwrap();
            assert_eq!(towers.len(), 2);
            if towers
                .iter()
                .flat_map(|t| &t.session_info)
                .any(|i| i.num_sessions > 0 && i.sessions.iter().any(|s| s.num_backups > 0))
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "watchtower session did not receive a backup"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    let before = runtime.block_on(alice.balance()).unwrap();
    let backup = runtime.block_on(alice.channel_backup()).unwrap();
    assert_eq!(backup.channels, 1);
    drop(alice);
    // No channel.db, wallet.db, or other database survives this operation.
    nodes.restore(&seed, backup.encrypted_bytes()).unwrap();
    let (config, credential) = nodes.config("alice").unwrap();
    let restored = runtime
        .block_on(Client::connect(&config, credential))
        .unwrap();
    assert_eq!(identity, restored.node_id());
    nodes
        .wait(|| {
            let pending = nodes.lncli("bob", &["pendingchannels"])?;
            if pending["waiting_close_channels"]
                .as_array()
                .is_some_and(|c| !c.is_empty())
                || pending["pending_force_closing_channels"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty())
            {
                Ok(())
            } else {
                Err(Error::NotSynced)
            }
        })
        .unwrap();
    let minimum = before.onchain_total_sats + before.lightning_total_sats - 20000;
    for attempt in 0..50 {
        nodes.mine(10).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let recovered = runtime.block_on(restored.balance()).unwrap();
        if recovered.onchain_spendable_sats >= minimum {
            let closed = nodes.lncli("bob", &["closedchannels"]).unwrap();
            assert!(
                closed["channels"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["close_type"] == "LOCAL_FORCE_CLOSE")
            );
            println!(
                "Seed plus SCB recovered {} sats; two towers registered and an acknowledged session verified.",
                recovered.onchain_spendable_sats
            );
            return;
        }
        assert!(attempt < 49, "peer force-close funds were not recovered");
    }
}
