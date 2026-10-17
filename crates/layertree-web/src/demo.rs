//! A large, seeded, simulated `layertree.events.v1` run for the demo page.
//!
//! One run opens a depth-12 tree (4,096 slots) with a few thousand members,
//! then streams transactions: members join, move their share to a new key,
//! pay each other (a voucher claim), split their share in two, and leave
//! with a payout. Blocks arrive every few transactions, light clients check
//! them, and the signer refreshes the shared output and passes its check
//! every so often. Near the end the signer is switched off; after the
//! unroll delay the tree is broadcast in three levels and every member is
//! paid their exact balance.
//!
//! The same seed always gives the same run. The run says it is synthetic:
//! the network is `lab`, the opening event carries `synthetic: true`, and
//! splits use the viewer's `simulation` event. Keys, transaction IDs, and
//! the signer's measurement are random hex; root hashes are SHA-256
//! Merkle-sum hashes over the simulated leaves, in this module's own
//! encoding rather than the lab's. The replay in [`crate::layertree::model`]
//! checks every balance and leaf, so a broken run fails its tests.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// What one demo run looks like.
#[derive(Clone, Debug)]
pub struct Config {
    pub seed: u64,
    /// Tree depth; the tree has `2^depth` member slots.
    pub depth: u32,
    /// Members when the tree opens.
    pub members: usize,
    /// Transactions before the signer is switched off.
    pub transactions: usize,
    /// Members who also run a light client.
    pub followers: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1,
            depth: 12,
            members: 2_600,
            transactions: 420,
            followers: 16,
        }
    }
}

/// The blocks a member waits after the signer stops before the unroll can
/// be mined.
pub const UNROLL_DELAY: u32 = 12;
/// Transactions per block.
const PER_BLOCK: usize = 3;
/// Transactions between refreshes of the shared output.
const REFRESH_EVERY: usize = 36;
/// Payouts per block during the unroll.
const PAYOUTS_PER_BLOCK: usize = 160;
/// Unroll transactions per level: one root package, then its children.
const UNROLL_LEVELS: [usize; 3] = [1, 8, 64];
const DUST: u64 = 330;
const FIRST_HEIGHT: u32 = 912_000;

/// SplitMix64: small, fast, and the same on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn hex(&mut self) -> String {
        (0..4).map(|_| format!("{:016x}", self.next())).collect()
    }

    /// A balance between 2,000 and 2,000,000 sats, evenly spread in
    /// magnitude, rounded to 100 sats.
    fn amount(&mut self) -> u64 {
        let sats = 2_000f64 * 1_000f64.powf(self.unit());
        ((sats / 100.0).round() as u64).max(1) * 100
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Leaf {
    amount: u64,
    exit_key: String,
    owner_key: String,
    evictor_key: String,
    evict_after: u32,
    nonce: u32,
    seq: u32,
}

impl Leaf {
    fn json(&self) -> Value {
        json!({
            "amount": self.amount,
            "exit_key": self.exit_key,
            "owner_key": self.owner_key,
            "evictor_key": self.evictor_key,
            "evict_after": self.evict_after,
            "nonce": self.nonce,
            "seq": self.seq,
        })
    }

    fn hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"layertree-demo/leaf");
        h.update(self.amount.to_le_bytes());
        for key in [&self.exit_key, &self.owner_key, &self.evictor_key] {
            h.update(key.as_bytes());
        }
        h.update(self.evict_after.to_le_bytes());
        h.update(self.nonce.to_le_bytes());
        h.update(self.seq.to_le_bytes());
        h.finalize().into()
    }
}

/// The simulated tree: leaves by slot, and every node's hash and sum, kept
/// up to date one path at a time.
struct Tree {
    depth: u32,
    leaves: BTreeMap<u64, Leaf>,
    /// `nodes[height][index]`: (hash, sum). Empty subtrees hash to zeros.
    nodes: Vec<Vec<([u8; 32], u64)>>,
}

impl Tree {
    fn new(depth: u32) -> Self {
        let nodes = (0..=depth)
            .map(|h| vec![([0; 32], 0); 1 << (depth - h)])
            .collect();
        Self {
            depth,
            leaves: BTreeMap::new(),
            nodes,
        }
    }

