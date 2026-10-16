//! Byte decoders for everything the enclave and light clients read from
//! outside: `enc64`, the 116-byte leaf, the transition record (the bytes
//! whose tagged hash sits in `OP_RETURN`), and the unroll package a member
//! receives. Every decoder returns a typed error and never panics, and each
//! is the exact inverse of its encoder: `encode(decode(x)) == x` whenever
//! decoding succeeds (fuzzed in `fuzz/`).
use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::Hash;
use bitcoin::key::XOnlyPublicKey;
use bitcoin::{OutPoint, Transaction, Txid};

use crate::rules::{Change, Record};
use crate::tree::{LEAF_LEN, Leaf, Node, PathStep};
use crate::unroll::UnrollPackage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Too few or too many bytes.
    Length,
    /// A flag or presence byte other than 0 or 1, or `enc64`'s marker.
    Flag,
    /// A key that is not a valid x-only point.
    Key,
    /// A count beyond the decoder's bound.
    TooMany,
    /// A transaction that is not canonical consensus bytes.
    Transaction,
}

/// Most changes one record may carry.
pub const MAX_CHANGES: u32 = 1 << 12;
/// Deepest sibling path (the tree's depth is far below this).
pub const MAX_PATH: u8 = 64;
/// Most transactions in one unroll package, and the largest one.
pub const MAX_UNROLL_TXS: u32 = 1 << 16;
pub const MAX_TX_BYTES: u32 = 400_000;

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.bytes.len() < n {
            return Err(DecodeError::Length);
        }
        let (head, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn flag(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Flag),
        }
    }
    fn end(&self) -> Result<(), DecodeError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::Length)
        }
    }
}

/// Decode `enc64(n) = le64(n) || 0x01`.
pub fn dec64(bytes: &[u8]) -> Result<u64, DecodeError> {
    let b: [u8; 9] = bytes.try_into().map_err(|_| DecodeError::Length)?;
    if b[8] != 1 {
        return Err(DecodeError::Flag);
    }
    Ok(u64::from_le_bytes(
        b[..8].try_into().map_err(|_| DecodeError::Length)?,
    ))
}

impl Leaf {
    /// The 116-byte leaf, field by field. Structural only: `validate`
    /// still applies the spec's value checks.
    pub fn parse(bytes: &[u8]) -> Result<Leaf, DecodeError> {
        if bytes.len() != LEAF_LEN {
            return Err(DecodeError::Length);
        }
        let mut r = Reader { bytes };
        Ok(Leaf {
            amount: r.u64()?,
            exit_key: r.array()?,
            owner_key: r.array()?,
            evictor_key: r.array()?,
            evict_after: r.u32()?,
            nonce: r.u32()?,
            seq: r.u32()?,
        })
    }
}

fn leaf_slot(r: &mut Reader<'_>) -> Result<Option<Leaf>, DecodeError> {
    if r.flag()? {
        Ok(Some(Leaf::parse(r.take(LEAF_LEN)?)?))
    } else {
        Ok(None)
    }
}

impl Record {
    /// The record's bytes: what its tagged hash commits to.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.prev.txid.to_byte_array());
        buf.extend_from_slice(&self.prev.vout.to_le_bytes());
        buf.extend_from_slice(&(self.changes.len() as u32).to_le_bytes());
        for c in &self.changes {
            buf.extend_from_slice(&c.pos.to_le_bytes());
            for l in [&c.old, &c.new] {
                match l {
                    Some(l) => {
                        buf.push(1);
                        buf.extend_from_slice(&l.serialize());
                    }
                    None => buf.push(0),
                }
            }
            buf.push(c.path.len() as u8);
            for s in &c.path {
                buf.push(s.right as u8);
                buf.extend_from_slice(&s.sibling.hash);
                buf.extend_from_slice(&s.sibling.sum.to_le_bytes());
            }
        }
        buf.extend_from_slice(&self.new_root.hash);
        buf.extend_from_slice(&self.new_root.sum.to_le_bytes());
        match self.next_unroll_key {
            Some(k) => buf.extend_from_slice(&k.serialize()),
            None => buf.extend_from_slice(&[0u8; 32]),
        }
        buf
    }

    /// The inverse of [`Record::encode`]. All-zero key bytes mean no next
    /// unroll key; any other bytes must be a valid x-only key.
    pub fn decode(bytes: &[u8]) -> Result<Record, DecodeError> {
        let mut r = Reader { bytes };
        let txid = Txid::from_byte_array(r.array()?);
        let vout = r.u32()?;
        let count = r.u32()?;
        if count > MAX_CHANGES {
            return Err(DecodeError::TooMany);
        }
        let mut changes = Vec::new();
        for _ in 0..count {
            let pos = r.u64()?;
            let old = leaf_slot(&mut r)?;
            let new = leaf_slot(&mut r)?;
            let len = r.u8()?;
            if len > MAX_PATH {
                return Err(DecodeError::TooMany);
            }
            let mut path = Vec::with_capacity(usize::from(len));
            for _ in 0..len {
                let right = r.flag()?;
                let hash = r.array()?;
                let sum = r.u64()?;
                path.push(PathStep {
                    right,
                    sibling: Node { hash, sum },
                });
            }
            changes.push(Change {
                pos,
                old,
                new,
                path,
            });
        }
        let new_root = Node {
            hash: r.array()?,
            sum: r.u64()?,
        };
        let key: [u8; 32] = r.array()?;
        r.end()?;
        let next_unroll_key = if key == [0u8; 32] {
            None
        } else {
            Some(XOnlyPublicKey::from_slice(&key).map_err(|_| DecodeError::Key)?)
        };
        Ok(Record {
            prev: OutPoint { txid, vout },
            changes,
            new_root,
            next_unroll_key,
        })
    }
}

impl UnrollPackage {
    /// Wire form: the tree outpoint (txid, vout), a transaction count, then
    /// each transaction as a length and its consensus bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.tree_outpoint.txid.to_byte_array());
        buf.extend_from_slice(&self.tree_outpoint.vout.to_le_bytes());
        buf.extend_from_slice(&(self.txs.len() as u32).to_le_bytes());
        for tx in &self.txs {
            let bytes = serialize(tx);
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(&bytes);
        }
        buf
    }

    /// The inverse of [`UnrollPackage::encode`], refusing non-canonical
    /// transaction bytes.
    pub fn decode(bytes: &[u8]) -> Result<UnrollPackage, DecodeError> {
        let mut r = Reader { bytes };
        let txid = Txid::from_byte_array(r.array()?);
        let vout = r.u32()?;
        let count = r.u32()?;
        if count > MAX_UNROLL_TXS {
            return Err(DecodeError::TooMany);
        }
        let mut txs = Vec::new();
        for _ in 0..count {
            let len = r.u32()?;
            if len > MAX_TX_BYTES {
                return Err(DecodeError::TooMany);
            }
            let raw = r.take(len as usize)?;
            let tx: Transaction = deserialize(raw).map_err(|_| DecodeError::Transaction)?;
            if serialize(&tx) != raw {
                return Err(DecodeError::Transaction);
            }
            txs.push(tx);
        }
        r.end()?;
        Ok(UnrollPackage {
            tree_outpoint: OutPoint { txid, vout },
            txs,
        })
    }
}
