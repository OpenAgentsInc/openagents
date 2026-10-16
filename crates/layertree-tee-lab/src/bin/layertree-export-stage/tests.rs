use super::*;
use bitcoin::{
    Amount, Block, BlockHash, CompactTarget, ScriptBuf, Sequence, TxIn, TxMerkleNode, TxOut,
    Witness, absolute::LockTime, block, consensus::serialize, hashes::Hash, script::Builder,
    transaction::Version,
};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    input: PathBuf,
    primary: PathBuf,
}

fn save(path: &Path, value: &Value) {
    write(path, serde_json::to_vec(value).unwrap());
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn transaction(point: OutPoint, amount: u64, record_hash: Option<[u8; 32]>) -> Transaction {
    let mut outputs = vec![TxOut {
        value: Amount::from_sat(amount),
        script_pubkey: ScriptBuf::new(),
    }];
    if let Some(hash) = record_hash {
        let mut script = vec![0x6a, 0x20];
        script.extend(hash);
        outputs.push(TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::from_bytes(script),
        });
    }
    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: point,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: outputs,
    }
}

fn core(tree: &Path, scenario: &str, tx: &Transaction, height: u32) {
    // These synthetic blocks exercise saved-byte checks, not consensus validation.
    let mut coinbase = transaction(OutPoint::null(), 1, None);
    coinbase.input[0].script_sig = Builder::new().push_int(i64::from(height)).into_script();
    let mut block = Block {
        header: Header {
            version: block::Version::TWO,
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 1_800_000_000 + height,
            bits: CompactTarget::from_consensus(0x207fffff),
            nonce: 0,
        },
        txdata: vec![coinbase, tx.clone()],
    };
    block.header.merkle_root = block.compute_merkle_root().unwrap();
    let hash = block.block_hash().to_string();
    write(&tree.join(format!("{scenario}.tx")), serialize(tx));
    save(
        &tree.join(format!("core/{scenario}-transaction.json")),
        &json!({
            "txid":tx.compute_txid().to_string(), "hex":hex(&serialize(tx)),
            "blockhash":hash, "confirmations":1
        }),
    );
    save(
        &tree.join(format!("core/{scenario}-header.json")),
        &json!({
            "hash":hash, "height":height, "confirmations":1,
            "merkleroot":block.header.merkle_root.to_string(), "time":block.header.time
        }),
    );
    write(
        &tree.join(format!("core/{scenario}-header.hex")),
        hex(&serialize(&block.header)),
    );
    write(
        &tree.join(format!("core/{scenario}-block.hex")),
        hex(&serialize(&block)),
    );
}

impl Fixture {
    fn new() -> Self {
        Self::with_shortfall(5_000)
    }

