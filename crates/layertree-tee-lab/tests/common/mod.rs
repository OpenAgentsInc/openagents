//! A small world for the tests: an operator, members with keys, an
//! enclave, and a builder that turns an `Op` into a transaction the way a
//! client would.

#![allow(dead_code)]

use std::collections::HashMap;

use bitcoin::hashes::{Hash, sha256d};
use bitcoin::key::{Keypair, XOnlyPublicKey};
use bitcoin::secp256k1::{All, Message, Secp256k1, SecretKey, schnorr::Signature};
use bitcoin::transaction::Version;
use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    absolute::LockTime,
};

use layertree_tee_lab::attest::{FakeVendor, Platform, Report};
use layertree_tee_lab::enclave::{Config, Enclave, Signed};
use layertree_tee_lab::mirror::{self, MemMirror, Mirror, MirrorPolicy};
use layertree_tee_lab::output::{TreeOutput, ccv_tree_spk};
use layertree_tee_lab::rules::{self, Op, Reject, TreeParams};
use layertree_tee_lab::tree::{DUST_MIN, Leaf, Tree, tree_id, voucher_msg};
use layertree_tee_lab::unroll::UnrollPackage;

pub const MEASUREMENT: [u8; 48] = [7u8; 48];
pub const DEPTH: u32 = 8;
pub const DELAY: u16 = 144;
pub const LEASE: u32 = 1_000;

pub fn sk(i: u32) -> SecretKey {
    let mut b = [0u8; 32];
    b[..4].copy_from_slice(&(i + 1).to_be_bytes());
    b[31] = 0x42;
    SecretKey::from_slice(&b).unwrap()
}

pub struct World {
    pub secp: Secp256k1<All>,
    pub vendor: FakeVendor,
    pub report: Report,
    pub enclave: Enclave,
    pub keys: HashMap<[u8; 32], SecretKey>,
    pub operator: [u8; 32],
    pub genesis_unroll: UnrollPackage,
    pub last_unroll: Option<UnrollPackage>,
    fee_counter: u32,
    /// With the liveness rules on (`World::with_rules`): the mirrors whose
    /// receipts the enclave waits for. Empty: one-call `authorize`.
    pub mirrors: Vec<MemMirror>,
}

/// The guardian's key in every world (it can sign `HALT`).
pub fn guardian() -> SecretKey {
    sk(2)
}

pub fn xonly(secp: &Secp256k1<All>, k: &SecretKey) -> [u8; 32] {
    XOnlyPublicKey::from(k.public_key(secp)).serialize()
}

pub fn fake_outpoint(tag: u32) -> OutPoint {
    OutPoint {
        txid: Txid::from_raw_hash(sha256d::Hash::hash(&tag.to_le_bytes())),
        vout: 0,
    }
}

impl World {
    /// Owner key of member i is sk(100 + i), exit key sk(200 + i).
    pub fn member_leaf(&mut self, i: u32, amount: u64) -> Leaf {
        let owner = sk(100 + i);
        let exit = sk(200 + i);
        let o = xonly(&self.secp, &owner);
        let e = xonly(&self.secp, &exit);
        self.keys.insert(o, owner);
        self.keys.insert(e, exit);
        Leaf {
            amount,
            exit_key: e,
            owner_key: o,
            evictor_key: self.operator,
            evict_after: LEASE,
            nonce: i,
            seq: 0,
        }
    }

    /// A tree with `n` members at positions 0..n, member i holding
    /// 10_000 * (i + 1) sat.
    pub fn new(n: u32, radix: usize, migration_m: Option<[u8; 32]>) -> World {
        Self::with_funding(n, radix, migration_m, fake_outpoint(999), |_, _, _, _| {
            fake_outpoint(1)
        })
    }

    /// Test-only funding hook: construct the real creation transaction before
    /// opening the tree, so its escape signatures bind the actual outpoint.
    pub fn with_funding(
        n: u32,
        radix: usize,
        migration_m: Option<[u8; 32]>,
        creation_input: OutPoint,
        create: impl FnOnce(&Secp256k1<All>, &TreeParams, &Tree, XOnlyPublicKey) -> OutPoint,
    ) -> World {
        let secp = Secp256k1::new();
        let vendor = FakeVendor::new(&secp);
        let op_sk = sk(1);
        let operator = xonly(&secp, &op_sk);
        let cfg = Config {
            operator_key: operator,
            tree_id: tree_id(&creation_input.txid.to_byte_array(), creation_input.vout),
            depth: DEPTH,
            dust_min: DUST_MIN,
            unroll_delay: DELAY,
            migration_m,
            radix,
        };
        let (enclave, report) = Enclave::launch(cfg, MEASUREMENT, &vendor, Platform::SevSnp);
        let mut keys = HashMap::new();
        keys.insert(operator, op_sk);
        let mut w = World {
            secp,
            vendor,
            report,
            enclave,
            keys,
            operator,
            genesis_unroll: UnrollPackage {
                tree_outpoint: fake_outpoint(0),
                txs: vec![],
            },
            last_unroll: None,
            fee_counter: 1_000,
            mirrors: Vec::new(),
        };
        let mut tree = Tree::new(DEPTH).unwrap();
        for i in 0..n {
            let l = w.member_leaf(i, 10_000 * (i as u64 + 1));
            tree.insert(i as u64, l).unwrap();
        }
        let k0 = w.enclave.next_unroll_key();
        let outpoint = create(&w.secp, &w.params(), &tree, k0);
        let pkg = w.enclave.open(tree, outpoint).unwrap();
        w.genesis_unroll = pkg.clone();
        w.last_unroll = Some(pkg);
        w
    }

