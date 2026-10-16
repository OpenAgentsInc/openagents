//! An explicit regtest check: recovering an old key cannot revive a spent input.
use super::*;
use crate::{
    output::TreeOutput,
    rules::{self, Op},
    tree::{Leaf, Tree},
};
use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    consensus::encode::{deserialize_hex, serialize_hex},
    hashes::Hash,
    key::{Keypair, TapTweak},
    secp256k1::{Message, Secp256k1},
    sighash::{Prevouts, SighashCache, TapSighashType},
    transaction::Version,
};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
};

struct Rpc {
    address: String,
    wallet: String,
}
impl Rpc {
    fn call(&self, method: &str, params: Value, wallet: bool) -> Value {
        let path = if wallet {
            format!("/wallet/{}", self.wallet)
        } else {
            "/".into()
        };
        let body = json!({"jsonrpc":"1.0","id":"sealed-rollback","method":method,"params":params})
            .to_string();
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(stream, "POST {path} HTTP/1.0\r\nHost: localhost\r\nAuthorization: Basic cmVndGVzdDpyZWd0ZXN0LW9ubHk=\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let reply: Value = serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(reply["error"].is_null(), "{method}: {}", reply["error"]);
        reply["result"].clone()
    }
    fn mine(&self, n: u32, address: &str) {
        self.call("generatetoaddress", json!([n, address]), false);
    }
    fn pay(&self, script: &ScriptBuf, sats: u64, miner: &str) -> (OutPoint, TxOut) {
        let address = bitcoin::Address::from_script(script, bitcoin::Network::Regtest).unwrap();
        let id = self.call(
            "sendtoaddress",
            json!([address.to_string(), sats as f64 / 100_000_000.0]),
            true,
        );
        let detail = self.call("gettransaction", json!([id]), true);
        let tx: Transaction = deserialize_hex(detail["hex"].as_str().unwrap()).unwrap();
        let index = tx
            .output
            .iter()
            .position(|o| o.script_pubkey == *script && o.value.to_sat() == sats)
            .unwrap();
        self.mine(1, miner);
        (
            OutPoint {
                txid: tx.compute_txid(),
                vout: index as u32,
            },
            tx.output[index].clone(),
        )
    }
}

fn refresh(enclave: &mut Enclave, fee: (OutPoint, TxOut), operator: &SecretKey) -> Transaction {
    let secp = Secp256k1::new();
    let params = enclave.params().clone();
    let state = enclave.state().unwrap().clone();
    let next = enclave.next_unroll_key();
    let effects = rules::effects(&secp, &params, &state, &Op::Refresh).unwrap();
    let record = rules::record(&state, &effects, &Op::Refresh, next);
    let root = effects.tree.root().unwrap();
    let input = |outpoint| TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    };
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![input(state.outpoint), input(fee.0)],
        output: vec![
            TxOut {
                value: Amount::from_sat(root.sum),
                script_pubkey: TreeOutput {
                    root_hash: root.hash,
                    unroll_key: next,
                }
                .script_pubkey(&secp, &params.enclave_key, params.unroll_delay),
            },
            TxOut {
                value: Amount::ZERO,
                script_pubkey: record.op_return(),
            },
        ],
    };
    let prevouts = [state.txout(&secp, &params), fee.1];
    let signature = secp.sign_schnorr(
        &Message::from_digest(rules::sighash(&tx, &prevouts)),
        &Keypair::from_secret_key(&secp, operator),
    );
    let mut signed = enclave
        .authorize(tx, &prevouts, &Op::Refresh, &[signature])
        .unwrap()
        .tx;
    let hash = SighashCache::new(&signed)
        .taproot_key_spend_signature_hash(1, &Prevouts::All(&prevouts), TapSighashType::Default)
        .unwrap();
    let fee_key = Keypair::from_secret_key(&secp, operator)
        .tap_tweak(&secp, None)
        .to_keypair();
    signed.input[1].witness.push(
        secp.sign_schnorr(&Message::from_digest(hash.to_byte_array()), &fee_key)
            .serialize(),
    );
    assert_eq!(enclave.ephemeral_keys_held(), 0);
    signed
}

