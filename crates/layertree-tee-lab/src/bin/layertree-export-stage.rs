//! Offline export of saved staged software-signer alarms for Verse.
//! Checks saved public records; it does not attest hardware or contact a node.
use bitcoin::{OutPoint, Transaction, block::Header, consensus::deserialize};
use layertree_tee_lab::{
    events::{self, Body, Network, OpKind, Writer},
    follow::anchored_hashes,
    tree::tagged_hash,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

const SCENARIOS: [(&str, &str); 3] = [
    ("withheld", "RecordMissing"),
    ("mismatch", "RecordNotAnchored"),
    ("theft", "OutputMismatch"),
];
const LIMIT: u64 = 128 * 1024 * 1024;

fn bytes(path: &Path) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| format!("Could not open {}.", path.display()))?
        .take(LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|_| format!("Could not read {}.", path.display()))?;
    if data.len() as u64 > LIMIT {
        return Err(format!("{} exceeds 128 MiB.", path.display()));
    }
    Ok(data)
}

fn text(path: &Path) -> Result<String, String> {
    String::from_utf8(bytes(path)?).map_err(|_| format!("{} is not text.", path.display()))
}

fn json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&bytes(path)?)
        .map_err(|_| format!("{} has invalid JSON or unsupported fields.", path.display()))
}

fn rows(path: &Path) -> Result<Vec<Value>, String> {
    let data = text(path)?;
    if !data.is_empty() && !data.ends_with('\n') {
        return Err(format!("{} has an unfinished last line.", path.display()));
    }
    data.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|_| format!("{} has an invalid JSON line.", path.display()))
        })
        .collect()
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(data: &str) -> Result<Vec<u8>, String> {
    if !data.len().is_multiple_of(2) {
        return Err("Hex data has an invalid length.".into());
    }
    data.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| "Invalid hex data.".into())
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    schema: String,
    kind: String,
    network: String,
    tree_id: String,
    funding: String,
    until: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attack {
    scenario: String,
    txid: String,
    height: u32,
    alarms: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    schema: String,
    kind: String,
    id: String,
    network: String,
    tree_id: String,
    member: u64,
    height: u32,
    txid: Option<String>,
    result: String,
    observed_at: u64,
    observed_tip: u32,
}

impl Observation {
    fn parse(row: &Value, stage: &Stage, member: u64) -> Result<Self, String> {
        let obs: Self = serde_json::from_value(row.clone())
            .map_err(|_| "A follower observation has unsupported fields.")?;
        if obs.schema != "layertree.follower.observation.v1"
            || obs.kind != "light_client"
            || obs.network != stage.network
            || obs.tree_id != stage.tree_id
            || obs.member != member
            || obs.observed_tip < obs.height
            || obs.observed_at == 0
        {
            return Err("A follower observation names another tree, member, or network, or has an invalid time or height.".into());
        }
        let identity = serde_json::to_vec(&(
            &obs.network,
            &obs.tree_id,
            obs.member,
            obs.height,
            &obs.txid,
            &obs.result,
        ))
        .map_err(|_| "Could not check a follower observation.")?;
        if obs.id
            != hex(&tagged_hash(
                "LayerTreeFollower/observation/v1",
                &[&identity],
            ))
        {
            return Err("A follower observation has the wrong ID.".into());
        }
        Ok(obs)
    }
}

fn tree_id(log: &events::Replay) -> Result<&str, String> {
    match log.events.first().map(|e| &e.body) {
        Some(Body::TreeOpen { tree_id, .. }) => Ok(tree_id),
        _ => Err("Choose a complete tree event log.".into()),
    }
}

/// Checks saved Core replies and block bytes, including transaction inclusion
/// and the BIP-34 height. This does not validate the signet consensus rules.
fn chain_tx(tree: &Path, scenario: &str, txid: &str, height: u32) -> Result<Transaction, String> {
    let base = tree.join("core").join(scenario);
    let raw = json_file::<Value>(&base.with_file_name(format!("{scenario}-transaction.json")))?;
    let report = json_file::<Value>(&base.with_file_name(format!("{scenario}-header.json")))?;
    let tx_bytes = bytes(&tree.join(format!("{scenario}.tx")))?;
    let tx: Transaction = deserialize(&tx_bytes).map_err(|_| "Invalid saved transaction bytes.")?;
    let header_bytes = unhex(text(&base.with_file_name(format!("{scenario}-header.hex")))?.trim())?;
    let header: Header = deserialize(&header_bytes).map_err(|_| "Invalid saved block header.")?;
    let hash = header.block_hash().to_string();
    if tx.compute_txid().to_string() != txid
        || raw["txid"] != txid
        || raw["hex"].as_str().map(unhex).transpose()?.as_deref() != Some(&tx_bytes)
        || raw["blockhash"] != hash
        || raw["confirmations"].as_u64().unwrap_or(0) == 0
        || report["hash"] != hash
        || report["height"].as_u64() != Some(u64::from(height))
        || report["confirmations"].as_u64().unwrap_or(0) == 0
        || report["merkleroot"] != header.merkle_root.to_string()
        || report["time"].as_u64() != Some(u64::from(header.time))
    {
        return Err(format!(
            "The saved {scenario} transaction, header, and confirmation reports disagree."
        ));
    }
    let block_path = base.with_file_name(format!("{scenario}-block.hex"));
    let block: bitcoin::Block = deserialize(&unhex(text(&block_path)?.trim())?)
        .map_err(|_| "Invalid saved block bytes.")?;
    if block.header != header
        || !block.check_merkle_root()
        || block.bip34_block_height().ok() != Some(u64::from(height))
        || !block.txdata.iter().any(|candidate| candidate == &tx)
    {
        return Err(format!(
            "The saved {scenario} block does not contain the transaction at its stated height."
        ));
    }
    Ok(tx)
}

