//! Export public capture evidence with actual confirmation heights for Verse.
use bitcoin::{
    consensus::encode::deserialize_hex,
    hashes::{Hash, sha256},
};
use layertree_attest::{Binding, Evidence, Policy, TcbFloor, TokenPolicy, verify, verify_token};
use layertree_signet_service::Core;
use layertree_tee_lab::{
    events::{self, Body, Network, OpKind, Writer},
    rpc,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
const PUBLISHER: &str = "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d";
fn read<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    serde_json::from_slice(&fs::read(path).map_err(|_| "cannot read public capture")?)
        .map_err(|_| "invalid public capture".into())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::File::create_new(path).map_err(|_| "use a new public output path")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "cannot save public output".into())
}
fn recorded(root: &Path, point: &str) -> Result<PathBuf> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| "cannot read public capture directory")? {
        let path = entry.map_err(|_| "cannot read public capture path")?.path();
        if path.join("verified-output.json").is_file() {
            let observed: Value = read(path.join("verified-output.json"))?;
            if observed["outpoint"].as_str() == Some(point) {
                matches.push(path);
            }
        }
    }
    matches.sort();
    matches
        .pop()
        .ok_or("no verified capture for this confirmed output".into())
}
fn confirmed(core: &Core, txid: &str) -> Result<(bitcoin::Transaction, u32)> {
    let value = core.call("getrawtransaction", json!([txid, true]))?;
    if value["confirmations"].as_u64().unwrap_or(0) == 0 {
        return Err("refusing an unconfirmed event".into());
    }
    let header = core.call("getblockheader", json!([value["blockhash"]]))?;
    let height = u32::try_from(
        header["height"]
            .as_u64()
            .ok_or("missing confirmation height")?,
    )
    .map_err(|_| "invalid confirmation height")?;
    let tx = rpc::transaction(value["hex"].as_str().ok_or("missing chain transaction")?)?;
    if tx.compute_txid().to_string() != txid {
        return Err("chain transaction identity differs".into());
    }
    Ok((tx, height))
}
fn transaction_fee(core: &Core, tx: &bitcoin::Transaction) -> Result<u64> {
    let mut inputs = 0u64;
    for input in &tx.input {
        let previous = core.call(
            "getrawtransaction",
            json!([input.previous_output.txid.to_string(), true]),
        )?;
        let previous = rpc::transaction(
            previous["hex"]
                .as_str()
                .ok_or("missing input transaction")?,
        )?;
        inputs = inputs
            .checked_add(
                previous
                    .output
                    .get(input.previous_output.vout as usize)
                    .ok_or("missing input output")?
                    .value
                    .to_sat(),
            )
            .ok_or("input overflow")?;
    }
    let outputs = tx
        .output
        .iter()
        .try_fold(0u64, |sum, o| sum.checked_add(o.value.to_sat()))
        .ok_or("output overflow")?;
    inputs
        .checked_sub(outputs)
        .ok_or("negative transaction fee".into())
}
/// The tree a recovery spends and its saved escape, checked for every member.
struct Recovery {
    params: layertree_tee_lab::rules::TreeParams,
    state: layertree_tee_lab::rules::TreeState,
    package: layertree_tee_lab::unroll::UnrollPackage,
}
fn recovery(root: &Path, point_text: &str) -> Result<Recovery> {
    use layertree_tee_lab::{output::TreeOutput, rules::TreeState, unroll::UnrollPackage};
    let point = rpc::outpoint(point_text)?;
    let snapshot: rpc::Snapshot = read(recorded(root, point_text)?.join("state-before.json"))?;
    let params = snapshot.params.params()?;
    let tree = rpc::tree(params.depth, &snapshot.leaves)?;
    let node = tree.root().map_err(|_| "invalid recovery tree")?;
    let state = TreeState {
        tree,
        outpoint: point,
        output: TreeOutput {
            root_hash: node.hash,
            unroll_key: snapshot
                .unroll_key
                .as_deref()
                .ok_or("missing saved unroll key")?
                .parse()
                .map_err(|_| "invalid saved unroll key")?,
        },
    };
    let package = UnrollPackage::decode(&rpc::bytes(
        snapshot.unroll.as_deref().ok_or("missing saved unroll")?,
    )?)
    .map_err(|_| "invalid saved unroll")?;
    for member in state.tree.leaves().values() {
        layertree_tee_lab::unroll::verify_package(
            &bitcoin::secp256k1::Secp256k1::verification_only(),
            &params,
            state.output,
            state.outpoint,
            state.amount(),
            state.tree.leaves(),
            &package,
            member,
        )
        .map_err(|e| format!("saved escape refused: {e:?}"))?;
    }
    if package.txs.len() != 1 {
        return Err("this experiment requires a flat escape package".into());
    }
    Ok(Recovery {
        params,
        state,
        package,
    })
}
fn halt_events(root: &Path, core: &Core, tip: u32, writer: &mut Writer) -> Result<Value> {
    let path = root.join("halt-observer-state.json");
    let status: Value = read(&path)?;
    let point_text = status["halted_state"]["outpoint"]
        .as_str()
        .ok_or("missing halted output")?;
    let point = rpc::outpoint(point_text)?;
    let halted = u32::try_from(
        status["halted_at_height"]
            .as_u64()
            .ok_or("missing HALT height")?,
    )
    .map_err(|_| "invalid HALT height")?;
    if status["halted_state"]["halted"] != true
        || status["signing_refusal"] != "Halted"
        || halted > tip
    {
        return Err("missing observed HALT and exact signing refusal".into());
    }
    let Recovery {
        params,
        state,
        package,
    } = recovery(root, point_text)?;
    let (_, opened) = confirmed(core, &point.txid.to_string())?;
    let first = opened
        .checked_add(u32::from(params.unroll_delay))
        .ok_or("height overflow")?;
    if halted < opened || status["first_valid_unroll_block"] != first {
        return Err("HALT timing differs from the confirmed output and CSV delay".into());
    }
    writer.push(Some(halted), Body::EnclaveDown {});
    writer.push(
        Some(halted),
        Body::Rejected {
            op: OpKind::Refresh,
            reject: "Halted".into(),
        },
    );
    let parent = &package.txs[0];
    let parent_id = parent.compute_txid().to_string();
    let mut result = json!({"kind":"guardian_halt","physical_vm_kill":false,
        "halt_height":halted,"tree_confirmation_height":opened,"first_valid_block":first,
        "halt_state_sha256":sha256::Hash::hash(&fs::read(path).map_err(|_|"cannot read HALT observation")?).to_string(),
        "parent_txid":parent_id,"status":"waiting_csv"});
    if status["phase"] != "paid" {
        core.output(point, true)?;
        if status["unroll"]["status"] != "waiting_csv"
            || tip.checked_add(1).ok_or("height overflow")? >= first
        {
            return Err(
                "capture the original broadcast or payout before exporting this recovery stage"
                    .into(),
            );
        }
        let tests = status["unroll"]["testmempoolaccept"]
            .as_array()
            .ok_or("missing early CSV test")?;
        if tests.len() != 1
            || tests[0]["txid"] != parent_id
            || tests[0]["allowed"] != false
            || tests[0]["reject-reason"] != "non-BIP68-final"
        {
            return Err("the early escape was not refused for its actual CSV delay".into());
        }
        writer.push(Some(tip), Body::Block { spent: vec![] });
        writer.push(
            Some(tip),
            Body::Measured {
                what: "blocks_until_first_valid_block".into(),
                value: u64::from(first - tip - 1),
                unit: "blocks".into(),
                level: Some(0),
            },
        );
        result["observed_tip"] = tip.into();
        return Ok(result);
    }
    paid_events(root, core, writer, &state, parent, first, result)
}
/// The confirmed escape and its fee child: exact payouts to every member.
fn paid_events(
    root: &Path,
    core: &Core,
    writer: &mut Writer,
    state: &layertree_tee_lab::rules::TreeState,
    parent: &bitcoin::Transaction,
    first: u32,
    mut result: Value,
) -> Result<Value> {
    let parent_id = parent.compute_txid().to_string();
    let plan: Value = read(root.join("unroll-package.json"))?;
    let (actual, paid) = confirmed(core, &parent_id)?;
    let child_id = plan["child_txid"].as_str().ok_or("missing CPFP child")?;
    let (child, child_height) = confirmed(core, child_id)?;
    let planned_parent = rpc::transaction(plan["parent"].as_str().ok_or("missing parent bytes")?)?;
    let planned_child = rpc::transaction(plan["child"].as_str().ok_or("missing child bytes")?)?;
    let fee = transaction_fee(core, &child)?;
    let submitted = u32::try_from(plan["height"].as_u64().ok_or("missing broadcast height")?)
        .map_err(|_| "invalid broadcast height")?;
    if &actual != parent
        || actual != planned_parent
        || child != planned_child
        || paid < first
        || submitted.checked_add(1).ok_or("height overflow")? < first
        || submitted > paid
        || fee > 10000
        || plan["fee_sat"] != fee
        || transaction_fee(core, &actual)? != 0
    {
        return Err("confirmed escape or CPFP differs from its saved package and timing".into());
    }
    let anchor = bitcoin::OutPoint {
        txid: parent.compute_txid(),
        vout: state.tree.leaves().len() as u32,
    };
    if !child.input.iter().any(|i| i.previous_output == anchor) {
        return Err("confirmed fee child does not spend the saved escape anchor".into());
    }
    writer.push(
        Some(submitted),
        Body::UnrollBroadcast {
            level: 0,
            txid: parent_id.clone(),
            cpfp_fee: fee,
        },
    );
    writer.push(
        Some(paid),
        Body::UnrollConfirmed {
            level: 0,
            txid: parent_id.clone(),
        },
    );
    let mut payouts = Vec::new();
    for (index, (pos, leaf)) in state.tree.leaves().iter().enumerate() {
        let output = actual
            .output
            .get(index)
            .ok_or("missing confirmed member payout")?;
        if output.value.to_sat() != leaf.amount
            || output.script_pubkey != layertree_tee_lab::output::p2tr_raw(&leaf.exit_key)
        {
            return Err("confirmed escape pays a different member or amount".into());
        }
        writer.push(Some(paid), Body::payout(&leaf.exit_key, leaf.amount));
        payouts.push(
            json!({"position":pos,"amount_sat":leaf.amount,"exit_key":rpc::hex(&leaf.exit_key),
            "outpoint":format!("{parent_id}:{index}")}),
        );
    }
    writer.push(
        Some(child_height),
        Body::Measured {
            what: "escape_package_fee".into(),
            value: fee,
            unit: "sat".into(),
            level: Some(0),
        },
    );
    result["status"] = "paid".into();
    result["confirmation_height"] = paid.into();
    result["child_confirmation_height"] = child_height.into();
    result["fee_sat"] = fee.into();
    result["payouts"] = payouts.into();
    Ok(result)
}
/// The highest block whose header time is at or before `unix`.
fn height_at(core: &Core, tip: u32, unix: u64) -> Result<u32> {
    let mut height = tip;
    loop {
        let hash = core.call("getblockhash", json!([height]))?;
        let header = core.call("getblockheader", json!([hash]))?;
        if header["time"].as_u64().ok_or("missing block time")? <= unix {
            return Ok(height);
        }
        height = height.checked_sub(1).ok_or("no block before the kill")?;
    }
}
/// The deliberate physical kill (#11793): the scheduled deletion removes the
/// enclave's VM with no HALT, after the qualified soak, and members recover
/// through the saved escape alone.
fn kill_events(root: &Path, core: &Core, tip: u32, writer: &mut Writer) -> Result<Value> {
    let path = root.join("observer-state.json");
    let status: Value = read(&path)?;
    let audit_path = root.join("vm-deletion-audit.json");
    let audit: Value = read(&audit_path)?;
    let progress: Value = read(root.join("progress.json"))?;
    let last = progress["last_transaction"]
        .as_str()
        .ok_or("missing last tree transaction")?;
    let point_text = format!("{last}:0");
    let point = rpc::outpoint(&point_text)?;
    let time = |value: &Value, what: &str| value.as_f64().ok_or(format!("missing {what}"));
    let soak_end = time(&status["soak_ends_at_unix"], "soak end")?;
    let qualified = time(&status["elapsed_soak_observed_at_unix"], "qualified soak")?;
    let scheduled = time(&status["qualified_kill_at_unix"], "scheduled kill")?;
    let lost = time(&status["endpoint_lost_at_unix"], "observed endpoint loss")?;
    let started = time(&audit["started_at_unix"], "deletion start")?;
    let deleted = time(&audit["completed_at_unix"], "deletion completion")?;
    if !matches!(status["phase"].as_str(), Some("unroll" | "paid"))
        || status["late_heartbeats"].as_array().is_some_and(|late| {
            late.iter()
                .any(|row| row["height"].as_u64() > status["soak_began_height"].as_u64())
        })
        || qualified < soak_end
        || scheduled < qualified
        || started + 5.0 < scheduled
        || deleted < started
        || lost < started
        || audit["method"] != "v1.compute.instances.delete"
        || audit["resource"]
            != "projects/openagentsgemini/zones/us-central1-a/instances/oa-layertree-signet-tdx"
        || audit["error"] != Value::Null
        || !audit["instance_id"].as_str().is_some_and(|id| {
            status["qualified_kill_task"]
                .as_str()
                .is_some_and(|task| task.ends_with(&format!("/delete-{id}-clean24")))
        })
    {
        return Err("missing an audited scheduled deletion after a clean qualified soak".into());
    }
    let Recovery {
        params,
        state,
        package,
    } = recovery(root, &point_text)?;
    let (_, opened) = confirmed(core, last)?;
    let first = opened
        .checked_add(u32::from(params.unroll_delay))
        .ok_or("height overflow")?;
    let killed = height_at(core, tip, deleted as u64)?;
    if killed < opened || killed >= first {
        return Err("the kill does not fall inside the last output's CSV window".into());
    }
    writer.push(Some(killed), Body::EnclaveDown {});
    writer.push(
        Some(killed),
        Body::Measured {
            what: "blocks_from_kill_to_first_valid_block".into(),
            value: u64::from(first - killed),
            unit: "blocks".into(),
            level: Some(0),
        },
    );
    let parent = &package.txs[0];
    let parent_id = parent.compute_txid().to_string();
    let hash = |path: &Path, what: &str| -> Result<String> {
        Ok(
            sha256::Hash::hash(&fs::read(path).map_err(|_| format!("cannot read {what}"))?)
                .to_string(),
        )
    };
    let mut result = json!({"kind":"physical_vm_kill","physical_vm_kill":true,
        "deletion_started_at_unix":started,"deletion_completed_at_unix":deleted,
        "endpoint_lost_at_unix":lost,"kill_height":killed,"tree_confirmation_height":opened,
        "first_valid_block":first,"observer_state_sha256":hash(&path, "observer state")?,
        "deletion_audit_sha256":hash(&audit_path, "deletion audit")?,
        "parent_txid":parent_id,"status":"waiting_csv"});
    if status["phase"] != "paid" {
        core.output(point, true)?;
        let pending = &status["unroll_last_result"];
        let tests = pending["testmempoolaccept"]
            .as_array()
            .ok_or("missing early CSV test")?;
        if pending["status"] != "waiting_csv"
            || tip.checked_add(1).ok_or("height overflow")? >= first
            || tests.len() != 1
            || tests[0]["txid"] != parent_id
            || tests[0]["allowed"] != false
            || tests[0]["reject-reason"] != "non-BIP68-final"
        {
            return Err("the early escape was not refused for its actual CSV delay".into());
        }
        writer.push(Some(tip), Body::Block { spent: vec![] });
        result["observed_tip"] = tip.into();
        return Ok(result);
    }
    paid_events(root, core, writer, &state, parent, first, result)
}
fn export(
    root: &Path,
    release_dir: &Path,
    output: &Path,
    refusal_output: Option<&Path>,
    recovery: Option<&str>,
) -> Result<Value> {
    let core = Core::new(
        std::env::var_os("LAYERTREE_CHAIN_COOKIE").ok_or("set the scratch signet cookie path")?,
    )?;
    let tip = core.ready_height()?;
    let release = read(release_dir.join("release.json"))?;
    let head = read(release_dir.join("head.json"))?;
    let entry = read(release_dir.join("rekor-entry.json"))?;
    let rebuilds = [
        read(release_dir.join("local-rebuild.json"))?,
        read(release_dir.join("gcp-rebuild.json"))?,
    ];
    let floor = TcbFloor {
        evaluation_data_number: 19,
        attester_date_unix: 1_747_180_800,
        confidential_space_build: 260900,
    };
    let mut logs = Vec::new();
    for file in fs::read_dir(root).map_err(|_| "cannot read public captures")? {
        let path = file
            .map_err(|_| "cannot read public capture path")?
            .path()
            .join("events.json");
        if path.is_file() {
            logs.push(path);
        }
    }
    logs.sort();
    let log_path = logs.last().ok_or("no captured server events")?;
    let raw: Value = read(log_path)?;
    let text = raw["events"].as_str().ok_or("missing server event text")?;
    let replay = events::read(text).map_err(|e| format!("captured events refused: {e:?}"))?;
    let mut writer = Writer::new(Network::Signet);
    let mut sources = Vec::new();
    let mut negative = None;
    for event in &replay.events {
        if event.network != Network::Signet {
            return Err("refusing non-signet events".into());
        }
        let (point, expected_root) = match &event.body {
            Body::TreeOpen { outpoint, root, .. } => (
                outpoint.clone(),
                layertree_tee_lab::tree::Node {
                    hash: rpc::array(&root.hash)?,
                    sum: root.sum,
                },
            ),
            Body::Transition { txid, new_root, .. } => (
                format!("{txid}:0"),
                layertree_tee_lab::tree::Node {
                    hash: rpc::array(&new_root.hash)?,
                    sum: new_root.sum,
                },
            ),
            _ => return Err("normal export accepts only creation and transition events".into()),
        };
        let outpoint = rpc::outpoint(&point)?;
        let (tx, height) = confirmed(&core, &outpoint.txid.to_string())?;
        let actual = tx
            .output
            .get(outpoint.vout as usize)
            .ok_or("missing actual output")?;
        let capture = recorded(root, &point)?;
        let snapshot: rpc::Snapshot = read(capture.join("state-before.json"))?;
        let observation: Value = read(capture.join("verified-output.json"))?;
        let attestation: Value = read(capture.join("attestation.json"))?;
        let channel: Value = read(capture.join("channel.json"))?;
        let params = snapshot.params.params()?;
        let root_node = rpc::tree(params.depth, &snapshot.leaves)?
            .root()
            .map_err(|_| "invalid captured tree")?;
        let claimed: bitcoin::TxOut = deserialize_hex(
            observation["output"]
                .as_str()
                .ok_or("missing observed output")?,
        )
        .map_err(|_| "invalid observed output")?;
        if actual != &claimed
            || root_node != expected_root
            || snapshot.outpoint.as_deref() != Some(&point)
        {
            return Err("event, capture, and confirmed transaction differ".into());
        }
        let certificate = rpc::bytes(
            channel["certificate_der"]
                .as_str()
                .ok_or("missing certificate")?,
        )?;
        let certificate_sha256 = sha256::Hash::hash(&certificate).to_byte_array();
        let challenge = rpc::array(
            channel["challenge"]
                .as_str()
                .ok_or("missing recorded challenge")?,
        )?;
        let binding = Binding::Live {
            challenge: &challenge,
            certificate_sha256: &certificate_sha256,
        };
        let now = channel["checked_at_unix"]
            .as_u64()
            .ok_or("missing capture time")?;
        let token = attestation["token"]
            .as_str()
            .ok_or("missing captured token")?;
        let policy = Policy {
            params: &params,
            publisher: PUBLISHER,
            workload: "layertree-signet",
            min_rebuilds: 2,
            seen_generation: None,
            max_age_seconds: 900,
            floor: &floor,
        };
        let checked = verify(
            &Evidence {
                token,
                release: &release,
                head: &head,
                log_entry: &entry,
                rebuilds: &rebuilds,
                consistency: None,
                output: actual,
                root: root_node,
                unroll_key: snapshot
                    .unroll_key
                    .as_deref()
                    .ok_or("missing unroll key")?
                    .parse()
                    .map_err(|_| "invalid unroll key")?,
            },
            &policy,
            &binding,
            &mut att_release::rekor::Witness::default(),
            now,
        )
        .map_err(|e| format!("recorded evidence refused: {}", e.code()))?;
        let mut body = event.body.clone();
        if let Body::Transition {
            observed_output_amount,
            record_hash,
            ..
        } = &mut body
        {
            let hash = rpc::array::<32>(record_hash)?;
            let script = bitcoin::ScriptBuf::new_op_return(
                <&bitcoin::script::PushBytes>::try_from(&hash[..])
                    .map_err(|_| "invalid record hash")?,
            );
            if !tx.output.iter().any(|o| o.script_pubkey == script) {
                return Err("event record hash is not committed by the transaction".into());
            }
            *observed_output_amount = Some(actual.value.to_sat());
        }
        writer.push(Some(height), body);
        writer.push(Some(height), checked.event());
        let mut inputs = 0u64;
        for input in &tx.input {
            let previous = core.call(
                "getrawtransaction",
                json!([input.previous_output.txid.to_string(), true]),
            )?;
            let previous = rpc::transaction(
                previous["hex"]
                    .as_str()
                    .ok_or("missing previous transaction")?,
            )?;
            let value = previous
                .output
                .get(input.previous_output.vout as usize)
                .ok_or("missing previous output")?
                .value
                .to_sat();
            inputs = inputs.checked_add(value).ok_or("input overflow")?;
        }
        let outputs = tx
            .output
            .iter()
            .try_fold(0u64, |sum, o| sum.checked_add(o.value.to_sat()))
            .ok_or("output overflow")?;
        let fee = inputs
            .checked_sub(outputs)
            .ok_or("negative transaction fee")?;
        writer.push(
            Some(height),
            Body::Measured {
                what: "transaction_fee".into(),
                value: fee,
                unit: "sat".into(),
                level: None,
            },
        );
        writer.push(
            Some(height),
            Body::Measured {
                what: "transaction_vsize".into(),
                value: tx.vsize() as u64,
                unit: "vB".into(),
                level: None,
            },
        );
        if let Body::Transition {
            op: OpKind::Exit | OpKind::Evict,
            changes,
            ..
        } = &event.body
        {
            for change in changes {
                let old = change.old.as_ref().ok_or("missing exited leaf")?;
                let key = rpc::array(&old.exit_key)?;
                if !tx.output.iter().any(|o| {
                    o.value.to_sat() == old.amount
                        && o.script_pubkey == layertree_tee_lab::output::p2tr_raw(&key)
                }) {
                    return Err("actual exit payout differs".into());
                }
                writer.push(Some(height), Body::payout(&key, old.amount));
            }
        }
        // A separate view records a real verifier's refusal when its expected
        // tree ID is deliberately changed. It is never normal service history.
        let mut changed = params.clone();
        changed.tree_id[0] ^= 1;
        let token_policy = TokenPolicy {
            image_digest: &checked.image_digest,
            max_age_seconds: 900,
            floor: &floor,
        };
        let refused = verify_token(token, &changed, &token_policy, &binding, now).unwrap_err();
        if refused != layertree_attest::Error::ReportDataMismatch {
            return Err("negative check failed for an unrelated reason".into());
        }
        negative = Some((height, refused.event(&checked.image_digest)));
        sources.push(json!({"txid":outpoint.txid.to_string(),"confirmation_height":height,"authorization_height":event.height,
            "capture":capture.file_name().and_then(|s|s.to_str()),"capture_time":now,"source_commit":checked.source_commit,
            "token_sha256":sha256::Hash::hash(token.as_bytes()).to_string(),"fee_sat":fee,"vsize":tx.vsize()}));
    }
    let recovery = match recovery {
        Some("--halt") => Some(halt_events(root, &core, tip, &mut writer)?),
        Some("--kill") => Some(kill_events(root, &core, tip, &mut writer)?),
        _ => None,
    };
    let text = writer.to_jsonl();
    let checked = events::read(&text).map_err(|e| format!("exported events refused: {e:?}"))?;
    write_new(output, text.as_bytes())?;
    if let Some(path) = refusal_output {
        let (height, body) = negative.ok_or("missing negative evidence")?;
        writer.push(Some(height), body);
        let text = writer.to_jsonl();
        events::read(&text).map_err(|e| format!("negative view refused: {e:?}"))?;
        write_new(path, text.as_bytes())?;
    }
    let summary = json!({"schema":"openagents.layertree.signet-view.v1","chain_reader_tip":tip,
        "historical_attestation":true,"recovery":recovery,"negative_view":"Expected tree ID changed; real captured token rejected with report_data_mismatch; service history remains separate.",
        "output_sha256":sha256::Hash::hash(text.as_bytes()).to_string(),"events":checked.events.len(),"members":checked.tree.leaves().len(),"paid_sat":checked.paid,"sources":sources});
    write_new(
        &output.with_extension("sources.json"),
        serde_json::to_string_pretty(&summary)
            .map_err(|_| "cannot encode sources")?
            .as_bytes(),
    )?;
    Ok(summary)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = if (3..=4).contains(&args.len()) {
        export(
            Path::new(&args[0]),
            Path::new(&args[1]),
            Path::new(&args[2]),
            args.get(3).filter(|s| !s.starts_with("--")).map(Path::new),
            args.get(3)
                .map(String::as_str)
                .filter(|s| s.starts_with("--")),
        )
    } else {
        Err(
            "use PUBLIC_CAPTURE_DIRECTORY RELEASE_DIRECTORY OUTPUT.jsonl [REFUSAL_VIEW.jsonl|--halt|--kill]"
                .into(),
        )
    };
    match result {
        Ok(summary) => println!("{summary}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
