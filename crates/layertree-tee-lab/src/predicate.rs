//! Pure transition and output checks used by the Bitcoin adapter and Kani.
//! Cryptography stays behind `Oracle`; amounts and payout keys stay here.

use crate::tree::{Leaf, SEQ_FINAL, Tree as State, TreeError, voucher_msg};

/// The fixed parameters needed by the transition rules.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub tree_id: [u8; 32],
    pub depth: u32,
    pub dust_min: u64,
    pub migration_m: Option<[u8; 32]>,
}

/// Cryptographic facts supplied by the Bitcoin adapter.
pub trait Oracle {
    fn valid_key(&self, key: &[u8; 32]) -> bool;
    fn voucher(&self, key: &[u8; 32], digest: [u8; 32], signature: &[u8; 64]) -> bool;
}

/// An exact payout to a member's exit key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Payout {
    pub amount: u64,
    pub exit_key: [u8; 32],
}

impl Payout {
    pub fn script(&self) -> [u8; 34] {
        let mut script = [0u8; 34];
        script[0] = 0x51;
        script[1] = 0x20;
        script[2..].copy_from_slice(&self.exit_key);
        script
    }
}

/// An operation expressed without Bitcoin transaction or signature types.
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
        voucher_sig: [u8; 64],
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
    /// A non-exit transition while a member's exit request has waited
    /// longer than the deadline (forced inclusion, spec 2.2 item 7).
    ExitCensored,
    /// The owner signed `HALT`: only exits are signed now (spec 5).
    Halted,
    /// Fewer than `m` of the `M` mirrors returned a valid receipt for the
    /// record and unroll (spec 4.1).
    MissingReceipts,
}

impl From<TreeError> for Reject {
    fn from(e: TreeError) -> Self {
        Reject::Tree(e)
    }
}

/// A leaf edit. The Bitcoin adapter adds the Merkle witness after the
/// pure rules accept it; witnesses never determine the edited amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edit {
    pub pos: u64,
    pub old: Option<Leaf>,
    pub new: Option<Leaf>,
}

/// The effects of an `Op` on the tree, before any transaction exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    pub sum: u64,
    pub changes: Vec<Edit>,
    /// Required payouts, in output order from output 1.
    pub payouts: Vec<Payout>,
    /// `Some(height)` when the transaction must have this lock time or later.
    pub min_locktime: Option<u32>,
    /// The new root is `E_D`: output 0 is free (spec 7.8).
    pub terminal: bool,
}

/// Resolve a leaf after the preceding edits in this plan.
fn lookup(current: &State, changes: &[Edit], pos: u64) -> Option<Leaf> {
    for change in changes.iter().rev() {
        if change.pos == pos {
            return change.new;
        }
    }
    current.get(pos).copied()
}

/// Add one edit to the plan and update its checked balance. All operations
/// use this function, including each side of a claim and each grafted leaf.
pub(crate) fn change(
    current: &State,
    fx: &mut Effects,
    pos: u64,
    new: Option<Leaf>,
) -> Result<Edit, Reject> {
    if pos >= current.capacity() {
        return Err(TreeError::PositionOutOfRange.into());
    }
    let old = lookup(current, &fx.changes, pos);
    if old.is_none() && new.is_none() {
        return Err(TreeError::PositionEmpty.into());
    }
    let next = change_sum(
        fx.sum,
        old.map(|l| l.amount).unwrap_or(0),
        new.map(|l| l.amount).unwrap_or(0),
    )?;
    let edit = Edit { pos, old, new };
    fx.sum = next;
    fx.changes.push(edit);
    Ok(edit)
}

/// Dispatch to the same operation-specific functions checked by Kani.
pub fn effects(
    oracle: &impl Oracle,
    p: &Params,
    current: &State,
    op: &Op,
) -> Result<Effects, Reject> {
    if p.depth != current.depth() {
        return Err(Reject::NotAllowed);
    }
    match op {
        Op::Exit { pos, evict } => exit_effects(current, *pos, *evict),
        Op::Join {
            height,
            index,
            leaves,
        } => join_effects(oracle, p, current, *height, *index, leaves),
        Op::Update { pos, new_leaf } => update_effects(oracle, current, *pos, new_leaf),
        Op::Claim {
            source_pos,
            claim_pos,
            claim_leaf,
            voucher_sig,
        } => claim_effects(
            oracle,
            p,
            current,
            *source_pos,
            *claim_pos,
            claim_leaf,
            voucher_sig,
        ),
        Op::Refresh => edit(current, false, |_| Ok(())),
        Op::Migrate { m } => {
            if p.migration_m != Some(*m) {
                return Err(Reject::NotAllowed);
            }
            edit(current, false, |_| Ok(()))
        }
    }
}

