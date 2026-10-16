//! The unilateral exit that does not need the enclave.
//!
//! For every state the enclave pre-signs an *unroll*: a small tree of
//! transactions that splits the tree output into one P2TR output per leaf,
//! each paying the leaf's `exit_key` its exact amount.
//!
//! * The first transaction spends the tree output by its tapscript leaf
//!   `<K_k> OP_CHECKSIGVERIFY <Δ> OP_CSV`, so it is valid only `Δ` blocks
//!   after the tree output confirms. Any later transition spends the same
//!   outpoint first and kills it, so it only matters when the enclave has
//!   stopped.
//! * When there are more leaves than `radix`, intermediate outputs are
//!   locked to fresh keys that sign exactly one child transaction and are
//!   then deleted: a deleted-key stand-in for `OP_CHECKTEMPLATEVERIFY`.
//! * Every transaction is TRUC (version 3), pays no fee, and carries a
//!   zero-value pay-to-anchor output, so anyone can bump it by CPFP and
//!   leaves are paid in full (the exact-payout rule of spec 7.2 rule 6).
//!
//! `verify_package` runs with public data only: no enclave, no secret.

use std::collections::{BTreeMap, HashMap};

use bitcoin::hashes::Hash;
use bitcoin::key::{Keypair, TapTweak, XOnlyPublicKey};
use bitcoin::secp256k1::{
    Message, Secp256k1, SecretKey, Signing, Verification, schnorr::Signature,
};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::{ControlBlock, LeafVersion};
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    absolute::LockTime,
};

use crate::output::{TreeOutput, p2a_script, p2tr_raw, unroll_leaf_script};
use crate::rules::TreeParams;
use crate::tree::Leaf;

/// Outputs per unroll transaction. 128 P2TR outputs keep a transaction
/// near 5,600 vB, under TRUC's 10,000 vB cap.
pub const DEFAULT_RADIX: usize = 128;

/// A pre-signed unroll: transactions in parent-before-child order. The
/// first spends the tree output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrollPackage {
    pub tree_outpoint: OutPoint,
    pub txs: Vec<Transaction>,
}

/// Errors a member's client reports when checking an unroll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnrollError {
    Empty,
    RootInput,
    RootDelay,
    RootScript,
    ControlBlock,
    Signature(Txid),
    UnknownParent(Txid),
    Value(Txid),
    Version(Txid),
    Payouts,
    NotPaid,
}

enum Signer<'a> {
    /// The state's unroll key, spending the tree output's tapscript leaf.
    Script {
        key: &'a SecretKey,
        output: TreeOutput,
    },
    /// A fresh intermediate key, spending a key-path-only output.
    Key(SecretKey),
}

fn base_tx(input: OutPoint, sequence: Sequence, outputs: Vec<TxOut>) -> Transaction {
    let mut outputs = outputs;
    outputs.push(TxOut {
        value: Amount::ZERO,
        script_pubkey: p2a_script(),
    });
    Transaction {
        version: Version(3),
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: input,
            script_sig: ScriptBuf::new(),
            sequence,
            witness: Witness::new(),
        }],
        output: outputs,
    }
}

/// Build and sign the unroll of a state. Called inside the enclave; every
/// intermediate key is erased before this returns. The caller erases
/// `unroll_key`.
pub fn build<C: Signing + Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    output: TreeOutput,
    outpoint: OutPoint,
    amount: u64,
    leaves: &BTreeMap<u64, Leaf>,
    unroll_key: &SecretKey,
    radix: usize,
) -> UnrollPackage {
    assert!(radix >= 2);
    let items: Vec<Leaf> = leaves.values().copied().collect();
    let prevout = TxOut {
        value: Amount::from_sat(amount),
        script_pubkey: output.script_pubkey(secp, &p.enclave_key, p.unroll_delay),
    };
    let mut txs = Vec::new();
    node(
        secp,
        p,
        &items,
        outpoint,
        prevout,
        Signer::Script {
            key: unroll_key,
            output,
        },
        Sequence::from_height(p.unroll_delay),
        radix,
        &mut txs,
    );
    UnrollPackage {
        tree_outpoint: outpoint,
        txs,
    }
}

