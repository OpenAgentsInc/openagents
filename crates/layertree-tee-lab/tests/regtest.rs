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
//!
//! The light-client scenario (#11782) runs three `follow::Follower`s
//! against the same node: BIP-158 filters find each spend, records and
//! unrolls come from a directory or an HTTP mirror stub, and withheld,
//! forged or rule-breaking steps raise their alarms.
//!
//! Regtest only: fake coins, test keys, no money.

#![cfg(feature = "regtest")]

mod common;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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
use layertree_tee_lab::enclave::{Config, Enclave, Signed, halt_msg};
use layertree_tee_lab::events::{Body, Network, Writer, read};
use layertree_tee_lab::follow::{
    ChainSource, DirSource, Follower, HttpMirror, RecordSource, Seen, record_name, unroll_name,
};
use layertree_tee_lab::light::{LightError, Member};
use layertree_tee_lab::output::{TreeOutput, p2a_script, p2tr_raw};
use layertree_tee_lab::rules::{self, Op, Record, Reject, TreeParams, TreeState};
use layertree_tee_lab::tree::{DUST_MIN, Leaf, Tree, tree_id, voucher_msg};
use layertree_tee_lab::unroll::{UnrollPackage, verify_package};

/// `regtest:regtest-only`, the throwaway credentials the script starts
/// bitcoind with.
const AUTH: &str = "cmVndGVzdDpyZWd0ZXN0LW9ubHk=";
const WALLET: &str = "miner";
/// Each coin: covers the creation, a transition fee, a join's new value, or a CPFP.
const FEE_COIN: u64 = 1_000_000;
const TRANSITION_FEE: u64 = 2_000;
/// CPFP feerates for unroll packages (the parent pays nothing), sat/vB;
/// unroll transactions take them in turn.
const CPFP_RATES: [u64; 3] = [1, 10, 50];

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
        // One sendmany per hundred coins: no long unconfirmed chains.
        let mut left = n;
        while left > 0 {
            let m = left.min(100);
            left -= m;
            let mut amounts = serde_json::Map::new();
            let mut fresh = Vec::new();
            for _ in 0..m {
                self.next_coin_key += 1;
                let k = sk(5_000 + self.next_coin_key);
                let spk = key_spk(&self.secp, &k);
                let addr = Address::from_script(&spk, bitcoin::Network::Regtest).unwrap();
                amounts.insert(addr.to_string(), btc(FEE_COIN));
                fresh.push((spk, k));
            }
            let txid = self.rpc.wallet("sendmany", json!(["", amounts]));
            let hex = self.rpc.wallet("gettransaction", json!([txid]))["hex"]
                .as_str()
                .unwrap()
                .to_owned();
            let tx: Transaction = deserialize_hex(&hex).unwrap();
            for (spk, k) in fresh {
                let vout = tx
                    .output
                    .iter()
                    .position(|o| o.script_pubkey == spk)
                    .unwrap();
                let op = OutPoint {
                    txid: tx.compute_txid(),
                    vout: vout as u32,
                };
                self.coins.push((op, tx.output[vout].clone(), k));
            }
        }
        self.mine(1);
    }

    /// One confirmed coin of `amount` under a fresh key.
    fn big_coin(&mut self, amount: u64) -> (OutPoint, TxOut, SecretKey) {
        self.next_coin_key += 1;
        let k = sk(5_000 + self.next_coin_key);
        let (op, out) = self.pay(&key_spk(&self.secp, &k), amount);
        self.mine(1);
        (op, out, k)
    }

    /// The node's vsize and base fee for a transaction in its mempool.
    fn mempool_cost(&self, txid: &Txid) -> (u64, u64) {
        let e = self.rpc.call("getmempoolentry", json!([txid.to_string()]));
        (e["vsize"].as_u64().unwrap(), sats(&e["fees"]["base"]))
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
    fn submit_package(&self, txs: &[&Transaction]) -> Result<Value, String> {
        let hexes: Vec<String> = txs.iter().map(|t| serialize_hex(*t)).collect();
        match self.rpc.raw("/", "submitpackage", json!([hexes])) {
            Err(e) => Err(e),
            Ok(v) if v["package_msg"] == "success" => Ok(v),
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
/// coins, paying for both at `rate` sat/vB. Returns it and its fee.
fn cpfp(chain: &mut Chain, parent: &Transaction, rate: u64) -> (Transaction, u64) {
    let anchor = parent
        .output
        .iter()
        .position(|o| o.script_pubkey == p2a_script())
        .expect("every unroll transaction has an anchor");
    let (op, coin, key) = chain.coin();
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
            value: coin.value,
            script_pubkey: coin.script_pubkey.clone(),
        }],
    };
    let prevouts = [parent.output[anchor].clone(), coin.clone()];
    // A signature is always 64 bytes, so the size is known after one pass.
    sign_key_input(&chain.secp, &mut tx, &prevouts, 1, &key);
    let fee = rate * (parent.vsize() + tx.vsize()) as u64;
    tx.output[0].value = Amount::from_sat(coin.value.to_sat() - fee);
    sign_key_input(&chain.secp, &mut tx, &prevouts, 1, &key);
    (tx, fee)
}

