//! P0 on a real node (#11781): every transition the lab signs is broadcast
//! to bitcoind on regtest and mined, the tree output on chain is checked
//! against `TreeState::txout` after each step, then the enclave is dropped
//! and the pre-signed unroll runs: refused one block early (BIP-68), mined
//! exactly Δ blocks after the tree output, with a CPFP on each anchor, and
//! every leaf paid in full to its exit key. One and two unroll levels.
//!
//! Run through `scripts/layertree-regtest.sh`, which starts the node in
//! Docker and sets `LAYERTREE_REGTEST_RPC` (host:port) and
//! `LAYERTREE_REGTEST_ROOT` (where the `layertree.events.v1` logs go).
//! `LAYERTREE_WRITE_EVENTS=1` also refreshes `fixtures/events/regtest*.jsonl`.
//! Regtest only: fake coins, test keys, no money.

#![cfg(feature = "regtest")]

mod common;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bitcoin::consensus::encode::{deserialize_hex, serialize_hex};
use bitcoin::hashes::Hash;
use bitcoin::key::{Keypair, TapTweak, XOnlyPublicKey};
use bitcoin::secp256k1::{All, Message, Secp256k1, SecretKey};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    absolute::LockTime,
};
use serde_json::{Value, json};

use common::{DELAY, DEPTH, LEASE, MEASUREMENT, sk, xonly};
use layertree_tee_lab::attest::{FakeVendor, LogEntry, Platform, Policy, verify};
use layertree_tee_lab::enclave::{Config, Enclave};
use layertree_tee_lab::events::{Body, Network, Writer, read};
use layertree_tee_lab::light::Member;
use layertree_tee_lab::output::{TreeOutput, p2a_script, p2tr_raw};
use layertree_tee_lab::rules::{self, Op, TreeParams};
use layertree_tee_lab::tree::{DUST_MIN, Leaf, Tree, tree_id, voucher_msg};
use layertree_tee_lab::unroll::{UnrollPackage, verify_package};

/// `regtest:regtest-only`, the throwaway credentials the script starts
/// bitcoind with.
const AUTH: &str = "cmVndGVzdDpyZWd0ZXN0LW9ubHk=";
const WALLET: &str = "miner";
/// Each coin: covers the creation, a transition fee, a join's new value, or a CPFP.
const FEE_COIN: u64 = 1_000_000;
const TRANSITION_FEE: u64 = 2_000;
/// CPFP feerate for an unroll package (parent pays nothing), sat/vB.
const CPFP_RATE: u64 = 3;

// ---------------------------------------------------------------- the node

/// A bare JSON-RPC client over HTTP/1.0 (no extra dependencies).
struct Rpc {
    addr: String,
}

impl Rpc {
    fn raw(&self, path: &str, method: &str, params: Value) -> Result<Value, String> {
        let body =
            json!({"jsonrpc": "1.0", "id": "lt", "method": method, "params": params}).to_string();
        let mut s = TcpStream::connect(&self.addr).map_err(|e| format!("connect: {e}"))?;
        s.set_read_timeout(Some(Duration::from_secs(120))).ok();
        write!(
            s,
            "POST {path} HTTP/1.0\r\nHost: {}\r\nAuthorization: Basic {AUTH}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.addr,
            body.len()
        )
        .map_err(|e| format!("write: {e}"))?;
        let mut resp = String::new();
        s.read_to_string(&mut resp)
            .map_err(|e| format!("read: {e}"))?;
        let json_body = resp
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .ok_or("no HTTP body")?;
        let v: Value =
            serde_json::from_str(json_body).map_err(|e| format!("{method}: {e}: {json_body}"))?;
        match &v["error"] {
            Value::Null => Ok(v["result"].clone()),
            e => Err(e.to_string()),
        }
    }

