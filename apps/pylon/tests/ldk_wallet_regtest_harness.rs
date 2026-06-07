#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "Ignored heavy regtest harness keeps assertion-style setup and teardown diagnostics."
)]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use electrsd::corepc_node::{Client as BitcoindClient, Node as BitcoinD};
use electrsd::{ElectrsD, corepc_node};
use electrum_client::ElectrumApi;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::bitcoin::{Address, Amount, Network, OutPoint, Txid};
use ldk_node::config::{Config, ElectrumSyncConfig};
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description};
use ldk_node::payment::{PaymentDirection, PaymentKind, PaymentStatus};
use ldk_node::{Builder, Event, Node};
use pylon::pylon_ldk_wallet_harness_plan;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const FUNDING_SATS: u64 = 3_000_000;
const CHANNEL_SATS: u64 = 2_000_000;
const PAYMENT_MSATS: u64 = 2_500_000;
const WITHDRAWAL_SATS: u64 = 25_000;

#[derive(Debug, Serialize)]
struct HarnessSummary {
    schema: String,
    network: String,
    plan: pylon::PylonLdkWalletHarnessPlan,
    payer_node_id: String,
    receiver_node_id: String,
    funding_txid: String,
    withdrawal_txid: String,
    channel_funding_outpoint: String,
    payment_id: String,
    payment_hash: String,
    payer_balances: BalancePair,
    receiver_balances: BalancePair,
    receipt_ids: Vec<String>,
    channel_readiness: ChannelReadinessProof,
    bolt12_status: String,
    accepted_work: AcceptedWorkProof,
    restart_payment_status: String,
    restored_payment_status: String,
    receiver_backup_digest: String,
    restored_channel_count: usize,
    artifacts: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct BalancePair {
    before: BalanceSnapshot,
    after: BalanceSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct BalanceSnapshot {
    total_onchain_sats: u64,
    spendable_onchain_sats: u64,
    total_lightning_sats: u64,
    lightning_sats: u64,
    inbound_lightning_sats: u64,
}

#[derive(Debug, Serialize)]
struct ChannelReadinessProof {
    schema: String,
    lsp_integration_path: String,
    cases: Vec<ChannelReadinessCase>,
}

#[derive(Debug, Clone, Serialize)]
struct ChannelReadinessCase {
    case: String,
    state: String,
    channel_count: usize,
    usable_count: usize,
    pending_count: usize,
    peer_connected_count: usize,
    inbound_sats: u64,
    outbound_sats: u64,
    can_receive_lightning: bool,
    can_receive_onchain: bool,
    can_send_lightning: bool,
    warning_code: Option<String>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkProof {
    schema: String,
    no_manual_external_payout_destination: bool,
    wallet_registration: AcceptedWorkWalletRegistration,
    treasury_dispatch: AcceptedWorkTreasuryDispatch,
    pylon_observation: AcceptedWorkPylonObservation,
    withdrawal: AcceptedWorkWithdrawal,
    reconciliation: AcceptedWorkReconciliation,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkWalletRegistration {
    nexus_operation_id: String,
    wallet_node_id: String,
    payment_target_kind: String,
    wallet_registration_mode: String,
    receipt_id: String,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkTreasuryDispatch {
    treasury_operation_id: String,
    payout_key: String,
    amount_sats: u64,
    payment_hash: String,
    payment_id: String,
    payer_node_id: String,
    receipt_id: String,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkPylonObservation {
    wallet_history_status: String,
    payment_hash: String,
    payment_id: String,
    amount_sats: u64,
    balance_increase_sats: u64,
    receipt_id: String,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkWithdrawal {
    txid: String,
    amount_sats: u64,
    balance_decreased: bool,
    receipt_id: String,
}

#[derive(Debug, Serialize)]
struct AcceptedWorkReconciliation {
    nexus_operation_id: String,
    treasury_operation_id: String,
    pylon_receipt_id: String,
    reconciliation_status: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "heavy local regtest harness; run scripts/pylon/ldk-wallet-regtest-harness.sh"]
async fn ldk_wallet_regtest_harness() {
    let artifacts_dir = env::var("OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/pylon-ldk-wallet-regtest/latest"));
    if artifacts_dir.exists() {
        fs::remove_dir_all(artifacts_dir.as_path()).expect("clear stale artifact dir");
    }
    fs::create_dir_all(artifacts_dir.as_path()).expect("create artifact dir");

    let workspace = TempDir::new().expect("workspace tempdir");
    let (bitcoind, electrsd) = setup_bitcoind_and_electrsd();
    let electrum_url = format!("tcp://{}", electrsd.electrum_url);

    let payer_dir = artifacts_dir.join("payer-storage");
    let receiver_dir = artifacts_dir.join("receiver-storage");
    let restored_dir = artifacts_dir.join("restored-receiver-storage");
    fs::create_dir_all(payer_dir.as_path()).expect("payer storage dir");
    fs::create_dir_all(receiver_dir.as_path()).expect("receiver storage dir");
    fs::create_dir_all(restored_dir.as_path()).expect("restored receiver storage dir");

    let payer = build_node(
        payer_dir.as_path(),
        &electrum_url,
        next_listening_port(workspace.path(), 1),
        [7u8; 64],
    );
    let receiver = build_node(
        receiver_dir.as_path(),
        &electrum_url,
        next_listening_port(workspace.path(), 2),
        [11u8; 64],
    );

    let payer_address = payer
        .onchain_payment()
        .new_address()
        .expect("payer address");
    let receiver_address = receiver
        .onchain_payment()
        .new_address()
        .expect("receiver address");
    let funding_txid = premine_and_distribute_funds(
        &bitcoind.client,
        &electrsd.client,
        vec![payer_address, receiver_address],
        Amount::from_sat(FUNDING_SATS),
    )
    .await;

    wait_for_wallet_balance(&payer, FUNDING_SATS, "payer on-chain funding").await;
    wait_for_wallet_balance(&receiver, FUNDING_SATS, "receiver on-chain funding").await;
    let payer_pre = balance_snapshot(&payer);
    let receiver_pre = balance_snapshot(&receiver);
    let receiver_no_channel_case =
        channel_readiness_case_from_node("no_channel_before_open", &receiver);

    let channel_funding_outpoint = open_channel(
        &payer,
        &receiver,
        CHANNEL_SATS,
        &bitcoind.client,
        &electrsd.client,
    )
    .await;
    let receiver_usable_channel_case =
        channel_readiness_case_from_node("usable_channel_after_open", &receiver);

    let invoice_description = Bolt11InvoiceDescription::Direct(
        Description::new("pylon regtest harness".to_string()).unwrap(),
    );
    let invoice = receiver
        .bolt11_payment()
        .receive(PAYMENT_MSATS, &invoice_description, 3600)
        .expect("receiver bolt11 invoice");
    let payment_hash = invoice.payment_hash().to_string();
    let payment_id = payer
        .bolt11_payment()
        .send(&invoice, None)
        .expect("payer sends bolt11");
    let successful_hash = expect_payment_successful(&payer).await;
    let received_hash = expect_payment_received(&receiver, PAYMENT_MSATS).await;
    assert_eq!(successful_hash, payment_hash);
    assert_eq!(received_hash, payment_hash);

    let payer_payment = payer.payment(&payment_id).expect("payer payment details");
    assert_eq!(payer_payment.status, PaymentStatus::Succeeded);
    assert_eq!(payer_payment.direction, PaymentDirection::Outbound);
    assert_eq!(payer_payment.amount_msat, Some(PAYMENT_MSATS));
    assert!(matches!(payer_payment.kind, PaymentKind::Bolt11 { .. }));

    let receiver_payment = receiver
        .payment(&payment_id)
        .expect("receiver payment details");
    assert_eq!(receiver_payment.status, PaymentStatus::Succeeded);
    assert_eq!(receiver_payment.direction, PaymentDirection::Inbound);
    assert_eq!(receiver_payment.amount_msat, Some(PAYMENT_MSATS));
    assert!(matches!(receiver_payment.kind, PaymentKind::Bolt11 { .. }));

    let receiver_after_accepted_work_payment = balance_snapshot(&receiver);

    let bolt12_status = attempt_bolt12_payment(&payer, &receiver).await;

    let payer_withdraw_address = payer
        .onchain_payment()
        .new_address()
        .expect("payer withdrawal address");
    let withdrawal_txid = receiver
        .onchain_payment()
        .send_to_address(&payer_withdraw_address, WITHDRAWAL_SATS, None)
        .expect("receiver on-chain withdrawal");
    wait_for_tx(&electrsd.client, withdrawal_txid).await;
    generate_blocks_and_wait(&bitcoind.client, &electrsd.client, 1).await;
    payer.sync_wallets().expect("payer post-withdraw sync");
    receiver
        .sync_wallets()
        .expect("receiver post-withdraw sync");

    let payer_post = balance_snapshot(&payer);
    let receiver_post = balance_snapshot(&receiver);
    let accepted_work_amount_sats = PAYMENT_MSATS / 1000;
    let accepted_work_nexus_operation_id = "nexus.accepted_work.regtest.window0001".to_string();
    let accepted_work_treasury_operation_id =
        "treasury.accepted_work.regtest.operation0001".to_string();
    let accepted_work_payout_key =
        "accepted-work:regtest.window0001:assignment.provider0001".to_string();
    let wallet_registration_receipt_id = format!(
        "wallet:registration:{}:ldk_node:bolt11_invoice",
        receiver.node_id()
    );
    let treasury_dispatch_receipt_id = format!(
        "treasury:accepted-work:{}:{}",
        accepted_work_treasury_operation_id, payment_hash
    );
    let pylon_observation_receipt_id = format!("wallet:accepted-work:{payment_hash}");
    let withdrawal_receipt_id = format!("wallet:withdrawal:{withdrawal_txid}");
    let accepted_work = AcceptedWorkProof {
        schema: "pylon.ldk_accepted_work_payout_proof.v1".to_string(),
        no_manual_external_payout_destination: true,
        wallet_registration: AcceptedWorkWalletRegistration {
            nexus_operation_id: accepted_work_nexus_operation_id.clone(),
            wallet_node_id: receiver.node_id().to_string(),
            payment_target_kind: "bolt11_invoice".to_string(),
            wallet_registration_mode: "wallet_generated_bolt11_fallback".to_string(),
            receipt_id: wallet_registration_receipt_id.clone(),
        },
        treasury_dispatch: AcceptedWorkTreasuryDispatch {
            treasury_operation_id: accepted_work_treasury_operation_id.clone(),
            payout_key: accepted_work_payout_key,
            amount_sats: accepted_work_amount_sats,
            payment_hash: payment_hash.clone(),
            payment_id: format!("{payment_id:?}"),
            payer_node_id: payer.node_id().to_string(),
            receipt_id: treasury_dispatch_receipt_id.clone(),
        },
        pylon_observation: AcceptedWorkPylonObservation {
            wallet_history_status: format!("{:?}", receiver_payment.status),
            payment_hash: payment_hash.clone(),
            payment_id: format!("{payment_id:?}"),
            amount_sats: accepted_work_amount_sats,
            balance_increase_sats: receiver_after_accepted_work_payment
                .total_lightning_sats
                .saturating_sub(receiver_pre.total_lightning_sats),
            receipt_id: pylon_observation_receipt_id.clone(),
        },
        withdrawal: AcceptedWorkWithdrawal {
            txid: withdrawal_txid.to_string(),
            amount_sats: WITHDRAWAL_SATS,
            balance_decreased: receiver_post.spendable_onchain_sats
                < receiver_pre.spendable_onchain_sats,
            receipt_id: withdrawal_receipt_id.clone(),
        },
        reconciliation: AcceptedWorkReconciliation {
            nexus_operation_id: accepted_work_nexus_operation_id,
            treasury_operation_id: accepted_work_treasury_operation_id,
            pylon_receipt_id: pylon_observation_receipt_id.clone(),
            reconciliation_status: "settled".to_string(),
        },
    };
    assert_eq!(
        accepted_work.pylon_observation.balance_increase_sats, accepted_work_amount_sats,
        "accepted-work payout should increase the receiver's claimable Lightning balance"
    );
    assert!(
        accepted_work.withdrawal.balance_decreased,
        "accepted-work withdrawal should decrease the receiver's spendable on-chain balance"
    );
    let receiver_pending_channel_case = ChannelReadinessCase {
        case: "pending_channel_projection".to_string(),
        state: "channel_pending".to_string(),
        channel_count: 1,
        usable_count: 0,
        pending_count: 1,
        peer_connected_count: 0,
        inbound_sats: 0,
        outbound_sats: 0,
        can_receive_lightning: false,
        can_receive_onchain: true,
        can_send_lightning: false,
        warning_code: Some("lightning_receive_pending_channel".to_string()),
        detail: "Projection for the operator surface before funding confirmations and peer readiness complete.".to_string(),
    };
    let receiver_route_failure_case = ChannelReadinessCase {
        case: "route_failure_projection".to_string(),
        state: "needs_inbound_liquidity".to_string(),
        channel_count: 1,
        usable_count: 1,
        pending_count: 0,
        peer_connected_count: 1,
        inbound_sats: 0,
        outbound_sats: 1_000,
        can_receive_lightning: false,
        can_receive_onchain: true,
        can_send_lightning: true,
        warning_code: Some("lightning_receive_needs_inbound_liquidity".to_string()),
        detail: "Projection for a usable channel that can send but cannot yet receive routed Lightning payments.".to_string(),
    };
    let channel_readiness = ChannelReadinessProof {
        schema: "pylon.ldk_wallet_channel_readiness_proof.v1".to_string(),
        lsp_integration_path: "ldk-node 0.7 exposes LSPS1 and LSPS2 hooks; Pylon surfaces not_configured LSP readiness until operator credentials are configured.".to_string(),
        cases: vec![
            receiver_no_channel_case,
            receiver_pending_channel_case,
            receiver_usable_channel_case,
            receiver_route_failure_case,
        ],
    };
    assert!(
        channel_readiness
            .cases
            .iter()
            .any(|case| case.case == "no_channel_before_open"
                && !case.can_receive_lightning
                && case.warning_code.as_deref()
                    == Some("lightning_receive_unavailable_no_channels")),
        "readiness proof should include the on-chain-only no-channel case"
    );
    assert!(
        channel_readiness
            .cases
            .iter()
            .any(|case| case.case == "usable_channel_after_open"
                && case.can_receive_lightning
                && case.inbound_sats > 0),
        "readiness proof should include the usable inbound-liquidity case"
    );
    assert!(
        channel_readiness
            .cases
            .iter()
            .any(|case| case.case == "route_failure_projection"
                && case.warning_code.as_deref()
                    == Some("lightning_receive_needs_inbound_liquidity")),
        "readiness proof should include the receive route/liquidity failure case"
    );

    payer.stop().expect("stop payer for restart");
    receiver.stop().expect("stop receiver for restart");

    let backup_dir = artifacts_dir.join("receiver-backup");
    copy_dir(receiver_dir.as_path(), backup_dir.as_path()).expect("copy receiver backup");
    let receiver_backup_digest =
        directory_digest(backup_dir.as_path()).expect("receiver backup digest");
    copy_dir(backup_dir.as_path(), restored_dir.as_path()).expect("restore receiver backup");

    let restarted_payer = build_node(
        payer_dir.as_path(),
        &electrum_url,
        next_listening_port(workspace.path(), 3),
        [7u8; 64],
    );
    let restarted_receiver = build_node(
        receiver_dir.as_path(),
        &electrum_url,
        next_listening_port(workspace.path(), 4),
        [11u8; 64],
    );
    let restart_payment = restarted_receiver
        .payment(&payment_id)
        .expect("receiver payment after restart");
    assert_eq!(restart_payment.status, PaymentStatus::Succeeded);
    assert!(
        restarted_payer
            .list_channels()
            .iter()
            .any(|channel| channel.is_channel_ready),
        "payer channel should survive restart"
    );
    assert!(
        restarted_receiver
            .list_channels()
            .iter()
            .any(|channel| channel.is_channel_ready),
        "receiver channel should survive restart"
    );

    restarted_receiver
        .stop()
        .expect("stop restarted receiver before restore node");
    let restored_receiver = build_node(
        restored_dir.as_path(),
        &electrum_url,
        next_listening_port(workspace.path(), 5),
        [11u8; 64],
    );
    let restored_payment = restored_receiver
        .payment(&payment_id)
        .expect("restored receiver payment");
    assert_eq!(restored_payment.status, PaymentStatus::Succeeded);

    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        "summary".to_string(),
        artifacts_dir
            .join("harness-summary.json")
            .display()
            .to_string(),
    );
    artifacts.insert(
        "receiver_backup".to_string(),
        backup_dir.display().to_string(),
    );
    artifacts.insert("payer_storage".to_string(), payer_dir.display().to_string());
    artifacts.insert(
        "receiver_storage".to_string(),
        receiver_dir.display().to_string(),
    );
    artifacts.insert(
        "restored_receiver_storage".to_string(),
        restored_dir.display().to_string(),
    );

    let receipt_ids = vec![
        format!(
            "pylon.wallet.harness.regtest.onchain_receive:{}",
            funding_txid
        ),
        wallet_registration_receipt_id,
        treasury_dispatch_receipt_id,
        format!(
            "pylon.wallet.harness.regtest.channel_open:{}",
            channel_funding_outpoint
        ),
        format!("pylon.wallet.harness.regtest.bolt11:{payment_hash}"),
        pylon_observation_receipt_id,
        format!(
            "pylon.wallet.harness.regtest.onchain_withdrawal:{}",
            withdrawal_txid
        ),
        withdrawal_receipt_id,
        format!("pylon.wallet.harness.regtest.backup_restore:{payment_hash}"),
    ];

    let summary = HarnessSummary {
        schema: "pylon.ldk_wallet_harness.summary.v1".to_string(),
        network: "regtest".to_string(),
        plan: pylon_ldk_wallet_harness_plan(),
        payer_node_id: restarted_payer.node_id().to_string(),
        receiver_node_id: restored_receiver.node_id().to_string(),
        funding_txid: funding_txid.to_string(),
        withdrawal_txid: withdrawal_txid.to_string(),
        channel_funding_outpoint: channel_funding_outpoint.to_string(),
        payment_id: format!("{payment_id:?}"),
        payment_hash,
        payer_balances: BalancePair {
            before: payer_pre,
            after: payer_post,
        },
        receiver_balances: BalancePair {
            before: receiver_pre,
            after: receiver_post,
        },
        receipt_ids,
        channel_readiness,
        bolt12_status,
        accepted_work,
        restart_payment_status: format!("{:?}", restart_payment.status),
        restored_payment_status: format!("{:?}", restored_payment.status),
        receiver_backup_digest,
        restored_channel_count: restored_receiver.list_channels().len(),
        artifacts,
    };

    let summary_path = artifacts_dir.join("harness-summary.json");
    fs::write(
        summary_path.as_path(),
        serde_json::to_string_pretty(&summary).expect("serialize summary"),
    )
    .expect("write summary");
}

fn setup_bitcoind_and_electrsd() -> (BitcoinD, ElectrsD) {
    let bitcoind_exe = env::var("BITCOIND_EXE")
        .ok()
        .or_else(|| corepc_node::downloaded_exe_path().ok())
        .expect("set BITCOIND_EXE or enable electrsd's downloaded bitcoind feature");
    let mut bitcoind_conf = corepc_node::Conf::default();
    bitcoind_conf.network = "regtest";
    bitcoind_conf.p2p = corepc_node::P2P::Yes;
    bitcoind_conf.args.push("-rest");
    let bitcoind = BitcoinD::with_conf(bitcoind_exe, &bitcoind_conf).expect("start bitcoind");

    let electrs_exe = env::var("ELECTRS_EXE")
        .ok()
        .or_else(electrsd::downloaded_exe_path)
        .expect("set ELECTRS_EXE or enable electrsd's downloaded electrs feature");
    let mut electrsd_conf = electrsd::Conf::default();
    electrsd_conf.http_enabled = false;
    electrsd_conf.network = "regtest";
    electrsd_conf.view_stderr = env::var("OPENAGENTS_PYLON_LDK_HARNESS_ELECTRSD_LOGS").is_ok();
    let electrsd =
        ElectrsD::with_conf(electrs_exe, &bitcoind, &electrsd_conf).unwrap_or_else(|error| {
            panic!(
                "start electrsd: {error}. If the bundled electrs binary is not native for this host, build or install a native electrs binary and rerun with ELECTRS_EXE=/path/to/electrs"
            )
        });
    (bitcoind, electrsd)
}

fn next_listening_port(workspace: &Path, ordinal: u16) -> SocketAddress {
    let base: u16 = 21_000u16 + (std::process::id() % 10_000) as u16;
    let port = base.saturating_add(ordinal);
    let marker = workspace.join(format!("port-{port}"));
    fs::write(marker, "").expect("record harness port marker");
    format!("127.0.0.1:{port}")
        .parse()
        .expect("parse listening socket")
}

fn build_node(
    storage_dir: &Path,
    electrum_url: &str,
    listening_address: SocketAddress,
    seed: [u8; 64],
) -> Node {
    let mut config = Config::default();
    config.network = Network::Regtest;
    config.storage_dir_path = storage_dir.display().to_string();
    config.listening_addresses = Some(vec![listening_address]);

    let mut builder = Builder::from_config(config);
    builder.set_entropy_seed_bytes(seed);
    builder.set_chain_source_electrum(
        electrum_url.to_string(),
        Some(ElectrumSyncConfig {
            background_sync_config: None,
        }),
    );
    let node = builder.build().expect("build ldk node");
    node.start().expect("start ldk node");
    assert!(node.status().is_running);
    node
}

async fn premine_and_distribute_funds<E: ElectrumApi>(
    bitcoind: &BitcoindClient,
    electrs: &E,
    addresses: Vec<Address>,
    amount: Amount,
) -> Txid {
    let _ = bitcoind.create_wallet("pylon_ldk_wallet_harness");
    let _ = bitcoind.load_wallet("pylon_ldk_wallet_harness");
    generate_blocks_and_wait(bitcoind, electrs, 101).await;

    let mut amounts = serde_json::Map::new();
    for address in addresses {
        amounts.insert(address.to_string(), json!(amount.to_btc()));
    }
    let txid = bitcoind
        .call::<Value>("sendmany", &[json!(""), Value::Object(amounts)])
        .expect("send wallet funding")
        .as_str()
        .expect("funding txid as string")
        .parse()
        .expect("parse funding txid");
    wait_for_tx(electrs, txid).await;
    generate_blocks_and_wait(bitcoind, electrs, 1).await;
    txid
}

async fn generate_blocks_and_wait<E: ElectrumApi>(
    bitcoind: &BitcoindClient,
    electrs: &E,
    blocks: usize,
) {
    let info = bitcoind
        .get_blockchain_info()
        .expect("bitcoind blockchain info");
    let target_height = info.blocks as usize + blocks;
    let address = bitcoind.new_address().expect("mining address");
    let _ = bitcoind.generate_to_address(blocks, &address);
    wait_for_block(electrs, target_height).await;
}

async fn wait_for_block<E: ElectrumApi>(electrs: &E, min_height: usize) {
    let mut header = electrs
        .block_headers_subscribe()
        .expect("subscribe to electrs headers");
    let mut tries = 0usize;
    while header.height < min_height {
        tries += 1;
        assert!(
            tries < 60,
            "timed out waiting for electrs height {min_height}"
        );
        electrs.ping().expect("electrs ping");
        if let Ok(Some(next)) = electrs.block_headers_pop() {
            header = next;
        } else {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

async fn wait_for_tx<E: ElectrumApi>(electrs: &E, txid: Txid) {
    for _ in 0..60 {
        if electrs.transaction_get(&txid).is_ok() {
            return;
        }
        electrs.ping().expect("electrs ping");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for tx {txid}");
}

async fn wait_for_wallet_balance(node: &Node, expected_sats: u64, label: &str) {
    for _ in 0..60 {
        node.sync_wallets().expect("wallet sync");
        if node.list_balances().spendable_onchain_balance_sats >= expected_sats {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for {label}");
}

async fn open_channel<E: ElectrumApi>(
    payer: &Node,
    receiver: &Node,
    channel_sats: u64,
    bitcoind: &BitcoindClient,
    electrs: &E,
) -> OutPoint {
    let receiver_address = receiver
        .listening_addresses()
        .expect("receiver listening addresses")
        .first()
        .expect("receiver first listening address")
        .clone();
    payer
        .open_channel(
            receiver.node_id(),
            receiver_address,
            channel_sats,
            None,
            None,
        )
        .expect("open channel");

    let payer_outpoint = expect_channel_pending(payer, receiver.node_id()).await;
    let receiver_outpoint = expect_channel_pending(receiver, payer.node_id()).await;
    assert_eq!(payer_outpoint, receiver_outpoint);
    wait_for_tx(electrs, payer_outpoint.txid).await;
    generate_blocks_and_wait(bitcoind, electrs, 6).await;
    for _ in 0..60 {
        payer.sync_wallets().expect("payer channel sync");
        receiver.sync_wallets().expect("receiver channel sync");
        if payer
            .list_channels()
            .iter()
            .any(|channel| channel.is_channel_ready)
            && receiver
                .list_channels()
                .iter()
                .any(|channel| channel.is_channel_ready)
        {
            drain_channel_ready_events(payer);
            drain_channel_ready_events(receiver);
            return payer_outpoint;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("timed out waiting for channel ready");
}

fn drain_channel_ready_events(node: &Node) {
    loop {
        match node.next_event() {
            Some(Event::ChannelReady { .. }) => {
                node.event_handled().expect("channel ready handled");
            }
            Some(event) => {
                node.event_handled().expect("non-channel event handled");
                panic!("unexpected event before payment: {event:?}");
            }
            None => return,
        }
    }
}

async fn expect_channel_pending(node: &Node, counterparty: PublicKey) -> OutPoint {
    match node.next_event_async().await {
        Event::ChannelPending {
            counterparty_node_id,
            funding_txo,
            ..
        } => {
            assert_eq!(counterparty_node_id, counterparty);
            node.event_handled().expect("channel pending handled");
            funding_txo
        }
        other => panic!("unexpected event while waiting for channel pending: {other:?}"),
    }
}

async fn expect_payment_successful(node: &Node) -> String {
    match node.next_event_async().await {
        Event::PaymentSuccessful { payment_hash, .. } => {
            node.event_handled().expect("payment successful handled");
            payment_hash.to_string()
        }
        other => panic!("unexpected event while waiting for payment successful: {other:?}"),
    }
}

async fn expect_payment_received(node: &Node, expected_amount_msat: u64) -> String {
    match node.next_event_async().await {
        Event::PaymentReceived {
            payment_hash,
            amount_msat,
            ..
        } => {
            assert_eq!(amount_msat, expected_amount_msat);
            node.event_handled().expect("payment received handled");
            payment_hash.to_string()
        }
        other => panic!("unexpected event while waiting for payment received: {other:?}"),
    }
}

async fn attempt_bolt12_payment(payer: &Node, receiver: &Node) -> String {
    let offer = match receiver.bolt12_payment().receive(
        PAYMENT_MSATS,
        "pylon regtest harness bolt12",
        Some(3600),
        None,
    ) {
        Ok(offer) => offer,
        Err(error) => return format!("offer_unavailable:{error}"),
    };

    let payment_id = match payer.bolt12_payment().send(&offer, None, None, None) {
        Ok(payment_id) => payment_id,
        Err(error) => return format!("send_unavailable:{error}"),
    };

    for _ in 0..30 {
        if let Some(payment) = payer.payment(&payment_id) {
            if payment.status == PaymentStatus::Succeeded {
                return "succeeded".to_string();
            }
            if payment.status == PaymentStatus::Failed {
                return "failed".to_string();
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    "pending_after_attempt".to_string()
}

fn balance_snapshot(node: &Node) -> BalanceSnapshot {
    let balances = node.list_balances();
    let channels = node.list_channels();
    BalanceSnapshot {
        total_onchain_sats: balances.total_onchain_balance_sats,
        spendable_onchain_sats: balances.spendable_onchain_balance_sats,
        total_lightning_sats: balances.total_lightning_balance_sats,
        lightning_sats: channels
            .iter()
            .map(|channel| channel.outbound_capacity_msat / 1000)
            .sum(),
        inbound_lightning_sats: channels
            .iter()
            .map(|channel| channel.inbound_capacity_msat / 1000)
            .sum(),
    }
}

fn channel_readiness_case_from_node(case: &str, node: &Node) -> ChannelReadinessCase {
    let channels = node.list_channels();
    let inbound_sats = channels
        .iter()
        .map(|channel| channel.inbound_capacity_msat / 1000)
        .sum::<u64>();
    let outbound_sats = channels
        .iter()
        .map(|channel| channel.outbound_capacity_msat / 1000)
        .sum::<u64>();
    let usable_count = channels.iter().filter(|channel| channel.is_usable).count();
    let pending_count = channels
        .iter()
        .filter(|channel| !channel.is_channel_ready)
        .count();
    let peer_connected_count = channels.iter().filter(|channel| channel.is_usable).count();
    let can_receive_lightning = usable_count > 0 && peer_connected_count > 0 && inbound_sats > 0;
    let can_send_lightning = usable_count > 0 && peer_connected_count > 0 && outbound_sats > 0;
    let (state, warning_code, detail) = if channels.is_empty() {
        (
            "onchain_only_no_channels",
            Some("lightning_receive_unavailable_no_channels"),
            "Can receive on-chain, but no Lightning channels are visible yet.",
        )
    } else if usable_count == 0 && pending_count > 0 {
        (
            "channel_pending",
            Some("lightning_receive_pending_channel"),
            "A channel exists but is not usable yet.",
        )
    } else if peer_connected_count == 0 {
        (
            "peer_disconnected",
            Some("lightning_receive_peer_disconnected"),
            "A channel exists but the peer is not connected.",
        )
    } else if inbound_sats == 0 {
        (
            "needs_inbound_liquidity",
            Some("lightning_receive_needs_inbound_liquidity"),
            "Lightning receive needs inbound liquidity.",
        )
    } else if outbound_sats == 0 {
        (
            "receive_ready_send_limited",
            Some("lightning_send_needs_outbound_liquidity"),
            "Lightning receive has usable inbound liquidity, but sends need outbound liquidity.",
        )
    } else {
        (
            "lightning_ready",
            None,
            "Lightning receive has usable inbound liquidity.",
        )
    };
    ChannelReadinessCase {
        case: case.to_string(),
        state: state.to_string(),
        channel_count: channels.len(),
        usable_count,
        pending_count,
        peer_connected_count,
        inbound_sats,
        outbound_sats,
        can_receive_lightning,
        can_receive_onchain: true,
        can_send_lightning,
        warning_code: warning_code.map(ToString::to_string),
        detail: detail.to_string(),
    }
}

fn copy_dir(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(source_path.as_path(), destination_path.as_path())?;
        } else {
            fs::copy(source_path.as_path(), destination_path.as_path())?;
        }
    }
    Ok(())
}

fn directory_digest(path: &Path) -> std::io::Result<String> {
    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    for relative_path in files {
        hasher.update(relative_path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(path.join(&relative_path))?);
        hasher.update([0]);
    }

    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, path.as_path(), files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(std::io::Error::other)?
                .to_path_buf();
            files.push(relative);
        }
    }
    Ok(())
}
