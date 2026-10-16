//! A light client that follows the tree on a real chain (spec 4.1).
//!
//! The follower knows the scriptPubKey of the tree output it trusts (the
//! output for its root and the current unroll key), so a BIP-158 block
//! filter tells it which block spends it without downloading the others.
//! In that block it finds the spend, reads the 32-byte `OP_RETURN`s, asks a
//! record source for the record with that hash, and accepts the record only
//! if it hashes to exactly that value. Then `Member::follow` replays it.
//!
//! Alarms (`LightError`), each raised at most once per spend:
//!
//! * `RecordNotAnchored`: the source served bytes for the spend's hash that
//!   do not hash to it (or the spend anchors no record at all);
//! * `RecordMissing`: no record `missing_after` blocks after the spend;
//! * `OldStateMismatch`, `NewRootMismatch`, `OutputMismatch`,
//!   `ProofBroken`: the record or the spend breaks the rules;
//! * `UnrollNotHeld`: `missing_after` blocks after a spend, the member
//!   still holds no unroll for the new state that pays its leaf.
//!
//! A rule-breaking spend stops the follower: the tree is no longer one the
//! member can trust, and the member's way out is the last good unroll.
//!
//! The chain and record sources are traits, so the same follower runs on
//! regtest (`tests/regtest.rs`), signet, or in memory. The two record
//! backends here are a local directory and a minimal HTTP mirror client.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use bitcoin::bip158::BlockFilter;
use bitcoin::secp256k1::{Secp256k1, Verification};
use bitcoin::{Block, BlockHash, OutPoint, ScriptBuf, Transaction};

use crate::light::{LightError, Member};
use crate::output::TreeOutput;
use crate::rules::{Record, TreeParams};
use crate::tree::tagged_hash;
use crate::unroll::{UnrollPackage, verify_branch};

/// Read access to a chain: what a BIP-157/158 light client gets from peers,
/// or a node's RPC on regtest.
pub trait ChainSource {
    fn tip(&mut self) -> Result<u32, String>;
    fn block_hash(&mut self, height: u32) -> Result<BlockHash, String>;
    /// The BIP-158 basic filter's bytes for the block.
    fn filter(&mut self, hash: &BlockHash) -> Result<Vec<u8>, String>;
    /// Sources should fetch just the header; the default supports in-memory adapters.
    fn header(&mut self, hash: &BlockHash) -> Result<bitcoin::block::Header, String> {
        self.block(hash).map(|block| block.header)
    }
    fn block(&mut self, hash: &BlockHash) -> Result<Block, String>;
}

/// Where records and unroll packages are published: the enclave's mirror,
/// a member's own directory, any third party. Nothing from a source is
/// trusted; the follower checks everything it gets.
pub trait RecordSource {
    /// The bytes published under a record hash (`Record::encode` form).
    fn record(&mut self, hash: &[u8; 32]) -> Option<Vec<u8>>;
    /// The unroll package for the tree output at `outpoint`.
    fn unroll(&mut self, outpoint: &OutPoint) -> Option<Vec<u8>>;
    /// Try independent mirrors before concluding that no valid package is held.
    fn unroll_candidates(&mut self, outpoint: &OutPoint) -> Vec<Vec<u8>> {
        self.unroll(outpoint).into_iter().collect()
    }
    fn unroll_matching(
        &mut self,
        outpoint: &OutPoint,
        check: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Option<Vec<u8>> {
        self.unroll_candidates(outpoint)
            .into_iter()
            .find(|bytes| check(bytes))
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// File name of a record: `record-<hash hex>.bin`.
pub fn record_name(hash: &[u8; 32]) -> String {
    format!("record-{}.bin", hex(hash))
}

/// File name of an unroll: `unroll-<txid>-<vout>.bin`.
pub fn unroll_name(outpoint: &OutPoint) -> String {
    format!("unroll-{}-{}.bin", outpoint.txid, outpoint.vout)
}

/// Records and unrolls as files in one directory.
pub struct DirSource {
    pub dir: PathBuf,
}

impl RecordSource for DirSource {
    fn record(&mut self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(record_name(hash))).ok()
    }
    fn unroll(&mut self, outpoint: &OutPoint) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(unroll_name(outpoint))).ok()
    }
}

/// A mirror over plain HTTP: `GET /<record_name>` and `GET /<unroll_name>`
/// on `addr` (host:port). A stub for the lab: no TLS, no redirects; a
/// non-200 answer or any error is "not published".
pub struct HttpMirror {
    pub addr: String,
}

