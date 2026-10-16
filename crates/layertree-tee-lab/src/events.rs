//! `layertree.events.v1`: one JSON Lines record of a whole run, written by
//! the lab and the regtest/signet harnesses, read back by the light client,
//! and replayed by the Verse scenes.
//!
//! Every line has `seq` (contiguous from 0), `height` (a block height, or
//! null off chain), `network` (`lab`, `regtest` or `signet`, the same on
//! every line) and `kind`, plus that kind's fields. [`read`] replays a log:
//! it rebuilds the tree from `tree_open`, applies every `transition`'s
//! changes against the stated paths and roots, checks conservation of
//! value per operation and across payouts, and refuses unknown kinds and
//! fields.
use crate::attest::{AttestError, Platform, Report};
use crate::light::LightError;
use crate::rules::{Op, Record, Reject, TreeParams, TreeState};
use crate::tree::{Leaf, Node, PathStep, Tree, fold};
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FORMAT: &str = "layertree.events.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Lab,
    Regtest,
    Signet,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex<const N: usize>(s: &str) -> Result<[u8; N], EventError> {
    if s.len() != 2 * N {
        return Err(EventError::Field("a hex field has the wrong length"));
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|_| EventError::Field("a hex field is not hex"))?;
    }
    Ok(out)
}

/// A Merkle-sum node: `{hash, sum}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeJson {
    pub hash: String,
    pub sum: u64,
}

impl From<Node> for NodeJson {
    fn from(n: Node) -> Self {
        Self {
            hash: hex(&n.hash),
            sum: n.sum,
        }
    }
}

impl NodeJson {
    fn node(&self) -> Result<Node, EventError> {
        Ok(Node {
            hash: unhex(&self.hash)?,
            sum: self.sum,
        })
    }
}

/// A 116-byte leaf, field by field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeafJson {
    pub amount: u64,
    pub exit_key: String,
    pub owner_key: String,
    pub evictor_key: String,
    pub evict_after: u32,
    pub nonce: u32,
    pub seq: u32,
}

impl From<&Leaf> for LeafJson {
    fn from(l: &Leaf) -> Self {
        Self {
            amount: l.amount,
            exit_key: hex(&l.exit_key),
            owner_key: hex(&l.owner_key),
            evictor_key: hex(&l.evictor_key),
            evict_after: l.evict_after,
            nonce: l.nonce,
            seq: l.seq,
        }
    }
}