    fn call(&self, method: &str, params: Value) -> Value {
        self.raw("/", method, params)
            .unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    fn wallet(&self, method: &str, params: Value) -> Value {
        self.raw(&format!("/wallet/{WALLET}"), method, params)
            .unwrap_or_else(|e| panic!("{method}: {e}"))
    }
}

struct Chain {
    rpc: Rpc,
    secp: Secp256k1<All>,
    miner: String,
    /// Confirmed coins under keys we hold, each spent once.
    coins: Vec<(OutPoint, TxOut, SecretKey)>,
    next_coin_key: u32,
}

fn sats(btc: &Value) -> u64 {
    (btc.as_f64().expect("amount") * 1e8).round() as u64
}

fn btc(sats: u64) -> Value {
    json!(sats as f64 / 1e8)
}

fn key_spk(secp: &Secp256k1<All>, k: &SecretKey) -> ScriptBuf {
    ScriptBuf::new_p2tr(secp, XOnlyPublicKey::from(k.public_key(secp)), None)
}

impl Chain {
    fn connect() -> Chain {
        let addr = std::env::var("LAYERTREE_REGTEST_RPC")
            .expect("LAYERTREE_REGTEST_RPC: run scripts/layertree-regtest.sh");
        let rpc = Rpc { addr };
        let start = Instant::now();
        while let Err(e) = rpc.raw("/", "getblockchaininfo", json!([])) {
            assert!(start.elapsed() < Duration::from_secs(90), "bitcoind: {e}");
            std::thread::sleep(Duration::from_millis(500));
        }
        let info = rpc.call("getnetworkinfo", json!([]));
        assert!(
            info["version"].as_u64().unwrap() >= 280_000,
            "TRUC and pay-to-anchor need Bitcoin Core 28 or later"
        );
        // One wallet for the whole run; a second test reuses it.
        if rpc.raw("/", "createwallet", json!([WALLET])).is_err() {
            let _ = rpc.raw("/", "loadwallet", json!([WALLET]));
        }
        let miner = rpc
            .raw(&format!("/wallet/{WALLET}"), "getnewaddress", json!([]))
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let c = Chain {
            rpc,
            secp: Secp256k1::new(),
            miner,
            coins: Vec::new(),
            next_coin_key: 0,
        };
        if c.height() < 101 {
            c.mine(101);
        }
        c
    }

    fn height(&self) -> u32 {
        self.rpc.call("getblockcount", json!([])).as_u64().unwrap() as u32
    }

    /// Mine `n` blocks; returns the new tip height.
    fn mine(&self, n: u32) -> u32 {
        self.rpc.call("generatetoaddress", json!([n, self.miner]));
        self.height()
    }

    fn mine_to(&self, height: u32) -> u32 {
        let h = self.height();
        assert!(h <= height, "already past {height}");
        if h < height {
            self.mine(height - h);
        }
        self.height()
    }

