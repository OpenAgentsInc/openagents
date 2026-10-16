//! The off-chain Merkle-sum tree, written from LayerTree spec sections 5.1
//! to 5.5 (v0.15.2). The byte layout and the hash tags follow the spec, so
//! a tree built here has the same root as an upstream tree with the same
//! leaves. No upstream code is copied (the repository has no licence).

use std::collections::BTreeMap;

use bitcoin::hashes::{Hash, HashEngine, sha256};
use bitcoin::secp256k1::XOnlyPublicKey;

/// Hash tags (spec 5.1).
pub const TAG_LEAF: &str = "LayerTree/leaf";
pub const TAG_NODE: &str = "LayerTree/node";
pub const TAG_STATE: &str = "LayerTree/state";
pub const TAG_VOUCHER: &str = "LayerTree/voucher";
pub const TAG_EXIT: &str = "LayerTree/exit";
pub const TAG_TREE_ID: &str = "LayerTree/tree-id";

/// Serialized leaf length (spec 5.2).
pub const LEAF_LEN: usize = 116;
/// Leaf amounts are below 2^51 sat (spec 5.2).
pub const MAX_AMOUNT: u64 = 1 << 51;
/// The RECOMMENDED `DUST_MIN` (spec 6.2): the P2TR dust threshold.
pub const DUST_MIN: u64 = 330;
/// `seq` value that makes a leaf non-claimable in the 0xC2 tiers (spec 5.2).
pub const SEQ_FINAL: u32 = u32::MAX;

/// BIP-340 tagged hash of the concatenation of `parts`.
pub fn tagged_hash(tag: &str, parts: &[&[u8]]) -> [u8; 32] {
    let t = sha256::Hash::hash(tag.as_bytes());
    let mut e = sha256::Hash::engine();
    e.input(t.as_ref());
    e.input(t.as_ref());
    for p in parts {
        e.input(p);
    }
    sha256::Hash::from_engine(e).to_byte_array()
}

/// `enc64(n) = le64(n) || 0x01` (spec 2.1, 5.3).
pub fn enc64(n: u64) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[..8].copy_from_slice(&n.to_le_bytes());
    b[8] = 1;
    b
}

/// Errors from tree arithmetic and lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    SumOverflow,
    BadDepth,
    PositionOutOfRange,
    PositionOccupied,
    PositionEmpty,
    InvalidLeaf(&'static str),
}

/// A Merkle-sum node: a hash and the sum of the amounts below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Node {
    pub hash: [u8; 32],
    pub sum: u64,
}

impl Node {
    /// `E_0`: 32 zero bytes, sum 0.
    pub const EMPTY: Node = Node {
        hash: [0u8; 32],
        sum: 0,
    };

    /// The parent of `left` and `right` (spec 5.3). The sum is recomputed
    /// here and never taken on trust; an overflow is an error.
    pub fn parent(left: &Node, right: &Node) -> Result<Node, TreeError> {
        let sum = left
            .sum
            .checked_add(right.sum)
            .ok_or(TreeError::SumOverflow)?;
        let hash = tagged_hash(
            TAG_NODE,
            &[&left.hash, &enc64(left.sum), &right.hash, &enc64(right.sum)],
        );
        Ok(Node { hash, sum })
    }

    /// `E_h`, the node over `2^h` empty positions.
    pub fn empty(height: u32) -> Node {
        let mut n = Node::EMPTY;
        for _ in 0..height {
            n = Node::parent(&n, &n).expect("zero sums cannot overflow");
        }
        n
    }
}

/// A 116-byte LayerTree leaf (spec 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Leaf {
    pub amount: u64,
    pub exit_key: [u8; 32],
    pub owner_key: [u8; 32],
    pub evictor_key: [u8; 32],
    pub evict_after: u32,
    pub nonce: u32,
    pub seq: u32,
}

impl Leaf {
    pub fn serialize(&self) -> [u8; LEAF_LEN] {
        let mut b = [0u8; LEAF_LEN];
        b[0..8].copy_from_slice(&self.amount.to_le_bytes());
        b[8..40].copy_from_slice(&self.exit_key);
        b[40..72].copy_from_slice(&self.owner_key);
        b[72..104].copy_from_slice(&self.evictor_key);
        b[104..108].copy_from_slice(&self.evict_after.to_le_bytes());
        b[108..112].copy_from_slice(&self.nonce.to_le_bytes());
        b[112..116].copy_from_slice(&self.seq.to_le_bytes());
        b
    }

