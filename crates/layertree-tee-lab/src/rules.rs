//! The rule predicate the enclave enforces: LayerTree's transition rules
//! (spec 7.1 to 7.5) restated for a tree output whose key path the enclave
//! signs. Each check names the spec rule the covenant script would enforce.
//!
//! `effects` computes what a transition does to the tree (no transaction,
//! no signatures). `check` is the whole predicate: the effects, the shape
//! of the spending transaction, and every signature.

use bitcoin::hashes::Hash;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::{Message, Secp256k1, Verification, schnorr::Signature};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::{Amount, OutPoint, ScriptBuf, Transaction, TxOut, absolute::LockTime};

use crate::output::{TreeOutput, ccv_tree_spk, p2tr_raw};
use crate::tree::{Leaf, Node, PathStep, SEQ_FINAL, Tree, TreeError, tagged_hash, voucher_msg};

/// Tag of the transition record hash carried in an `OP_RETURN`.
pub const TAG_RECORD: &str = "LayerTreeTEE/record";

/// Creation parameters, fixed for the tree's life. They are part of what
/// the enclave's attestation binds (see `attest`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeParams {
    /// `Q`, the enclave's key (one enclave in this lab; an aggregate key of
    /// several enclaves in the spec).
    pub enclave_key: XOnlyPublicKey,
    /// `OPERATOR_KEY` (spec 6.2).
    pub operator_key: [u8; 32],
    /// `tree_id` (spec 6.2); vouchers bind it.
    pub tree_id: [u8; 32],
    /// `D`.
    pub depth: u32,
    /// `DUST_MIN` (spec 6.2).
    pub dust_min: u64,
    /// `Δ`, the CSV delay of the unilateral unroll, in blocks.
    pub unroll_delay: u16,
    /// The CCV-tier taptree root this tree may migrate to, once fixed by a
    /// code release. `None` refuses every migration.
    pub migration_m: Option<[u8; 32]>,
}

/// A tree output as the enclave tracks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeState {
    pub tree: Tree,
    pub outpoint: OutPoint,
    pub output: TreeOutput,
}

impl TreeState {
    pub fn amount(&self) -> u64 {
        self.tree
            .root()
            .expect("tree held by the enclave is consistent")
            .sum
    }

    pub fn txout<C: Verification>(&self, secp: &Secp256k1<C>, p: &TreeParams) -> TxOut {
        TxOut {
            value: Amount::from_sat(self.amount()),
            script_pubkey: self
                .output
                .script_pubkey(secp, &p.enclave_key, p.unroll_delay),
        }
    }
}

/// What a transition does. Signatures over the transaction travel apart
/// from it (`check`'s `sigs`), in the order `signers` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// `EXIT_1^=` (spec 7.2, exact payout): the owner, or the evictor after
    /// the lease, removes the leaf; output 1 pays it in full.
    Exit { pos: u64, evict: bool },
    /// `JOIN_h` (spec 7.3), with one strengthening: the enclave sees every
    /// grafted leaf, not just the subtree's hash and sum, so a graft cannot
    /// freeze value behind a node with no real leaves.
    Join {
        height: u32,
        index: u64,
        leaves: Vec<(u64, Leaf)>,
    },
    /// `UPDATE` (spec 7.4): owner and operator replace a leaf.
    Update { pos: u64, new_leaf: Leaf },
    /// `CLAIM` (spec 7.5): redeem a voucher into an empty position.
    Claim {
        source_pos: u64,
        claim_pos: u64,
        claim_leaf: Leaf,
        voucher_sig: Signature,
    },
    /// No tree change; a fresh unroll key. Keeps the unilateral window shut
    /// while the enclave is alive (the heartbeat).
    Refresh,
    /// Move the same root and amount into the CCV-tier output once real
    /// covenants exist: the rotation of the spec's migration section.
    Migrate { m: [u8; 32] },
}

