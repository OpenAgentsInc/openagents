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
use crate::tree::{Leaf, Node, Tree, TreeError, tagged_hash};

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

pub use crate::predicate::Reject;

/// One leaf change and its sibling path immediately before the edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub pos: u64,
    pub old: Option<Leaf>,
    pub new: Option<Leaf>,
    pub path: Vec<crate::tree::PathStep>,
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

/// Apply the pure predicate with real BIP-340 key and voucher checks.
pub fn effects<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    state: &TreeState,
    op: &Op,
) -> Result<Effects, Reject> {
    let fx = crate::predicate::effects(&Crypto(secp), &core_params(p), &state.tree, &core_op(op))?;
    bitcoin_effects(&state.tree, &fx)
}

fn bitcoin_effects(current: &Tree, fx: &crate::predicate::Effects) -> Result<Effects, Reject> {
    let mut witness_tree = current.clone();
    let mut changes = Vec::with_capacity(fx.changes.len());
    for edit in fx.changes.iter() {
        let path = witness_tree.path(edit.pos, 0)?;
        match edit.new {
            Some(leaf) => witness_tree.set(edit.pos, Some(leaf))?,
            None => witness_tree.set(edit.pos, None)?,
        }
        changes.push(Change {
            pos: edit.pos,
            old: edit.old,
            new: edit.new,
            path,
        });
    }
    // Merkle-sum construction also checks every internal addition.
    let tree = witness_tree;
    if tree.root()?.sum != fx.sum {
        return Err(Reject::OutputZero);
    }
    Ok(Effects {
        tree,
        changes,
        payouts: fx
            .payouts
            .iter()
            .map(|p| TxOut {
                value: Amount::from_sat(p.amount),
                script_pubkey: p2tr_raw(&p.exit_key),
            })
            .collect(),
        min_locktime: fx.min_locktime,
        terminal: fx.terminal,
    })
}

/// Bitcoin outputs for the effects of the pure predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    pub tree: Tree,
    pub changes: Vec<Change>,
    pub payouts: Vec<TxOut>,
    pub min_locktime: Option<u32>,
    pub terminal: bool,
}

struct Crypto<'a, C: Verification>(&'a Secp256k1<C>);
impl<C: Verification> crate::predicate::Oracle for Crypto<'_, C> {
    fn valid_key(&self, key: &[u8; 32]) -> bool {
        XOnlyPublicKey::from_slice(key).is_ok()
    }
    fn voucher(&self, key: &[u8; 32], digest: [u8; 32], signature: &[u8; 64]) -> bool {
        XOnlyPublicKey::from_slice(key).ok().is_some_and(|key| {
            self.0
                .verify_schnorr(
                    &Signature::from_slice(signature).expect("64-byte signature"),
                    &Message::from_digest(digest),
                    &key,
                )
                .is_ok()
        })
    }
}

fn core_params(p: &TreeParams) -> crate::predicate::Params {
    crate::predicate::Params {
        tree_id: p.tree_id,
        depth: p.depth,
        dust_min: p.dust_min,
        migration_m: p.migration_m,
    }
}

fn core_op(op: &Op) -> crate::predicate::Op {
    use crate::predicate::Op as Core;
    match op {
        Op::Exit { pos, evict } => Core::Exit {
            pos: *pos,
            evict: *evict,
        },
        Op::Join {
            height,
            index,
            leaves,
        } => Core::Join {
            height: *height,
            index: *index,
            leaves: leaves.clone(),
        },
        Op::Update { pos, new_leaf } => Core::Update {
            pos: *pos,
            new_leaf: *new_leaf,
        },
        Op::Claim {
            source_pos,
            claim_pos,
            claim_leaf,
            voucher_sig,
        } => Core::Claim {
            source_pos: *source_pos,
            claim_pos: *claim_pos,
            claim_leaf: *claim_leaf,
            voucher_sig: voucher_sig.serialize(),
        },
        Op::Refresh => Core::Refresh,
        Op::Migrate { m } => Core::Migrate { m: *m },
    }
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

/// Everything but the parties' signatures: what the enclave checks before
/// it hands a transaction out for signing (two-phase signing, spec 3.2).
pub fn check_unsigned<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    state: &TreeState,
    tx: &Transaction,
    prevouts: &[TxOut],
    op: &Op,
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
    let pure =
        crate::predicate::effects(&Crypto(secp), &core_params(p), &state.tree, &core_op(op))?;
    let fx = bitcoin_effects(&state.tree, &pure)?;
    let root = fx.tree.root()?;
    let rec = record(state, &fx, op, next_unroll_key);
    let expected_script = if let Op::Migrate { m } = op {
        Some(ccv_tree_spk(secp, &root.hash, m))
    } else if !pure.terminal {
        Some(
            TreeOutput {
                root_hash: root.hash,
                unroll_key: next_unroll_key,
            }
            .script_pubkey(secp, &p.enclave_key, p.unroll_delay),
        )
    } else {
        None
    };
    let outputs: Vec<_> = tx
        .output
        .iter()
        .map(|o| crate::predicate::Output {
            amount: o.value.to_sat(),
            script: o.script_pubkey.as_bytes(),
        })
        .collect();
    let expected = expected_script
        .as_ref()
        .map(|script| crate::predicate::Output {
            amount: root.sum,
            script: script.as_bytes(),
        });
    let lock_height = match tx.lock_time {
        LockTime::Blocks(b) => Some(b.to_consensus_u32()),
        LockTime::Seconds(_) => None,
    };
    crate::predicate::check_outputs(
        &pure.output_rules(),
        &outputs,
        expected,
        rec.op_return().as_bytes(),
        lock_height,
        tx.input[0].sequence.0 != u32::MAX,
    )?;
    Ok((fx, rec))
}

/// The signatures `op` needs, each over `digest` (the key-path sighash of
/// input 0 of the exact transaction), in `signers` order.
pub fn check_signatures<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    state: &TreeState,
    op: &Op,
    digest: [u8; 32],
    sigs: &[Signature],
) -> Result<(), Reject> {
    let want = signers(p, state, op)?;
    if want.len() != sigs.len() {
        return Err(Reject::SignatureCount);
    }
    let msg = Message::from_digest(digest);
    for (k, s) in want.iter().zip(sigs) {
        let key = XOnlyPublicKey::from_slice(k).map_err(|_| Reject::BadSignature("signer key"))?;
        secp.verify_schnorr(s, &msg, &key)
            .map_err(|_| Reject::BadSignature("transaction"))?;
    }
    Ok(())
}

/// The whole predicate. On success returns the effects and the record;
/// the caller (the enclave) may then sign. Every refusal is a `Reject`.
#[allow(clippy::too_many_arguments)]
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
    let out = check_unsigned(secp, p, state, tx, prevouts, op, next_unroll_key)?;
    check_signatures(secp, p, state, op, sighash(tx, prevouts), sigs)?;
    Ok(out)
}