    /// The leaf without its amount: the 108 bytes vouchers and exit
    /// authorisations bind (spec 5.1).
    pub fn body(&self) -> [u8; LEAF_LEN - 8] {
        let s = self.serialize();
        let mut b = [0u8; LEAF_LEN - 8];
        b.copy_from_slice(&s[8..]);
        b
    }

    pub fn hash(&self) -> [u8; 32] {
        tagged_hash(TAG_LEAF, &[&self.serialize()])
    }

    pub fn node(&self) -> Node {
        Node {
            hash: self.hash(),
            sum: self.amount,
        }
    }

    /// Spec 5.2 checks: amount in range, lease is a block height, every
    /// key is a valid x-only point.
    pub fn validate(&self) -> Result<(), TreeError> {
        self.validate_with(|key| XOnlyPublicKey::from_slice(key).is_ok())
    }

    /// Validate amounts and lease terms with an injected BIP-340 key check.
    pub fn validate_with(
        &self,
        mut valid_key: impl FnMut(&[u8; 32]) -> bool,
    ) -> Result<(), TreeError> {
        if self.amount == 0 || self.amount >= MAX_AMOUNT {
            return Err(TreeError::InvalidLeaf("amount out of range"));
        }
        if self.evict_after >= 500_000_000 {
            return Err(TreeError::InvalidLeaf("evict_after must be a height"));
        }
        for k in [&self.exit_key, &self.owner_key, &self.evictor_key] {
            if !valid_key(k) {
                return Err(TreeError::InvalidLeaf("bad x-only key"));
            }
        }
        Ok(())
    }
}

/// One level of a sibling path, bottom up (spec 5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathStep {
    /// True when the node on the path is the right child.
    pub right: bool,
    pub sibling: Node,
}

/// Fold a node up its path to the root, checking every sum.
pub fn fold(node: Node, path: &[PathStep]) -> Result<Node, TreeError> {
    let mut n = node;
    for s in path {
        n = if s.right {
            Node::parent(&s.sibling, &n)?
        } else {
            Node::parent(&n, &s.sibling)?
        };
    }
    Ok(n)
}

/// An inclusion proof for one leaf: what a third party hands a light client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub position: u64,
    pub leaf: Leaf,
    pub path: Vec<PathStep>,
}

impl Proof {
    /// True when the leaf folds to exactly `root`, hash and sum. The path's
    /// directions must also spell out `position`, so a proof cannot be
    /// replayed for another slot.
    pub fn verify(&self, root: &Node, depth: u32) -> bool {
        if self.path.len() != depth as usize {
            return false;
        }
        for (k, s) in self.path.iter().enumerate() {
            if s.right != ((self.position >> k) & 1 == 1) {
                return false;
            }
        }
        matches!(fold(self.leaf.node(), &self.path), Ok(r) if r == *root)
    }
}

/// A dense LayerTree of fixed depth (spec 5.4): positions `0..2^depth`,
/// unoccupied positions are `E_0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    depth: u32,
    leaves: BTreeMap<u64, Leaf>,
    empties: Vec<Node>,
}