    fn with_shortfall(shortfall: u64) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let input = std::env::temp_dir().join(format!(
            "layertree-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let primary = input.join("primary.jsonl");
        let primary_log = include_str!("../../../fixtures/events/signet-normal.jsonl");
        write(&primary, primary_log);
        let mut open = events::read(primary_log).unwrap().events[0].body.clone();
        let Body::TreeOpen {
            tree_id,
            root,
            outpoint,
            ..
        } = &mut open
        else {
            unreachable!()
        };
        *tree_id = "ab".repeat(32);
        let stage_tree = tree_id.clone();
        let root = root.clone();
        let funding = OutPoint::null();
        let genesis = transaction(funding, root.sum, None);
        let mut point = OutPoint {
            txid: genesis.compute_txid(),
            vout: 0,
        };
        *outpoint = point.to_string();
        let tree = input.join("tree");
        core(&tree, "genesis", &genesis, 100);
        save(
            &tree.join("stage.json"),
            &json!({
                "schema":"layertree.stage.v1", "kind":"deliberately_broken_software_test",
                "network":"signet", "tree_id":stage_tree, "funding":funding.to_string(), "until":1_900_000_000
            }),
        );
        let mut log = Writer::new(Network::Signet);
        log.push(Some(100), open);
        let mut attacks = Vec::new();
        let mut journals = [String::new(), String::new(), String::new()];
        let mut received = String::new();
        for (i, (scenario, result)) in SCENARIOS.iter().enumerate() {
            let height = 101 + i as u32 * 2;
            let hash = [i as u8 + 1; 32];
            let amount = root.sum - if *scenario == "theft" { shortfall } else { 0 };
            let tx = transaction(point, amount, Some(hash));
            let txid = tx.compute_txid().to_string();
            core(&tree, scenario, &tx, height);
            log.push(
                Some(height),
                Body::Transition {
                    op: OpKind::Refresh,
                    prev: point.to_string(),
                    changes: vec![],
                    old_root: root.clone(),
                    new_root: root.clone(),
                    observed_output_amount: Some(amount),
                    txid: txid.clone(),
                    record_hash: hex(&hash),
                    next_unroll_key: None,
                },
            );
            log.push(
                Some(height),
                Body::Block {
                    spent: vec![point.to_string()],
                },
            );
            let mut alarms = Vec::new();
            for member in 0..3 {
                let alarm_height = height + u32::from(*scenario == "withheld");
                let id = hex(&tagged_hash(
                    "LayerTreeFollower/observation/v1",
                    &[&serde_json::to_vec(&(
                        "signet",
                        &stage_tree,
                        member as u64,
                        alarm_height,
                        Some(&txid),
                        result,
                    ))
                    .unwrap()],
                ));
                let row = json!({
                    "schema":"layertree.follower.observation.v1", "kind":"light_client", "id":id,
                    "network":"signet", "tree_id":stage_tree, "member":member, "height":alarm_height,
                    "txid":txid, "result":result, "observed_at":1_800_000_000+height,
                    "observed_tip":alarm_height
                });
                let line = row.to_string() + "\n";
                journals[member].push_str(&line);
                received.push_str(&line);
                journals[member].push_str(&(json!({"schema":"layertree.follower.observation.v1","kind":"webhook_ack","id":id,"observed_at":1_800_000_001+height}).to_string()+"\n"));
                alarms.push(row);
            }
            attacks.push(json!({"scenario":scenario,"txid":txid,"height":height,"alarms":alarms}));
            point = OutPoint {
                txid: tx.compute_txid(),
                vout: 0,
            };
        }
        write(&tree.join("events.jsonl"), log.to_jsonl());
        save(&tree.join("attack-evidence.json"), &json!(attacks));
        for (member, journal) in journals.iter().enumerate() {
            write(
                &tree.join(format!("member-{member}/state/observations.jsonl")),
                journal,
            );
        }
        write(&input.join("notifications/received.jsonl"), received);
        Self { input, primary }
    }

    fn export(&self) -> Result<(String, Value), String> {
        export(&self.input, &self.primary)
    }

    fn attacks(&self, change: impl FnOnce(&mut Value)) {
        let path = self.input.join("tree/attack-evidence.json");
        let mut attacks: Value = json_file(&path).unwrap();
        change(&mut attacks);
        save(&path, &attacks);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.input);
    }
}

#[test]
fn consistent_saved_records_export_nine_alarms_without_an_attestation_claim() {
    let fixture = Fixture::new();
    let (output, report) = fixture.export().unwrap();
    let replay = events::read(&output).unwrap();
    assert_eq!(replay.events.len(), 16);
    assert_eq!(
        replay
            .events
            .iter()
            .filter(|e| matches!(e.body, Body::LightClient { .. }))
            .count(),
        9
    );
    assert_eq!(report["hardware_attestation"], false);
    assert_eq!(report["full_blocks_checked"], true);
    assert_ne!(report["tree_id"], report["primary_tree_id"]);
}

#[test]
fn a_duplicate_member_cannot_stand_in_for_three_followers() {
    let fixture = Fixture::new();
    fixture.attacks(|a| a[0]["alarms"][2] = a[0]["alarms"][1].clone());
    assert!(fixture.export().unwrap_err().contains("alarm disagrees"));
}