impl HttpMirror {
    fn get(&self, path: &str) -> Option<Vec<u8>> {
        let mut s = TcpStream::connect(&self.addr).ok()?;
        s.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
        let req = format!("GET /{path} HTTP/1.0\r\nHost: {}\r\n\r\n", self.addr);
        s.write_all(req.as_bytes()).ok()?;
        let mut resp = Vec::new();
        s.read_to_end(&mut resp).ok()?;
        let split = resp.windows(4).position(|w| w == b"\r\n\r\n")?;
        let head = std::str::from_utf8(&resp[..split]).ok()?;
        let status = head.split_whitespace().nth(1)?;
        (status == "200").then(|| resp[split + 4..].to_vec())
    }
}

impl RecordSource for HttpMirror {
    fn record(&mut self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        self.get(&record_name(hash))
    }
    fn unroll(&mut self, outpoint: &OutPoint) -> Option<Vec<u8>> {
        self.get(&unroll_name(outpoint))
    }
}

/// The 32-byte pushes of a transaction's `OP_RETURN` outputs, in order.
pub fn anchored_hashes(tx: &Transaction) -> Vec<[u8; 32]> {
    tx.output
        .iter()
        .filter_map(|o| {
            let b = o.script_pubkey.as_bytes();
            (b.len() == 34 && b[0] == 0x6a && b[1] == 0x20).then(|| {
                let mut h = [0u8; 32];
                h.copy_from_slice(&b[2..]);
                h
            })
        })
        .collect()
}

/// A spend of the tracked output, waiting for its record.
#[derive(Debug, Clone)]
struct Pending {
    tx: Transaction,
    height: u32,
    raised: Vec<LightError>,
}

/// A new state whose unroll the member has not got yet.
#[derive(Debug, Clone)]
struct AwaitUnroll {
    since: u32,
    raised: bool,
}

/// What the follower saw: a followed spend, or an alarm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// The block height the observation is about (the tip when raised).
    pub height: u32,
    /// The spend, when there is one.
    pub txid: Option<bitcoin::Txid>,
    pub result: Result<(), LightError>,
}

/// A source read failure or a changed chain. A reorg requires replay from
/// a trusted earlier state rather than treating a rolled-back spend as theft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    Source(String),
    Reorganized { height: u32 },
}
impl From<String> for PollError {
    fn from(error: String) -> Self {
        Self::Source(error)
    }
}
impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "{error}"),
            Self::Reorganized { height } => write!(
                f,
                "The chain changed at block {height}. Replay from an earlier trusted state."
            ),
        }
    }
}
impl std::error::Error for PollError {}
impl From<PollError> for String {
    fn from(error: PollError) -> Self {
        error.to_string()
    }
}

#[derive(Clone)]
pub struct Follower {
    pub member: Member,
    pub params: TreeParams,
    /// Blocks after a spend before `RecordMissing` / `UnrollNotHeld`.
    pub missing_after: u32,
    /// The highest block scanned for the current tracked output.
    pub scanned: u32,
    /// The node's tip when this poll completed, including any catch-up delay.
    pub observed_tip: u32,
    /// The unroll the member holds for its current state, checked.
    pub unroll: Option<UnrollPackage>,
    /// Set by a rule-breaking spend; the follower stops.
    pub stopped: Option<LightError>,
    /// The checked unroll branch has paid this member in full.
    pub exited: bool,
    pending: Option<Pending>,
    await_unroll: Option<AwaitUnroll>,
    checkpoint: Option<(u32, BlockHash)>,
    unroll_path: Option<VecDeque<Transaction>>,
    unroll_spk: Option<ScriptBuf>,
}

impl Follower {
    /// Start following from the state `member` holds, scanning blocks
    /// after `from_height`. `unroll` is the package the member checked
    /// before it funded (or joined).
    pub fn new(
        member: Member,
        params: TreeParams,
        from_height: u32,
        missing_after: u32,
        unroll: Option<UnrollPackage>,
    ) -> Follower {
        Follower {
            member,
            params,
            missing_after,
            scanned: from_height,
            observed_tip: from_height,
            unroll,
            stopped: None,
            exited: false,
            pending: None,
            await_unroll: None,
            checkpoint: None,
            unroll_path: None,
            unroll_spk: None,
        }
    }

    fn output(&self) -> TreeOutput {
        TreeOutput {
            root_hash: self.member.root.hash,
            unroll_key: self.member.unroll_key,
        }
    }

    /// The scriptPubKey of the tree output this member trusts.
    pub fn tracked_spk<C: Verification>(&self, secp: &Secp256k1<C>) -> ScriptBuf {
        self.unroll_spk.clone().unwrap_or_else(|| {
            self.output()
                .script_pubkey(secp, &self.params.enclave_key, self.params.unroll_delay)
        })
    }