fn edit(
    current: &State,
    exit: bool,
    apply: impl FnOnce(&mut Effects) -> Result<(), Reject>,
) -> Result<Effects, Reject> {
    let mut fx = Effects {
        sum: current.checked_total()?,
        changes: Vec::new(),
        payouts: Vec::new(),
        min_locktime: None,
        terminal: false,
    };
    apply(&mut fx)?;
    let empty = fx.sum == 0;
    fx.terminal = empty && exit;
    if empty && !fx.terminal {
        return Err(Reject::NotAllowed);
    }
    Ok(fx)
}

/// Apply this operation's pure rules, before witness construction.
pub fn exit_effects(current: &State, pos: u64, evict: bool) -> Result<Effects, Reject> {
    edit(current, true, |fx| {
        let leaf = *current.get(pos).ok_or(TreeError::PositionEmpty)?;
        if evict {
            fx.min_locktime = Some(leaf.evict_after);
        }
        let terms = exit_terms(fx.sum, &leaf)?;
        fx.payouts.push(terms.payout);
        change(current, fx, pos, None)?;
        if fx.sum != terms.sum {
            return Err(Reject::OutputZero);
        }
        Ok(())
    })
}

/// Apply this operation's pure rules, before witness construction.
pub fn join_effects(
    oracle: &impl Oracle,
    p: &Params,
    current: &State,
    height: u32,
    index: u64,
    leaves: &[(u64, Leaf)],
) -> Result<Effects, Reject> {
    edit(current, false, |fx| {
        if leaves.is_empty() || height > p.depth || index >= (current.capacity() >> height) {
            return Err(Reject::NotAllowed);
        }
        if current.occupied(height, index)? {
            return Err(Reject::SlotNotEmpty);
        }
        for (off, leaf) in leaves {
            if *off >= (1u64 << height) {
                return Err(Reject::SlotNotEmpty);
            }
            leaf.validate_with(|k| oracle.valid_key(k))?;
            change(current, fx, (index << height) + *off, Some(*leaf))?;
        }
        Ok(())
    })
}

/// Apply this operation's pure rules, before witness construction.
pub fn update_effects(
    oracle: &impl Oracle,
    current: &State,
    pos: u64,
    new_leaf: &Leaf,
) -> Result<Effects, Reject> {
    edit(current, false, |fx| {
        current.get(pos).ok_or(TreeError::PositionEmpty)?;
        new_leaf.validate_with(|k| oracle.valid_key(k))?;
        change(current, fx, pos, Some(*new_leaf))?;
        Ok(())
    })
}

/// Apply this operation's pure rules, before witness construction.
pub fn claim_effects(
    oracle: &impl Oracle,
    p: &Params,
    current: &State,
    source_pos: u64,
    claim_pos: u64,
    claim_leaf: &Leaf,
    voucher_sig: &[u8; 64],
) -> Result<Effects, Reject> {
    edit(current, false, |fx| {
        let src = *current.get(source_pos).ok_or(TreeError::PositionEmpty)?;
        let new_src = claim_terms(oracle, p, &src, claim_leaf, voucher_sig)?;
        if current.get(claim_pos).is_some() || claim_pos == source_pos {
            return Err(Reject::SlotNotEmpty);
        }
        change(current, fx, source_pos, new_src)?;
        change(current, fx, claim_pos, Some(*claim_leaf))?;
        Ok(())
    })
}

/// The checked arithmetic used by every edit in an accepted plan.
pub fn change_sum(sum: u64, old: u64, new: u64) -> Result<u64, TreeError> {
    sum.checked_sub(old)
        .ok_or(TreeError::SumOverflow)?
        .checked_add(new)
        .ok_or(TreeError::SumOverflow)
}

#[derive(Debug, Clone, Copy)]
pub struct ExitTerms {
    pub sum: u64,
    pub payout: Payout,
}

/// The tree's balance after an exit, and the member's exact payout.
pub fn exit_terms(sum: u64, leaf: &Leaf) -> Result<ExitTerms, TreeError> {
    Ok(ExitTerms {
        sum: change_sum(sum, leaf.amount, 0)?,
        payout: Payout {
            amount: leaf.amount,
            exit_key: leaf.exit_key,
        },
    })
}