impl ChainSource for Chain {
    fn tip(&mut self) -> Result<u32, String> {
        Ok(self.height())
    }
    fn block_hash(&mut self, height: u32) -> Result<bitcoin::BlockHash, String> {
        let v = self.rpc.raw("/", "getblockhash", json!([height]))?;
        v.as_str()
            .ok_or("hash")?
            .parse()
            .map_err(|e| format!("{e:?}"))
    }
    fn filter(&mut self, hash: &bitcoin::BlockHash) -> Result<Vec<u8>, String> {
        let v = self
            .rpc
            .raw("/", "getblockfilter", json!([hash.to_string(), "basic"]))?;
        let hex = v["filter"].as_str().ok_or("filter")?;
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
    fn block(&mut self, hash: &bitcoin::BlockHash) -> Result<bitcoin::Block, String> {
        let v = self
            .rpc
            .raw("/", "getblock", json!([hash.to_string(), 0]))?;
        deserialize_hex(v.as_str().ok_or("block")?).map_err(|e| e.to_string())
    }
}

/// A mirror stub: serves the files of `dir` over HTTP/1.0 on a free local
/// port until the process exits. Returns host:port.
fn serve_dir(dir: PathBuf) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            // Read the whole request head before answering.
            let mut head = Vec::new();
            let mut buf = [0u8; 512];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buf[..n]),
                }
            }
            let req = String::from_utf8_lossy(&head);
            let name = req
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/')
                .to_owned();
            let file = (!name.contains('/') && !name.contains(".."))
                .then(|| std::fs::read(dir.join(&name)).ok())
                .flatten();
            let _ = match file {
                Some(b) => s
                    .write_all(
                        format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n", b.len())
                            .as_bytes(),
                    )
                    .and_then(|_| s.write_all(&b)),
                None => s.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n"),
            };
        }
    });
    addr
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
    /// Where records and unrolls are published for followers (#11782).
    store: Option<PathBuf>,
    withhold_record: bool,
    forge_record: bool,
    withhold_unroll: bool,
    last_record: Option<Record>,
    last_unroll_pkg: Option<UnrollPackage>,
    /// The node's (vsize, fee) for the last transition (#11785).
    last_cost: (u64, u64),
}

struct Built {
    tx: Transaction,
    prevouts: Vec<TxOut>,
    sigs: Vec<bitcoin::secp256k1::schnorr::Signature>,
    fee_key: SecretKey,
    before: TreeState,
}

