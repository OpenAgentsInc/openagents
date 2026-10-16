//! Convert a public, attested-client snapshot into follower inputs.
use super::{atomic_file, hex, mirror::Sources, unhex};
use crate::{
    events::{Body, Network, Writer},
    output::TreeOutput,
    rpc,
    rules::TreeState,
    unroll::{UnrollPackage, verify_branch},
};
use bitcoin::{TxOut, secp256k1::Secp256k1};
use serde_json::json;
use std::{io::Read, path::Path};

pub fn snapshot(
    snapshot_path: &Path,
    sources_path: &Path,
    output: &Path,
    network: &str,
    rpc_url: &str,
    height: u32,
    member: u64,
    webhook_env: &str,
) -> Result<(), String> {
    let network_value = match network {
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        _ => return Err("Choose signet or regtest.".into()),
    };
    super::chain::endpoint(rpc_url)?;
    fn read(path: &Path) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "Could not open public bootstrap input.")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read public bootstrap input.")?;
        if bytes.len() > 1024 * 1024 {
            return Err("Public bootstrap input exceeds 1 MiB.".into());
        }
        Ok(bytes)
    }
    let snapshot: rpc::Snapshot = serde_json::from_slice(&read(snapshot_path)?)
        .map_err(|_| "Invalid public state snapshot.")?;
    let source_bytes = read(sources_path)?;
    let sources: Sources = serde_json::from_slice(&source_bytes)
        .map_err(|_| "Invalid public mirror configuration.")?;
    // Make source directory paths independent of the generated config location.
    let mut sources_value = serde_json::from_slice::<serde_json::Value>(&source_bytes)
        .map_err(|_| "Invalid mirror input.")?;
    let base = sources_path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(directories) = sources_value
        .get_mut("directories")
        .and_then(|v| v.as_array_mut())
    {
        for (value, path) in directories.iter_mut().zip(&sources.directories) {
            *value = json!(
                std::fs::canonicalize(base.join(path))
                    .map_err(|_| "Could not find a public mirror directory.")?
            );
        }
    }
    let params = snapshot.params.params()?;
    let tree = rpc::tree(params.depth, &snapshot.leaves)?;
    let outpoint = rpc::outpoint(
        snapshot
            .outpoint
            .as_deref()
            .ok_or("The snapshot has no tree output.")?,
    )?;
    let key = snapshot
        .unroll_key
        .as_deref()
        .ok_or("The snapshot has no unroll key.")?
        .parse()
        .map_err(|_| "Invalid unroll key.")?;
    let state = TreeState {
        output: TreeOutput {
            root_hash: tree.root().map_err(|_| "Invalid tree root.")?.hash,
            unroll_key: key,
        },
        tree,
        outpoint,
    };
    let package_bytes = unhex(
        snapshot
            .unroll
            .as_deref()
            .ok_or("The snapshot has no escape package.")?,
        super::mirror::MIRROR_LIMIT as usize,
    )?;
    let package = UnrollPackage::decode(&package_bytes).map_err(|_| "Invalid escape package.")?;
    let secp = Secp256k1::verification_only();
    let leaf = state
        .tree
        .get(member)
        .ok_or("The selected member has no leaf.")?;
    verify_branch(
        &secp,
        &params,
        state.output,
        outpoint,
        state.amount(),
        &package,
        leaf,
    )
    .map_err(|_| "The snapshot's escape package does not pay this member exactly.")?;
    let coin: TxOut = bitcoin::consensus::deserialize(&unhex(
        snapshot
            .output
            .as_deref()
            .ok_or("The snapshot has no output bytes.")?,
        1024,
    )?)
    .map_err(|_| "Invalid snapshot output bytes.")?;
    if coin != state.txout(&secp, &params) {
        return Err("The snapshot's output bytes do not match its tree.".into());
    }
    let mut log = Writer::new(network_value);
    log.push(Some(height), Body::tree_open(&params, &state));
    std::fs::create_dir(output).map_err(|_| "Choose a new bootstrap output directory.")?;
    atomic_file(&output.join("creation.jsonl"), log.to_jsonl().as_bytes())?;
    atomic_file(&output.join("initial-unroll.bin"), &package_bytes)?;
    let config = json!({"schema":"layertree.follower.v1","network":network,"rpc":rpc_url,"tree_open":"creation.jsonl","operator_key":hex(&params.operator_key),"unroll_key":key.to_string(),"unroll":"initial-unroll.bin","member":member,"sources":sources_value,"state":"state","webhook_env":webhook_env,"poll_seconds":10,"missing_after":3});
    atomic_file(
        &output.join("follower.json"),
        &serde_json::to_vec_pretty(&config)
            .map_err(|_| "Could not encode follower configuration.")?,
    )?;
    Ok(())
}