#[allow(clippy::too_many_arguments)]
fn node<C: Signing + Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    items: &[Leaf],
    input: OutPoint,
    prevout: TxOut,
    signer: Signer<'_>,
    sequence: Sequence,
    radix: usize,
    txs: &mut Vec<Transaction>,
) {
    let mut children: Vec<(&[Leaf], SecretKey)> = Vec::new();
    let outputs: Vec<TxOut> = if items.len() <= radix {
        items
            .iter()
            .map(|l| TxOut {
                value: Amount::from_sat(l.amount),
                script_pubkey: p2tr_raw(&l.exit_key),
            })
            .collect()
    } else {
        // Group size: the smallest power of radix that needs at most radix groups.
        let mut chunk = 1usize;
        while items.len().div_ceil(chunk) > radix {
            chunk *= radix;
        }
        items
            .chunks(chunk)
            .map(|g| {
                let (sk, _) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
                let x = XOnlyPublicKey::from(sk.public_key(secp));
                let value = g.iter().map(|l| l.amount).sum();
                children.push((g, sk));
                TxOut {
                    value: Amount::from_sat(value),
                    script_pubkey: ScriptBuf::new_p2tr(secp, x, None),
                }
            })
            .collect()
    };
    let mut tx = base_tx(input, sequence, outputs);
    let mut cache = SighashCache::new(&tx);
    let prevs = [prevout.clone()];
    let witness = match signer {
        Signer::Script { key, output } => {
            let script = unroll_leaf_script(&output.unroll_key, p.unroll_delay);
            let h = cache
                .taproot_script_spend_signature_hash(
                    0,
                    &Prevouts::All(&prevs),
                    output.unroll_leaf_hash(p.unroll_delay),
                    TapSighashType::Default,
                )
                .expect("one input");
            let kp = Keypair::from_secret_key(secp, key);
            let sig = secp.sign_schnorr(&Message::from_digest(h.to_byte_array()), &kp);
            let cb = output
                .spend_info(secp, &p.enclave_key, p.unroll_delay)
                .control_block(&(script.clone(), LeafVersion::TapScript))
                .expect("leaf is in the tree");
            let mut w = Witness::new();
            w.push(sig.as_ref());
            w.push(script.as_bytes());
            w.push(cb.serialize());
            w
        }
        Signer::Key(mut sk) => {
            let h = cache
                .taproot_key_spend_signature_hash(
                    0,
                    &Prevouts::All(&prevs),
                    TapSighashType::Default,
                )
                .expect("one input");
            let kp = Keypair::from_secret_key(secp, &sk)
                .tap_tweak(secp, None)
                .to_keypair();
            let sig = secp.sign_schnorr(&Message::from_digest(h.to_byte_array()), &kp);
            // The deletion that makes this output spendable by this child only.
            sk.non_secure_erase();
            let mut w = Witness::new();
            w.push(sig.as_ref());
            w
        }
    };
    tx.input[0].witness = witness;
    let txid = tx.compute_txid();
    let outs = tx.output.clone();
    txs.push(tx);
    for (vout, (group, sk)) in children.into_iter().enumerate() {
        node(
            secp,
            p,
            group,
            OutPoint {
                txid,
                vout: vout as u32,
            },
            outs[vout].clone(),
            Signer::Key(sk),
            Sequence::ENABLE_RBF_NO_LOCKTIME,
            radix,
            txs,
        );
    }
}

fn output_key(spk: &ScriptBuf) -> Option<XOnlyPublicKey> {
    let b = spk.as_bytes();
    (b.len() == 34 && b[0] == 0x51 && b[1] == 0x20)
        .then(|| XOnlyPublicKey::from_slice(&b[2..]).ok())
        .flatten()
}

/// Check a whole unroll against the state it belongs to, with public data
/// only, and return the transactions that pay `mine`, root first. This is
/// what a member's client runs before it treats a state as safe, and again
/// before it broadcasts.
pub fn verify_package<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    output: TreeOutput,
    outpoint: OutPoint,
    amount: u64,
    leaves: &BTreeMap<u64, Leaf>,
    pkg: &UnrollPackage,
    mine: &Leaf,
) -> Result<Vec<Transaction>, UnrollError> {
    verify(secp, p, output, outpoint, amount, Some(leaves), pkg, mine)
}

/// What a light client can check without the other leaves: the package
/// spends this state's tree output after `Δ` by its unroll leaf, every
/// signature verifies, every transaction conserves value, and a branch
/// pays `mine` in full. It cannot see that every other leaf is paid; the
/// full check is `verify_package`.
#[allow(clippy::too_many_arguments)]
pub fn verify_branch<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    output: TreeOutput,
    outpoint: OutPoint,
    amount: u64,
    pkg: &UnrollPackage,
    mine: &Leaf,
) -> Result<Vec<Transaction>, UnrollError> {
    verify(secp, p, output, outpoint, amount, None, pkg, mine)
}