impl Tree {
    pub fn new(depth: u32) -> Result<Tree, TreeError> {
        if depth == 0 || depth > 32 {
            return Err(TreeError::BadDepth);
        }
        let empties = (0..=depth).map(Node::empty).collect();
        Ok(Tree {
            depth,
            leaves: BTreeMap::new(),
            empties,
        })
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn capacity(&self) -> u64 {
        1u64 << self.depth
    }

    pub fn leaves(&self) -> &BTreeMap<u64, Leaf> {
        &self.leaves
    }

    pub fn get(&self, pos: u64) -> Option<&Leaf> {
        self.leaves.get(&pos)
    }

    pub fn empty_node(&self, height: u32) -> Node {
        self.empties[height as usize]
    }

    /// Put a leaf into an empty position.
    pub fn insert(&mut self, pos: u64, leaf: Leaf) -> Result<(), TreeError> {
        if pos >= self.capacity() {
            return Err(TreeError::PositionOutOfRange);
        }
        if self.leaves.contains_key(&pos) {
            return Err(TreeError::PositionOccupied);
        }
        self.leaves.insert(pos, leaf);
        Ok(())
    }

    /// Replace (or clear, with `None`) an occupied position.
    pub fn set(&mut self, pos: u64, leaf: Option<Leaf>) -> Result<(), TreeError> {
        if pos >= self.capacity() {
            return Err(TreeError::PositionOutOfRange);
        }
        match leaf {
            Some(l) => {
                self.leaves.insert(pos, l);
            }
            None => {
                self.leaves.remove(&pos).ok_or(TreeError::PositionEmpty)?;
            }
        }
        Ok(())
    }

    pub fn occupied(&self, height: u32, index: u64) -> Result<bool, TreeError> {
        if height > self.depth || index >= (self.capacity() >> height) {
            return Err(TreeError::PositionOutOfRange);
        }
        Ok(self
            .leaves
            .range((index << height)..((index + 1) << height))
            .next()
            .is_some())
    }

    pub fn checked_total(&self) -> Result<u64, TreeError> {
        self.leaves.values().try_fold(0u64, |sum, leaf| {
            sum.checked_add(leaf.amount).ok_or(TreeError::SumOverflow)
        })
    }

    /// The node at `(height, index)`, covering positions
    /// `[index * 2^height, (index + 1) * 2^height)`.
    pub fn subtree(&self, height: u32, index: u64) -> Result<Node, TreeError> {
        let lo = index << height;
        let hi = (index + 1) << height;
        if self.leaves.range(lo..hi).next().is_none() {
            return Ok(self.empty_node(height));
        }
        if height == 0 {
            return Ok(self.leaves[&index].node());
        }
        let l = self.subtree(height - 1, 2 * index)?;
        let r = self.subtree(height - 1, 2 * index + 1)?;
        Node::parent(&l, &r)
    }

    pub fn root(&self) -> Result<Node, TreeError> {
        self.subtree(self.depth, 0)
    }

    /// Sibling path of the node at `(height, pos >> height)`, bottom up.
    pub fn path(&self, pos: u64, height: u32) -> Result<Vec<PathStep>, TreeError> {
        let mut idx = pos >> height;
        let mut out = Vec::with_capacity((self.depth - height) as usize);
        for h in height..self.depth {
            out.push(PathStep {
                right: idx & 1 == 1,
                sibling: self.subtree(h, idx ^ 1)?,
            });
            idx >>= 1;
        }
        Ok(out)
    }

    pub fn proof(&self, pos: u64) -> Result<Proof, TreeError> {
        let leaf = *self.leaves.get(&pos).ok_or(TreeError::PositionEmpty)?;
        Ok(Proof {
            position: pos,
            leaf,
            path: self.path(pos, 0)?,
        })
    }

    /// Sum of all leaf amounts, computed directly (for cross-checks).
    pub fn total(&self) -> u64 {
        self.leaves.values().map(|l| l.amount).sum()
    }
}

/// `H_voucher(tree_id || body(source) || claim)`, what a CLAIM voucher
/// signs (spec 7.5).
pub fn voucher_msg(tree_id: &[u8; 32], source: &Leaf, claim: &Leaf) -> [u8; 32] {
    tagged_hash(TAG_VOUCHER, &[tree_id, &source.body(), &claim.serialize()])
}

/// `H_exit(tree_id || body(leaf))`, the tower exit authorisation (spec 7.2
/// rule 7).
pub fn exit_auth_msg(tree_id: &[u8; 32], leaf: &Leaf) -> [u8; 32] {
    tagged_hash(TAG_EXIT, &[tree_id, &leaf.body()])
}

/// The RECOMMENDED `tree_id` (spec 6.2) over the creation outpoint.
pub fn tree_id(txid_internal: &[u8; 32], vout: u32) -> [u8; 32] {
    tagged_hash(TAG_TREE_ID, &[txid_internal, &vout.to_le_bytes()])
}

/// Update a member's path after another position changed, using only
/// the changed position's path (from the transition record) and its new
/// node. This is spec 9.7's path maintenance: at most one sibling of the
/// member's path changes, the one at the level where the two paths meet.
pub fn maintain_path(
    my_pos: u64,
    my_path: &[PathStep],
    changed_pos: u64,
    changed_new: Node,
    changed_path: &[PathStep],
) -> Result<Vec<PathStep>, TreeError> {
    let mut out = my_path.to_vec();
    if my_pos == changed_pos {
        return Ok(out);
    }
    // The level where the two positions share a parent.
    let level = 63 - (my_pos ^ changed_pos).leading_zeros() as usize;
    // Fold the changed node up to that level: the new value of my sibling.
    let mut n = changed_new;
    for s in &changed_path[..level] {
        n = if s.right {
            Node::parent(&s.sibling, &n)?
        } else {
            Node::parent(&n, &s.sibling)?
        };
    }
    out[level].sibling = n;
    Ok(out)
}
