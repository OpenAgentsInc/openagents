//! Exercise image-bound key recovery and predicate signing on a real TDX guest.
//! All transaction inputs are synthetic; this probe accepts no network requests.
#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use bitcoin::{
        Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        absolute::LockTime, transaction::Version,
    };
    use bitcoin::{
        key::{Keypair, XOnlyPublicKey},
        secp256k1::{Message, Secp256k1, SecretKey},
    };
    use layertree_tee_lab::{
        attest::report_data,
        enclave::{
            Config, Enclave,
            sealed::{GcpKms, SealedKey},
        },
        guest::{hex, token},
        output::TreeOutput,
        rules::{self, Op},
        tree::{Leaf, Tree},
    };

    let secp = Secp256k1::new();
    // A public test operator, never a wallet credential.
    let operator = SecretKey::from_slice(&[9; 32])?;
    let operator_key = XOnlyPublicKey::from(operator.public_key(&secp)).serialize();
    let cfg = Config {
        operator_key,
        tree_id: [0x17; 32],
        depth: 8,
        dust_min: 330,
        unroll_delay: 144,
        migration_m: None,
        radix: 2,
    };
    let kms = GcpKms::connect()?;
    let (mut enclave, restored) = match std::env::var("LAYERTREE_SEALED_BLOB") {
        Ok(blob) => {
            if blob.len() > 16_384 {
                return Err("sealed input is too large".into());
            }
            let sealed: SealedKey = serde_json::from_slice(&STANDARD.decode(blob)?)?;
            match Enclave::restore_in_guest(cfg, &sealed, &kms) {
                Ok(enclave) => (enclave, true),
                Err(reason) => {
                    println!(
                        "{}",
                        serde_json::json!({"schema":"openagents.layertree.key-recovery-refused.v1","reason":reason,"private_key_exported":false})
                    );
                    return Err("image-bound key recovery refused".into());
                }
            }
        }
        Err(std::env::VarError::NotPresent) => (Enclave::launch_in_guest(cfg), false),
        Err(_) => return Err("invalid sealed input".into()),
    };
    let mut tree = Tree::new(8).map_err(|_| "test tree failed")?;
    tree.insert(
        0,
        Leaf {
            amount: 10_000,
            exit_key: operator_key,
            owner_key: operator_key,
            evictor_key: operator_key,
            evict_after: 1000,
            nonce: 0,
            seq: 0,
        },
    )
    .map_err(|_| "test leaf failed")?;
    enclave.next_unroll_key();
    enclave
        .open(tree, OutPoint::null())
        .map_err(|_| "test unroll failed")?;
    assert_eq!(enclave.ephemeral_keys_held(), 0);
    let params = enclave.params().clone();
    let state = enclave.state().ok_or("test state absent")?.clone();
    let next = enclave.next_unroll_key();
    let op = Op::Refresh;
    let effects = rules::effects(&secp, &params, &state, &op).map_err(|_| "test refresh failed")?;
    let record = rules::record(&state, &effects, &op, next);
    let root = effects.tree.root().map_err(|_| "test root failed")?;
    let input = |outpoint| TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    };
    let mut fee_outpoint = OutPoint::null();
    fee_outpoint.vout = 1;
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![input(state.outpoint), input(fee_outpoint)],
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
    let fee = TxOut {
        value: Amount::from_sat(1000),
        script_pubkey: ScriptBuf::new_p2tr(
            &secp,
            XOnlyPublicKey::from(operator.public_key(&secp)),
            None,
        ),
    };
    let prevouts = vec![state.txout(&secp, &params), fee];
    let signature = secp.sign_schnorr(
        &Message::from_digest(rules::sighash(&tx, &prevouts)),
        &Keypair::from_secret_key(&secp, &operator),
    );
    let signed = enclave
        .authorize(tx, &prevouts, &op, &[signature])
        .map_err(|_| "test signing refused")?;
    assert_eq!(enclave.ephemeral_keys_held(), 0);
    assert!(!signed.tx.input[0].witness.is_empty());
    let sealed = enclave.seal_for_restart(&kms)?;
    let report = report_data(enclave.params());
    let token = token(&report)?;
    println!(
        "{}",
        serde_json::json!({
            "schema":"openagents.layertree.key-lifecycle.v1",
            "source_commit":option_env!("LAYERTREE_SOURCE_COMMIT").unwrap_or("unrecorded"),
            "enclave_key":params.enclave_key.to_string(),"operator_key":hex(&params.operator_key),
            "tree_id":hex(&params.tree_id),"depth":params.depth,"dust_min":params.dust_min,
            "unroll_delay":params.unroll_delay,"migration_m":null,"report_data":hex(&report),
            "nonce":hex(&report[..32]),"audience":"openagents.layertree.route.v1","token":token,
            "restored":restored,"sealed":sealed,"ephemeral_keys_after_signing":enclave.ephemeral_keys_held(),
            "synthetic_refresh_txid":signed.tx.compute_txid().to_string(),"accepts_external_transactions":false,
        })
    );
    Ok(())
}
#[cfg(not(unix))]
fn main() {
    eprintln!("Key recovery requires a Linux Confidential Space guest.");
    std::process::exit(1);
}