    fn set(&mut self, pos: u64, leaf: Option<Leaf>) {
        self.nodes[0][pos as usize] = match &leaf {
            Some(l) => (l.hash(), l.amount),
            None => ([0; 32], 0),
        };
        match leaf {
            Some(l) => self.leaves.insert(pos, l),
            None => self.leaves.remove(&pos),
        };
        let mut index = pos as usize;
        for h in 1..=self.depth as usize {
            index /= 2;
            let (lh, ls) = self.nodes[h - 1][index * 2];
            let (rh, rs) = self.nodes[h - 1][index * 2 + 1];
            self.nodes[h][index] = if ls == 0 && rs == 0 {
                ([0; 32], 0)
            } else {
                let mut d = Sha256::new();
                d.update(b"layertree-demo/node");
                d.update(lh);
                d.update(ls.to_le_bytes());
                d.update(rh);
                d.update(rs.to_le_bytes());
                (d.finalize().into(), ls + rs)
            };
        }
    }

    fn root(&self) -> Value {
        let (hash, sum) = self.nodes[self.depth as usize][0];
        json!({ "hash": hex(&hash), "sum": sum })
    }

    fn sum(&self) -> u64 {
        self.nodes[self.depth as usize][0].1
    }

    fn free_slot(&self, rng: &mut Rng) -> Option<u64> {
        let slots = 1u64 << self.depth;
        if self.leaves.len() as u64 >= slots {
            return None;
        }
        loop {
            let pos = rng.below(slots);
            if !self.leaves.contains_key(&pos) {
                return Some(pos);
            }
        }
    }