#[test]
fn an_alarm_needs_both_receiver_receipt_and_webhook_acknowledgment() {
    let fixture = Fixture::new();
    let path = fixture.input.join("notifications/received.jsonl");
    let receipt = text(&path).unwrap();
    write(
        &path,
        receipt
            .lines()
            .skip(1)
            .map(|s| s.to_owned() + "\n")
            .collect::<String>(),
    );
    assert!(fixture.export().unwrap_err().contains("delivery receipts"));
    write(&path, receipt);
    let path = fixture.input.join("tree/member-0/state/observations.jsonl");
    let mut journal = rows(&path).unwrap();
    journal.remove(1);
    write(
        &path,
        journal
            .iter()
            .map(|r| r.to_string() + "\n")
            .collect::<String>(),
    );
    assert!(fixture.export().unwrap_err().contains("delivery receipts"));
}

#[test]
fn theft_must_be_reported_within_one_block_even_when_all_copies_agree() {
    let fixture = Fixture::new();
    fixture.attacks(|a| a[2]["alarms"][0]["observed_tip"] = json!(107));
    let attacks: Value = json_file(&fixture.input.join("tree/attack-evidence.json")).unwrap();
    let late = &attacks[2]["alarms"][0];
    for name in [
        "notifications/received.jsonl",
        "tree/member-0/state/observations.jsonl",
    ] {
        let path = fixture.input.join(name);
        let mut log = rows(&path).unwrap();
        for row in &mut log {
            if row["kind"] == "light_client" && row["id"] == late["id"] {
                *row = late.clone();
            }
        }
        write(
            &path,
            log.iter().map(|r| r.to_string() + "\n").collect::<String>(),
        );
    }
    assert!(fixture.export().unwrap_err().contains("too late"));
}

#[test]
fn wrong_transaction_header_block_or_reported_height_is_refused() {
    let fixture = Fixture::new();
    let path = fixture.input.join("tree/theft.tx");
    let original = bytes(&path).unwrap();
    let mut tx: Transaction = deserialize(&original).unwrap();
    tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() + 1);
    write(&path, serialize(&tx));
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("confirmation reports disagree")
    );
    write(&path, original);
    let path = fixture.input.join("tree/core/theft-header.json");
    let mut report: Value = json_file(&path).unwrap();
    report["height"] = json!(106);
    save(&path, &report);
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("confirmation reports disagree")
    );
    report["height"] = json!(105);
    save(&path, &report);
    let path = fixture.input.join("tree/core/theft-block.hex");
    let mut block: Block = deserialize(&unhex(text(&path).unwrap().trim()).unwrap()).unwrap();
    block.txdata.pop();
    write(&path, hex(&serialize(&block)));
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("does not contain the transaction")
    );
}

#[test]
fn a_consistent_but_different_theft_amount_is_refused() {
    for amount in [0, 4_999] {
        let fixture = Fixture::with_shortfall(amount);
        assert!(
            fixture
                .export()
                .unwrap_err()
                .contains("exact output amount")
        );
    }
}

#[test]
fn hardware_claims_and_using_the_primary_tree_for_attacks_are_refused() {
    let fixture = Fixture::new();
    let path = fixture.input.join("tree/stage.json");
    let mut stage: Value = json_file(&path).unwrap();
    stage["kind"] = json!("attested_hardware");
    save(&path, &stage);
    assert!(fixture.export().unwrap_err().contains("software-signer"));
    stage["kind"] = json!("deliberately_broken_software_test");
    save(&path, &stage);
    write(
        &fixture.primary,
        text(&fixture.input.join("tree/events.jsonl")).unwrap(),
    );
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("different primary tree")
    );
}

#[test]
fn an_incomplete_scenario_or_unfinished_log_is_refused() {
    let fixture = Fixture::new();
    fixture.attacks(|a| {
        a.as_array_mut().unwrap().pop();
    });
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("all three attack scenarios")
    );
    let fixture = Fixture::new();
    let path = fixture.input.join("notifications/received.jsonl");
    write(&path, "{unfinished");
    assert!(
        fixture
            .export()
            .unwrap_err()
            .contains("unfinished last line")
    );
}
