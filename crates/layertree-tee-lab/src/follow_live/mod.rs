//! Opt-in research follower: no wallet, signing endpoint, or mainnet mode.
pub mod bootstrap;
pub mod chain;
mod inline_file;
mod journal;
pub mod mirror;
pub mod publish;
use crate::{
    events::{self, Body, Network},
    follow::{ChainSource, Follower, PollError},
    light::Member,
    output::TreeOutput,
    rules::TreeParams,
    tree::tagged_hash,
    unroll::{UnrollPackage, verify_branch},
};
use bitcoin::{
    OutPoint,
    secp256k1::{Secp256k1, XOnlyPublicKey},
};
use chain::RpcChain;
use journal::Journal;
use mirror::{Mirrors, Sources};
use serde::Deserialize;
use serde_json::json;
use std::{
    io::{BufRead, Read},
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(text: &str, limit: usize) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) || text.len() / 2 > limit {
        return Err("Hex data has an invalid length.".into());
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let s = std::str::from_utf8(pair).map_err(|_| "Invalid hex data.")?;
            u8::from_str_radix(s, 16).map_err(|_| "Invalid hex data.".into())
        })
        .collect()
}
pub fn atomic_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|_| "Could not create the follower's cache directory.")?;
    let temp = path.with_extension("new");
    let mut file =
        std::fs::File::create(&temp).map_err(|_| "Could not create the follower's cache file.")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Could not save the follower's cache file.")?;
    std::fs::rename(&temp, path).map_err(|_| "Could not update the follower's cache file.")?;
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Could not save the follower's cache directory.".into())
}
fn key(text: &str) -> Result<[u8; 32], String> {
    unhex(text, 32)?
        .try_into()
        .map_err(|_| "Choose a 32-byte key.".into())
}
pub fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "The clock is before Unix time.".into())
}
fn interval() -> u64 {
    10
}
fn missing() -> u32 {
    3
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub network: String,
    pub rpc: String,
    pub tree_open: PathBuf,
    pub operator_key: String,
    pub unroll_key: String,
    pub migration_m: Option<String>,
    pub unroll: PathBuf,
    pub member: u64,
    pub sources: Sources,
    pub state: PathBuf,
    pub webhook_env: Option<String>,
    #[serde(default = "interval")]
    pub poll_seconds: u64,
    #[serde(default = "missing")]
    pub missing_after: u32,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "Could not open the follower configuration.")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read the follower configuration.")?;
        if bytes.len() > 1024 * 1024 {
            return Err("The follower configuration exceeds 1 MiB.".into());
        }
        let mut cfg: Self =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid follower configuration.")?;
        if cfg.schema != "layertree.follower.v1"
            || !matches!(cfg.network.as_str(), "regtest" | "signet")
            || !(5..=60).contains(&cfg.poll_seconds)
            || !(1..=144).contains(&cfg.missing_after)
        {
            return Err("Choose a signet or regtest follower, a 5–60 second poll interval, and a 1–144 block missing-record deadline.".into());
        }
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        for p in [&mut cfg.tree_open, &mut cfg.unroll, &mut cfg.state] {
            if p.is_relative() {
                *p = base.join(&*p);
            }
        }
        for p in &mut cfg.sources.directories {
            if p.is_relative() {
                *p = base.join(&*p);
            }
        }
        Ok(cfg)
    }
    fn bootstrap(&self, chain: &mut RpcChain) -> Result<(Follower, String), String> {
        let file = std::fs::File::open(&self.tree_open)
            .map_err(|_| "Could not open the tree's creation log.")?;
        let mut reader = std::io::BufReader::new(file.take(1024 * 1024 + 1));
        let mut first = String::new();
        reader
            .read_line(&mut first)
            .map_err(|_| "Could not read the tree's creation event.")?;
        if first.len() > 1024 * 1024 {
            return Err("The tree's creation event exceeds 1 MiB.".into());
        }
        let replay = events::read(&first).map_err(|_| "The tree's creation event is invalid.")?;
        let event = &replay.events[0];
        if !matches!(
            (event.network, self.network.as_str()),
            (Network::Signet, "signet") | (Network::Regtest, "regtest")
        ) {
            return Err("The tree's creation log is on a different network.".into());
        }
        let Body::TreeOpen {
            tree_id,
            depth,
            dust_min,
            unroll_delay,
            enclave_key,
            outpoint,
            ..
        } = &event.body
        else {
            return Err("The log must begin with the tree's creation event.".into());
        };
        let params = TreeParams {
            enclave_key: XOnlyPublicKey::from_str(enclave_key)
                .map_err(|_| "Invalid enclave public key.")?,
            operator_key: key(&self.operator_key)?,
            tree_id: key(tree_id)?,
            depth: *depth,
            dust_min: *dust_min,
            unroll_delay: *unroll_delay,
            migration_m: self.migration_m.as_deref().map(key).transpose()?,
        };
        let outpoint = OutPoint::from_str(outpoint).map_err(|_| "Invalid tree outpoint.")?;
        let unroll_key =
            XOnlyPublicKey::from_str(&self.unroll_key).map_err(|_| "Invalid unroll public key.")?;
        let root = replay
            .tree
            .root()
            .map_err(|_| "The tree root could not be computed.")?;
        let leaf = replay
            .tree
            .get(self.member)
            .copied()
            .ok_or("The selected member has no leaf in the tree.")?;
        let mut bytes = Vec::new();
        std::fs::File::open(&self.unroll)
            .map_err(|_| "Could not open the initial unroll package.")?
            .take(mirror::MIRROR_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read the initial unroll package.")?;
        if bytes.len() as u64 > mirror::MIRROR_LIMIT {
            return Err("The initial unroll exceeds the research follower's size limit.".into());
        }
        let package =
            UnrollPackage::decode(&bytes).map_err(|_| "Invalid initial unroll package.")?;
        let output = TreeOutput {
            root_hash: root.hash,
            unroll_key,
        };
        let secp = Secp256k1::verification_only();
        verify_branch(&secp, &params, output, outpoint, root.sum, &package, &leaf)
            .map_err(|_| "The initial unroll does not pay this member exactly.")?;
        let height = event
            .height
            .ok_or("The tree's creation event needs its confirmation height.")?;
        let hash = chain.block_hash(height)?;
        let block = chain.block(&hash)?;
        let coin = block
            .txdata
            .iter()
            .find(|tx| tx.compute_txid() == outpoint.txid)
            .and_then(|tx| tx.output.get(outpoint.vout as usize))
            .ok_or("The tree output is not in its stated confirmation block.")?;
        if coin.value.to_sat() != root.sum
            || coin.script_pubkey
                != output.script_pubkey(&secp, &params.enclave_key, params.unroll_delay)
        {
            return Err("The creation event does not match the tree output on chain.".into());
        }
        let member = Member {
            pos: self.member,
            leaf: Some(leaf),
            path: replay
                .tree
                .path(self.member, 0)
                .map_err(|_| "The member's initial path is invalid.")?,
            root,
            outpoint,
            unroll_key,
        };
        Ok((
            Follower::new(member, params, height, self.missing_after, Some(package)),
            tree_id.clone(),
        ))
    }
}

