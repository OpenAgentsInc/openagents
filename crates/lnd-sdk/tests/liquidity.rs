#![cfg(all(feature = "supervisor", unix))]
use lnd_sdk::{Client, Error, service::LightningRail, supervisor::Regtest};
use std::{path::PathBuf, time::Duration};
#[test]
#[ignore = "isolated Docker regtest; no real-money orders"]
fn anchor_channel_and_realistic_inbound_payment() {
    let root =
        PathBuf::from(std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root"))
            .join("liquidity");
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
        let invoice = bob
            .create_invoice_with_hints(100_000_000, [55; 32], 600, hints)
            .await
            .unwrap();
        let proof = alice
            .pay(&invoice.bolt11, 10_000, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(proof.amount_msat, 100_000_000);
        assert_eq!(
            bob.received_msat(openagents_wallet::parse_hash32(&proof.payment_hash).unwrap())
                .await
                .unwrap(),
            Some(100_000_000)
        );
        assert!(bob.health().await.unwrap().inbound_msat > 0);
    });
}