    /// Scan every block up to the tip and retry anything pending. Returns
    /// what happened, in order.
    pub fn poll<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        chain: &mut dyn ChainSource,
        records: &mut dyn RecordSource,
    ) -> Result<Vec<Seen>, PollError> {
        // Commit the state and its observations together. An RPC failure
        // after one block must not silently consume that block's observations.
        let mut next = self.clone();
        let seen = next.poll_inner(secp, chain, records)?;
        *self = next;
        Ok(seen)
    }

    fn poll_inner<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        chain: &mut dyn ChainSource,
        records: &mut dyn RecordSource,
    ) -> Result<Vec<Seen>, PollError> {
        let mut seen = Vec::new();
        let tip = chain.tip()?;
        if let Some((height, hash)) = self.checkpoint {
            if tip < height || chain.block_hash(height)? != hash {
                return Err(PollError::Reorganized { height });
            }
        }
        let start_height = self.scanned;
        if tip < start_height {
            return Err(PollError::Reorganized {
                height: start_height,
            });
        }
        let start_hash = chain.block_hash(start_height)?;
        let now = self.scanned;
        self.settle(secp, now, records, &mut seen);
        let mut last_hash = chain.block_hash(self.scanned)?;
        let mut examined = Vec::new();
        while self.scanned < tip && self.stopped.is_none() && !self.exited {
            let h = self.scanned + 1;
            let hash = chain.block_hash(h)?;
            let header = chain.header(&hash)?;
            if header.block_hash() != hash || header.prev_blockhash != last_hash {
                return Err(PollError::Reorganized { height: h - 1 });
            }
            if self.pending.is_none() {
                let spk = self.tracked_spk(secp);
                let filter = BlockFilter::new(&chain.filter(&hash)?);
                if filter
                    .match_any(&hash, std::iter::once(spk.as_bytes()))
                    .map_err(|e| e.to_string())?
                {
                    let block = chain.block(&hash)?;
                    examined.push((h, hash));
                    self.find_spends(secp, &block, h, records, &mut seen);
                }
            }
            self.scanned = h;
            last_hash = hash;
            self.settle(secp, h, records, &mut seen);
            if self.scanned != h {
                last_hash = chain.block_hash(self.scanned)?;
            }
        }
        let final_tip = chain.tip()?;
        if final_tip < self.scanned
            || chain.block_hash(start_height)? != start_hash
            || chain.block_hash(self.scanned)? != last_hash
        {
            return Err(PollError::Reorganized {
                height: self.scanned,
            });
        }
        // A recovered record may rewind the scan across a fork. Check every
        // block used to change member state against the final active chain.
        for (height, hash) in examined {
            if chain.block_hash(height)? != hash {
                return Err(PollError::Reorganized { height });
            }
        }
        self.observed_tip = final_tip;
        self.checkpoint = Some((self.scanned, last_hash));
        Ok(seen)
    }

    /// Follow every spend of the tracked outpoint in `block`, in order.
    fn find_spends<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        block: &Block,
        h: u32,
        records: &mut dyn RecordSource,
        seen: &mut Vec<Seen>,
    ) {
        loop {
            if self.pending.is_some() || self.stopped.is_some() || self.exited {
                return;
            }
            let Some(tx) = block
                .txdata
                .iter()
                .find(|t| {
                    t.input
                        .iter()
                        .any(|i| i.previous_output == self.member.outpoint)
                })
                .cloned()
            else {
                return;
            };
            if self.follow_unroll(secp, &tx, h, seen) {
                continue;
            }
            self.pending = Some(Pending {
                tx,
                height: h,
                raised: Vec::new(),
            });
            let before = self.member.outpoint;
            self.settle(secp, h, records, seen);
            if self.member.outpoint == before {
                return;
            }
        }
    }

    pub fn unrolling(&self) -> bool {
        self.unroll_path.is_some() && !self.exited
    }

    /// A held package is already checked against this member's exact payout.
    /// Its script-path spend needs no record or enclave signature at exit time.
    fn follow_unroll<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        tx: &Transaction,
        height: u32,
        seen: &mut Vec<Seen>,
    ) -> bool {
        if self.unroll_path.is_none() {
            let (Some(pkg), Some(leaf)) = (&self.unroll, self.member.leaf) else {
                return false;
            };
            if pkg.txs.first() != Some(tx) {
                return false;
            }
            let Ok(branch) = verify_branch(
                secp,
                &self.params,
                self.output(),
                self.member.outpoint,
                self.member.root.sum,
                pkg,
                &leaf,
            ) else {
                return false;
            };
            self.unroll_path = Some(branch.into());
            self.await_unroll = None;
        }
        let expected = self.unroll_path.as_mut().and_then(VecDeque::pop_front);
        if expected.as_ref() != Some(tx) {
            seen.push(Seen {
                height,
                txid: Some(tx.compute_txid()),
                result: Err(LightError::OutputMismatch),
            });
            self.stop(LightError::OutputMismatch);
            return true;
        }
        if let Some(next) = self.unroll_path.as_ref().and_then(|path| path.front()) {
            let next_input = next.input[0].previous_output;
            if next_input.txid != tx.compute_txid() || next_input.vout as usize >= tx.output.len() {
                seen.push(Seen {
                    height,
                    txid: Some(tx.compute_txid()),
                    result: Err(LightError::OutputMismatch),
                });
                self.stop(LightError::OutputMismatch);
                return true;
            }
            self.member.outpoint = next_input;
            self.unroll_spk = Some(tx.output[next_input.vout as usize].script_pubkey.clone());
        } else {
            self.exited = true;
            self.member.leaf = None;
            self.unroll_spk = None;
        }
        seen.push(Seen {
            height,
            txid: Some(tx.compute_txid()),
            result: Ok(()),
        });
        true
    }

    fn raise(&mut self, seen: &mut Vec<Seen>, height: u32, e: LightError) {
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        if !p.raised.contains(&e) {
            p.raised.push(e.clone());
            seen.push(Seen {
                height,
                txid: Some(p.tx.compute_txid()),
                result: Err(e),
            });
        }
    }

    /// Try to resolve the pending spend and the missing unroll at height `h`.
    fn settle<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        h: u32,
        records: &mut dyn RecordSource,
        seen: &mut Vec<Seen>,
    ) {
        if self.stopped.is_some() || self.exited {
            return;
        }
        if let Some(p) = self.pending.clone() {
            let hashes = anchored_hashes(&p.tx);
            if hashes.is_empty() {
                self.raise(seen, h, LightError::RecordNotAnchored);
                self.stop(LightError::RecordNotAnchored);
                return;
            }
            let mut record = None;
            let mut forged = false;
            for want in &hashes {
                if let Some(bytes) = records.record(want) {
                    // Only bytes that hash to the anchored value count.
                    if tagged_hash(crate::rules::TAG_RECORD, &[&bytes]) != *want {
                        forged = true;
                        continue;
                    }
                    if let Ok(r) = Record::decode(&bytes) {
                        record = Some(r);
                        break;
                    }
                    forged = true;
                }
            }
            match record {
                Some(rec) => {
                    let txid = p.tx.compute_txid();
                    let result = self.member.follow(secp, &self.params, &p.tx, &rec);
                    self.pending = None;
                    seen.push(Seen {
                        height: p.height,
                        txid: Some(txid),
                        result: result.clone(),
                    });
                    match result {
                        Ok(()) => {
                            // A late record changes the tracked output. Scan its
                            // block again so neither same-block children nor
                            // later spends are skipped while catching up.
                            self.scanned = self.scanned.min(p.height.saturating_sub(1));
                            self.unroll = None;
                            self.await_unroll = (self.member.leaf.is_some()
                                && rec.next_unroll_key.is_some())
                            .then_some(AwaitUnroll {
                                since: p.height,
                                raised: false,
                            });
                        }
                        Err(e) => self.stop(e),
                    }
                }
                None => {
                    if forged {
                        self.raise(seen, h, LightError::RecordNotAnchored);
                    }
                    if h.saturating_sub(p.height) >= self.missing_after {
                        self.raise(seen, h, LightError::RecordMissing);
                    }
                }
            }
        }
        if self.stopped.is_none() {
            self.settle_unroll(secp, h, records, seen);
        }
    }

    fn settle_unroll<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        h: u32,
        records: &mut dyn RecordSource,
        seen: &mut Vec<Seen>,
    ) {
        let Some(a) = self.await_unroll.clone() else {
            return;
        };
        let (Some(leaf), outpoint) = (self.member.leaf, self.member.outpoint) else {
            self.await_unroll = None;
            return;
        };
        let output = self.output();
        let check = |bytes: &[u8]| {
            UnrollPackage::decode(bytes).is_ok_and(|pkg| {
                verify_branch(
                    secp,
                    &self.params,
                    output,
                    outpoint,
                    self.member.root.sum,
                    &pkg,
                    &leaf,
                )
                .is_ok()
            })
        };
        let held = records
            .unroll_matching(&outpoint, &mut { check })
            .and_then(|bytes| UnrollPackage::decode(&bytes).ok())
            .filter(|pkg| {
                verify_branch(
                    secp,
                    &self.params,
                    output,
                    outpoint,
                    self.member.root.sum,
                    pkg,
                    &leaf,
                )
                .is_ok()
            });
        if let Some(pkg) = held {
            self.unroll = Some(pkg);
            self.await_unroll = None;
        } else if !a.raised && h.saturating_sub(a.since) >= self.missing_after {
            self.await_unroll = Some(AwaitUnroll { raised: true, ..a });
            seen.push(Seen {
                height: h,
                txid: Some(outpoint.txid),
                result: Err(LightError::UnrollNotHeld),
            });
        }
    }

    fn stop(&mut self, e: LightError) {
        self.stopped = Some(e);
    }
}