fn export(input: &Path, primary: &Path) -> Result<(String, Value), String> {
    let tree = input.join("tree");
    let stage: Stage = json_file(&tree.join("stage.json"))?;
    if stage.schema != "layertree.stage.v1"
        || stage.kind != "deliberately_broken_software_test"
        || stage.network != "signet"
        || stage.until == 0
    {
        return Err("Choose a completed staged signet software-signer test.".into());
    }
    let source = events::read(&text(&tree.join("events.jsonl"))?)
        .map_err(|e| format!("The staged event log is invalid: {e:?}."))?;
    let honest = events::read(&text(primary)?)
        .map_err(|e| format!("The primary tree log is invalid: {e:?}."))?;
    if tree_id(&source)? != stage.tree_id
        || tree_id(&honest)? == stage.tree_id
        || source.events.len() != 7
        || source.events.iter().any(|e| e.network != Network::Signet)
    {
        return Err(
            "Choose the staged tree's seven chain events and a different primary tree.".into(),
        );
    }
    let attacks: Vec<Attack> = json_file(&tree.join("attack-evidence.json"))?;
    if attacks.len() != 3 {
        return Err("The staged test needs all three attack scenarios.".into());
    }
    let mut delivered = BTreeMap::new();
    for row in rows(&input.join("notifications/received.jsonl"))? {
        let id = row["id"]
            .as_str()
            .ok_or("A receiver receipt has no alarm ID.")?
            .to_owned();
        if delivered
            .insert(id.clone(), row.clone())
            .is_some_and(|old| old != row)
        {
            return Err("Receiver receipts disagree for the same alarm ID.".into());
        }
    }
    let mut observed = BTreeMap::new();
    let mut acknowledged = BTreeMap::new();
    for member in 0..3 {
        for row in rows(&tree.join(format!("member-{member}/state/observations.jsonl")))? {
            if row["schema"] != "layertree.follower.observation.v1" {
                return Err("A follower log has an unsupported format.".into());
            }
            match row["kind"].as_str() {
                Some("light_client") => {
                    let obs = Observation::parse(&row, &stage, member)?;
                    if observed
                        .insert(obs.id.clone(), row.clone())
                        .is_some_and(|old| old != row)
                    {
                        return Err("Follower logs disagree for the same observation ID.".into());
                    }
                }
                Some("webhook_ack") => {
                    let id = row["id"]
                        .as_str()
                        .ok_or("A webhook acknowledgment has no ID.")?;
                    if !observed.contains_key(id) {
                        return Err("A webhook acknowledgment has no earlier observation.".into());
                    }
                    let at = row["observed_at"]
                        .as_u64()
                        .ok_or("A webhook acknowledgment has no time.")?;
                    acknowledged.insert((member, id.to_owned()), at);
                }
                Some("started" | "checked" | "chain_changed" | "source_unavailable") => {}
                _ => return Err("A follower log has an unsupported observation kind.".into()),
            }
        }
    }
    let open = &source.events[0];
    let Body::TreeOpen { outpoint, root, .. } = &open.body else {
        unreachable!()
    };
    let mut point = OutPoint::from_str(outpoint).map_err(|_| "Invalid staged creation output.")?;
    let genesis_height = open
        .height
        .ok_or("The staged creation output has no mined height.")?;
    let genesis = chain_tx(&tree, "genesis", &point.txid.to_string(), genesis_height)?;
    let funding =
        OutPoint::from_str(&stage.funding).map_err(|_| "Invalid staged funding output.")?;
    if point.vout != 0
        || genesis.output.first().map(|o| o.value.to_sat()) != Some(root.sum)
        || !genesis.input.iter().any(|i| i.previous_output == funding)
    {
        return Err("The staged creation event does not match its transaction or funding.".into());
    }
    let mut writer = Writer::new(Network::Signet);
    writer.push(open.height, open.body.clone());
    let mut used_ids = BTreeSet::new();
    let mut previous_height = genesis_height;
    for (i, (scenario, result)) in SCENARIOS.iter().enumerate() {
        let attack = &attacks[i];
        let transition = &source.events[1 + i * 2];
        let block = &source.events[2 + i * 2];
        let Body::Transition {
            op,
            prev,
            txid,
            record_hash,
            new_root,
            observed_output_amount,
            ..
        } = &transition.body
        else {
            return Err("A staged attack has no transition.".into());
        };
        if attack.scenario != *scenario
            || attack.txid != *txid
            || transition.height != Some(attack.height)
            || attack.height < previous_height
            || block.height != Some(attack.height)
            || !matches!(&block.body, Body::Block { spent } if spent == &vec![point.to_string()])
            || *op != OpKind::Refresh
            || *prev != point.to_string()
        {
            return Err(
                "The staged scenarios, transaction IDs, mined heights, or spent outputs disagree."
                    .into(),
            );
        }
        let tx = chain_tx(&tree, scenario, txid, attack.height)?;
        let amount = tx
            .output
            .first()
            .ok_or("A staged spend has no tree output.")?
            .value
            .to_sat();
        if tx.input.first().map(|i| i.previous_output) != Some(point)
            || *observed_output_amount != Some(amount)
            || !anchored_hashes(&tx).iter().any(|h| hex(h) == *record_hash)
            || new_root.sum.checked_sub(amount)
                != Some(if *scenario == "theft" { 5_000 } else { 0 })
        {
            return Err("A staged transaction does not match its recorded spend, record hash, or exact output amount.".into());
        }
        if attack.alarms.len() != 3 {
            return Err("Each staged scenario needs alarms from three unique members.".into());
        }
        let mut members = BTreeSet::new();
        let mut alarms = Vec::new();
        let alarm_height = attack
            .height
            .checked_add(u32::from(*scenario == "withheld"))
            .ok_or("The staged alarm height overflows.")?;
        for row in &attack.alarms {
            let member = row["member"]
                .as_u64()
                .ok_or("An attack alarm has no member.")?;
            let obs = Observation::parse(row, &stage, member)?;
            if member >= 3
                || !members.insert(member)
                || !used_ids.insert(obs.id.clone())
                || obs.txid.as_deref() != Some(txid)
                || obs.result != *result
                || obs.height != alarm_height
                || obs.observed_at > stage.until
                || (*scenario == "theft" && obs.observed_tip > attack.height.saturating_add(1))
                || observed.get(&obs.id) != Some(row)
                || delivered.get(&obs.id) != Some(row)
                || acknowledged
                    .get(&(member, obs.id.clone()))
                    .is_none_or(|at| *at < obs.observed_at)
            {
                return Err("A staged alarm disagrees with its follower, mined spend, or delivery receipts, or the theft alarm arrived too late.".into());
            }
            alarms.push(obs);
        }
        writer.push(transition.height, transition.body.clone());
        writer.push(block.height, block.body.clone());
        alarms.sort_by_key(|obs| obs.member);
        for obs in alarms {
            writer.push(
                Some(obs.height),
                Body::LightClient {
                    member: obs.member,
                    result: obs.result,
                },
            );
        }
        point = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        previous_height = attack.height;
    }
    let output = writer.to_jsonl();
    events::read(&output).map_err(|e| format!("The exported event log is invalid: {e:?}."))?;
    Ok((
        output,
        json!({
            "schema":"layertree.stage.export.v1", "network":"signet",
            "signer":"deliberately_broken_software_test", "hardware_attestation":false,
            "tree_id":stage.tree_id, "primary_tree_id":tree_id(&honest)?,
            "scenarios":3, "alarms":used_ids.len(), "full_blocks_checked":true,
            "chain_source":"saved_bitcoin_core_replies", "consensus_validation":false
        }),
    ))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut paths = BTreeMap::new();
    while let Some(flag) = args.next() {
        if !matches!(flag.as_str(), "--input" | "--primary-log" | "--output")
            || paths.contains_key(&flag)
        {
            return Err("Use --input DIRECTORY --primary-log FILE --output NEW_FILE.".into());
        }
        paths.insert(
            flag,
            PathBuf::from(args.next().ok_or("An option is missing its path.")?),
        );
    }
    if paths.len() != 3 {
        return Err("Use --input DIRECTORY --primary-log FILE --output NEW_FILE.".into());
    }
    let (log, report) = export(&paths["--input"], &paths["--primary-log"])?;
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&paths["--output"])
        .map_err(|_| "Could not create a new output file.")?;
    output
        .write_all(log.as_bytes())
        .and_then(|_| output.sync_all())
        .map_err(|_| "Could not save the exported event log.")?;
    println!("{report}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "layertree-export-stage/tests.rs"]
mod tests;
