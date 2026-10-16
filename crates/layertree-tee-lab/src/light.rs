//! A light client: one member following the tree from the chain plus the
//! enclave's published records, with no full copy of the tree.
//!
//! For each spend of the tree output it checks that the spending
//! transaction anchors the record's hash, replays every change against the
//! root it already trusts (old node folds to the old root; new node folds
//! to the next), updates its own path (spec 9.7), and checks output 0 is
//! the tree output for the new root with the new sum. A rule-breaking
//! enclave can still get a transaction mined; the client sees the
//! mismatch at once and raises the alarm (bounded loss, not prevention).

use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::{Secp256k1, Verification};
use bitcoin::{OutPoint, Transaction};

use crate::output::TreeOutput;
use crate::rules::{Record, TreeParams};
use crate::tree::{Leaf, Node, PathStep, Proof, fold, maintain_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightError {
    /// The transaction does not spend the outpoint we follow.
    NotOurSpend,
    /// No `OP_RETURN` with this record's hash.
    RecordNotAnchored,
    /// A change does not fold to the root we hold: the record lies.
    OldStateMismatch,
    /// The record's new root is not what its changes produce.
    NewRootMismatch,
    /// Output 0 is not the tree output for the new root and sum: the
    /// enclave signed something the rules forbid.
    OutputMismatch,
    /// Our own proof no longer verifies.
    ProofBroken,
    /// No record for a spend of the tree output, some blocks after it
    /// confirmed (`follow::Follower`): the enclave or its mirror withholds it.
    RecordMissing,
    /// Some blocks after a transition, we still hold no unroll for the new
    /// state that pays our leaf (`follow::Follower`).
    UnrollNotHeld,
}

/// What one member keeps.
#[derive(Debug, Clone)]
pub struct Member {
    pub pos: u64,
    /// `None` once our leaf left the tree.
    pub leaf: Option<Leaf>,
    pub path: Vec<PathStep>,
    pub root: Node,
    pub outpoint: OutPoint,
    pub unroll_key: XOnlyPublicKey,
}

impl Member {
    /// Follow one spend of the tree output.
    pub fn follow<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        p: &TreeParams,
        tx: &Transaction,
        rec: &Record,
    ) -> Result<(), LightError> {
        if tx.input.first().map(|i| i.previous_output) != Some(self.outpoint)
            || rec.prev != self.outpoint
        {
            return Err(LightError::NotOurSpend);
        }
        if !tx.output.iter().any(|o| o.script_pubkey == rec.op_return()) {
            return Err(LightError::RecordNotAnchored);
        }
        let mut root = self.root;
        let mut path = self.path.clone();
        let mut leaf = self.leaf;
        for c in &rec.changes {
            if fold(c.old_node(), &c.path).ok() != Some(root) {
                return Err(LightError::OldStateMismatch);
            }
            root = fold(c.new_node(), &c.path).map_err(|_| LightError::NewRootMismatch)?;
            if c.pos == self.pos {
                leaf = c.new;
            } else {
                path = maintain_path(self.pos, &path, c.pos, c.new_node(), &c.path)
                    .map_err(|_| LightError::NewRootMismatch)?;
            }
        }
        if root != rec.new_root {
            return Err(LightError::NewRootMismatch);
        }
        if let Some(k) = rec.next_unroll_key {
            let out = TreeOutput {
                root_hash: root.hash,
                unroll_key: k,
            };
            let o0 = tx.output.first().ok_or(LightError::OutputMismatch)?;
            if o0.script_pubkey != out.script_pubkey(secp, &p.enclave_key, p.unroll_delay)
                || o0.value.to_sat() != root.sum
            {
                return Err(LightError::OutputMismatch);
            }
            self.unroll_key = k;
        }
        if let Some(l) = leaf {
            let proof = Proof {
                position: self.pos,
                leaf: l,
                path: path.clone(),
            };
            if !proof.verify(&root, p.depth) {
                return Err(LightError::ProofBroken);
            }
        }
        self.root = root;
        self.path = path;
        self.leaf = leaf;
        self.outpoint = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        Ok(())
    }
}
