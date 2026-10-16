//! Replay the public event file without depending on the excluded lab crate.

use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec3;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Leaf {
    pub amount: u64,
    pub exit_key: String,
    pub owner_key: String,
    pub evictor_key: String,
    pub evict_after: u32,
    pub nonce: u32,
    pub seq: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Root {
    pub hash: String,
    pub sum: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Placed {
    pub pos: u64,
    pub leaf: Leaf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Change {
    pub pos: u64,
    pub old: Option<Leaf>,
    pub new: Option<Leaf>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub height: Option<u32>,
    pub network: String,
    pub kind: String,
    #[serde(flatten)]
    pub data: BTreeMap<String, Value>,
}

impl Event {
    pub fn field<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, String> {
        serde_json::from_value(
            self.data
                .get(key)
                .cloned()
                .ok_or_else(|| format!("Event {} needs {key}.", self.seq))?,
        )
        .map_err(|_| format!("Event {} has an invalid {key}.", self.seq))
    }
}

pub struct Run {
    pub events: Vec<Event>,
    pub source: String,
}

impl Run {
    pub fn load(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)
            .map_err(|e| format!("Could not open {}: {e}", path.display()))?
            .take(128 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 128 * 1024 * 1024 {
            return Err("The run is larger than 128 MiB.".into());
        }
        Self::parse(&text, path.display().to_string())
    }

    pub fn parse(text: &str, source: String) -> Result<Self, String> {
        let mut events: Vec<Event> = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let event: Event = serde_json::from_str(line)
                .map_err(|_| format!("Could not read event {}.", events.len()))?;
            if event.seq != events.len() as u64 {
                return Err(format!("The run skips event {}.", events.len()));
            }
            if !matches!(event.network.as_str(), "lab" | "regtest" | "signet") {
                return Err("Choose a lab, regtest, or signet run.".into());
            }
            if events
                .first()
                .is_some_and(|first| first.network != event.network)
            {
                return Err("The run mixes Bitcoin networks.".into());
            }
            if !matches!(
                event.kind.as_str(),
                "tree_open"
                    | "transition"
                    | "simulation"
                    | "rejected"
                    | "attestation"
                    | "enclave_down"
                    | "block"
                    | "unroll_broadcast"
                    | "unroll_confirmed"
                    | "payout"
                    | "light_client"
                    | "measured"
            ) {
                return Err(format!(
                    "Event {} uses an unsupported kind: {}.",
                    event.seq, event.kind
                ));
            }
            events.push(event);
        }
        if events.is_empty() {
            return Err("The event file is empty.".into());
        }
        let run = Self { events, source };
        run.snapshot(run.events.len() as u64 - 1)?;
        Ok(run)
    }

    pub fn snapshot(&self, at: u64) -> Result<Snapshot, String> {
        if at >= self.events.len() as u64 {
            return Err(format!("The run ends at event {}.", self.events.len() - 1));
        }
        let mut state = Snapshot::default();
        for event in &self.events[..=at as usize] {
            state.apply(event)?;
        }
        if state.depth == 0 {
            return Err("Choose a run that includes a tree.".into());
        }
        Ok(state)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub watch: super::watch::Watch,
    pub seq: u64,
    pub height: Option<u32>,
    pub network: String,
    pub depth: u32,
    pub dust: u64,
    pub delay: u32,
    pub tree_id: String,
    pub enclave_key: String,
    pub root: String,
    pub output_amount: u64,
    pub outpoint: String,
    pub leaves: BTreeMap<u64, Leaf>,
    pub last_op: String,
}

fn checked_sum(leaves: &BTreeMap<u64, Leaf>) -> Result<u64, String> {
    leaves.values().try_fold(0u64, |sum, leaf| {
        sum.checked_add(leaf.amount)
            .ok_or_else(|| "The tree balance is too large.".into())
    })
}

fn valid_leaf(leaf: &Leaf) -> bool {
    leaf.amount > 0
        && leaf.amount < (1 << 51)
        && leaf.evict_after < 500_000_000
        && [&leaf.exit_key, &leaf.owner_key, &leaf.evictor_key]
            .iter()
            .all(|key| key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Snapshot {
    pub fn apply(&mut self, event: &Event) -> Result<(), String> {
        self.seq = event.seq;
        self.height = event.height.or(self.height);
        self.network.clone_from(&event.network);
        match event.kind.as_str() {
            "tree_open" => {
                if self.depth != 0 {
                    return Err("The run opens a second tree.".into());
                }
                self.depth = event.field("depth")?;
                if !(1..=16).contains(&self.depth) {
                    return Err("The viewer supports tree depths from 1 to 16.".into());
                }
                self.dust = event.field("dust_min")?;
                self.delay = event.field("unroll_delay")?;
                self.tree_id = event.field("tree_id")?;
                self.enclave_key = event.field("enclave_key")?;
                self.outpoint = event.field("outpoint")?;
                let leaves: Vec<Placed> = event.field("leaves")?;
                for placed in leaves {
                    if placed.pos >= (1 << self.depth)
                        || !valid_leaf(&placed.leaf)
                        || self.leaves.insert(placed.pos, placed.leaf).is_some()
                    {
                        return Err("The starting tree has an invalid or repeated leaf.".into());
                    }
                }
                let root: Root = event.field("root")?;
                self.accept_root(root)?;
            }
            "transition" | "simulation" => {
                let simulated = event.kind == "simulation";
                if simulated && event.data.get("synthetic") != Some(&Value::Bool(true)) {
                    return Err("Simulated operations must be marked synthetic.".into());
                }
                if self.depth == 0 {
                    return Err("A transition appears before the tree opens.".into());
                }
                let old: Root = event.field("old_root")?;
                let previous: String = event.field("prev")?;
                if old.hash != self.root
                    || old.sum != self.output_amount
                    || previous != self.outpoint
                {
                    return Err(format!(
                        "Event {} does not follow the tree's previous state.",
                        event.seq
                    ));
                }
                let op: String = event.field("op")?;
                if !(matches!(
                    op.as_str(),
                    "exit" | "evict" | "join" | "update" | "claim" | "refresh" | "migrate"
                ) || simulated
                    && matches!(op.as_str(), "split" | "merge" | "rewrite" | "detach"))
                {
                    return Err(format!("Unsupported transition: {op}."));
                }
                let changes: Vec<Change> = event.field("changes")?;
                for change in changes {
                    if change.pos >= (1 << self.depth)
                        || change.old.as_ref() != self.leaves.get(&change.pos)
                        || change.new.as_ref().is_some_and(|leaf| !valid_leaf(leaf))
                    {
                        return Err(format!(
                            "Event {} changes a leaf that does not match the tree.",
                            event.seq
                        ));
                    }
                    if let Some(leaf) = change.new {
                        self.leaves.insert(change.pos, leaf);
                    } else {
                        self.leaves.remove(&change.pos);
                    }
                }
                let root: Root = event.field("new_root")?;
                let conserved = match op.as_str() {
                    "join" => root.sum >= old.sum,
                    "exit" | "evict" | "detach" => root.sum < old.sum,
                    "update" => true,
                    _ => root.sum == old.sum,
                };
                if !conserved {
                    return Err(format!(
                        "Event {} changes the balance incorrectly.",
                        event.seq
                    ));
                }
                self.accept_root(root)?;
                let txid: String = event.field("txid")?;
                self.outpoint = format!("{txid}:0");
                self.last_op = op;
            }
            _ => {}
        }
        self.watch.apply(event)?;
        Ok(())
    }

    fn accept_root(&mut self, root: Root) -> Result<(), String> {
        if checked_sum(&self.leaves)? != root.sum {
            return Err("The leaves do not add up to the output balance.".into());
        }
        if root.hash.len() != 64 || !root.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("The tree root is invalid.".into());
        }
        self.output_amount = root.sum;
        self.root = root.hash;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub height: u32,
    pub index: u64,
    pub sum: u64,
    pub members: usize,
    pub position: Vec3,
    pub radius: f32,
}

pub struct Layout {
    pub nodes: BTreeMap<(u32, u64), Node>,
    pub depth: u32,
    pub cutoff: u32,
    pub width: f32,
}

pub fn radius(amount: u64, dust: u64) -> f32 {
    (1.0 + amount as f64 / dust.max(1) as f64)
        .log2()
        .max(1.0)
        .sqrt() as f32
        * 0.16
}

impl Layout {
    pub fn new(state: &Snapshot) -> Result<Self, String> {
        let count = state.leaves.len().max(1);
        let width = (count.min(128) as f32 * 2.1).max(10.0);
        let mut nodes: BTreeMap<(u32, u64), Node> = BTreeMap::new();
        let mut bounds: BTreeMap<(u32, u64), (f32, f32)> = BTreeMap::new();
        for (rank, (&pos, leaf)) in state.leaves.iter().enumerate() {
            let x = if count == 1 {
                0.0
            } else {
                (rank as f32 / (count - 1) as f32 - 0.5) * width
            };
            for height in 0..=state.depth {
                let index = pos >> height;
                let node = nodes.entry((height, index)).or_insert_with(|| Node {
                    height,
                    index,
                    sum: 0,
                    members: 0,
                    position: Vec3::ZERO,
                    radius: 0.16,
                });
                node.sum = node
                    .sum
                    .checked_add(leaf.amount)
                    .ok_or("The tree balance is too large.")?;
                node.members += 1;
                bounds
                    .entry((height, index))
                    .and_modify(|range| {
                        range.0 = range.0.min(x);
                        range.1 = range.1.max(x);
                    })
                    .or_insert((x, x));
            }
        }
        nodes.entry((state.depth, 0)).or_insert(Node {
            height: state.depth,
            index: 0,
            sum: 0,
            members: 0,
            position: Vec3::ZERO,
            radius: 0.3,
        });
        for (&key, node) in &mut nodes {
            let range = bounds.get(&key).copied().unwrap_or((0.0, 0.0));
            let x = (range.0 + range.1) * 0.5;
            node.position = Vec3::new(x, 2.0 + node.height as f32 * 1.55, (x / width).sin() * 0.6);
            node.radius = if node.height == 0 {
                radius(node.sum, state.dust)
            } else if node.height == state.depth {
                0.43
            } else {
                0.15
            };
        }
        if nodes[&(state.depth, 0)].sum != state.output_amount {
            return Err("The displayed total does not match the output.".into());
        }
        let cutoff = if count <= 128 {
            0
        } else {
            ((count as f32 / 128.0).log2().ceil() as u32).min(state.depth)
        };
        Ok(Self {
            nodes,
            depth: state.depth,
            cutoff,
            width,
        })
    }

    pub fn visible(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .values()
            .filter(|node| node.height >= self.cutoff)
    }
    pub fn coin(&self) -> Vec3 {
        Vec3::new(self.width * 0.5 + 4.0, 1.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str) -> Run {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../layertree-tee-lab/fixtures/events");
        Run::load(&root.join(name)).unwrap()
    }

    #[test]
    fn every_parent_sum_matches_its_children_at_every_step() {
        for name in [
            "happy.jsonl",
            "regtest.jsonl",
            "regtest-two-level.jsonl",
            "theft-caught.jsonl",
            "unroll.jsonl",
        ] {
            let run = fixture(name);
            for event in &run.events {
                let state = run.snapshot(event.seq).unwrap();
                let layout = Layout::new(&state).unwrap();
                for node in layout.nodes.values().filter(|node| node.height > 0) {
                    let sum: u64 = [0, 1]
                        .iter()
                        .map(|side| {
                            layout
                                .nodes
                                .get(&(node.height - 1, node.index * 2 + side))
                                .map_or(0, |child| child.sum)
                        })
                        .sum();
                    assert_eq!(node.sum, sum, "{name}: {}", event.seq);
                }
                assert_eq!(layout.nodes[&(state.depth, 0)].sum, state.output_amount);
            }
        }
    }

    #[test]
    fn sparse_depth_sixteen_trees_keep_empty_branches_collapsed() {
        let run = fixture("happy.jsonl");
        let mut state = run.snapshot(0).unwrap();
        let first = state.leaves.values().next().unwrap().clone();
        let last = state.leaves.values().last().unwrap().clone();
        state.depth = 16;
        state.output_amount = first.amount + last.amount;
        state.leaves = BTreeMap::from([(0, first), (65_535, last)]);
        let layout = Layout::new(&state).unwrap();
        assert_eq!(layout.nodes.len(), 33);
        assert_eq!(layout.nodes[&(16, 0)].members, 2);
        assert_eq!(layout.nodes[&(16, 0)].sum, state.output_amount);
        assert!(layout.nodes[&(0, 65_535)].radius > layout.nodes[&(0, 0)].radius);
    }

    #[test]
    fn a_forged_total_missing_event_or_bad_leaf_is_refused() {
        let run = fixture("happy.jsonl");
        let mut event = run.events[0].clone();
        event.data.get_mut("root").unwrap()["sum"] = Value::from(1);
        assert!(Snapshot::default().apply(&event).is_err());
        let mut event = run.events[0].clone();
        event.data.get_mut("leaves").unwrap()[0]["leaf"]["amount"] = Value::from(0);
        assert!(Snapshot::default().apply(&event).is_err());
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../layertree-tee-lab/fixtures/events/happy.jsonl"),
        )
        .unwrap();
        assert!(Run::parse(&text.replacen("\"seq\":1,", "\"seq\":2,", 1), "bad".into()).is_err());
        assert!(radius(60_000, 330) > radius(1_000, 330));
    }
}
