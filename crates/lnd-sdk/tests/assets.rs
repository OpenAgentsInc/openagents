#![cfg(all(feature = "supervisor", unix))]
use lnd_sdk::{
    Client, Error,
    assets::{Approval, Assets},
    liquidity::Journal,
    service::LightningRail,
    supervisor::Regtest,
};
use std::{path::PathBuf, time::Duration};
#[test]
#[ignore = "isolated Docker regtest mint, asset channel and RFQ; no real assets"]
fn quoted_asset_receive_and_payout_credit_once() {
    let root =
        PathBuf::from(std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root"))
            .join("assets");
    let nodes = Regtest::asset_network(&root).unwrap();
    nodes.start_asset_peer().unwrap();
    for node in ["alice", "bob"] {
        let address = nodes.lncli(node, &["newaddress", "p2wkh"]).unwrap()["address"]
            .as_str()
            .unwrap()
            .to_owned();
        nodes.bitcoin(&["sendtoaddress", &address, "0.1"]).unwrap();
    }
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            for n in ["alice", "bob"] {
                if nodes.lncli(n, &["walletbalance"])?["confirmed_balance"]
                    .as_str()
                    .unwrap_or("0")
                    == "0"
                {
                    return Err(Error::NotSynced);
                }
            }
            Ok(())
        })
        .unwrap();
    nodes
        .tapcli(
            "tapd",
            &[
                "assets",
                "mint",
                "--type=normal",
                "--name=regtest-test-usd",
                "--supply=1000000",
            ],
        )
        .unwrap();
    nodes
        .tapcli("tapd", &["assets", "mint", "finalize"])
        .unwrap();
    nodes.mine(6).unwrap();
    let mut asset_id = String::new();
    nodes
        .wait(|| {
            let list = nodes.tapcli("tapd", &["assets", "list"])?;
            asset_id = list["assets"][0]["asset_genesis"]["asset_id"]
                .as_str()
                .unwrap_or("")
                .into();
            if asset_id.is_empty() {
                Err(Error::NotSynced)
            } else {
                Ok(())
            }
        })
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(exercise(&nodes, &root, asset_id));
}

async fn exercise(nodes: &Regtest, root: &std::path::Path, asset_id: String) {
    let (c, m) = nodes.config("alice").unwrap();
    let alice = Client::connect(&c, m).await.unwrap();
    let (c, m) = nodes.config("bob").unwrap();
    let bob = Client::connect(&c, m).await.unwrap();

    nodes
        .lncli(
            "alice",
            &["connect", &format!("{}@bob:9735", bob.node_id())],
        )
        .unwrap();

    // Sync the fresh regtest mint into Bob's universe before channel negotiation.
    nodes
        .tapcli(
            "tapd-bob",
            &["universe", "sync", "--universe_host=alice:10009"],
        )
        .unwrap();

    let (c, m) = nodes.asset_config("tapd").unwrap();
    let assets_alice = Assets::connect(&c, m, alice.clone()).await.unwrap();
    let (c, m) = nodes.asset_config("tapd-bob").unwrap();
    let assets_bob = Assets::connect(&c, m, bob.clone()).await.unwrap();

    assets_alice
        .fund_regtest_channel(
            hex::decode(&asset_id).unwrap().try_into().unwrap(),
            hex::decode(bob.node_id()).unwrap().try_into().unwrap(),
            500000,
        )
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
    nodes.mine(1).unwrap();
    nodes
        .wait(|| {
            for n in ["alice", "bob"] {
                let channels = nodes.lncli(n, &["listchannels"])?;
                let id = channels["channels"][0]["scid"]
                    .as_str()
                    .ok_or(Error::NotSynced)?;
                nodes.lncli(n, &["getchaninfo", id])?;
            }
            Ok(())
        })
        .unwrap();

    let asset: [u8; 32] = hex::decode(&asset_id).unwrap().try_into().unwrap();
    let edge = hex::decode(alice.node_id()).unwrap().try_into().unwrap();

    let approval = Approval {
        mainnet_receive_approved: false,
        custody_review_approved: false,
        mainnet_payout_approved: false,
        asset_ids: vec![asset_id.clone()],
        maximum_units: 20000,
        maximum_credit_msat: 20_000_000,
        maximum_fee_msat: 10000,
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut journal = Journal::open(&root.join("asset-actions.json")).unwrap();

    let q = assets_bob
        .receive(
            &mut journal,
            "test-receive",
            "test-assets",
            asset,
            edge,
            10000,
            &approval,
            now,
        )
        .await
        .unwrap();
    assert_eq!(q.credit_msat, 10_000_000);

    let ledger_path = root.join("asset-ledger.db");
    let mut ledger = pay_ledger::Ledger::open(&ledger_path).unwrap();
    ledger.create_compute_account("test-assets", now).unwrap();
    ledger.open_asset_quote(&q).unwrap();
    assert!(assets_bob.settled(&q).await.is_err());

    let payer_edge = hex::decode(bob.node_id()).unwrap().try_into().unwrap();

    let paid = assets_alice
        .pay(&mut journal, &q, payer_edge, &approval)
        .await
        .unwrap();
    assert_eq!(paid["status"], 2);

    let observed = bob
        .lookup_invoice(hex::decode(&q.payment_hash).unwrap().try_into().unwrap())
        .await
        .unwrap();
    let observed = serde_json::json!({"invoice_rfq_id":q.rfq_id,"htlcs":observed.htlcs.iter().map(|h|serde_json::json!({"channel":h.chan_id,"amount_msat":h.amt_msat,"data":serde_json::from_slice::<serde_json::Value>(&h.custom_channel_data).unwrap()})).collect::<Vec<_>>()});
    std::fs::write(
        root.join("observed-asset-htlcs.json"),
        serde_json::to_vec_pretty(&observed).unwrap(),
    )
    .unwrap();
    let receipt = assets_bob.settled(&q).await.unwrap();
    assert!(receipt.units >= 10000);
    ledger
        .credit_asset(&q.node, &q.payment_hash, &receipt)
        .unwrap();
    drop(ledger);

    let mut ledger = pay_ledger::Ledger::open(&ledger_path).unwrap();
    ledger
        .credit_asset(&q.node, &q.payment_hash, &receipt)
        .unwrap();
    assert_eq!(ledger.credited("test-assets").unwrap(), 10_000_000);
    assert_eq!(ledger.asset_receipts().unwrap().len(), 1);

    let recovered = assets_alice
        .pay(&mut journal, &q, payer_edge, &approval)
        .await
        .unwrap();
    assert_eq!(recovered["payment_hash"], q.payment_hash);

    let channels = alice.channel_details().await.unwrap();
    assert!(!channels[0].custom_channel_data.is_empty());

    std::fs::write(root.join("asset-evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"quote":q,"receipt":receipt,"payment":paid,"credited_msat":10000000})).unwrap()).unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;
}