pub fn run(cfg: Config, once: bool, until: Option<u64>) -> Result<u8, String> {
    if once && until.is_some() {
        return Err("Choose either --once or --until.".into());
    }
    if !once && until.is_none() {
        return Err("Choose --once or an end time with --until UNIX_SECONDS.".into());
    }
    if until.is_some_and(|end| end <= now().unwrap_or(0)) {
        return Err("Choose an end time in the future.".into());
    }
    let mut chain = RpcChain::new(&cfg.rpc, &cfg.network)?;
    let (mut follower, tree) = cfg.bootstrap(&mut chain)?;
    if cfg
        .sources
        .relays
        .iter()
        .any(|relay| relay.inline_file && relay.tree_id.as_deref() != Some(&tree))
    {
        return Err("The relay configuration names another tree.".into());
    }
    let binding = hex(&tagged_hash(
        "LayerTreeFollower/config/v1",
        &[&serde_json::to_vec(&(
            &cfg.network,
            &tree,
            cfg.member,
            follower.member.outpoint.to_string(),
            cfg.operator_key.as_str(),
            cfg.unroll_key.as_str(),
        ))
        .map_err(|_| "Invalid follower identity.")?],
    ));
    let mut journal = Journal::open(
        &cfg.state,
        &cfg.network,
        &tree,
        cfg.member,
        &binding,
        cfg.webhook_env.as_deref(),
    )?;
    if !once && !journal.has_webhook() {
        return Err(
            "Set the configured alarm webhook before starting a continuous follower.".into(),
        );
    }
    journal.event(
        "started",
        "Follower started from its checked creation output.",
    )?;
    let secp = Secp256k1::verification_only();
    let mut sources = Mirrors::new(cfg.sources.clone(), cfg.state.join("cache"))?;
    let mut saved_unroll = None;
    loop {
        journal.retry_notifications()?;
        match follower.poll(&secp, &mut chain, &mut sources) {
            Ok(seen) => {
                if sources.cache_failed {
                    return Err("Could not save the follower's records. Fix its state directory before restarting.".into());
                }
                if let Some(package) = &follower.unroll {
                    if saved_unroll != Some(package.tree_outpoint) {
                        atomic_file(
                            &cfg.state
                                .join("cache")
                                .join(crate::follow::unroll_name(&package.tree_outpoint)),
                            &package.encode(),
                        )?;
                        saved_unroll = Some(package.tree_outpoint);
                    }
                }
                for observation in &seen {
                    journal.observe(observation, follower.observed_tip)?;
                }
                journal.retry_notifications()?;
                let status = json!({"schema":"layertree.follower.status.v1","network":cfg.network,"tree_id":tree,"member":cfg.member,"checked_at":now()?,"pid":std::process::id(),"scanned":follower.scanned,"observed_tip":follower.observed_tip,"outpoint":follower.member.outpoint.to_string(),"unroll_held":follower.unroll.is_some(),"unrolling":follower.unrolling(),"exited":follower.exited,"stopped":follower.stopped.as_ref().map(|e|format!("{e:?}")),"pending_notifications":journal.pending()});
                journal.status(status)?;
                if once || follower.stopped.is_some() || follower.exited {
                    return Ok(if follower.stopped.is_some() {
                        2
                    } else if journal.pending() > 0 {
                        3
                    } else {
                        0
                    });
                }
            }
            Err(PollError::Reorganized { .. }) => {
                journal.event(
                    "chain_changed",
                    "Replaying from the checked creation output after a chain change.",
                )?;
                follower = cfg.bootstrap(&mut chain)?.0;
                if once {
                    return Err("The chain changed; retry from the creation output.".into());
                }
            }
            Err(PollError::Source(error)) => {
                journal.event("source_unavailable", &error)?;
                if once {
                    return Err(error);
                }
            }
        }
        if until.is_some_and(|end| now().is_ok_and(|t| t >= end)) {
            return Ok(if journal.pending() > 0 { 3 } else { 0 });
        }
        std::thread::sleep(Duration::from_secs(cfg.poll_seconds));
    }
}

#[cfg(test)]
mod tests;