impl LeafJson {
    fn leaf(&self) -> Result<Leaf, EventError> {
        Ok(Leaf {
            amount: self.amount,
            exit_key: unhex(&self.exit_key)?,
            owner_key: unhex(&self.owner_key)?,
            evictor_key: unhex(&self.evictor_key)?,
            evict_after: self.evict_after,
            nonce: self.nonce,
            seq: self.seq,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacedLeaf {
    pub pos: u64,
    pub leaf: LeafJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepJson {
    pub right: bool,
    pub sibling: NodeJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeJson {
    pub pos: u64,
    pub old: Option<LeafJson>,
    pub new: Option<LeafJson>,
    pub path: Vec<StepJson>,
}

/// The transition kinds of the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Exit,
    Evict,
    Join,
    Update,
    Claim,
    Refresh,
    Migrate,
}

impl From<&Op> for OpKind {
    fn from(op: &Op) -> Self {
        match op {
            Op::Exit { evict: false, .. } => Self::Exit,
            Op::Exit { evict: true, .. } => Self::Evict,
            Op::Join { .. } => Self::Join,
            Op::Update { .. } => Self::Update,
            Op::Claim { .. } => Self::Claim,
            Op::Refresh => Self::Refresh,
            Op::Migrate { .. } => Self::Migrate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogEntryJson {
    pub measurement: String,
    pub source_commit: String,
}

/// What happened, by kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    TreeOpen {
        tree_id: String,
        depth: u32,
        dust_min: u64,
        /// Δ, the unroll's CSV delay in blocks.
        unroll_delay: u16,
        enclave_key: String,
        outpoint: String,
        leaves: Vec<PlacedLeaf>,
        root: NodeJson,
    },
    Transition {
        op: OpKind,
        prev: String,
        changes: Vec<ChangeJson>,
        old_root: NodeJson,
        new_root: NodeJson,
        txid: String,
        record_hash: String,
        next_unroll_key: Option<String>,
    },
    Rejected {
        op: OpKind,
        reject: String,
    },
    Attestation {
        platform: String,
        measurement: String,
        log_entry: Option<LogEntryJson>,
        result: String,
    },
    EnclaveDown {},
    Block {
        spent: Vec<String>,
    },
    UnrollBroadcast {
        level: u32,
        txid: String,
        cpfp_fee: u64,
    },
    UnrollConfirmed {
        level: u32,
        txid: String,
    },
    Payout {
        exit_key: String,
        amount: u64,
    },
    LightClient {
        member: u64,
        result: String,
    },
}

/// One line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub seq: u64,
    pub height: Option<u32>,
    pub network: Network,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    /// A line is not a JSON object, or a field is missing or unknown.
    Malformed(usize, String),
    /// A field's value is out of shape.
    Field(&'static str),
    /// `seq` is not contiguous from 0.
    Sequence(usize),
    /// Lines name different networks.
    Network(usize),
    /// The first line is not `tree_open`, or a second one appears.
    Open(usize),
    /// A stated root, path or old leaf differs from the replayed tree.
    Replay(usize, &'static str),
    /// Value was created or destroyed.
    Conservation(usize, &'static str),
}

fn outpoint(o: &OutPoint) -> String {
    format!("{}:{}", o.txid, o.vout)
}

impl Body {
    /// `tree_open` for a freshly opened tree.
    #[must_use]
    pub fn tree_open(p: &TreeParams, state: &TreeState) -> Self {
        Self::TreeOpen {
            tree_id: hex(&p.tree_id),
            depth: p.depth,
            dust_min: p.dust_min,
            unroll_delay: p.unroll_delay,
            enclave_key: hex(&p.enclave_key.serialize()),
            outpoint: outpoint(&state.outpoint),
            leaves: state
                .tree
                .leaves()
                .iter()
                .map(|(pos, l)| PlacedLeaf {
                    pos: *pos,
                    leaf: l.into(),
                })
                .collect(),
            root: state
                .tree
                .root()
                .expect("an open tree is consistent")
                .into(),
        }
    }

    /// `transition` for a signed spend of `old`.
    #[must_use]
    pub fn transition(op: &Op, old: &TreeState, record: &Record, txid: &bitcoin::Txid) -> Self {
        Self::Transition {
            op: op.into(),
            prev: outpoint(&record.prev),
            changes: record
                .changes
                .iter()
                .map(|c| ChangeJson {
                    pos: c.pos,
                    old: c.old.as_ref().map(Into::into),
                    new: c.new.as_ref().map(Into::into),
                    path: c
                        .path
                        .iter()
                        .map(|s| StepJson {
                            right: s.right,
                            sibling: s.sibling.into(),
                        })
                        .collect(),
                })
                .collect(),
            old_root: old.tree.root().expect("consistent").into(),
            new_root: record.new_root.into(),
            txid: txid.to_string(),
            record_hash: hex(&record.hash()),
            next_unroll_key: record.next_unroll_key.map(|k| hex(&k.serialize())),
        }
    }

    #[must_use]
    pub fn rejected(op: &Op, reject: &Reject) -> Self {
        let name = match reject {
            Reject::Tree(e) => format!("Tree({e:?})"),
            other => format!("{other:?}"),
        };
        Self::Rejected {
            op: op.into(),
            reject: name,
        }
    }

    #[must_use]
    pub fn attestation(
        report: &Report,
        entry: Option<&crate::attest::LogEntry>,
        result: Result<&str, AttestError>,
    ) -> Self {
        Self::Attestation {
            platform: match report.platform {
                Platform::SevSnp => "sev-snp".into(),
                Platform::Tdx => "tdx".into(),
            },
            measurement: hex(&report.measurement),
            log_entry: entry.map(|e| LogEntryJson {
                measurement: hex(&e.measurement),
                source_commit: e.source_commit.clone(),
            }),
            result: match result {
                Ok(_) => "attested".into(),
                Err(e) => format!("{e:?}"),
            },
        }
    }

    #[must_use]
    pub fn light_client(member: u64, result: &Result<(), LightError>) -> Self {
        Self::LightClient {
            member,
            result: match result {
                Ok(()) => "ok".into(),
                Err(e) => format!("{e:?}"),
            },
        }
    }

    #[must_use]
    pub fn payout(exit_key: &[u8; 32], amount: u64) -> Self {
        Self::Payout {
            exit_key: hex(exit_key),
            amount,
        }
    }
}

/// Appends events with contiguous `seq` on one network.
#[derive(Debug, Clone)]
pub struct Writer {
    network: Network,
    events: Vec<Event>,
}

impl Writer {
    #[must_use]
    pub fn new(network: Network) -> Self {
        Self {
            network,
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, height: Option<u32>, body: Body) {
        let seq = self.events.len() as u64;
        self.events.push(Event {
            seq,
            height,
            network: self.network,
            body,
        });
    }

    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The log as JSON Lines, one event per line, ending in a newline.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        self.events.iter().map(|e| line(e) + "\n").collect()
    }
}

/// One event as its JSON line: `seq`, `height`, `network` and `kind` first,
/// then the kind's fields in their documented order.
#[must_use]
pub fn line(e: &Event) -> String {
    let body = serde_json::to_string(&e.body).expect("body");
    let height = e.height.map_or("null".to_string(), |h| h.to_string());
    let network = serde_json::to_string(&e.network).expect("network");
    format!(
        "{{\"seq\":{},\"height\":{height},\"network\":{network},{}",
        e.seq,
        &body[1..]
    )
}

/// What a valid log replays to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub events: Vec<Event>,
    /// The root after `tree_open` and after each transition, in order.
    pub roots: Vec<Node>,
    /// The tree after the last transition.
    pub tree: Tree,
    pub paid: u64,
    pub enclave_down: bool,
}

fn parse_line(n: usize, text: &str) -> Result<Event, EventError> {
    let bad = |why: String| EventError::Malformed(n, why);
    let mut map = match serde_json::from_str::<Value>(text).map_err(|e| bad(e.to_string()))? {
        Value::Object(m) => m,
        _ => return Err(bad("not an object".into())),
    };
    let seq = map
        .remove("seq")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| bad("seq".into()))?;
    let height = match map.remove("height") {
        Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .and_then(|h| u32::try_from(h).ok())
                .ok_or_else(|| bad("height".into()))?,
        ),
        None => return Err(bad("height".into())),
    };
    let network: Network =
        serde_json::from_value(map.remove("network").ok_or_else(|| bad("network".into()))?)
            .map_err(|e| bad(e.to_string()))?;
    let body: Body = serde_json::from_value(Value::Object(map)).map_err(|e| bad(e.to_string()))?;
    Ok(Event {
        seq,
        height,
        network,
        body,
    })
}

fn apply(n: usize, tree: &mut Tree, changes: &[ChangeJson]) -> Result<(u64, u64), EventError> {
    let (mut removed, mut added) = (0u64, 0u64);
    for c in changes {
        let old = c.old.as_ref().map(LeafJson::leaf).transpose()?;
        let new = c.new.as_ref().map(LeafJson::leaf).transpose()?;
        if tree.get(c.pos).copied() != old {
            return Err(EventError::Replay(
                n,
                "a change's old leaf is not the tree's",
            ));
        }
        let path: Vec<PathStep> = c
            .path
            .iter()
            .map(|s| {
                Ok(PathStep {
                    right: s.right,
                    sibling: s.sibling.node()?,
                })
            })
            .collect::<Result<_, EventError>>()?;
        let before = old.map(|l| l.node()).unwrap_or(Node::EMPTY);
        if fold(before, &path).ok() != tree.root().ok() {
            return Err(EventError::Replay(
                n,
                "a change's path does not lead to the current root",
            ));
        }
        match (old, new) {
            (None, Some(l)) => tree.insert(c.pos, l),
            (Some(_), l) => tree.set(c.pos, l),
            (None, None) => return Err(EventError::Replay(n, "a change changes nothing")),
        }
        .map_err(|_| EventError::Replay(n, "a change does not apply to the tree"))?;
        removed += old.map_or(0, |l| l.amount);
        added += new.map_or(0, |l| l.amount);
    }
    Ok((removed, added))
}

/// Read and replay a log.
///
/// # Errors
///
/// On the first malformed line, broken sequence, replay mismatch, or
/// conservation failure.
pub fn read(text: &str) -> Result<Replay, EventError> {
    let mut events = Vec::new();
    for (n, l) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        events.push(parse_line(n, l)?);
    }
    let mut tree: Option<Tree> = None;
    let mut roots = Vec::new();
    let mut network = None;
    // Value owed to members outside the tree (exited leaves, or every leaf
    // once the enclave is down and the unroll runs), by exit key.
    let mut owed: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let (mut created, mut paid, mut enclave_down, mut unrolled) = (0u64, 0u64, false, false);
    for (n, e) in events.iter().enumerate() {
        if e.seq != n as u64 {
            return Err(EventError::Sequence(n));
        }
        if *network.get_or_insert(e.network) != e.network {
            return Err(EventError::Network(n));
        }
        match (&e.body, tree.as_mut()) {
            (
                Body::TreeOpen {
                    depth,
                    leaves,
                    root,
                    tree_id,
                    enclave_key,
                    ..
                },
                None,
            ) => {
                unhex::<32>(tree_id)?;
                unhex::<32>(enclave_key)?;
                let mut t = Tree::new(*depth).map_err(|_| EventError::Field("depth"))?;
                for p in leaves {
                    t.insert(p.pos, p.leaf.leaf()?)
                        .map_err(|_| EventError::Replay(n, "an opening leaf does not fit"))?;
                }
                let r = t
                    .root()
                    .map_err(|_| EventError::Replay(n, "the opening tree overflows"))?;
                if r != root.node()? {
                    return Err(EventError::Replay(
                        n,
                        "the opening root is not the leaves' root",
                    ));
                }
                created = r.sum;
                roots.push(r);
                tree = Some(t);
            }
            (Body::TreeOpen { .. }, Some(_)) | (_, None) => return Err(EventError::Open(n)),
            (
                Body::Transition {
                    op,
                    changes,
                    old_root,
                    new_root,
                    txid,
                    record_hash,
                    ..
                },
                Some(t),
            ) => {
                if enclave_down {
                    return Err(EventError::Replay(
                        n,
                        "a transition after the enclave went down",
                    ));
                }
                unhex::<32>(txid)?;
                unhex::<32>(record_hash)?;
                if t.root().ok() != Some(old_root.node()?) {
                    return Err(EventError::Replay(
                        n,
                        "the old root is not the replayed root",
                    ));
                }
                let (removed, added) = apply(n, t, changes)?;
                let r = t
                    .root()
                    .map_err(|_| EventError::Replay(n, "the tree overflows"))?;
                if r != new_root.node()? {
                    return Err(EventError::Replay(
                        n,
                        "the new root is not the replayed root",
                    ));
                }
                let preserved = removed == added;
                let ok = match op {
                    OpKind::Join => removed == 0 && added > 0,
                    OpKind::Exit | OpKind::Evict => added == 0 && changes.len() == 1 && removed > 0,
                    OpKind::Update | OpKind::Claim | OpKind::Refresh | OpKind::Migrate => preserved,
                };
                if !ok {
                    return Err(EventError::Conservation(
                        n,
                        "the transition creates or destroys value for its kind",
                    ));
                }
                if matches!(op, OpKind::Exit | OpKind::Evict) {
                    for c in changes {
                        if let Some(old) = &c.old {
                            owed.entry(old.exit_key.clone())
                                .or_default()
                                .push(old.amount);
                        }
                    }
                }
                if *op == OpKind::Join {
                    created += added;
                }
                roots.push(r);
            }
            (Body::EnclaveDown {}, Some(_)) => enclave_down = true,
            (Body::UnrollBroadcast { txid, .. } | Body::UnrollConfirmed { txid, .. }, Some(t)) => {
                unhex::<32>(txid)?;
                if !enclave_down {
                    return Err(EventError::Replay(n, "an unroll while the enclave is up"));
                }
                if matches!(e.body, Body::UnrollConfirmed { .. }) && !unrolled {
                    // The whole remaining tree is now owed to its members.
                    for l in t.leaves().values() {
                        owed.entry(hex(&l.exit_key)).or_default().push(l.amount);
                    }
                    unrolled = true;
                }
            }
            (Body::Payout { exit_key, amount }, Some(_)) => {
                let due = owed
                    .get_mut(exit_key)
                    .and_then(|v| v.iter().position(|a| a == amount).map(|i| v.remove(i)));
                if due.is_none() {
                    return Err(EventError::Conservation(
                        n,
                        "a payout no exit or unroll owes",
                    ));
                }
                paid += amount;
            }
            (
                Body::Rejected { .. }
                | Body::Attestation { .. }
                | Body::Block { .. }
                | Body::LightClient { .. },
                Some(_),
            ) => {}
        }
    }
    let tree = tree.ok_or(EventError::Open(0))?;
    // Conservation across the run: everything ever in the tree is still in
    // it, owed outside it, or paid.
    let in_tree = if unrolled { 0 } else { tree.total() };
    let outstanding: u64 = owed.values().flatten().sum();
    if in_tree + outstanding + paid != created {
        return Err(EventError::Conservation(
            events.len(),
            "value in the tree, owed and paid does not add up",
        ));
    }
    Ok(Replay {
        events,
        roots,
        tree,
        paid,
        enclave_down,
    })
}