    fn member(&self, rng: &mut Rng, skip: &[u64]) -> Option<u64> {
        if self.leaves.len() <= skip.len() {
            return None;
        }
        loop {
            let n = rng.below(self.leaves.len() as u64) as usize;
            let pos = *self.leaves.keys().nth(n)?;
            if !skip.contains(&pos) {
                return Some(pos);
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The shared output as transitions spend it.
struct Chain {
    outpoint: String,
    /// Outputs spent since the last block.
    spent: Vec<String>,
    /// The height of the newest output, which the unroll delay counts from.
    coin_height: u32,
}

impl Chain {
    fn transition(
        &mut self,
        log: &mut Log,
        tree: &mut Tree,
        rng: &mut Rng,
        op: &str,
        changes: Vec<(u64, Option<Leaf>)>,
    ) {
        let old_root = tree.root();
        let mut written = Vec::new();
        for (pos, new) in changes {
            let old = tree.leaves.get(&pos).cloned();
            written.push(json!({
                "pos": pos,
                "old": old.as_ref().map(Leaf::json),
                "new": new.as_ref().map(Leaf::json),
            }));
            tree.set(pos, new);
        }
        let txid = rng.hex();
        let mut body = json!({
            "op": op,
            "prev": self.outpoint,
            "changes": written,
            "old_root": old_root,
            "new_root": tree.root(),
            "txid": txid,
            "observed_output_amount": tree.sum(),
        });
        // A split is the viewer's illustration event, not a lab transition.
        let kind = if op == "split" {
            body["synthetic"] = true.into();
            "simulation"
        } else {
            "transition"
        };
        log.push(kind, body);
        self.spent
            .push(std::mem::replace(&mut self.outpoint, format!("{txid}:0")));
        self.coin_height = log.height;
    }
}

/// Writes events in order, numbering them.
struct Log {
    events: Vec<Value>,
    height: u32,
}

impl Log {
    fn push(&mut self, kind: &str, mut body: Value) {
        let seq = self.events.len() as u64;
        let fields = body.as_object_mut().expect("event bodies are objects");
        fields.insert("seq".into(), seq.into());
        fields.insert("height".into(), self.height.into());
        fields.insert("network".into(), "lab".into());
        fields.insert("kind".into(), kind.into());
        self.events.push(body);
    }
}

/// The same seed and configuration always give the same events.
#[must_use]
pub fn generate(config: &Config) -> Vec<Value> {
    let depth = config.depth.clamp(4, 16);
    let slots = 1usize << depth;
    let mut rng = Rng(config.seed ^ 0x6c61_7965_7274_7265);
    let mut tree = Tree::new(depth);
    let evictor = rng.hex();
    let new_leaf = |rng: &mut Rng, amount: u64| {
        let owner = rng.hex();
        Leaf {
            amount,
            exit_key: owner.clone(),
            owner_key: owner,
            evictor_key: evictor.clone(),
            evict_after: 500_000,
            nonce: 0,
            seq: 0,
        }
    };
    for _ in 0..config.members.min(slots) {
        let pos = tree.free_slot(&mut rng).expect("fewer members than slots");
        let amount = rng.amount();
        tree.set(pos, Some(new_leaf(&mut rng, amount)));
    }
    let followers: Vec<u64> = tree
        .leaves
        .keys()
        .copied()
        .step_by((tree.leaves.len() / config.followers.max(1)).max(1))
        .take(config.followers)
        .collect();
    let mut log = Log {
        events: Vec::new(),
        height: FIRST_HEIGHT,
    };
    let outpoint = format!("{}:0", rng.hex());
    log.push(
        "tree_open",
        json!({
            "synthetic": true,
            "tree_id": rng.hex(),
            "depth": depth,
            "dust_min": DUST,
            "unroll_delay": UNROLL_DELAY,
            "enclave_key": rng.hex(),
            "outpoint": outpoint,
            "observers": followers,
            "leaves": tree.leaves.iter().map(|(pos, leaf)| json!({ "pos": pos, "leaf": leaf.json() })).collect::<Vec<_>>(),
            "root": tree.root(),
        }),
    );
    let measurement = format!("sha256:{}", rng.hex());
    let release: String = rng.hex().chars().take(40).collect();
    let attest = |log: &mut Log| {
        log.push(
            "attestation",
            json!({
                "platform": "gcp-intel-tdx",
                "measurement": measurement,
                "log_entry": { "measurement": measurement, "source_commit": release },
                "result": "attested",
            }),
        );
    };
    attest(&mut log);

    let mut chain = Chain {
        outpoint,
        spent: Vec::new(),
        coin_height: log.height,
    };

    for t in 0..config.transactions {
        if t % PER_BLOCK == 0 && t > 0 {
            log.height += 1;
            log.push(
                "block",
                json!({ "spent": std::mem::take(&mut chain.spent) }),
            );
            for _ in 0..2 {
                let member = followers[rng.below(followers.len() as u64) as usize];
                log.push("light_client", json!({ "member": member, "result": "ok" }));
            }
        }
        if t % REFRESH_EVERY == REFRESH_EVERY - 1 {
            chain.transition(&mut log, &mut tree, &mut rng, "refresh", Vec::new());
            attest(&mut log);
            continue;
        }
        let full = tree.leaves.len() + 8 >= slots;
        let roll = rng.below(100);
        let op = match roll {
            0..=23 if !full => "join",
            0..=23 | 24..=41 => "update",
            42..=67 => "claim",
            68..=77 if !full => "split",
            _ => "exit",
        };
        match op {
            "join" => {
                let pos = tree.free_slot(&mut rng).expect("not full");
                let amount = rng.amount();
                let leaf = new_leaf(&mut rng, amount);
                chain.transition(
                    &mut log,
                    &mut tree,
                    &mut rng,
                    "join",
                    vec![(pos, Some(leaf))],
                );
            }
            "update" => {
                let pos = tree.member(&mut rng, &[]).expect("members");
                let mut leaf = tree.leaves[&pos].clone();
                leaf.exit_key = rng.hex();
                leaf.nonce += 1;
                chain.transition(
                    &mut log,
                    &mut tree,
                    &mut rng,
                    "update",
                    vec![(pos, Some(leaf))],
                );
            }
            "claim" | "split" => {
                let from = tree.member(&mut rng, &[]).expect("members");
                let mut source = tree.leaves[&from].clone();
                if source.amount < 4 * DUST {
                    continue;
                }
                let share = (source.amount as f64 * (0.1 + 0.4 * rng.unit())) as u64;
                let share = share.clamp(DUST, source.amount - DUST);
                source.amount -= share;
                source.seq += 1;
                let (to, target) = if op == "claim" {
                    let to = tree.member(&mut rng, &[from]).expect("two members");
                    let mut target = tree.leaves[&to].clone();
                    target.amount += share;
                    target.seq += 1;
                    (to, target)
                } else {
                    let to = tree.free_slot(&mut rng).expect("not full");
                    let mut target = source.clone();
                    target.amount = share;
                    target.exit_key = rng.hex();
                    target.seq = 0;
                    (to, target)
                };
                chain.transition(
                    &mut log,
                    &mut tree,
                    &mut rng,
                    op,
                    vec![(from, Some(source)), (to, Some(target))],
                );
            }
            _ => {
                let Some(pos) = tree.member(&mut rng, &followers) else {
                    continue;
                };
                let leaf = tree.leaves[&pos].clone();
                chain.transition(&mut log, &mut tree, &mut rng, "exit", vec![(pos, None)]);
                log.push(
                    "payout",
                    json!({ "exit_key": leaf.exit_key, "amount": leaf.amount }),
                );
            }
        }
    }

    // The signer goes dark. Members wait out the delay on the last output,
    // then broadcast the tree level by level and are paid back.
    log.height += 1;
    log.push(
        "block",
        json!({ "spent": std::mem::take(&mut chain.spent) }),
    );
    log.push("enclave_down", json!({}));
    let mature = chain.coin_height + UNROLL_DELAY;
    while log.height + 1 < mature {
        log.height += 1;
        log.push("block", json!({ "spent": [] }));
    }
    for (level, count) in UNROLL_LEVELS.iter().enumerate() {
        let txids: Vec<String> = (0..*count).map(|_| rng.hex()).collect();
        for txid in &txids {
            let fee = 300 + rng.below(900);
            log.push(
                "unroll_broadcast",
                json!({ "level": level, "txid": txid, "cpfp_fee": fee }),
            );
        }
        log.height += 1;
        log.push("block", json!({ "spent": [] }));
        for txid in &txids {
            log.push("unroll_confirmed", json!({ "level": level, "txid": txid }));
        }
    }
    let mut owed: Vec<Leaf> = tree.leaves.values().cloned().collect();
    for i in (1..owed.len()).rev() {
        owed.swap(i, rng.below(i as u64 + 1) as usize);
    }
    for (i, leaf) in owed.iter().enumerate() {
        if i > 0 && i % PAYOUTS_PER_BLOCK == 0 {
            log.height += 1;
            log.push("block", json!({ "spent": [] }));
        }
        log.push(
            "payout",
            json!({ "exit_key": leaf.exit_key, "amount": leaf.amount }),
        );
    }
    log.events
}

/// One event per line, as the viewer reads a run file.
#[must_use]
pub fn jsonl(events: &[Value]) -> String {
    let mut out = String::new();
    for event in events {
        out.push_str(&event.to_string());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layertree::model::Run;

    fn small() -> Config {
        Config {
            seed: 7,
            depth: 8,
            members: 120,
            transactions: 150,
            followers: 6,
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_run_and_another_seed_another() {
        let a = jsonl(&generate(&small()));
        let b = jsonl(&generate(&small()));
        assert_eq!(a, b);
        let c = jsonl(&generate(&Config { seed: 8, ..small() }));
        assert_ne!(a, c);
    }

    #[test]
    fn the_default_run_is_large_and_the_replay_accepts_every_event() {
        let events = generate(&Config::default());
        let text = jsonl(&events);
        let run = Run::parse(&text, "demo".into()).unwrap();
        let open = run.snapshot(0).unwrap();
        assert_eq!(open.depth, 12);
        assert_eq!(open.leaves.len(), 2_600);
        assert_eq!(open.network, "lab");
        assert_eq!(run.events[0].data["synthetic"], Value::Bool(true));

        let count = |kind: &str, op: Option<&str>| {
            run.events
                .iter()
                .filter(|e| {
                    e.kind == kind
                        && op.is_none_or(|op| e.data.get("op").and_then(Value::as_str) == Some(op))
                })
                .count()
        };
        for op in ["join", "update", "claim", "exit", "refresh"] {
            assert!(count("transition", Some(op)) > 5, "{op}");
        }
        assert!(count("simulation", Some("split")) > 5);
        assert_eq!(count("enclave_down", None), 1);
        assert_eq!(count("unroll_broadcast", None), 73);
        assert_eq!(count("unroll_confirmed", None), 73);
        assert!(count("light_client", None) > 100);
        assert!(count("attestation", None) > 5);

        // Every member at the switch-off is paid exactly, once.
        let end = run.snapshot(run.events.len() as u64 - 1).unwrap();
        assert!(end.escape.down);
        assert!(end.leaves.len() > 2_000);
        assert_eq!(end.escape.paid.len(), end.leaves.len());
        assert_eq!(end.escape.total, end.escape.expected_total);
        assert_eq!(end.escape.total, end.output_amount);
        assert!(end.escape.steps.values().all(|s| s.confirmed));
        assert!(end.watch.alarm.is_none());
        assert_eq!(end.watch.attestation.result, "attested");
        // Blocks before the switch-off spend the tracked outputs.
        let down = run.events.iter().find(|e| e.kind == "enclave_down").unwrap();
        let at_down = run.snapshot(down.seq).unwrap();
        assert!(at_down.watch.blocks.iter().any(|b| b.hit));
    }

    #[test]
    fn root_hashes_follow_the_leaves() {
        let mut tree = Tree::new(4);
        let empty = tree.root();
        let mut rng = Rng(1);
        let leaf = Leaf {
            amount: 5_000,
            exit_key: rng.hex(),
            owner_key: rng.hex(),
            evictor_key: rng.hex(),
            evict_after: 1,
            nonce: 0,
            seq: 0,
        };
        tree.set(3, Some(leaf.clone()));
        let one = tree.root();
        assert_ne!(one, empty);
        assert_eq!(one["sum"], 5_000);
        tree.set(9, Some(leaf));
        assert_eq!(tree.root()["sum"], 10_000);
        tree.set(9, None);
        assert_eq!(tree.root(), one);
        tree.set(3, None);
        assert_eq!(tree.root(), empty);
    }
}