fn leaf(
    keys: &mut BTreeMap<[u8; 32], SecretKey>,
    secp: &Secp256k1<All>,
    i: u32,
    amount: u64,
    operator: [u8; 32],
) -> Leaf {
    // Members below 100 keep the lab's key scheme; above, keys that never
    // collide with another member's.
    let (owner, exit) = if i < 100 {
        (sk(100 + i), sk(200 + i))
    } else {
        (sk(1_000_000 + 2 * i), sk(1_000_001 + 2 * i))
    };
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
    /// Large trees check a sample: each call already checks that every
    /// leaf is paid; the sample varies only whose branch is extracted.
    fn check_unroll(&self) {
        let p = self.params();
        let st = self.enclave.state().unwrap();
        let all: Vec<&Leaf> = st.tree.leaves().values().collect();
        let sample: Vec<&Leaf> = if all.len() <= 64 {
            all
        } else {
            vec![all[0], all[1], all[all.len() / 2], all[all.len() - 1]]
        };
        for l in sample {
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
        Run::open_with(n, radix, DEPTH, 16, &|i| 10_000 * (u64::from(i) + 1))
    }

    /// A tree of `n` members at positions 0..n holding `amount(i)`, in a
    /// tree of depth `depth`, with `coins` fee coins funded.
    fn open_with(
        n: u32,
        radix: usize,
        depth: u32,
        coins: usize,
        amount: &dyn Fn(u32) -> u64,
    ) -> Run {
        let mut chain = Chain::connect();
        chain.fund_coins(coins);
        let secp = chain.secp.clone();
        let vendor = FakeVendor::new(&secp);
        let op_sk = sk(1);
        let operator = xonly(&secp, &op_sk);
        let mut keys = BTreeMap::new();
        keys.insert(operator, op_sk);

        let mut tree = Tree::new(depth).unwrap();
        for i in 0..n {
            tree.insert(u64::from(i), leaf(&mut keys, &secp, i, amount(i), operator))
                .unwrap();
        }
        let root = tree.root().unwrap();
        let (cin, cprev, ckey) = chain.big_coin(root.sum + 100_000);
        let cfg = Config {
            operator_key: operator,
            tree_id: tree_id(&cin.txid.to_byte_array(), cin.vout),
            depth,
            dust_min: DUST_MIN,
            unroll_delay: DELAY,
            migration_m: None,
            radix,
        };
        let (mut enclave, report) = Enclave::launch(cfg, MEASUREMENT, &vendor, Platform::SevSnp);
        let p = enclave.params().clone();
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
            store: None,
            withhold_record: false,
            forge_record: false,
            withhold_unroll: false,
            last_record: None,
            last_unroll_pkg: None,
            last_cost: (0, 0),
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
    /// the record's `OP_RETURN`, change) and collect the signatures.
    /// `steal` moves that much from output 0 to the change: only a broken
    /// enclave signs that.
    fn build(&mut self, op: &Op, steal: u64) -> Built {
        let secp = self.chain.secp.clone();
        let p = self.params();
        let before = self.enclave.state().unwrap().clone();
        let next = self.enclave.next_unroll_key();
        let fx = rules::effects(&secp, &p, &before, op).expect("effects");
        let rec = rules::record(&before, &fx, op, next);
        let root = fx.tree.root().unwrap();
        assert!(!fx.terminal, "the harness keeps the tree alive");
        let mut outputs = vec![TxOut {
            value: Amount::from_sat(root.sum - steal),
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
        let sigs: Vec<_> = rules::signers(&p, &before, op)
            .unwrap()
            .iter()
            .map(|k| secp.sign_schnorr(&msg, &Keypair::from_secret_key(&secp, &self.keys[k])))
            .collect();
        Built {
            tx,
            prevouts,
            sigs,
            fee_key: fkey,
            before,
        }
    }

    /// Have the enclave authorize, sign the fee input, broadcast and mine.
    fn mine_signed(&mut self, op: &Op, b: Built) -> (Transaction, Signed, u32) {
        let signed = self
            .enclave
            .authorize(b.tx, &b.prevouts, op, &b.sigs)
            .expect("the enclave signs");
        let mut tx = signed.tx.clone();
        sign_key_input(&self.chain.secp, &mut tx, &b.prevouts, 1, &b.fee_key);
        let txid = self.chain.send(&tx);
        self.last_cost = self.chain.mempool_cost(&txid);
        let h = self.chain.mine(1);
        assert!(
            self.chain.tip_txids().contains(&txid.to_string()),
            "{op:?} confirmed"
        );
        self.publish(&signed);
        (tx, signed, h)
    }

    /// Publish the record and the new state's unroll where followers look,
    /// unless this step withholds or forges them.
    fn publish(&mut self, signed: &Signed) {
        let Some(dir) = self.store.clone() else {
            return;
        };
        let hash = signed.record.hash();
        let bytes = if self.forge_record {
            // A mirror serves other bytes under this record's hash.
            self.last_record
                .as_ref()
                .map(|r| r.encode())
                .unwrap_or_else(|| vec![0; 8])
        } else {
            signed.record.encode()
        };
        if !self.withhold_record {
            std::fs::write(dir.join(record_name(&hash)), bytes).unwrap();
        }
        if let (Some(pkg), false) = (&signed.unroll, self.withhold_unroll) {
            std::fs::write(dir.join(unroll_name(&pkg.tree_outpoint)), pkg.encode()).unwrap();
        }
        self.last_record = Some(signed.record.clone());
        self.last_unroll_pkg = signed.unroll.clone();
    }

    /// Publish what this step withheld or forged.
    fn publish_late(&mut self) {
        let dir = self.store.clone().unwrap();
        let rec = self.last_record.clone().unwrap();
        std::fs::write(dir.join(record_name(&rec.hash())), rec.encode()).unwrap();
        if let Some(pkg) = &self.last_unroll_pkg {
            std::fs::write(dir.join(unroll_name(&pkg.tree_outpoint)), pkg.encode()).unwrap();
        }
        self.withhold_record = false;
        self.forge_record = false;
        self.withhold_unroll = false;
    }

    /// Build, authorize, broadcast and mine `op`, then check the chain.
    fn step(&mut self, op: Op) -> u32 {
        let secp = self.chain.secp.clone();
        let p = self.params();
        let b = self.build(&op, 0);
        let before = b.before.clone();
        let (tx, signed, h) = self.mine_signed(&op, b);
        let txid = tx.compute_txid();
        self.confirmed_at = h;
        self.unroll = signed.unroll.clone().expect("the tree continues");
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
        h
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
    /// carried by a CPFP on its anchor at 1, 10 or 50 sat/vB in turn;
    /// every leaf is paid exactly. Measures sizes and timing on the node.
    fn unroll_with_enclave_gone(self, name: &str) -> UnrollStats {
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
        let (child, _) = cpfp(&mut chain, root, 1);
        let early = chain.submit_package(&[root, &child]);
        let why = early.expect_err("the unroll must wait Δ blocks");
        assert!(why.contains("non-BIP68-final"), "{why}");

        // Now the next block is Δ after the tree output's: accepted, and
        // mined in exactly that block.
        chain.mine(1);
        let accepted_at = chain.height();
        let mut stats = UnrollStats {
            levels,
            blocks_to_accept: accepted_at + 1 - c,
            ..UnrollStats::default()
        };
        let mut turn = 0usize;
        for l in 0..levels {
            let txs: Vec<&Transaction> = pkg
                .txs
                .iter()
                .filter(|t| level[&t.compute_txid()] == l)
                .collect();
            let tip = chain.height();
            let mut lv = LevelStats {
                txs: txs.len() as u32,
                ..LevelStats::default()
            };
            for tx in &txs {
                let rate = CPFP_RATES[turn % CPFP_RATES.len()];
                turn += 1;
                let (child, fee) = cpfp(&mut chain, tx, rate);
                let answer = chain
                    .submit_package(&[tx, &child])
                    .unwrap_or_else(|e| panic!("level {l}: {e}"));
                // The node's own sizes and the package feerate it applied.
                let results = answer["tx-results"].as_object().unwrap();
                let by_txid = |t: &Transaction| {
                    results
                        .values()
                        .find(|r| r["txid"] == t.compute_txid().to_string())
                        .unwrap_or_else(|| panic!("{answer}"))
                        .clone()
                };
                let (pr, cr) = (by_txid(tx), by_txid(&child));
                let (pv, cv) = (pr["vsize"].as_u64().unwrap(), cr["vsize"].as_u64().unwrap());
                assert_eq!(pv, tx.vsize() as u64);
                assert_eq!(cv, child.vsize() as u64);
                assert!(pv <= 10_000, "TRUC parent limit");
                assert!(cv <= 1_000, "TRUC child limit");
                let eff = (cr["fees"]["effective-feerate"].as_f64().unwrap() * 1e5).round() as u64;
                assert!(eff >= rate, "package feerate {eff} sat/vB below {rate}");
                lv.max_vsize = lv.max_vsize.max(pv);
                lv.min_vsize = if lv.min_vsize == 0 {
                    pv
                } else {
                    lv.min_vsize.min(pv)
                };
                lv.max_outputs = lv.max_outputs.max(tx.output.len() as u32);
                stats.child_vsize = stats.child_vsize.max(cv);
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
            let mined: std::collections::HashSet<String> = chain.tip_txids().into_iter().collect();
            for tx in &txs {
                let txid = tx.compute_txid().to_string();
                assert!(mined.contains(&txid), "level {l} tx {txid} mined");
                log.push(Some(h), Body::UnrollConfirmed { level: l, txid });
            }
            for (what, value, unit) in [
                ("unroll_level_txs", u64::from(lv.txs), "txs"),
                ("unroll_tx_vsize_max", lv.max_vsize, "vB"),
                ("unroll_tx_vsize_min", lv.min_vsize, "vB"),
            ] {
                log.push(
                    Some(h),
                    Body::Measured {
                        what: what.into(),
                        value,
                        unit: unit.into(),
                        level: Some(l),
                    },
                );
            }
            stats.per_level.push(lv);
        }
        let paid_at = chain.height();
        stats.blocks_to_paid = paid_at - (accepted_at + 1);

        // Every leaf paid its exact amount to its exit key, nothing else.
        let mut payouts: std::collections::HashMap<(ScriptBuf, u64), Vec<OutPoint>> =
            std::collections::HashMap::new();
        for t in &pkg.txs {
            for (vout, o) in t.output.iter().enumerate() {
                payouts
                    .entry((o.script_pubkey.clone(), o.value.to_sat()))
                    .or_default()
                    .push(OutPoint {
                        txid: t.compute_txid(),
                        vout: vout as u32,
                    });
            }
        }
        let n = st.tree.leaves().len();
        let mut total = 0;
        for (k, l) in st.tree.leaves().values().enumerate() {
            let spk = p2tr_raw(&l.exit_key);
            let op = payouts
                .get_mut(&(spk.clone(), l.amount))
                .and_then(Vec::pop)
                .expect("a payout per leaf");
            // Every mined transaction was checked above; on big trees the
            // UTXO set is sampled.
            if n <= 256 || k % 97 == 0 || k + 1 == n {
                assert_eq!(
                    chain.utxo(&op),
                    Some(TxOut {
                        value: Amount::from_sat(l.amount),
                        script_pubkey: spk,
                    })
                );
            }
            total += l.amount;
            log.push(Some(paid_at), Body::payout(&l.exit_key, l.amount));
        }
        assert_eq!(total, st.amount(), "the whole tree paid out");
        for (what, value, unit) in [
            (
                "unroll_blocks_to_accept",
                u64::from(stats.blocks_to_accept),
                "blocks",
            ),
            (
                "unroll_blocks_to_paid",
                u64::from(stats.blocks_to_paid),
                "blocks",
            ),
            ("cpfp_child_vsize", stats.child_vsize, "vB"),
        ] {
            log.push(
                Some(paid_at),
                Body::Measured {
                    what: what.into(),
                    value,
                    unit: unit.into(),
                    level: None,
                },
            );
        }

        // The run log replays under the #11780 reader.
        let text = log.to_jsonl();
        let replay = read(&text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(replay.enclave_down);
        if let Some(dir) = std::env::var_os("LAYERTREE_REGTEST_ROOT") {
            std::fs::write(PathBuf::from(dir).join(format!("{name}.jsonl")), &text).unwrap();
        }
        let fixture = ["regtest", "regtest-two-level"].contains(&name);
        if fixture && std::env::var_os("LAYERTREE_WRITE_EVENTS").is_some() {
            let f = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/events")
                .join(format!("{name}.jsonl"));
            std::fs::write(f, &text).unwrap();
        }
        stats
    }

    /// Log a measured number for the last transition.
    fn measure(&mut self, what: &str, value: u64, unit: &str) {
        self.log.push(
            Some(self.confirmed_at),
            Body::Measured {
                what: what.into(),
                value,
                unit: unit.into(),
                level: None,
            },
        );
    }
}

#[derive(Debug, Default)]
struct LevelStats {
    txs: u32,
    max_outputs: u32,
    max_vsize: u64,
    min_vsize: u64,
}

#[derive(Debug, Default)]
struct UnrollStats {
    levels: u32,
    /// Blocks from the last tree output's block to the block that mines
    /// the unroll's root (Δ when on time).
    blocks_to_accept: u32,
    /// Further blocks until every leaf is paid (one per extra level).
    blocks_to_paid: u32,
    per_level: Vec<LevelStats>,
    child_vsize: u64,
}

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn transitions_confirm_and_a_flat_unroll_pays_everyone_after_delta() {
    let mut run = Run::open(6, 128);
    run.transitions();
    assert_eq!(run.unroll_with_enclave_gone("regtest").levels, 1);
}

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn a_two_level_unroll_pays_everyone_after_delta() {
    // radix 4, 9 leaves after the transitions below: 3 groups, then leaves.
    let mut run = Run::open(8, 4);
    run.transitions();
    assert_eq!(run.unroll_with_enclave_gone("regtest-two-level").levels, 2);
}

// ------------------------------------------------------- light clients (#11782)

/// Blocks a follower waits for a record or an unroll before alarming.
const MISSING_AFTER: u32 = 3;

struct Watch {
    f: Follower,
    src: Box<dyn RecordSource>,
}

/// Every follower polls; each observation goes into the run log.
fn poll_all(run: &mut Run, ws: &mut [Watch]) -> Vec<Vec<Seen>> {
    let secp = run.chain.secp.clone();
    let out: Vec<Vec<Seen>> = ws
        .iter_mut()
        .map(|w| w.f.poll(&secp, &mut run.chain, w.src.as_mut()).unwrap())
        .collect();
    for (w, seen) in ws.iter().zip(&out) {
        for s in seen {
            run.log.push(
                Some(s.height),
                Body::light_client(w.f.member.pos, &s.result),
            );
        }
    }
    out
}

/// Each follower saw exactly `want` at height `h`.
fn expect_each(seen: &[Vec<Seen>], h: u32, want: Result<(), LightError>) {
    for s in seen {
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0].height, h, "{s:?}");
        assert_eq!(s[0].result, want, "{s:?}");
    }
}

/// Each follower's path, leaf, root and outpoint equal the full tree's.
fn check_paths(run: &Run, ws: &[Watch]) {
    let st = run.enclave.state().unwrap();
    for w in ws {
        let m = &w.f.member;
        assert_eq!(m.root, st.tree.root().unwrap());
        assert_eq!(m.path, st.tree.path(m.pos, 0).unwrap(), "member {}", m.pos);
        assert_eq!(m.leaf, st.tree.get(m.pos).copied());
        assert_eq!(m.outpoint, st.outpoint);
    }
}

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn light_clients_follow_twenty_transitions_and_raise_every_alarm() {
    let mut run = Run::open(8, 128);
    run.chain.fund_coins(32);
    let base = std::env::var_os("LAYERTREE_REGTEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let store = base.join(format!("mirror-{}", std::process::id()));
    std::fs::create_dir_all(&store).unwrap();
    run.store = Some(store.clone());
    std::fs::write(
        store.join(unroll_name(&run.unroll.tree_outpoint)),
        run.unroll.encode(),
    )
    .unwrap();
    let mirror = serve_dir(store.clone());

    let p = run.params();
    let st = run.enclave.state().unwrap().clone();
    let mut ws: Vec<Watch> = [0u64, 2, 5]
        .iter()
        .enumerate()
        .map(|(i, &pos)| {
            let member = Member {
                pos,
                leaf: st.tree.get(pos).copied(),
                path: st.tree.path(pos, 0).unwrap(),
                root: st.tree.root().unwrap(),
                outpoint: st.outpoint,
                unroll_key: st.output.unroll_key,
            };
            // One member reads the HTTP mirror, the others the directory.
            let src: Box<dyn RecordSource> = if i == 1 {
                Box::new(HttpMirror {
                    addr: mirror.clone(),
                })
            } else {
                Box::new(DirSource { dir: store.clone() })
            };
            Watch {
                f: Follower::new(
                    member,
                    p.clone(),
                    run.confirmed_at,
                    MISSING_AFTER,
                    Some(run.unroll.clone()),
                ),
                src,
            }
        })
        .collect();
    let secp = run.chain.secp.clone();

    // Twenty honest transitions: every member follows each in its block.
    for i in 0..20u32 {
        let op = match i % 5 {
            0 => Op::Join {
                height: 0,
                index: 100 + u64::from(i),
                leaves: vec![(0, leaf(&mut run.keys, &secp, 300 + i, 5_000, run.operator))],
            },
            1 => {
                let old = *run.enclave.state().unwrap().tree.get(0).unwrap();
                let fresh = leaf(&mut run.keys, &secp, 400 + i, old.amount, run.operator);
                Op::Update {
                    pos: 0,
                    new_leaf: Leaf {
                        exit_key: fresh.exit_key,
                        nonce: old.nonce + 1,
                        ..old
                    },
                }
            }
            2 => {
                let src = *run.enclave.state().unwrap().tree.get(3).unwrap();
                let mut claim = leaf(&mut run.keys, &secp, 500 + i, 1_000, run.operator);
                claim.evictor_key = src.evictor_key;
                claim.evict_after = src.evict_after;
                let voucher = secp.sign_schnorr(
                    &Message::from_digest(voucher_msg(&p.tree_id, &src, &claim)),
                    &Keypair::from_secret_key(&secp, &run.keys[&src.owner_key]),
                );
                Op::Claim {
                    source_pos: 3,
                    claim_pos: 150 + u64::from(i),
                    claim_leaf: claim,
                    voucher_sig: voucher,
                }
            }
            3 => Op::Exit {
                pos: 100 + u64::from(i - 3),
                evict: false,
            },
            _ => Op::Refresh,
        };
        let h = run.step(op);
        let seen = poll_all(&mut run, &mut ws);
        expect_each(&seen, h, Ok(()));
        check_paths(&run, &ws);
        assert!(ws.iter().all(|w| w.f.unroll.is_some()), "unroll held");
    }

    // A withheld record: RecordMissing after MISSING_AFTER blocks, then the
    // late record is accepted and the members catch up.
    run.withhold_record = true;
    let h = run.step(Op::Refresh);
    assert!(poll_all(&mut run, &mut ws).iter().all(Vec::is_empty));
    for k in 1..=MISSING_AFTER {
        run.chain.mine(1);
        let seen = poll_all(&mut run, &mut ws);
        if k < MISSING_AFTER {
            assert!(seen.iter().all(Vec::is_empty), "{seen:?}");
        } else {
            expect_each(&seen, h + k, Err(LightError::RecordMissing));
        }
    }
    run.publish_late();
    expect_each(&poll_all(&mut run, &mut ws), h, Ok(()));
    check_paths(&run, &ws);

    // A mirror serves bytes that do not hash to the anchored value: refused
    // in the spend's block, then the real record is accepted.
    run.forge_record = true;
    let h = run.step(Op::Refresh);
    expect_each(
        &poll_all(&mut run, &mut ws),
        h,
        Err(LightError::RecordNotAnchored),
    );
    run.publish_late();
    expect_each(&poll_all(&mut run, &mut ws), h, Ok(()));
    check_paths(&run, &ws);

    // The new state's unroll is withheld: UnrollNotHeld after
    // MISSING_AFTER blocks, cleared once it is published.
    run.withhold_unroll = true;
    let h = run.step(Op::Refresh);
    expect_each(&poll_all(&mut run, &mut ws), h, Ok(()));
    assert!(ws.iter().all(|w| w.f.unroll.is_none()));
    for k in 1..=MISSING_AFTER {
        run.chain.mine(1);
        let seen = poll_all(&mut run, &mut ws);
        if k < MISSING_AFTER {
            assert!(seen.iter().all(Vec::is_empty), "{seen:?}");
        } else {
            expect_each(&seen, h + k, Err(LightError::UnrollNotHeld));
        }
    }
    run.publish_late();
    assert!(poll_all(&mut run, &mut ws).iter().all(Vec::is_empty));
    assert!(ws.iter().all(|w| w.f.unroll.is_some()));

    // A broken enclave signs a theft (output 0 short by 50,000 sat) and it
    // is mined: every member alarms in that block, and stops.
    run.enclave.break_for_test();
    let op = Op::Refresh;
    let b = run.build(&op, 50_000);
    let before = b.before.clone();
    let (tx, signed, h) = run.mine_signed(&op, b);
    let txid = tx.compute_txid();
    run.log.push(
        Some(h),
        Body::transition(&op, &before, &signed.record, &txid),
    );
    assert_eq!(
        run.chain
            .utxo(&OutPoint { txid, vout: 0 })
            .unwrap()
            .value
            .to_sat(),
        before.amount() - 50_000,
        "the theft is on chain"
    );
    expect_each(
        &poll_all(&mut run, &mut ws),
        h,
        Err(LightError::OutputMismatch),
    );
    assert!(
        ws.iter()
            .all(|w| w.f.stopped == Some(LightError::OutputMismatch))
    );

    let text = run.log.to_jsonl();
    read(&text).unwrap_or_else(|e| panic!("light log: {e:?}"));
    for alarm in [
        "RecordMissing",
        "RecordNotAnchored",
        "UnrollNotHeld",
        "OutputMismatch",
    ] {
        assert!(text.contains(&format!("\"result\":\"{alarm}\"")), "{alarm}");
    }
    std::fs::write(base.join("regtest-light.jsonl"), &text).unwrap();
}

// ------------------------------------------------------------ measurements (#11785)

/// Blocks in a week.
const WEEK: u64 = 1_008;

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn unroll_timing_sizes_and_heartbeat_fees_measured() {
    // After the exit below: 128 leaves (one level of 128 outputs), 16,384
    // (two full levels) and 16,385 (three levels: 2, then 128 + 1, then 128).
    let cases = [
        ("measure-1-level", 129u32, 8u32, 1u32),
        ("measure-2-levels", 16_385, 15, 2),
        ("measure-3-levels", 16_386, 15, 3),
    ];
    let mut report = String::from(
        "| leaves | levels | txs per level | max unroll tx vsize per level (vB) | CPFP child (vB) | Δ to root mined (blocks) | then to all paid (blocks) |\n| --- | --- | --- | --- | --- | --- | --- |\n",
    );
    let mut refresh = (0, 0);
    let mut exit = (0, 0);
    for (name, n, depth, want_levels) in cases {
        let coins = 16 + (n as usize).div_ceil(128) + 4;
        let mut run = Run::open_with(n, 128, depth, coins, &|_| 1_000);
        run.step(Op::Refresh);
        refresh = run.last_cost;
        run.measure("refresh_vsize", refresh.0, "vB");
        run.measure("refresh_fee", refresh.1, "sat");
        run.step(Op::Exit {
            pos: 1,
            evict: false,
        });
        exit = run.last_cost;
        run.measure("exit_vsize", exit.0, "vB");
        run.measure("exit_fee", exit.1, "sat");
        let leaves = run.enclave.state().unwrap().tree.leaves().len();
        let stats = run.unroll_with_enclave_gone(name);
        assert_eq!(stats.levels, want_levels, "{name}");
        assert_eq!(stats.blocks_to_accept, u32::from(DELAY));
        assert_eq!(stats.blocks_to_paid, want_levels - 1);
        report.push_str(&format!(
            "| {leaves} | {} | {} | {} | {} | {} | {} |\n",
            stats.levels,
            stats
                .per_level
                .iter()
                .map(|l| l.txs.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            stats
                .per_level
                .iter()
                .map(|l| format!("{} ({} outputs)", l.max_vsize, l.max_outputs))
                .collect::<Vec<_>>()
                .join(", "),
            stats.child_vsize,
            stats.blocks_to_accept,
            stats.blocks_to_paid,
        ));
    }
    report.push_str(&format!(
        "\nrefresh (heartbeat): {} vB; key-path exit: {} vB\n\n",
        refresh.0, exit.0
    ));
    report.push_str("| Δ (blocks) | heartbeats a week (every Δ/2) | 1 sat/vB | 10 sat/vB | 50 sat/vB |\n| --- | --- | --- | --- | --- |\n");
    for delta in [144u64, 288, 1_008] {
        let beats = WEEK.div_ceil(delta / 2);
        let cost = |rate: u64| beats * refresh.0 * rate;
        report.push_str(&format!(
            "| {delta} | {beats} | {} sat | {} sat | {} sat |\n",
            cost(1),
            cost(10),
            cost(50)
        ));
    }
    println!("{report}");
    if let Some(dir) = std::env::var_os("LAYERTREE_REGTEST_ROOT") {
        std::fs::write(PathBuf::from(dir).join("measurements.md"), &report).unwrap();
    }
}

// ------------------------------------------------------------- HALT (#11786)

#[test]
#[ignore = "needs bitcoind on regtest; scripts/layertree-regtest.sh"]
fn after_halt_only_exits_are_signed_and_the_unroll_confirms_delta_later() {
    let mut run = Run::open(6, 128);
    run.step(Op::Refresh);
    let secp = run.chain.secp.clone();
    let g = sk(2);
    run.enclave
        .set_guardian(XOnlyPublicKey::from(g.public_key(&secp)));
    let msg = Message::from_digest(halt_msg(&run.params().tree_id));
    run.enclave
        .halt(&secp.sign_schnorr(&msg, &Keypair::from_secret_key(&secp, &g)))
        .unwrap();

    // The heartbeat is refused from now on.
    let op = Op::Refresh;
    let b = run.build(&op, 0);
    assert_eq!(
        run.enclave
            .authorize(b.tx, &b.prevouts, &op, &b.sigs)
            .unwrap_err(),
        Reject::Halted
    );
    run.log.push(None, Body::rejected(&op, &Reject::Halted));
    // An exit is still signed and mined.
    run.step(Op::Exit {
        pos: 1,
        evict: false,
    });
    // No more heartbeats: the unroll of the exit's state is mined exactly
    // Δ blocks after it, and pays everyone left.
    let stats = run.unroll_with_enclave_gone("regtest-halt");
    assert_eq!(stats.blocks_to_accept, u32::from(DELAY));
    assert_eq!(stats.levels, 1);
}