    /// `World::new`, and with `on` the spec's liveness rules switched on:
    /// two-phase signing through 2-of-3 mirror receipts (one mirror down),
    /// a guardian who could halt, and the forced-inclusion deadline, with
    /// the height advancing a block per step.
    pub fn with_rules(n: u32, radix: usize, migration_m: Option<[u8; 32]>, on: bool) -> World {
        let mut w = World::new(n, radix, migration_m);
        if on {
            w.mirrors = (0..3).map(|i| MemMirror::new(&sk(900 + i))).collect();
            w.mirrors[2].up = false;
            let keys = w.mirrors.iter().map(Mirror::key).collect();
            w.enclave.set_mirror_policy(MirrorPolicy {
                mirrors: keys,
                m: 2,
            });
            let g = XOnlyPublicKey::from(guardian().public_key(&w.secp));
            w.enclave.set_guardian(g);
            w.enclave.set_height(100);
        }
        w
    }

    pub fn params(&self) -> TreeParams {
        self.enclave.params().clone()
    }

    /// Build the transaction for `op`: input 0 the tree, input 1 a fee
    /// coin; output 0 the next tree output (or the CCV output for a
    /// migration, or a plain change output after a terminal exit), the
    /// payouts, then the record's OP_RETURN.
    pub fn build(&mut self, op: &Op) -> (Transaction, Vec<TxOut>, Vec<Signature>) {
        let p = self.params();
        let state = self.enclave.state().expect("open").clone();
        let next = self.enclave.next_unroll_key();
        let fx = rules::effects(&self.secp, &p, &state, op).expect("effects");
        let rec = rules::record(&state, &fx, op, next);
        let root = fx.tree.root().unwrap();
        let out0 = match op {
            Op::Migrate { m } => TxOut {
                value: Amount::from_sat(root.sum),
                script_pubkey: ccv_tree_spk(&self.secp, &root.hash, m),
            },
            _ if fx.terminal => TxOut {
                value: Amount::from_sat(1_000),
                script_pubkey: ScriptBuf::new_p2tr(
                    &self.secp,
                    XOnlyPublicKey::from_slice(&self.operator).unwrap(),
                    None,
                ),
            },
            _ => TxOut {
                value: Amount::from_sat(root.sum),
                script_pubkey: TreeOutput {
                    root_hash: root.hash,
                    unroll_key: next,
                }
                .script_pubkey(&self.secp, &p.enclave_key, p.unroll_delay),
            },
        };
        let mut outputs = vec![out0];
        outputs.extend(fx.payouts.iter().cloned());
        outputs.push(TxOut {
            value: Amount::ZERO,
            script_pubkey: rec.op_return(),
        });
        self.fee_counter += 1;
        let fee_in = fake_outpoint(self.fee_counter);
        let lock = fx.min_locktime.unwrap_or(0);
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::from_height(lock).unwrap(),
            input: vec![
                TxIn {
                    previous_output: state.outpoint,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                },
                TxIn {
                    previous_output: fee_in,
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                },
            ],
            output: outputs,
        };
        let fee_coin = TxOut {
            value: Amount::from_sat(100_000_000),
            script_pubkey: ScriptBuf::new_p2tr(
                &self.secp,
                XOnlyPublicKey::from_slice(&self.operator).unwrap(),
                None,
            ),
        };
        let prevouts = vec![state.txout(&self.secp, &p), fee_coin];
        let sigs = self.sign(&tx, &prevouts, op);
        (tx, prevouts, sigs)
    }

    pub fn sign(&self, tx: &Transaction, prevouts: &[TxOut], op: &Op) -> Vec<Signature> {
        let p = self.params();
        let state = self.enclave.state().unwrap();
        let msg = Message::from_digest(rules::sighash(tx, prevouts));
        rules::signers(&p, state, op)
            .unwrap()
            .iter()
            .map(|k| self.sign_with(k, &msg))
            .collect()
    }

    pub fn sign_with(&self, key: &[u8; 32], msg: &Message) -> Signature {
        let sk = self.keys.get(key).expect("known key");
        self.secp
            .sign_schnorr(msg, &Keypair::from_secret_key(&self.secp, sk))
    }

    pub fn voucher(&self, source: &Leaf, claim: &Leaf) -> Signature {
        let p = self.params();
        self.sign_with(
            &source.owner_key,
            &Message::from_digest(voucher_msg(&p.tree_id, source, claim)),
        )
    }

    /// Build, sign and authorize: in one call, or with the rules on in two
    /// phases with the mirrors' receipts.
    pub fn step(&mut self, op: Op) -> Result<Signed, Reject> {
        let (tx, prevouts, sigs) = self.build(&op);
        let r = if self.mirrors.is_empty() {
            self.enclave.authorize(tx, &prevouts, &op, &sigs)
        } else {
            let h = self.fee_counter;
            self.enclave.set_height(h);
            self.enclave.prepare(tx, &prevouts, &op).and_then(|prep| {
                let receipts =
                    mirror::collect(&prep.record, prep.unroll.as_ref(), &mut self.mirrors);
                self.enclave.finish(&sigs, &receipts)
            })
        };
        if let Ok(s) = &r {
            self.last_unroll = s.unroll.clone();
        }
        r
    }

    pub fn leaf(&self, pos: u64) -> Leaf {
        *self.enclave.state().unwrap().tree.get(pos).unwrap()
    }
}