    fn tip_txids(&self) -> Vec<String> {
        let hash = self.rpc.call("getbestblockhash", json!([]));
        self.rpc.call("getblock", json!([hash, 1]))["tx"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect()
    }

    /// The wallet pays `amount` to `spk`; unconfirmed until the next block.
    fn pay(&self, spk: &ScriptBuf, amount: u64) -> (OutPoint, TxOut) {
        let addr = Address::from_script(spk, bitcoin::Network::Regtest).unwrap();
        let txid = self
            .rpc
            .wallet("sendtoaddress", json!([addr.to_string(), btc(amount)]));
        let hex = self.rpc.wallet("gettransaction", json!([txid]))["hex"]
            .as_str()
            .unwrap()
            .to_owned();
        let tx: Transaction = deserialize_hex(&hex).unwrap();
        let vout = tx
            .output
            .iter()
            .position(|o| o.script_pubkey == *spk && o.value.to_sat() == amount)
            .unwrap();
        (
            OutPoint {
                txid: tx.compute_txid(),
                vout: vout as u32,
            },
            tx.output[vout].clone(),
        )
    }

    /// `n` fresh coins under fresh test keys, confirmed.
    fn fund_coins(&mut self, n: usize) {
        for _ in 0..n {
            self.next_coin_key += 1;
            let k = sk(5_000 + self.next_coin_key);
            let (op, out) = self.pay(&key_spk(&self.secp, &k), FEE_COIN);
            self.coins.push((op, out, k));
        }
        self.mine(1);
    }

    fn coin(&mut self) -> (OutPoint, TxOut, SecretKey) {
        self.coins.pop().expect("fund more coins")
    }

    fn send(&self, tx: &Transaction) -> Txid {
        let txid = self
            .rpc
            .call("sendrawtransaction", json!([serialize_hex(tx)]));
        assert_eq!(txid.as_str().unwrap(), tx.compute_txid().to_string());
        tx.compute_txid()
    }

    /// The unspent output, mempool included, or `None` once spent.
    fn utxo(&self, op: &OutPoint) -> Option<TxOut> {
        let v = self
            .rpc
            .call("gettxout", json!([op.txid.to_string(), op.vout, true]));
        (!v.is_null()).then(|| TxOut {
            value: Amount::from_sat(sats(&v["value"])),
            script_pubkey: ScriptBuf::from_hex(v["scriptPubKey"]["hex"].as_str().unwrap()).unwrap(),
        })
    }

    /// `submitpackage`; `Err` carries the node's whole answer.
    fn submit_package(&self, txs: &[&Transaction]) -> Result<(), String> {
        let hexes: Vec<String> = txs.iter().map(|t| serialize_hex(*t)).collect();
        match self.rpc.raw("/", "submitpackage", json!([hexes])) {
            Err(e) => Err(e),
            Ok(v) if v["package_msg"] == "success" => Ok(()),
            Ok(v) => Err(v.to_string()),
        }
    }
}

/// Sign input `i` of `tx` by the key path of `key`'s plain P2TR output.
fn sign_key_input(
    secp: &Secp256k1<All>,
    tx: &mut Transaction,
    prevouts: &[TxOut],
    i: usize,
    key: &SecretKey,
) {
    let h = SighashCache::new(&*tx)
        .taproot_key_spend_signature_hash(i, &Prevouts::All(prevouts), TapSighashType::Default)
        .unwrap();
    let kp = Keypair::from_secret_key(secp, key)
        .tap_tweak(secp, None)
        .to_keypair();
    let sig = secp.sign_schnorr(&Message::from_digest(h.to_byte_array()), &kp);
    let mut w = Witness::new();
    w.push(sig.as_ref());
    tx.input[i].witness = w;
}

fn txin(op: OutPoint, sequence: Sequence) -> TxIn {
    TxIn {
        previous_output: op,
        script_sig: ScriptBuf::new(),
        sequence,
        witness: Witness::new(),
    }
}

/// A TRUC child spending `parent`'s zero-value anchor and one of our
/// coins, paying for both at `CPFP_RATE`. Returns it and its fee.
fn cpfp(chain: &mut Chain, parent: &Transaction) -> (Transaction, u64) {
    let anchor = parent
        .output
        .iter()
        .position(|o| o.script_pubkey == p2a_script())
        .expect("every unroll transaction has an anchor");
    let (op, coin, key) = chain.coin();
    // Child is ~160 vB: one key-path input, the anchor, one output.
    let fee = CPFP_RATE * (parent.vsize() as u64 + 160);
    let mut tx = Transaction {
        version: Version(3),
        lock_time: LockTime::ZERO,
        input: vec![
            txin(
                OutPoint {
                    txid: parent.compute_txid(),
                    vout: anchor as u32,
                },
                Sequence::ENABLE_RBF_NO_LOCKTIME,
            ),
            txin(op, Sequence::ENABLE_RBF_NO_LOCKTIME),
        ],
        output: vec![TxOut {
            value: Amount::from_sat(coin.value.to_sat() - fee),
            script_pubkey: coin.script_pubkey.clone(),
        }],
    };
    let prevouts = [parent.output[anchor].clone(), coin];
    sign_key_input(&chain.secp, &mut tx, &prevouts, 1, &key);
    (tx, fee)
}

// ------------------------------------------------------------ the scenario

struct Run {
    chain: Chain,
    enclave: Enclave,
    keys: BTreeMap<[u8; 32], SecretKey>,
    operator: [u8; 32],
    log: Writer,
    me: Member,
    unroll: UnrollPackage,
    /// Height at which the current tree output confirmed.
    confirmed_at: u32,
}

fn leaf(
    keys: &mut BTreeMap<[u8; 32], SecretKey>,
    secp: &Secp256k1<All>,
    i: u32,
    amount: u64,
    operator: [u8; 32],
) -> Leaf {
    let owner = sk(100 + i);
    let exit = sk(200 + i);
    let (o, e) = (xonly(secp, &owner), xonly(secp, &exit));
    keys.insert(o, owner);
    keys.insert(e, exit);
    Leaf {
        amount,
        exit_key: e,
        owner_key: o,
        evictor_key: operator,
        evict_after: LEASE,
        nonce: i,
        seq: 0,
    }
}

impl Run {
    fn params(&self) -> TreeParams {
        self.enclave.params().clone()
    }

