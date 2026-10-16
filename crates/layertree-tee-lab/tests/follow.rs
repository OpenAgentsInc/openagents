//! `follow::Follower` against an in-memory chain (no Docker): blocks carry
//! the lab's signed transitions, BIP-158 filters are built the way a node
//! builds them, and records and unrolls come from an in-memory mirror. The
//! same follower runs against bitcoind in `tests/regtest.rs`.

mod common;

use std::collections::HashMap;

use bitcoin::bip158::BlockFilter;
use bitcoin::block::{Header, Version as BlockVersion};
use bitcoin::hashes::Hash;
use bitcoin::{
    Amount, Block, BlockHash, CompactTarget, OutPoint, ScriptBuf, Transaction, TxMerkleNode, TxOut,
};

use common::World;
use layertree_tee_lab::enclave::Signed;
use layertree_tee_lab::follow::{ChainSource, Follower, RecordSource, anchored_hashes};
use layertree_tee_lab::light::{LightError, Member};
use layertree_tee_lab::rules::{Op, TreeState};

#[derive(Default)]
struct MemChain {
    blocks: Vec<Block>,
    filters: Vec<Vec<u8>>,
    /// Every output ever created, for the filters' input scripts.
    coins: HashMap<OutPoint, ScriptBuf>,
}

impl MemChain {
    fn push(&mut self, txs: Vec<Transaction>, spent: &[(OutPoint, TxOut)]) -> u32 {
        for (op, o) in spent {
            self.coins.insert(*op, o.script_pubkey.clone());
        }
        let prev = self
            .blocks
            .last()
            .map_or(BlockHash::all_zeros(), Block::block_hash);
        let block = Block {
            header: Header {
                version: BlockVersion::TWO,
                prev_blockhash: prev,
                merkle_root: TxMerkleNode::all_zeros(),
                time: self.blocks.len() as u32,
                bits: CompactTarget::from_consensus(0x207f_ffff),
                nonce: 0,
            },
            // A stand-in coinbase first: filters skip the first
            // transaction's inputs, as for a real block.
            txdata: std::iter::once(Transaction {
                version: bitcoin::transaction::Version::TWO,
                lock_time: bitcoin::absolute::LockTime::from_height(self.blocks.len() as u32)
                    .unwrap(),
                input: vec![],
                output: vec![TxOut {
                    value: Amount::ZERO,
                    script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
                }],
            })
            .chain(txs)
            .collect(),
        };
        for tx in &block.txdata {
            for (vout, o) in tx.output.iter().enumerate() {
                self.coins.insert(
                    OutPoint {
                        txid: tx.compute_txid(),
                        vout: vout as u32,
                    },
                    o.script_pubkey.clone(),
                );
            }
        }
        let coins = &self.coins;
        let filter = BlockFilter::new_script_filter(&block, |op| {
            Ok::<_, bitcoin::bip158::Error>(coins.get(op).cloned().unwrap_or_default())
        })
        .unwrap();
        self.filters.push(filter.content);
        self.blocks.push(block);
        self.blocks.len() as u32 - 1
    }
}

impl ChainSource for MemChain {
    fn tip(&mut self) -> Result<u32, String> {
        Ok(self.blocks.len() as u32 - 1)
    }
    fn block_hash(&mut self, height: u32) -> Result<BlockHash, String> {
        Ok(self.blocks[height as usize].block_hash())
    }
    fn filter(&mut self, hash: &BlockHash) -> Result<Vec<u8>, String> {
        let i = self.blocks.iter().position(|b| b.block_hash() == *hash);
        Ok(self.filters[i.ok_or("no block")?].clone())
    }
    fn block(&mut self, hash: &BlockHash) -> Result<Block, String> {
        self.blocks
            .iter()
            .find(|b| b.block_hash() == *hash)
            .cloned()
            .ok_or_else(|| "no block".into())
    }
}

#[derive(Default)]
struct Mirror {
    records: HashMap<[u8; 32], Vec<u8>>,
    unrolls: HashMap<OutPoint, Vec<u8>>,
}

impl RecordSource for Mirror {
    fn record(&mut self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        self.records.get(hash).cloned()
    }
    fn unroll(&mut self, outpoint: &OutPoint) -> Option<Vec<u8>> {
        self.unrolls.get(outpoint).cloned()
    }
}

struct Lab {
    w: World,
    chain: MemChain,
    mirror: Mirror,
}

