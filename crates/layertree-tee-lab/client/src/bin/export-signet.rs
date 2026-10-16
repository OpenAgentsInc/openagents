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
fn export(
    root: &Path,
    release_dir: &Path,
    output: &Path,
    refusal_output: Option<&Path>,
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
        "historical_attestation":true,"negative_view":"Expected tree ID changed; real captured token rejected with report_data_mismatch; service history remains separate.",
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
            args.get(3).map(Path::new),
        )
    } else {
        Err(
            "use PUBLIC_CAPTURE_DIRECTORY RELEASE_DIRECTORY OUTPUT.jsonl [REFUSAL_VIEW.jsonl]"
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