    /// Every member checks the current unroll with public data only.
    fn check_unroll(&self) {
        let p = self.params();
        let st = self.enclave.state().unwrap();
        for l in st.tree.leaves().values() {
            verify_package(
                &self.chain.secp,
                &p,
                st.output,
                st.outpoint,
                st.amount(),
                st.tree.leaves(),
                &self.unroll,
                l,
            )
            .expect("member verifies its unroll");
        }
    }

    /// The tree output on chain is exactly what the enclave tracks.
    fn check_tree_output(&self) {
        let p = self.params();
        let st = self.enclave.state().unwrap();
        assert_eq!(
            self.chain.utxo(&st.outpoint),
            Some(st.txout(&self.chain.secp, &p)),
            "on-chain tree output equals TreeState::txout"
        );
    }

    /// Fund a tree of `n` members at positions 0..n under a fresh enclave:
    /// the creation transaction is built and the genesis unroll checked
    /// by every member before it is broadcast.
    fn open(n: u32, radix: usize) -> Run {
        let mut chain = Chain::connect();
        chain.fund_coins(16);
        let secp = chain.secp.clone();
        let vendor = FakeVendor::new(&secp);
        let op_sk = sk(1);
        let operator = xonly(&secp, &op_sk);
        let mut keys = BTreeMap::new();
        keys.insert(operator, op_sk);

        let (cin, cprev, ckey) = chain.coin();
        let cfg = Config {
            operator_key: operator,
            tree_id: tree_id(&cin.txid.to_byte_array(), cin.vout),
            depth: DEPTH,
            dust_min: DUST_MIN,
            unroll_delay: DELAY,
            migration_m: None,
            radix,
        };
        let (mut enclave, report) = Enclave::launch(cfg, MEASUREMENT, &vendor, Platform::SevSnp);
        let p = enclave.params().clone();
        let mut tree = Tree::new(DEPTH).unwrap();
        for i in 0..n {
            tree.insert(
                i as u64,
                leaf(&mut keys, &secp, i, 10_000 * (i as u64 + 1), operator),
            )
            .unwrap();
        }
        let root = tree.root().unwrap();
        let k0 = enclave.next_unroll_key();
        let out = TreeOutput {
            root_hash: root.hash,
            unroll_key: k0,
        };
        let mut create = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![txin(cin, Sequence::ENABLE_RBF_NO_LOCKTIME)],
            output: vec![
                TxOut {
                    value: Amount::from_sat(root.sum),
                    script_pubkey: out.script_pubkey(&secp, &p.enclave_key, p.unroll_delay),
                },
                TxOut {
                    value: Amount::from_sat(cprev.value.to_sat() - root.sum - TRANSITION_FEE),
                    script_pubkey: cprev.script_pubkey.clone(),
                },
            ],
        };
        sign_key_input(&secp, &mut create, &[cprev], 0, &ckey);
        let outpoint = OutPoint {
            txid: create.compute_txid(),
            vout: 0,
        };
        let unroll = enclave.open(tree, outpoint).unwrap();
        let st = enclave.state().unwrap().clone();
        let me = Member {
            pos: 2,
            leaf: st.tree.get(2).copied(),
            path: st.tree.path(2, 0).unwrap(),
            root: st.tree.root().unwrap(),
            outpoint,
            unroll_key: st.output.unroll_key,
        };
        let mut run = Run {
            chain,
            enclave,
            keys,
            operator,
            log: Writer::new(Network::Regtest),
            me,
            unroll,
            confirmed_at: 0,
        };
        run.check_unroll();
        run.chain.send(&create);
        run.confirmed_at = run.chain.mine(1);
        assert!(
            run.chain
                .tip_txids()
                .contains(&create.compute_txid().to_string())
        );
        run.check_tree_output();