impl Lab {
    fn new() -> Lab {
        let w = World::new(6, 128, None);
        let st = w.enclave.state().unwrap().clone();
        let mut chain = MemChain::default();
        // Block 0 creates the tree output.
        let creation = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![st.txout(&w.secp, &w.params())],
        };
        chain.push(vec![creation], &[]);
        let mut mirror = Mirror::default();
        let g = w.genesis_unroll.clone();
        mirror.unrolls.insert(g.tree_outpoint, g.encode());
        Lab { w, chain, mirror }
    }

    fn follower(&self, pos: u64) -> Follower {
        let st = self.w.enclave.state().unwrap();
        Follower::new(
            Member {
                pos,
                leaf: st.tree.get(pos).copied(),
                path: st.tree.path(pos, 0).unwrap(),
                root: st.tree.root().unwrap(),
                outpoint: st.outpoint,
                unroll_key: st.output.unroll_key,
            },
            self.w.params(),
            0,
            2,
            Some(self.w.genesis_unroll.clone()),
        )
    }

    /// Step, put the transaction in a new block; publish per the flags.
    fn step(&mut self, op: Op, record: bool, unroll: bool) -> (Signed, u32) {
        let before: TreeState = self.w.enclave.state().unwrap().clone();
        let s = self.w.step(op).unwrap();
        let prev = before.txout(&self.w.secp, &self.w.params());
        let h = self
            .chain
            .push(vec![s.tx.clone()], &[(before.outpoint, prev)]);
        self.publish(&s, record, unroll);
        (s, h)
    }

    fn publish(&mut self, s: &Signed, record: bool, unroll: bool) {
        if record {
            self.mirror
                .records
                .insert(s.record.hash(), s.record.encode());
        }
        if let (true, Some(pkg)) = (unroll, &s.unroll) {
            self.mirror.unrolls.insert(pkg.tree_outpoint, pkg.encode());
        }
    }

    fn empty_block(&mut self) -> u32 {
        self.chain.push(vec![], &[])
    }

    fn poll(&mut self, f: &mut Follower) -> Vec<(u32, Result<(), LightError>)> {
        let secp = self.w.secp.clone();
        f.poll(&secp, &mut self.chain, &mut self.mirror)
            .unwrap()
            .into_iter()
            .map(|s| (s.height, s.result))
            .collect()
    }

    fn check_path(&self, f: &Follower) {
        let st = self.w.enclave.state().unwrap();
        assert_eq!(f.member.root, st.tree.root().unwrap());
        assert_eq!(f.member.path, st.tree.path(f.member.pos, 0).unwrap());
        assert_eq!(f.member.outpoint, st.outpoint);
    }
}

#[test]
fn follows_honest_spends_found_by_block_filters() {
    let mut lab = Lab::new();
    let mut fs = vec![lab.follower(0), lab.follower(2), lab.follower(4)];
    let joiner = lab.w.member_leaf(40, 5_000);
    let ops = vec![
        Op::Exit {
            pos: 1,
            evict: false,
        },
        Op::Join {
            height: 0,
            index: 60,
            leaves: vec![(0, joiner)],
        },
        Op::Refresh,
    ];
    for op in ops {
        let (_, h) = lab.step(op, true, true);
        // A block that does not touch the tree: the filter skips it.
        lab.empty_block();
        for f in fs.iter_mut() {
            assert_eq!(lab.poll(f), vec![(h, Ok(()))]);
            lab.check_path(f);
            assert!(f.unroll.is_some());
        }
    }
}

#[test]
fn withheld_and_forged_records_and_a_withheld_unroll_alarm() {
    let mut lab = Lab::new();
    let mut f = lab.follower(2);

    // Withheld: nothing at the spend, RecordMissing two blocks later.
    let (s, h) = lab.step(Op::Refresh, false, true);
    assert_eq!(lab.poll(&mut f), vec![]);
    lab.empty_block();
    assert_eq!(lab.poll(&mut f), vec![]);
    let h2 = lab.empty_block();
    assert_eq!(lab.poll(&mut f), vec![(h2, Err(LightError::RecordMissing))]);
    lab.publish(&s, true, false);
    assert_eq!(lab.poll(&mut f), vec![(h, Ok(()))]);
    lab.check_path(&f);

    // Forged: other bytes under the anchored hash are refused.
    let (s, h) = lab.step(Op::Refresh, false, true);
    let hash = anchored_hashes(&s.tx)[0];
    lab.mirror.records.insert(hash, vec![1, 2, 3]);
    assert_eq!(
        lab.poll(&mut f),
        vec![(h, Err(LightError::RecordNotAnchored))]
    );
    lab.publish(&s, true, false);
    assert_eq!(lab.poll(&mut f), vec![(h, Ok(()))]);

    // The unroll for the new state is withheld.
    let (s, h) = lab.step(Op::Refresh, true, false);
    assert_eq!(lab.poll(&mut f), vec![(h, Ok(()))]);
    assert!(f.unroll.is_none());
    lab.empty_block();
    assert_eq!(lab.poll(&mut f), vec![]);
    let h2 = lab.empty_block();
    assert_eq!(lab.poll(&mut f), vec![(h2, Err(LightError::UnrollNotHeld))]);
    // A package for another state does not count.
    let other = lab.w.genesis_unroll.clone();
    lab.mirror.unrolls.insert(f.member.outpoint, other.encode());
    lab.empty_block();
    assert_eq!(lab.poll(&mut f), vec![]);
    assert!(f.unroll.is_none());
    lab.publish(&s, false, true);
    assert_eq!(lab.poll(&mut f), vec![]);
    assert!(f.unroll.is_some());
}

#[test]
fn a_mined_theft_stops_every_follower_in_its_block() {
    let mut lab = Lab::new();
    let mut fs = vec![lab.follower(0), lab.follower(3), lab.follower(5)];
    lab.w.enclave.break_for_test();
    let op = Op::Refresh;
    let before = lab.w.enclave.state().unwrap().clone();
    let (mut tx, prevouts, sigs) = lab.w.build(&op);
    tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() - 50_000);
    let s = lab.w.enclave.authorize(tx, &prevouts, &op, &sigs).unwrap();
    let prev = before.txout(&lab.w.secp, &lab.w.params());
    let h = lab
        .chain
        .push(vec![s.tx.clone()], &[(before.outpoint, prev)]);
    lab.publish(&s, true, true);
    for f in fs.iter_mut() {
        assert_eq!(lab.poll(f), vec![(h, Err(LightError::OutputMismatch))]);
        assert_eq!(f.stopped, Some(LightError::OutputMismatch));
        // Stopped: later blocks change nothing.
        lab.empty_block();
        assert_eq!(lab.poll(f), vec![]);
    }
}