/// Why the enclave refused to sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    Tree(TreeError),
    /// Input 0 does not spend the current tree output (spec 7.1 rule 1).
    WrongInput,
    /// The supplied prevout for input 0 is not the tree output.
    PrevoutMismatch,
    /// Output 0 is not the tree output for the new root, or its amount is
    /// not the new sum (spec 7.1 rules 2 and 5).
    OutputZero,
    /// A payout output is wrong (spec 7.2 rule 2 and 6).
    Payout,
    /// No `OP_RETURN` carries the transition record's hash.
    MissingRecord,
    /// Wrong number of signatures for the transition.
    SignatureCount,
    BadSignature(&'static str),
    /// Eviction before `evict_after`, or lock time not enabled (7.2 rule 4).
    Lease,
    /// A carve wrote a leaf easier to keep than its source (7.5 rule 5).
    EvictionTerms,
    /// A carve wrote or left less than `DUST_MIN` (7.5 rule 6).
    Dust,
    /// The source leaf is at `SEQ_FINAL` (spec 5.2).
    SeqFinal,
    /// Claim larger than the source leaf.
    Underflow,
    /// A graft over a non-empty slot, or leaves outside it.
    SlotNotEmpty,
    /// A transition this tree's parameters do not allow.
    NotAllowed,
}

impl From<TreeError> for Reject {
    fn from(e: TreeError) -> Self {
        Reject::Tree(e)
    }
}

/// One leaf change, with the changed position's sibling path in the tree
/// as it was just before the change. This is what a covenant witness
/// publishes (spec 9.7) and what lets a light client follow the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub pos: u64,
    pub old: Option<Leaf>,
    pub new: Option<Leaf>,
    pub path: Vec<PathStep>,
}

impl Change {
    pub fn old_node(&self) -> Node {
        self.old.map(|l| l.node()).unwrap_or(Node::EMPTY)
    }
    pub fn new_node(&self) -> Node {
        self.new.map(|l| l.node()).unwrap_or(Node::EMPTY)
    }
}

/// The transition record the enclave publishes for every spend. Its hash
/// sits in an `OP_RETURN` of the transaction, so the chain anchors it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub prev: OutPoint,
    pub changes: Vec<Change>,
    pub new_root: Node,
    /// `K_{k+1}`; `None` for a migration or a terminal spend.
    pub next_unroll_key: Option<XOnlyPublicKey>,
}

impl Record {
    /// The record's tagged hash over its bytes (`codec`'s `Record::encode`).
    pub fn hash(&self) -> [u8; 32] {
        tagged_hash(TAG_RECORD, &[&self.encode()])
    }

    pub fn op_return(&self) -> ScriptBuf {
        let h = self.hash();
        ScriptBuf::new_op_return(<&bitcoin::script::PushBytes>::try_from(&h[..]).expect("32 bytes"))
    }
}

/// The effects of an `Op` on the tree, before any transaction exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    pub tree: Tree,
    pub changes: Vec<Change>,
    /// Required payouts, in output order from output 1.
    pub payouts: Vec<TxOut>,
    /// `Some(height)` when the transaction must have this lock time or later.
    pub min_locktime: Option<u32>,
    /// The new root is `E_D`: output 0 is free (spec 7.8).
    pub terminal: bool,
}

fn change(tree: &mut Tree, pos: u64, new: Option<Leaf>) -> Result<Change, Reject> {
    let old = tree.get(pos).copied();
    let path = tree.path(pos, 0)?;
    match (old, new) {
        (None, Some(l)) => tree.insert(pos, l)?,
        (Some(_), n) => tree.set(pos, n)?,
        (None, None) => return Err(Reject::Tree(TreeError::PositionEmpty)),
    }
    Ok(Change {
        pos,
        old,
        new,
        path,
    })
}