        run.log
            .push(Some(run.confirmed_at), Body::tree_open(&p, &st));
        let entry = LogEntry {
            measurement: MEASUREMENT,
            source_commit: "regtest".into(),
        };
        let policy = Policy {
            vendor_root: vendor.root(&secp),
            releases: vec![entry.clone()],
        };
        run.log.push(
            None,
            Body::attestation(&report, Some(&entry), verify(&secp, &policy, &report, &p)),
        );
        run
    }

    /// Build the transaction for `op` the way a client would (input 0 the
    /// tree, input 1 a fee coin; output 0 the next tree output, payouts,
    /// the record's `OP_RETURN`, change), collect the signatures, let the
    /// enclave authorize it, broadcast, mine, and check the chain.
    fn step(&mut self, op: Op) {
        let secp = self.chain.secp.clone();
        let p = self.params();
        let before = self.enclave.state().unwrap().clone();
        let next = self.enclave.next_unroll_key();
        let fx = rules::effects(&secp, &p, &before, &op).expect("effects");
        let rec = rules::record(&before, &fx, &op, next);
        let root = fx.tree.root().unwrap();
        assert!(!fx.terminal, "the harness keeps the tree alive");
        let mut outputs = vec![TxOut {
            value: Amount::from_sat(root.sum),
            script_pubkey: TreeOutput {
                root_hash: root.hash,
                unroll_key: next,
            }
            .script_pubkey(&secp, &p.enclave_key, p.unroll_delay),
        }];
        outputs.extend(fx.payouts.iter().cloned());
        outputs.push(TxOut {
            value: Amount::ZERO,
            script_pubkey: rec.op_return(),
        });
        let (fop, fprev, fkey) = self.chain.coin();
        let tree_in = before.amount();
        let spent: u64 = outputs.iter().map(|o| o.value.to_sat()).sum();
        let change = tree_in + fprev.value.to_sat() - spent - TRANSITION_FEE;
        outputs.push(TxOut {
            value: Amount::from_sat(change),
            script_pubkey: fprev.script_pubkey.clone(),
        });
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::from_height(fx.min_locktime.unwrap_or(0)).unwrap(),
            input: vec![
                txin(before.outpoint, Sequence::ENABLE_RBF_NO_LOCKTIME),
                txin(fop, Sequence::ENABLE_RBF_NO_LOCKTIME),
            ],
            output: outputs,
        };
        let prevouts = vec![before.txout(&secp, &p), fprev];
        let msg = Message::from_digest(rules::sighash(&tx, &prevouts));
        let sigs: Vec<_> = rules::signers(&p, &before, &op)
            .unwrap()
            .iter()
            .map(|k| secp.sign_schnorr(&msg, &Keypair::from_secret_key(&secp, &self.keys[k])))
            .collect();
        let signed = self
            .enclave
            .authorize(tx, &prevouts, &op, &sigs)
            .expect("the enclave signs a valid transition");
        let mut tx = signed.tx;
        sign_key_input(&secp, &mut tx, &prevouts, 1, &fkey);