#[test]
#[ignore = "requires the isolated node from scripts/layertree/sealing-rollback.sh"]
fn stale_sealed_key_cannot_spend_the_consumed_tree_output() {
    let address =
        std::env::var("LAYERTREE_REGTEST_RPC").expect("set the isolated regtest RPC address");
    assert!(address.parse::<SocketAddr>().unwrap().ip().is_loopback());
    let rpc = Rpc {
        address,
        wallet: format!("sealed-rollback-{}", std::process::id()),
    };
    for _ in 0..60 {
        if TcpStream::connect(&rpc.address).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    assert_eq!(
        rpc.call("getblockchaininfo", json!([]), false)["chain"],
        "regtest"
    );
    rpc.call("createwallet", json!([rpc.wallet]), false);
    let miner = rpc
        .call("getnewaddress", json!([]), true)
        .as_str()
        .unwrap()
        .to_owned();
    rpc.mine(101, &miner);
    let secp = Secp256k1::new();
    let operator = SecretKey::from_slice(&[9; 32]).unwrap();
    let public = XOnlyPublicKey::from(operator.public_key(&secp));
    let cfg = Config {
        operator_key: public.serialize(),
        tree_id: [0x17; 32],
        depth: 8,
        dust_min: 330,
        unroll_delay: 144,
        migration_m: None,
        radix: 2,
    };
    let mut enclave = Enclave::launch_in_guest(cfg.clone());
    let mut tree = Tree::new(8).unwrap();
    tree.insert(
        0,
        Leaf {
            amount: 10_000,
            exit_key: public.serialize(),
            owner_key: public.serialize(),
            evictor_key: public.serialize(),
            evict_after: 1000,
            nonce: 0,
            seq: 0,
        },
    )
    .unwrap();
    let next = enclave.next_unroll_key();
    let script = TreeOutput {
        root_hash: tree.root().unwrap().hash,
        unroll_key: next,
    }
    .script_pubkey(&secp, &enclave.params().enclave_key, 144);
    let tree_coin = rpc.pay(&script, 10_000, &miner);
    enclave.open(tree, tree_coin.0).unwrap();
    let old_state = enclave.state().unwrap().clone();
    let sealed = enclave.seal_with_envelope(&[7; 32]).unwrap();
    let fee_script = ScriptBuf::new_p2tr(&secp, public, None);
    let fee1 = rpc.pay(&fee_script, 1000, &miner);
    let fee2 = rpc.pay(&fee_script, 1000, &miner);
    let first = refresh(&mut enclave, fee1, &operator);
    let mut rolled_back = Enclave::restore_with_envelope(cfg, &sealed, &[7; 32]).unwrap();
    assert_eq!(enclave.params(), rolled_back.params());
    assert!(rolled_back.state().is_none());
    // Deliberately supply an old public state, as a dishonest state source could.
    // Only this unit test can write the private field directly.
    rolled_back.state = Some(old_state);
    let stale = refresh(&mut rolled_back, fee2.clone(), &operator);
    let before = rpc.call(
        "testmempoolaccept",
        json!([[serialize_hex(&stale)], 0]),
        false,
    );
    assert_eq!(before[0]["allowed"], true, "{before}");
    rpc.call(
        "sendrawtransaction",
        json!([serialize_hex(&first), 0]),
        false,
    );
    rpc.mine(1, &miner);
    assert!(
        rpc.call(
            "gettxout",
            json!([tree_coin.0.txid.to_string(), tree_coin.0.vout]),
            false
        )
        .is_null()
    );
    assert!(
        !rpc.call(
            "gettxout",
            json!([fee2.0.txid.to_string(), fee2.0.vout]),
            false
        )
        .is_null()
    );
    let after = rpc.call(
        "testmempoolaccept",
        json!([[serialize_hex(&stale)], 0]),
        false,
    );
    assert_eq!(after[0]["allowed"], false, "{after}");
    assert!(
        matches!(
            after[0]["reject-reason"].as_str(),
            Some("missing-inputs" | "bad-txns-inputs-missingorspent")
        ),
        "{after}"
    );
    let evidence = json!({"network":"regtest","same_recovered_key":true,"enclave_key":enclave.params().enclave_key.to_string(),"tree_outpoint":tree_coin.0.to_string(),"first_txid":first.compute_txid().to_string(),"stale_txid":stale.compute_txid().to_string(),"before":before,"after":after,"separate_fee_coin_still_unspent":true});
    std::fs::write(
        std::env::var("LAYERTREE_SEAL_ROLLBACK_EVIDENCE")
            .expect("set an explicit scratch evidence path"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