/// What `op` does to `state`'s tree. Checks every rule that does not need
/// the transaction or a transaction signature.
pub fn effects<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    state: &TreeState,
    op: &Op,
) -> Result<Effects, Reject> {
    let mut tree = state.tree.clone();
    let mut changes = Vec::new();
    let mut payouts = Vec::new();
    let mut min_locktime = None;
    match op {
        Op::Exit { pos, evict } => {
            let leaf = *tree.get(*pos).ok_or(TreeError::PositionEmpty)?;
            if *evict {
                min_locktime = Some(leaf.evict_after);
            }
            payouts.push(TxOut {
                value: Amount::from_sat(leaf.amount),
                script_pubkey: p2tr_raw(&leaf.exit_key),
            });
            changes.push(change(&mut tree, *pos, None)?);
        }
        Op::Join {
            height,
            index,
            leaves,
        } => {
            if leaves.is_empty() || *height > p.depth {
                return Err(Reject::NotAllowed);
            }
            if tree.subtree(*height, *index)? != tree.empty_node(*height) {
                return Err(Reject::SlotNotEmpty);
            }
            for (off, leaf) in leaves {
                if *off >= (1u64 << height) {
                    return Err(Reject::SlotNotEmpty);
                }
                leaf.validate()?;
                changes.push(change(&mut tree, (index << height) + off, Some(*leaf))?);
            }
        }
        Op::Update { pos, new_leaf } => {
            tree.get(*pos).ok_or(TreeError::PositionEmpty)?;
            new_leaf.validate()?;
            changes.push(change(&mut tree, *pos, Some(*new_leaf))?);
        }
        Op::Claim {
            source_pos,
            claim_pos,
            claim_leaf,
            voucher_sig,
        } => {
            let src = *tree.get(*source_pos).ok_or(TreeError::PositionEmpty)?;
            // Rule 1: the voucher, by the source's owner, over this tree_id.
            let msg = Message::from_digest(voucher_msg(&p.tree_id, &src, claim_leaf));
            let owner = XOnlyPublicKey::from_slice(&src.owner_key)
                .map_err(|_| Reject::BadSignature("owner key"))?;
            secp.verify_schnorr(voucher_sig, &msg, &owner)
                .map_err(|_| Reject::BadSignature("voucher"))?;
            claim_leaf.validate()?;
            // Rule 2: no underflow; seq advances and must fit.
            let residual = src
                .amount
                .checked_sub(claim_leaf.amount)
                .ok_or(Reject::Underflow)?;
            let seq = src.seq.checked_add(1).ok_or(Reject::SeqFinal)?;
            debug_assert!(src.seq != SEQ_FINAL);
            // Rule 5: nothing carved is harder to evict than its source.
            if claim_leaf.evictor_key != src.evictor_key || claim_leaf.evict_after > src.evict_after
            {
                return Err(Reject::EvictionTerms);
            }
            // Rule 6: the dust floor on both sides.
            if claim_leaf.amount < p.dust_min || (residual != 0 && residual < p.dust_min) {
                return Err(Reject::Dust);
            }
            if tree.get(*claim_pos).is_some() || claim_pos == source_pos {
                return Err(Reject::SlotNotEmpty);
            }
            let new_src = if residual == 0 {
                None
            } else {
                Some(Leaf {
                    amount: residual,
                    seq,
                    ..src
                })
            };
            changes.push(change(&mut tree, *source_pos, new_src)?);
            changes.push(change(&mut tree, *claim_pos, Some(*claim_leaf))?);
        }
        Op::Refresh => {}
        Op::Migrate { m } => {
            if p.migration_m != Some(*m) {
                return Err(Reject::NotAllowed);
            }
        }
    }
    // Only an exit may leave the tree empty (spec 7.8); output 0 is then free.
    let empty = tree.root()? == tree.empty_node(p.depth);
    let terminal = empty && matches!(op, Op::Exit { .. });
    if empty && !terminal {
        return Err(Reject::NotAllowed);
    }
    Ok(Effects {
        tree,
        changes,
        payouts,
        min_locktime,
        terminal,
    })
}

/// The keys that must sign the transaction (BIP-341 key-path sighash of
/// input 0, `SIGHASH_DEFAULT`), in order.
pub fn signers(p: &TreeParams, state: &TreeState, op: &Op) -> Result<Vec<[u8; 32]>, Reject> {
    Ok(match op {
        Op::Exit { pos, evict } => {
            let l = state.tree.get(*pos).ok_or(TreeError::PositionEmpty)?;
            vec![if *evict { l.evictor_key } else { l.owner_key }]
        }
        Op::Join { .. } | Op::Refresh | Op::Migrate { .. } => vec![p.operator_key],
        Op::Update { pos, .. } => {
            let l = state.tree.get(*pos).ok_or(TreeError::PositionEmpty)?;
            vec![l.owner_key, p.operator_key]
        }
        // A voucher needs nothing beyond itself (spec 7.5).
        Op::Claim { .. } => vec![],
    })
}