        let txid = self.chain.send(&tx);
        let h = self.chain.mine(1);
        assert!(
            self.chain.tip_txids().contains(&txid.to_string()),
            "{op:?} confirmed"
        );
        self.confirmed_at = h;
        self.unroll = signed.unroll.expect("the tree continues");
        assert_eq!(
            self.chain.utxo(&before.outpoint),
            None,
            "old tree output spent"
        );
        self.check_tree_output();
        self.check_unroll();

        self.log.push(
            Some(h),
            Body::transition(&op, &before, &signed.record, &txid),
        );
        if let Op::Exit { pos, .. } = op {
            let l = before.tree.get(pos).unwrap();
            let paid = self
                .chain
                .utxo(&OutPoint { txid, vout: 1 })
                .expect("payout output");
            assert_eq!(paid.value.to_sat(), l.amount);
            assert_eq!(paid.script_pubkey, p2tr_raw(&l.exit_key));
            self.log.push(Some(h), Body::payout(&l.exit_key, l.amount));
        }
        let seen = self.me.follow(&secp, &p, &tx, &signed.record);
        assert_eq!(seen, Ok(()), "light client follows {op:?}");
        self.log
            .push(Some(h), Body::light_client(self.me.pos, &seen));
        self.log.push(
            Some(h),
            Body::Block {
                spent: vec![before.outpoint.to_string()],
            },
        );
    }

    /// Join, update, claim, exit and a refresh heartbeat, each mined.
    fn transitions(&mut self) {
        let secp = self.chain.secp.clone();
        let joiner = leaf(&mut self.keys, &secp, 31, 9_000, self.operator);
        self.step(Op::Join {
            height: 2,
            index: 20,
            leaves: vec![(1, joiner)],
        });
        // The owner rotates its exit key; amount unchanged.
        let old = *self.enclave.state().unwrap().tree.get(0).unwrap();
        let rotated = leaf(&mut self.keys, &secp, 40, old.amount, self.operator);
        self.step(Op::Update {
            pos: 0,
            new_leaf: Leaf {
                exit_key: rotated.exit_key,
                nonce: old.nonce + 1,
                ..old
            },
        });
        let src = *self.enclave.state().unwrap().tree.get(3).unwrap();
        let mut claim = leaf(&mut self.keys, &secp, 30, 2_000, self.operator);
        claim.evictor_key = src.evictor_key;
        claim.evict_after = src.evict_after;
        let p = self.params();
        let voucher = secp.sign_schnorr(
            &Message::from_digest(voucher_msg(&p.tree_id, &src, &claim)),
            &Keypair::from_secret_key(&secp, &self.keys[&src.owner_key]),
        );
        self.step(Op::Claim {
            source_pos: 3,
            claim_pos: 70,
            claim_leaf: claim,
            voucher_sig: voucher,
        });
        self.step(Op::Exit {
            pos: 1,
            evict: false,
        });
        self.step(Op::Refresh);
    }

