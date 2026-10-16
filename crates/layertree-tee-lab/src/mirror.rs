//! Record mirrors and their receipts (spec 4.1).
//!
//! The enclave must not publish a state without its unroll. Before it
//! releases its signature on a transition, the host hands the record and
//! the new unroll to `M` independent mirrors; each stores them and signs a
//! receipt over (record hash, unroll hash). The enclave counts valid
//! receipts from distinct mirrors it was configured with and signs only at
//! `m` or more (`Enclave::finish`).

use std::collections::HashMap;

use bitcoin::key::{Keypair, XOnlyPublicKey};
use bitcoin::secp256k1::{All, Message, Secp256k1, SecretKey, Verification, schnorr::Signature};

use crate::rules::Record;
use crate::tree::tagged_hash;
use crate::unroll::UnrollPackage;

pub const TAG_RECEIPT: &str = "LayerTreeTEE/receipt";
pub const TAG_UNROLL: &str = "LayerTreeTEE/unroll";

/// The hash of an unroll package's bytes; all zeros when there is none
/// (a terminal exit or a migration).
pub fn unroll_hash(pkg: Option<&UnrollPackage>) -> [u8; 32] {
    pkg.map_or([0u8; 32], |p| tagged_hash(TAG_UNROLL, &[&p.encode()]))
}

/// What a mirror signs: it holds this record and this unroll.
pub fn receipt_msg(record: &Record, pkg: Option<&UnrollPackage>) -> [u8; 32] {
    tagged_hash(TAG_RECEIPT, &[&record.hash(), &unroll_hash(pkg)])
}

/// A mirror's signed statement that it stored a record and its unroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    pub mirror: XOnlyPublicKey,
    pub sig: Signature,
}

/// The mirrors an enclave trusts to hold data, and how many must answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorPolicy {
    pub mirrors: Vec<XOnlyPublicKey>,
    pub m: usize,
}

impl MirrorPolicy {
    /// Valid receipts for `msg` from distinct configured mirrors.
    pub fn count<C: Verification>(
        &self,
        secp: &Secp256k1<C>,
        msg: [u8; 32],
        receipts: &[Receipt],
    ) -> usize {
        let mut seen: Vec<XOnlyPublicKey> = Vec::new();
        for r in receipts {
            if self.mirrors.contains(&r.mirror)
                && !seen.contains(&r.mirror)
                && secp
                    .verify_schnorr(&r.sig, &Message::from_digest(msg), &r.mirror)
                    .is_ok()
            {
                seen.push(r.mirror);
            }
        }
        seen.len()
    }
}

/// Somewhere records and unrolls are kept for members.
pub trait Mirror {
    fn key(&self) -> XOnlyPublicKey;
    /// Store both and return a receipt, or `None` if the mirror is down or
    /// refuses.
    fn store(&mut self, record: &Record, unroll: Option<&UnrollPackage>) -> Option<Receipt>;
}

/// An in-memory mirror for the lab.
pub struct MemMirror {
    secp: Secp256k1<All>,
    key: Keypair,
    /// A mirror that is down stores nothing and signs nothing.
    pub up: bool,
    pub records: HashMap<[u8; 32], Vec<u8>>,
    pub unrolls: HashMap<[u8; 32], Vec<u8>>,
}

impl MemMirror {
    pub fn new(sk: &SecretKey) -> MemMirror {
        let secp = Secp256k1::new();
        let key = Keypair::from_secret_key(&secp, sk);
        MemMirror {
            secp,
            key,
            up: true,
            records: HashMap::new(),
            unrolls: HashMap::new(),
        }
    }
}

impl Mirror for MemMirror {
    fn key(&self) -> XOnlyPublicKey {
        self.key.x_only_public_key().0
    }

    fn store(&mut self, record: &Record, unroll: Option<&UnrollPackage>) -> Option<Receipt> {
        if !self.up {
            return None;
        }
        self.records.insert(record.hash(), record.encode());
        if let Some(p) = unroll {
            self.unrolls.insert(unroll_hash(Some(p)), p.encode());
        }
        let msg = Message::from_digest(receipt_msg(record, unroll));
        Some(Receipt {
            mirror: self.key(),
            sig: self.secp.sign_schnorr(&msg, &self.key),
        })
    }
}

/// Hand a prepared transition's record and unroll to every mirror and
/// gather the receipts (the host's job; the enclave only checks them).
pub fn collect<M: Mirror>(
    record: &Record,
    unroll: Option<&UnrollPackage>,
    mirrors: &mut [M],
) -> Vec<Receipt> {
    mirrors
        .iter_mut()
        .filter_map(|m| m.store(record, unroll))
        .collect()
}