/// The digest every party signs: the BIP-341 key-path sighash of input 0.
pub fn sighash(tx: &Transaction, prevouts: &[TxOut]) -> [u8; 32] {
    SighashCache::new(tx)
        .taproot_key_spend_signature_hash(0, &Prevouts::All(prevouts), TapSighashType::Default)
        .expect("prevouts match inputs")
        .to_byte_array()
}

/// The record a transition produces.
pub fn record(state: &TreeState, fx: &Effects, op: &Op, next_unroll_key: XOnlyPublicKey) -> Record {
    let keep_going = !fx.terminal && !matches!(op, Op::Migrate { .. });
    Record {
        prev: state.outpoint,
        changes: fx.changes.clone(),
        new_root: fx.tree.root().expect("checked"),
        next_unroll_key: keep_going.then_some(next_unroll_key),
    }
}

/// The whole predicate. On success returns the effects and the record;
/// the caller (the enclave) may then sign. Every refusal is a `Reject`.
pub fn check<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    state: &TreeState,
    tx: &Transaction,
    prevouts: &[TxOut],
    op: &Op,
    sigs: &[Signature],
    next_unroll_key: XOnlyPublicKey,
) -> Result<(Effects, Record), Reject> {
    // Rule 7.1.1: input 0 spends the tree output, and nothing else does.
    if tx.input.first().map(|i| i.previous_output) != Some(state.outpoint)
        || tx
            .input
            .iter()
            .skip(1)
            .any(|i| i.previous_output == state.outpoint)
    {
        return Err(Reject::WrongInput);
    }
    if prevouts.len() != tx.input.len() || prevouts[0] != state.txout(secp, p) {
        return Err(Reject::PrevoutMismatch);
    }
    let fx = effects(secp, p, state, op)?;
    let rec = record(state, &fx, op, next_unroll_key);

    // Rules 7.1.2 and 7.1.5: output 0 under the new root, amount = new sum.
    let out0 = tx.output.first().ok_or(Reject::OutputZero)?;
    let new_root = fx.tree.root()?;
    if let Op::Migrate { m } = op {
        if out0.script_pubkey != ccv_tree_spk(secp, &new_root.hash, m)
            || out0.value.to_sat() != new_root.sum
        {
            return Err(Reject::OutputZero);
        }
    } else if !fx.terminal {
        let next = TreeOutput {
            root_hash: new_root.hash,
            unroll_key: next_unroll_key,
        };
        if out0.script_pubkey != next.script_pubkey(secp, &p.enclave_key, p.unroll_delay)
            || out0.value.to_sat() != new_root.sum
        {
            return Err(Reject::OutputZero);
        }
    }
    // Rule 7.1.6: payouts at outputs 1..K, exact.
    for (i, want) in fx.payouts.iter().enumerate() {
        if tx.output.get(i + 1) != Some(want) {
            return Err(Reject::Payout);
        }
    }
    // The record is anchored on chain.
    if !tx.output.iter().any(|o| o.script_pubkey == rec.op_return()) {
        return Err(Reject::MissingRecord);
    }
    // Rule 7.2.4: eviction waits for the lease, with lock time enabled.
    if let Some(h) = fx.min_locktime {
        let ok = match tx.lock_time {
            LockTime::Blocks(b) => b.to_consensus_u32() >= h,
            LockTime::Seconds(_) => false,
        };
        if !ok || tx.input[0].sequence.0 == u32::MAX {
            return Err(Reject::Lease);
        }
    }
    // Signatures over this exact transaction.
    let want = signers(p, state, op)?;
    if want.len() != sigs.len() {
        return Err(Reject::SignatureCount);
    }
    let msg = Message::from_digest(sighash(tx, prevouts));
    for (k, s) in want.iter().zip(sigs) {
        let key = XOnlyPublicKey::from_slice(k).map_err(|_| Reject::BadSignature("signer key"))?;
        secp.verify_schnorr(s, &msg, &key)
            .map_err(|_| Reject::BadSignature("transaction"))?;
    }
    Ok((fx, rec))
}