    /// The enclave is gone. The current unroll is refused until Δ blocks
    /// after the tree output, then confirms level by level, each package
    /// carried by a CPFP on its anchor; every leaf is paid exactly.
    /// Returns the number of levels.
    fn unroll_with_enclave_gone(self, name: &str) -> u32 {
        let st = self.enclave.state().unwrap().clone();
        let pkg = self.unroll.clone();
        let p = self.params();
        let c = self.confirmed_at;
        // The enclave dies: drop it and every secret with it.
        let Run {
            mut chain,
            enclave,
            mut log,
            ..
        } = self;
        drop(enclave);
        log.push(Some(c), Body::EnclaveDown {});

        let mut level: BTreeMap<Txid, u32> = BTreeMap::new();
        for tx in &pkg.txs {
            let l = level
                .get(&tx.input[0].previous_output.txid)
                .map_or(0, |p| p + 1);
            level.insert(tx.compute_txid(), l);
        }
        let levels = level.values().max().unwrap() + 1;
        let root = &pkg.txs[0];
        assert_eq!(root.input[0].previous_output, st.outpoint);

        // One block early: the next block would be Δ−1 after the tree
        // output's block, so BIP-68 refuses the package.
        let delta = u32::from(p.unroll_delay);
        chain.mine_to(c + delta - 2);
        let (child, _) = cpfp(&mut chain, root);
        let early = chain.submit_package(&[root, &child]);
        let why = early.expect_err("the unroll must wait Δ blocks");
        assert!(why.contains("non-BIP68-final"), "{why}");

        // Now the next block is Δ after the tree output's: accepted, and
        // mined in exactly that block.
        chain.mine(1);
        for l in 0..levels {
            let txs: Vec<&Transaction> = pkg
                .txs
                .iter()
                .filter(|t| level[&t.compute_txid()] == l)
                .collect();
            let tip = chain.height();
            for tx in &txs {
                let (child, fee) = cpfp(&mut chain, tx);
                chain
                    .submit_package(&[tx, &child])
                    .unwrap_or_else(|e| panic!("level {l}: {e}"));
                log.push(
                    Some(tip),
                    Body::UnrollBroadcast {
                        level: l,
                        txid: tx.compute_txid().to_string(),
                        cpfp_fee: fee,
                    },
                );
            }
            let h = chain.mine(1);
            if l == 0 {
                assert_eq!(h, c + delta, "the unroll confirms exactly Δ blocks later");
            }
            let mined = chain.tip_txids();
            for tx in &txs {
                let txid = tx.compute_txid().to_string();
                assert!(mined.contains(&txid), "level {l} tx {txid} mined");
                log.push(Some(h), Body::UnrollConfirmed { level: l, txid });
            }
        }

        // Every leaf paid its exact amount to its exit key, nothing else.
        let h = chain.height();
        let mut total = 0;
        for l in st.tree.leaves().values() {
            let spk = p2tr_raw(&l.exit_key);
            let (tx, vout) = pkg
                .txs
                .iter()
                .find_map(|t| {
                    t.output
                        .iter()
                        .position(|o| o.script_pubkey == spk && o.value.to_sat() == l.amount)
                        .map(|v| (t, v))
                })
                .expect("a payout per leaf");
            let got = chain.utxo(&OutPoint {
                txid: tx.compute_txid(),
                vout: vout as u32,
            });
            assert_eq!(
                got,
                Some(TxOut {
                    value: Amount::from_sat(l.amount),
                    script_pubkey: spk,
                })
            );
            total += l.amount;
            log.push(Some(h), Body::payout(&l.exit_key, l.amount));
        }
        assert_eq!(total, st.amount(), "the whole tree paid out");

        // The run log replays under the #11780 reader.
        let text = log.to_jsonl();
        let replay = read(&text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(replay.enclave_down);
        if let Some(dir) = std::env::var_os("LAYERTREE_REGTEST_ROOT") {
            std::fs::write(PathBuf::from(dir).join(format!("{name}.jsonl")), &text).unwrap();
        }
        if std::env::var_os("LAYERTREE_WRITE_EVENTS").is_some() {
            let f = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/events")
                .join(format!("{name}.jsonl"));
            std::fs::write(f, &text).unwrap();
        }
        levels
    }
}

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn transitions_confirm_and_a_flat_unroll_pays_everyone_after_delta() {
    let mut run = Run::open(6, 128);
    run.transitions();
    assert_eq!(run.unroll_with_enclave_gone("regtest"), 1);
}

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn a_two_level_unroll_pays_everyone_after_delta() {
    // radix 4, 9 leaves after the transitions below: 3 groups, then leaves.
    let mut run = Run::open(8, 4);
    run.transitions();
    assert_eq!(run.unroll_with_enclave_gone("regtest-two-level"), 2);
}