/// Validate a voucher and return the source's replacement leaf. A complete
/// claim removes the source; a partial claim advances its sequence.
pub fn claim_terms(
    oracle: &impl Oracle,
    p: &Params,
    src: &Leaf,
    claim_leaf: &Leaf,
    voucher_sig: &[u8; 64],
) -> Result<Option<Leaf>, Reject> {
    // Rule 1: the voucher, by the source's owner, over this tree_id.
    let msg = voucher_msg(&p.tree_id, src, claim_leaf);
    if !oracle.valid_key(&src.owner_key) {
        return Err(Reject::BadSignature("owner key"));
    }
    if !oracle.voucher(&src.owner_key, msg, voucher_sig) {
        return Err(Reject::BadSignature("voucher"));
    }
    claim_leaf.validate_with(|k| oracle.valid_key(k))?;
    // Rule 2: no underflow; seq advances and must fit.
    let residual = src
        .amount
        .checked_sub(claim_leaf.amount)
        .ok_or(Reject::Underflow)?;
    let seq = src.seq.checked_add(1).ok_or(Reject::SeqFinal)?;
    debug_assert!(src.seq != SEQ_FINAL);
    // Rule 5: nothing carved is harder to evict than its source.
    if !same_bytes(&claim_leaf.evictor_key, &src.evictor_key)
        || claim_leaf.evict_after > src.evict_after
    {
        return Err(Reject::EvictionTerms);
    }
    // Rule 6: the dust floor on both sides.
    if claim_leaf.amount < p.dust_min || (residual != 0 && residual < p.dust_min) {
        return Err(Reject::Dust);
    }
    let new_src = if residual == 0 {
        None
    } else {
        Some(Leaf {
            amount: residual,
            seq,
            ..*src
        })
    };
    Ok(new_src)
}

/// Exact output requirements, without a tree, allocation, or Bitcoin types.
#[derive(Debug, Clone, Copy)]
pub struct OutputRules<'a> {
    pub sum: u64,
    pub payouts: &'a [Payout],
    pub min_locktime: Option<u32>,
    pub terminal: bool,
}

impl Effects {
    pub fn output_rules(&self) -> OutputRules<'_> {
        OutputRules {
            sum: self.sum,
            payouts: &self.payouts,
            min_locktime: self.min_locktime,
            terminal: self.terminal,
        }
    }
}

/// The transaction information checked without Bitcoin or secp256k1 types.
#[derive(Debug, Clone, Copy)]
pub struct Output<'a> {
    pub amount: u64,
    pub script: &'a [u8],
}

/// Check output 0, exact payouts, record publication, and the eviction lease.
/// `tree_output` is absent only for a terminal exit: output 0 is then free.
pub fn check_outputs(
    fx: &OutputRules<'_>,
    outputs: &[Output<'_>],
    tree_output: Option<Output<'_>>,
    record_script: &[u8],
    lock_height: Option<u32>,
    lock_enabled: bool,
) -> Result<(), Reject> {
    let out0 = outputs.first().ok_or(Reject::OutputZero)?;
    if tree_output.is_none() != fx.terminal {
        return Err(Reject::OutputZero);
    }
    if let Some(expected) = tree_output {
        if expected.amount != fx.sum
            || out0.amount != expected.amount
            || !same_bytes(out0.script, expected.script)
        {
            return Err(Reject::OutputZero);
        }
    }
    for (i, payout) in fx.payouts.iter().enumerate() {
        let Some(got) = outputs.get(i + 1) else {
            return Err(Reject::Payout);
        };
        if got.amount != payout.amount || !same_bytes(got.script, &payout.script()) {
            return Err(Reject::Payout);
        }
    }
    if !outputs.iter().any(|o| same_bytes(o.script, record_script)) {
        return Err(Reject::MissingRecord);
    }
    if let Some(minimum) = fx.min_locktime {
        if !lock_enabled || !lock_height.is_some_and(|height| height >= minimum) {
            return Err(Reject::Lease);
        }
    }
    Ok(())
}

/// Compare scripts and keys in eight-byte words. This keeps the same byte
/// equality check inside the proof boundary without a byte-at-a-time C loop.
pub fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while a.len() - i >= 8 {
        let x = u64::from_le_bytes(a[i..i + 8].try_into().expect("eight bytes"));
        let y = u64::from_le_bytes(b[i..i + 8].try_into().expect("eight bytes"));
        if x != y {
            return false;
        }
        i += 8;
    }
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
