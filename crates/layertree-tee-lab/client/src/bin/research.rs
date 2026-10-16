//! Explicit signet-only experiments using fresh scratch keys and public evidence.
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    consensus::encode::serialize_hex,
    hashes::{Hash, sha256},
    key::{Keypair, TapTweak},
    secp256k1::{Message, Secp256k1, SecretKey},
    sighash::{Prevouts, SighashCache, TapSighashType},
    transaction::Version,
};
use layertree_attest::{
    Binding, Evidence, Policy, TcbFloor, TokenPolicy, channel::Channel, verify,
};
use layertree_signet_service::Core;
use layertree_tee_lab::{
    output::TreeOutput,
    rpc::{self, ServiceConfig},
    rules::{self, TreeState},
    tree::{Leaf, tree_id},
    unroll::UnrollPackage,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}
fn read<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, String> {
    serde_json::from_slice(&fs::read(path).map_err(|_| "cannot read research file")?)
        .map_err(|_| "invalid research JSON".into())
}
fn save(path: impl AsRef<Path>, value: &impl Serialize) -> Result<(), String> {
    let path = path.as_ref();
    let temporary = path.with_extension("writing");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "cannot write research file")?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|_| "cannot encode research file")?)
        .map_err(|_| "cannot write research file")?;
    file.sync_all().map_err(|_| "cannot sync research file")?;
    fs::rename(temporary, path).map_err(|_| "cannot save research file")?;
    fs::File::open(path.parent().ok_or("missing research directory")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "cannot sync research directory".into())
}
fn key(root: &Path, name: &str) -> Result<Keypair, String> {
    let path = root.join("private").join(format!("{name}.key"));
    if fs::metadata(&path)
        .map_err(|_| "cannot read research key")?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("research key must have mode 0600".into());
    }
    let text = fs::read_to_string(path).map_err(|_| "cannot read research key")?;
    let secret =
        SecretKey::from_slice(&rpc::bytes(text.trim())?).map_err(|_| "invalid research key")?;
    Ok(Keypair::from_secret_key(&Secp256k1::new(), &secret))
}
fn keys(root: &Path) -> Result<(), String> {
    let private = root.join("private");
    fs::create_dir_all(&private).map_err(|_| "cannot create research directory")?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700))
        .map_err(|_| "cannot protect key directory")?;
    let secp = Secp256k1::new();
    let mut public = serde_json::Map::new();
    let names = std::iter::once("operator".to_string()).chain((0..8).map(|n| format!("member{n}")));
    for name in names {
        let (secret, _) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(private.join(format!("{name}.key")))
            .map_err(|_| "research key already exists or cannot be created")?;
        file.write_all(rpc::hex(secret.as_ref()).as_bytes())
            .map_err(|_| "cannot save research key")?;
        file.sync_all().map_err(|_| "cannot sync research key")?;
        public.insert(
            name,
            json!(
                Keypair::from_secret_key(&secp, &secret)
                    .x_only_public_key()
                    .0
                    .to_string()
            ),
        );
    }
    let operator = key(root, "operator")?;
    let address = Address::p2tr(
        &secp,
        operator.x_only_public_key().0,
        None,
        bitcoin::Network::Signet,
    );
    public.insert("funding_address".into(), json!(address.to_string()));
    save(root.join("public-keys.json"), &public)?;
    println!(
        "{}",
        json!({"funding_address":address.to_string(),"public_keys":public})
    );
    Ok(())
}
fn configure(root: &Path, funding: &str, mirror_file: &str) -> Result<(), String> {
    if root.join("service.json").exists() || root.join("progress.json").exists() {
        return Err("research configuration already exists".into());
    }
    let point = rpc::outpoint(funding)?;
    let public: Value = read(root.join("public-keys.json"))?;
    let config = ServiceConfig {
        operator_key: public["operator"]
            .as_str()
            .ok_or("operator key missing")?
            .into(),
        tree_id: rpc::hex(&tree_id(&point.txid.to_byte_array(), point.vout)),
        depth: 4,
        dust_min: 330,
        unroll_delay: 12,
        radix: 128,
        mirrors: read(mirror_file)?,
        mirror_threshold: 2,
    };
    config.enclave()?;
    save(root.join("service.json"), &config)?;
    save(
        root.join("progress.json"),
        &json!({"funding":funding,"fee_outpoint":funding}),
    )?;
    println!("{}", serde_json::to_string(&config).expect("config"));
    Ok(())
}
fn floor() -> TcbFloor {
    TcbFloor {
        evaluation_data_number: 19,
        attester_date_unix: 1_747_180_800,
        confidential_space_build: 260900,
    }
}
fn input(point: OutPoint) -> TxIn {
    TxIn {
        previous_output: point,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    }
}
fn sign_input(
    tx: &mut Transaction,
    prevouts: &[TxOut],
    index: usize,
    key: &Keypair,
) -> Result<(), String> {
    let secp = Secp256k1::new();
    let digest = SighashCache::new(&*tx)
        .taproot_key_spend_signature_hash(index, &Prevouts::All(prevouts), TapSighashType::Default)
        .map_err(|_| "cannot compute fee signature")?;
    let signature = secp.sign_schnorr(
        &Message::from_digest(digest.to_byte_array()),
        &key.tap_tweak(&secp, None).to_keypair(),
    );
    let mut witness = Witness::new();
    witness.push(signature.as_ref());
    tx.input[index].witness = witness;
    Ok(())
}
fn leaf(root: &Path, index: usize) -> Result<Leaf, String> {
    let member = key(root, &format!("member{index}"))?
        .x_only_public_key()
        .0
        .serialize();
    let operator = key(root, "operator")?.x_only_public_key().0.serialize();
    Ok(Leaf {
        amount: 1000,
        exit_key: member,
        owner_key: member,
        evictor_key: operator,
        evict_after: 500_000,
        nonce: 0,
        seq: u32::MAX,
    })
}
fn check_unroll(
    params: &rules::TreeParams,
    state: &TreeState,
    pkg: &UnrollPackage,
) -> Result<(), String> {
    for member in state.tree.leaves().values() {
        layertree_tee_lab::unroll::verify_package(
            &Secp256k1::verification_only(),
            params,
            state.output,
            state.outpoint,
            state.amount(),
            state.tree.leaves(),
            pkg,
            member,
        )
        .map_err(|e| format!("unroll refused: {e:?}"))?;
    }
    Ok(())
}
fn publish(
    root: &Path,
    core: &Core,
    tx: &Transaction,
    fee_vout: usize,
    previous: &Value,
    action: &str,
) -> Result<(), String> {
    let txid = tx.compute_txid().to_string();
    let file = root.join("inflight.json");
    if file.exists() {
        return Err("inspect the previous in-flight transaction first".into());
    }
    let record = json!({"action":action,"transaction":serialize_hex(tx),"txid":txid,"fee_outpoint":format!("{txid}:{fee_vout}"),"previous":previous,"prepared_at_unix":now()});
    save(&file, &record)?; // Before submission: uncertain outcomes never create a new payment.
    let accepted = core.call("sendrawtransaction", json!([serialize_hex(tx)]))?;
    if accepted.as_str() != Some(&txid) {
        return Err("Bitcoin returned another transaction ID".into());
    }
    save(root.join(format!("{action}-{txid}.json")), &record)?;
    println!(
        "{}",
        json!({"action":action,"txid":txid,"status":"broadcast","fee_sat":500})
    );
    Ok(())
}
fn settle(root: &Path, core: &Core) -> Result<(), String> {
    let path = root.join("inflight.json");
    if !path.exists() {
        return Ok(());
    }
    let mut record: Value = read(&path)?;
    let tx: Transaction = rpc::transaction(
        record["transaction"]
            .as_str()
            .ok_or("missing in-flight transaction")?,
    )?;
    let point = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    core.output(point, true)?;
    record["confirmed_height"] = json!(core.ready_height()?);
    save(
        root.join(format!("confirmed-{}.json", tx.compute_txid())),
        &record,
    )?;
    let mut progress: Value = read(root.join("progress.json"))?;
    progress["fee_outpoint"] = record["fee_outpoint"].clone();
    progress["last_transaction"] = json!(tx.compute_txid().to_string());
    save(root.join("progress.json"), &progress)?;
    fs::remove_file(path).map_err(|_| "cannot clear confirmed transaction")?;
    Ok(())
}
struct Release {
    release: nostr::domain::Event,
    head: nostr::domain::Event,
    entry: att_release::rekor::Entry,
    rebuilds: Vec<nostr::domain::Event>,
}
impl Release {
    fn read(dir: &Path) -> Result<Self, String> {
        Ok(Self {
            release: read(dir.join("release-event.json"))?,
            head: read(dir.join("head-event.json"))?,
            entry: read(dir.join("rekor-entry.json"))?,
            rebuilds: vec![
                read(dir.join("local-rebuild.json"))?,
                read(dir.join("gcp-rebuild.json"))?,
            ],
        })
    }
    fn check(&self, now: u64) -> Result<String, String> {
        let key = att_release::rekor::LogKey::sigstore()?;
        att_release::check_release(
            &att_release::Evidence {
                release: &self.release,
                head: &self.head,
                entry: Some(&self.entry),
                consistency: None,
                rebuilds: &self.rebuilds,
                approvals: None,
            },
            &att_release::Policy {
                publisher: "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d",
                workload: "layertree-signet",
                log: Some(&key),
                min_rebuilds: 2,
                seen_generation: None,
            },
            &mut att_release::rekor::Witness::default(),
            now,
        )?;
        Ok(nostr::att::parse_release(&self.release)?.image.digest)
    }
}
fn run_tree(root: &Path, address: &str, release_dir: &Path, action: &str) -> Result<(), String> {
    let config: ServiceConfig = read(root.join("service.json"))?;
    let core = Core::new(
        std::env::var_os("LAYERTREE_CHAIN_COOKIE")
            .ok_or("set LAYERTREE_CHAIN_COOKIE to the scratch signet node cookie")?,
    )?;
    core.ready_height()?;
    settle(root, &core)?;
    let release = Release::read(release_dir)?;
    let image = release.check(now())?;
    let floor = floor();
    let channel = Channel::connect(
        address.parse().map_err(|_| "invalid service address")?,
        &config,
        &TokenPolicy {
            image_digest: &image,
            max_age_seconds: 900,
            floor: &floor,
        },
        now,
    )?;
    let capture = root.join(format!("capture-{}-{action}", now()));
    fs::create_dir(&capture).map_err(|_| "cannot create public capture")?;
    save(capture.join("attestation.json"), &channel.evidence)?;
    save(
        capture.join("channel.json"),
        &json!({"challenge":rpc::hex(&channel.challenge),"certificate_der":rpc::hex(&channel.certificate_der),"checked_at_unix":now()}),
    )?;
    let snapshot = channel.state()?;
    save(capture.join("state-before.json"), &snapshot)?;
    let progress: Value = read(root.join("progress.json"))?;
    let operator = key(root, "operator")?;
    let secp = Secp256k1::new();
    if action == "genesis" {
        let next = if let Some(k) = &snapshot.unroll_key {
            json!({"unroll_key":k})
        } else {
            channel.operator("/next", json!({}), &operator, now())?
        };
        let k = next["unroll_key"]
            .as_str()
            .ok_or("missing unroll key")?
            .parse()
            .map_err(|_| "invalid unroll key")?;
        let members = (0..5)
            .map(|n| {
                Ok(rpc::Placed {
                    pos: n as u64,
                    leaf: rpc::hex(&leaf(root, n)?.serialize()),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let tree = rpc::tree(config.depth, &members)?;
        let root_node = tree.root().map_err(|_| "invalid root")?;
        let funding = rpc::outpoint(
            progress["funding"]
                .as_str()
                .ok_or("missing funding input")?,
        )?;
        let prevout = core.output(funding, true)?;
        let output = TreeOutput {
            root_hash: root_node.hash,
            unroll_key: k,
        };
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![input(funding)],
            output: vec![
                TxOut {
                    value: Amount::from_sat(root_node.sum),
                    script_pubkey: output.script_pubkey(
                        &secp,
                        &channel.params.enclave_key,
                        config.unroll_delay,
                    ),
                },
                TxOut {
                    value: Amount::from_sat(
                        prevout
                            .value
                            .to_sat()
                            .checked_sub(root_node.sum + 500)
                            .ok_or("creation funding is too small")?,
                    ),
                    script_pubkey: ScriptBuf::new_p2tr(&secp, operator.x_only_public_key().0, None),
                },
            ],
        };
        save(
            capture.join("creation-plan.json"),
            &json!({"transaction":serialize_hex(&tx),"leaves":members}),
        )?;
        let response = channel.operator(
            "/genesis",
            serde_json::to_value(rpc::Genesis {
                transaction: serialize_hex(&tx),
                leaves: members,
            })
            .expect("genesis"),
            &operator,
            now(),
        )?;
        let pkg = UnrollPackage::decode(&rpc::bytes(
            response["unroll"]
                .as_str()
                .ok_or("missing creation unroll")?,
        )?)
        .map_err(|_| "invalid creation unroll")?;
        let state = TreeState {
            tree,
            outpoint: OutPoint {
                txid: tx.compute_txid(),
                vout: 0,
            },
            output,
        };
        check_unroll(&channel.params, &state, &pkg)?;
        save(capture.join("creation-response.json"), &response)?;
        sign_input(&mut tx, &[prevout], 0, &operator)?;
        publish(root, &core, &tx, 1, &progress, action)?;
    } else {
        let point = rpc::outpoint(snapshot.outpoint.as_deref().ok_or("tree is not open")?)?;
        let actual = core.output(point, true)?;
        let tree = rpc::tree(config.depth, &snapshot.leaves)?;
        let root_node = tree.root().map_err(|_| "invalid tree")?;
        let k = snapshot
            .unroll_key
            .as_deref()
            .ok_or("missing unroll key")?
            .parse()
            .map_err(|_| "invalid unroll key")?;
        let cert_hash = sha256::Hash::hash(&channel.certificate_der).to_byte_array();
        let verified = verify(
            &Evidence {
                token: channel.evidence["token"].as_str().ok_or("missing token")?,
                release: &release.release,
                head: &release.head,
                log_entry: &release.entry,
                rebuilds: &release.rebuilds,
                consistency: None,
                output: &actual,
                root: root_node,
                unroll_key: k,
            },
            &Policy {
                params: &channel.params,
                publisher: "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d",
                workload: "layertree-signet",
                min_rebuilds: 2,
                seen_generation: None,
                max_age_seconds: 900,
                floor: &floor,
            },
            &Binding::Live {
                challenge: &channel.challenge,
                certificate_sha256: &cert_hash,
            },
            &mut att_release::rekor::Witness::default(),
            now(),
        )
        .map_err(|e| format!("live output verification refused: {}", e.code()))?;
        save(
            capture.join("verified-output.json"),
            &json!({"event":serde_json::to_value(verified.event()).map_err(|_|"cannot encode attestation event")?,"output":serialize_hex(&actual),"outpoint":point.to_string(),"height":core.ready_height()?}),
        )?;
        if action == "status" {
            println!(
                "{}",
                json!({"status":"attested","outpoint":point.to_string(),"height":core.ready_height()?,"halted":snapshot.halted,"ephemeral_keys":snapshot.ephemeral_keys})
            );
            return Ok(());
        }
        if action == "halt" {
            let sig = secp.sign_schnorr(
                &Message::from_digest(layertree_tee_lab::enclave::halt_msg(
                    &channel.params.tree_id,
                )),
                &operator,
            );
            println!("{}", channel.halt(&sig)?);
            return Ok(());
        }
        if action == "request-exit" {
            let member = tree.get(1).ok_or("exit member is missing")?;
            let sig = secp.sign_schnorr(
                &Message::from_digest(layertree_tee_lab::tree::exit_auth_msg(
                    &channel.params.tree_id,
                    member,
                )),
                &key(root, "member1")?,
            );
            println!("{}", channel.exit_request(1, &sig)?);
            return Ok(());
        }
        let operation = match action {
            "join" => rpc::Operation::Join {
                height: 0,
                index: 5,
                leaves: vec![rpc::Placed {
                    pos: 0,
                    leaf: rpc::hex(&leaf(root, 5)?.serialize()),
                }],
            },
            "update" => {
                let mut changed = *tree.get(0).ok_or("update member missing")?;
                changed.nonce = changed.nonce.checked_add(1).ok_or("nonce overflow")?;
                changed.exit_key = key(root, "member6")?.x_only_public_key().0.serialize();
                rpc::Operation::Update {
                    pos: 0,
                    leaf: rpc::hex(&changed.serialize()),
                }
            }
            "exit" => rpc::Operation::Exit {
                pos: 1,
                evict: false,
            },
            "refresh" => rpc::Operation::Refresh,
            _ => return Err("unknown research action".into()),
        };
        let op = operation.op()?;
        let before = TreeState {
            tree,
            outpoint: point,
            output: TreeOutput {
                root_hash: root_node.hash,
                unroll_key: k,
            },
        };
        let next = channel.operator("/next", json!({}), &operator, now())?;
        let next_key = next["unroll_key"]
            .as_str()
            .ok_or("missing next key")?
            .parse()
            .map_err(|_| "invalid next key")?;
        let effects = rules::effects(&secp, &channel.params, &before, &op)
            .map_err(|e| format!("local rule refused: {e:?}"))?;
        let record = rules::record(&before, &effects, &op, next_key);
        let next_root = effects.tree.root().map_err(|_| "invalid next root")?;
        let fee_point = rpc::outpoint(
            progress["fee_outpoint"]
                .as_str()
                .ok_or("missing fee input")?,
        )?;
        let fee_prev = core.output(fee_point, true)?;
        let mut outputs = vec![TxOut {
            value: Amount::from_sat(next_root.sum),
            script_pubkey: TreeOutput {
                root_hash: next_root.hash,
                unroll_key: next_key,
            }
            .script_pubkey(&secp, &channel.params.enclave_key, config.unroll_delay),
        }];
        outputs.extend(effects.payouts.clone());
        outputs.push(TxOut {
            value: Amount::ZERO,
            script_pubkey: record.op_return(),
        });
        let spent = outputs.iter().map(|o| o.value.to_sat()).sum::<u64>();
        let fee_index = outputs.len();
        outputs.push(TxOut {
            value: Amount::from_sat(
                before
                    .amount()
                    .checked_add(fee_prev.value.to_sat())
                    .and_then(|v| v.checked_sub(spent + 500))
                    .ok_or("fee coin is too small")?,
            ),
            script_pubkey: fee_prev.script_pubkey.clone(),
        });
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::from_height(effects.min_locktime.unwrap_or(0))
                .map_err(|_| "invalid locktime")?,
            input: vec![input(point), input(fee_point)],
            output: outputs,
        };
        let prevouts = vec![actual, fee_prev];
        let prepared: rpc::Prepared = serde_json::from_value(
            channel.operator(
                "/prepare",
                serde_json::to_value(rpc::Prepare {
                    transaction: serialize_hex(&tx),
                    operation,
                })
                .expect("prepare"),
                &operator,
                now(),
            )?,
        )
        .map_err(|_| "invalid prepared response")?;
        let sighash = rules::sighash(&tx, &prevouts);
        if prepared.transaction != serialize_hex(&tx)
            || prepared.record != rpc::hex(&record.encode())
            || prepared.sighash != rpc::hex(&sighash)
        {
            return Err("guest prepared another transaction".into());
        }
        let pkg = UnrollPackage::decode(&rpc::bytes(
            prepared
                .unroll
                .as_deref()
                .ok_or("missing prepared unroll")?,
        )?)
        .map_err(|_| "invalid prepared unroll")?;
        check_unroll(
            &channel.params,
            &TreeState {
                tree: effects.tree,
                outpoint: OutPoint {
                    txid: tx.compute_txid(),
                    vout: 0,
                },
                output: TreeOutput {
                    root_hash: next_root.hash,
                    unroll_key: next_key,
                },
            },
            &pkg,
        )?;
        save(capture.join("prepared.json"), &prepared)?;
        let mut known = vec![operator];
        for n in 0..8 {
            known.push(key(root, &format!("member{n}"))?);
        }
        let signatures = rules::signers(&channel.params, &before, &op)
            .map_err(|_| "cannot select party keys")?
            .iter()
            .map(|public| {
                let key = known
                    .iter()
                    .find(|key| key.x_only_public_key().0.serialize() == *public)
                    .ok_or("missing research party key")?;
                Ok(secp
                    .sign_schnorr(&Message::from_digest(sighash), key)
                    .to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let signed = channel.operator(
            "/finish",
            serde_json::to_value(rpc::Finish {
                transaction_id: tx.compute_txid().to_string(),
                signatures,
            })
            .expect("finish"),
            &operator,
            now(),
        )?;
        save(capture.join("signed.json"), &signed)?;
        let mut final_tx = rpc::transaction(
            signed["transaction"]
                .as_str()
                .ok_or("missing signed transaction")?,
        )?;
        if final_tx.compute_txid() != tx.compute_txid() {
            return Err("guest signed another transaction".into());
        }
        sign_input(&mut final_tx, &prevouts, 1, &operator)?;
        publish(root, &core, &final_tx, fee_index, &progress, action)?;
    }
    save(capture.join("state-after.json"), &channel.state()?)?;
    save(capture.join("events.json"), &channel.events()?)?;
    Ok(())
}
fn main() {
    let result = (|| -> Result<(), String> {
        let args = std::env::args().collect::<Vec<_>>();
        match args.get(1).map(String::as_str){Some("keys")if args.len()==3=>keys(Path::new(&args[2])),Some("configure")if args.len()==5=>configure(Path::new(&args[2]),&args[3],&args[4]),Some("run")if args.len()==6=>run_tree(Path::new(&args[2]),&args[3],Path::new(&args[4]),&args[5]),_=>Err("use keys ROOT, configure ROOT FUNDING_OUTPOINT MIRRORS_JSON, or run ROOT ADDRESS RELEASE_DIRECTORY ACTION".into())}
    })();
    if let Err(error) = result {
        eprintln!("signet research stopped: {error}");
        std::process::exit(1);
    }
}