#[allow(clippy::too_many_arguments)]
fn verify<C: Verification>(
    secp: &Secp256k1<C>,
    p: &TreeParams,
    output: TreeOutput,
    outpoint: OutPoint,
    amount: u64,
    leaves: Option<&BTreeMap<u64, Leaf>>,
    pkg: &UnrollPackage,
    mine: &Leaf,
) -> Result<Vec<Transaction>, UnrollError> {
    let root = pkg.txs.first().ok_or(UnrollError::Empty)?;
    if pkg.tree_outpoint != outpoint
        || root.input.len() != 1
        || root.input[0].previous_output != outpoint
    {
        return Err(UnrollError::RootInput);
    }
    // The delay is in the transaction's nSequence (BIP-68, version >= 2)
    // and in the script (CSV); both must say Δ.
    if root.input[0].sequence != Sequence::from_height(p.unroll_delay) || root.version.0 < 2 {
        return Err(UnrollError::RootDelay);
    }
    let w = &root.input[0].witness;
    let script = unroll_leaf_script(&output.unroll_key, p.unroll_delay);
    if w.len() != 3 || w.nth(1) != Some(script.as_bytes()) {
        return Err(UnrollError::RootScript);
    }
    let tree_spk = output.script_pubkey(secp, &p.enclave_key, p.unroll_delay);
    let cb =
        ControlBlock::decode(w.nth(2).expect("len 3")).map_err(|_| UnrollError::ControlBlock)?;
    let out_key = output_key(&tree_spk).expect("p2tr");
    if !cb.verify_taproot_commitment(secp, out_key, &script) {
        return Err(UnrollError::ControlBlock);
    }

    // Walk every transaction: its parent must be the tree output or an
    // earlier transaction's output; its signature must verify; it must
    // conserve value.
    let mut utxos: HashMap<OutPoint, TxOut> = HashMap::new();
    utxos.insert(
        outpoint,
        TxOut {
            value: Amount::from_sat(amount),
            script_pubkey: tree_spk.clone(),
        },
    );
    let mut parent_of: HashMap<Txid, Txid> = HashMap::new();
    let mut spent = std::collections::HashSet::new();
    for (i, tx) in pkg.txs.iter().enumerate() {
        let txid = tx.compute_txid();
        if tx.input.len() != 1 || tx.version.0 != 3 {
            return Err(UnrollError::Version(txid));
        }
        let op = tx.input[0].previous_output;
        let prev = utxos
            .get(&op)
            .cloned()
            .ok_or(UnrollError::UnknownParent(txid))?;
        spent.insert(op);
        if i > 0 {
            parent_of.insert(txid, op.txid);
        }
        let out_sum: u64 = tx.output.iter().map(|o| o.value.to_sat()).sum();
        if out_sum != prev.value.to_sat() {
            return Err(UnrollError::Value(txid));
        }
        let prevs = [prev.clone()];
        let mut cache = SighashCache::new(tx);
        let (msg, key) = if i == 0 {
            let h = cache
                .taproot_script_spend_signature_hash(
                    0,
                    &Prevouts::All(&prevs),
                    output.unroll_leaf_hash(p.unroll_delay),
                    TapSighashType::Default,
                )
                .expect("one input");
            (h.to_byte_array(), output.unroll_key)
        } else {
            let h = cache
                .taproot_key_spend_signature_hash(
                    0,
                    &Prevouts::All(&prevs),
                    TapSighashType::Default,
                )
                .expect("one input");
            (
                h.to_byte_array(),
                output_key(&prev.script_pubkey).ok_or(UnrollError::Signature(txid))?,
            )
        };
        let sig_bytes = tx.input[0]
            .witness
            .nth(0)
            .ok_or(UnrollError::Signature(txid))?;
        let sig = Signature::from_slice(sig_bytes).map_err(|_| UnrollError::Signature(txid))?;
        secp.verify_schnorr(&sig, &Message::from_digest(msg), &key)
            .map_err(|_| UnrollError::Signature(txid))?;
        for (vout, o) in tx.output.iter().enumerate() {
            utxos.insert(
                OutPoint {
                    txid,
                    vout: vout as u32,
                },
                o.clone(),
            );
        }
    }

    // The final outputs (not spent by another unroll tx, not anchors) must
    // be exactly the leaves: each exit key paid its exact amount, once.
    if let Some(leaves) = leaves {
        let mut want: Vec<(ScriptBuf, u64)> = leaves
            .values()
            .map(|l| (p2tr_raw(&l.exit_key), l.amount))
            .collect();
        let mut got: Vec<(ScriptBuf, u64)> = utxos
            .iter()
            .filter(|(op, o)| !spent.contains(*op) && o.script_pubkey != p2a_script())
            .map(|(_, o)| (o.script_pubkey.clone(), o.value.to_sat()))
            .collect();
        want.sort();
        got.sort();
        if want != got {
            return Err(UnrollError::Payouts);
        }
    }

    // The branch to my payout.
    let my_spk = p2tr_raw(&mine.exit_key);
    let payer = pkg
        .txs
        .iter()
        .find(|tx| {
            tx.output
                .iter()
                .any(|o| o.script_pubkey == my_spk && o.value.to_sat() == mine.amount)
        })
        .ok_or(UnrollError::NotPaid)?;
    let mut branch = vec![payer.clone()];
    let mut cur = payer.compute_txid();
    while let Some(parent) = parent_of.get(&cur) {
        let tx = pkg
            .txs
            .iter()
            .find(|t| t.compute_txid() == *parent)
            .expect("parent checked");
        branch.push(tx.clone());
        cur = *parent;
    }
    branch.reverse();
    Ok(branch)
}
