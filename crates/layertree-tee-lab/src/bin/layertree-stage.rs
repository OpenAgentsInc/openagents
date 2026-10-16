//! Deliberately broken software signer for an isolated test tree (#11794).
//! No hardware attestation claim. Never part of the released signer image.
#[path = "../../tests/common/mod.rs"]
mod common;
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    consensus::serialize,
    hashes::Hash,
    key::{Keypair, TapTweak},
    secp256k1::{All, Message, Secp256k1, SecretKey},
    sighash::{Prevouts, SighashCache, TapSighashType},
    transaction::Version,
};
use layertree_tee_lab::{
    events::{Body, Network, Writer},
    follow::{record_name, unroll_name},
    follow_live::{
        atomic_file, bootstrap,
        chain::{RPC_LIMIT, client, endpoint, read_response},
        hex, now, unhex,
    },
    output::TreeOutput,
    rpc,
    rules::Op,
};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, Stdio},
    str::FromStr,
    time::Duration,
};
const FEE: u64 = 1_000;
struct Core {
    http: reqwest::blocking::Client,
    url: reqwest::Url,
    network: String,
    mine: Option<String>,
}
impl Core {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        if ![
            "getblockchaininfo",
            "gettxout",
            "sendrawtransaction",
            "getrawtransaction",
            "getblockheader",
            "getblockcount",
            "generatetoaddress",
        ]
        .contains(&method)
            || method == "generatetoaddress" && self.network != "regtest"
        {
            return Err("This test operation is not allowed.".into());
        }
        let mut request = self
            .http
            .post(self.url.clone())
            .json(&json!({"jsonrpc":"2.0","id":"stage","method":method,"params":params}));
        if let Ok(path) = std::env::var("LAYERTREE_RPC_COOKIE") {
            let mut text = String::new();
            std::fs::File::open(path)
                .map_err(|_| "Could not open the test node cookie.")?
                .take(4097)
                .read_to_string(&mut text)
                .map_err(|_| "Could not read the test node cookie.")?;
            if text.len() > 4096 {
                return Err("The test node cookie is too large.".into());
            }
            let (user, pass) = text
                .trim()
                .split_once(':')
                .ok_or("Invalid test node cookie.")?;
            request = request.basic_auth(user, Some(pass));
        } else if let Ok(user) = std::env::var("LAYERTREE_RPC_USER") {
            request = request.basic_auth(user, std::env::var("LAYERTREE_RPC_PASSWORD").ok());
        }
        let response = request
            .send()
            .map_err(|_| format!("The test node did not answer {method}."))?;
        let bytes = read_response(response, RPC_LIMIT)?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid test node response.")?;
        if !value["error"].is_null() {
            return Err(format!("The test node refused {method}."));
        }
        Ok(value["result"].clone())
    }
    fn tick(&mut self) -> Result<(), String> {
        if let Some(address) = self.mine.clone() {
            self.call("generatetoaddress", json!([1, address]))?;
        }
        Ok(())
    }
    fn confirm(&mut self, tx: &Transaction, end: u64) -> Result<u32, String> {
        self.call("sendrawtransaction", json!([hex(&serialize(tx))]))?;
        self.tick()?;
        loop {
            deadline(end)?;
            let value = self.call(
                "getrawtransaction",
                json!([tx.compute_txid().to_string(), true]),
            )?;
            if value["confirmations"].as_u64().unwrap_or(0) > 0 {
                return self.call("getblockheader", json!([value["blockhash"]]))?["height"]
                    .as_u64()
                    .and_then(|h| h.try_into().ok())
                    .ok_or("Invalid confirmation height.".into());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
fn deadline(end: u64) -> Result<(), String> {
    if now()? >= end {
        Err("The staging deadline passed; evidence is incomplete.".into())
    } else {
        Ok(())
    }
}
fn save(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    atomic_file(
        path,
        &serde_json::to_vec_pretty(value).map_err(|_| "Could not encode test evidence.")?,
    )
}
fn sign(
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
    tx.input[i].witness = Witness::from_slice(&[sig.as_ref()]);
}
fn input(point: OutPoint) -> TxIn {
    TxIn {
        previous_output: point,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    }
}
struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn observations(root: &Path, member: u64) -> Result<Vec<Value>, String> {
    let text =
        std::fs::read_to_string(root.join(format!("member-{member}/state/observations.jsonl")))
            .unwrap_or_default();
    // A writer may be appending the final line. Completed lines must parse.
    text.split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| {
            serde_json::from_str(line).map_err(|_| "Invalid staged follower evidence.".into())
        })
        .collect()
}
fn wait_for(
    root: &Path,
    end: u64,
    children: &mut Children,
    txid: Option<bitcoin::Txid>,
    result: &str,
) -> Result<Vec<Value>, String> {
    loop {
        deadline(end)?;
        let mut found = Vec::new();
        for member in 0..3 {
            let rows = observations(root, member)?;
            let matches = rows.iter().find(|row| {
                row["kind"] == "light_client"
                    && row["result"] == result
                    && txid.is_none_or(|id| row["txid"] == id.to_string())
            });
            if let Some(row) = matches {
                if result == "ok"
                    || rows
                        .iter()
                        .any(|ack| ack["kind"] == "webhook_ack" && ack["id"] == row["id"])
                {
                    found.push(row.clone());
                }
            }
            if let Some(code) = children.0[member as usize]
                .try_wait()
                .map_err(|_| "Could not check a test follower.")?
            {
                if result != "OutputMismatch" || code.code() != Some(2) {
                    return Err("A staged follower stopped before completing its check.".into());
                }
            }
        }
        if found.len() == 3 {
            return Ok(found);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
fn run(
    root: &Path,
    funding: OutPoint,
    follower: &Path,
    end: u64,
    network: &str,
    mine: Option<String>,
) -> Result<(), String> {
    if !matches!(network, "signet" | "regtest") || mine.is_some() && network != "regtest" {
        return Err("Choose signet, or isolated regtest mining.".into());
    }
    if !(now()?..=now()? + 8 * 3600).contains(&end) {
        return Err("Choose a future deadline within eight hours.".into());
    }
    let _lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("stage.lock"))
        .map_err(|_| "Could not open the staging lock.")?;
    _lock
        .try_lock()
        .map_err(|_| "Another staged run uses this directory.")?;
    if root.join("stage.json").exists() {
        return Err("This directory already contains a staged run.".into());
    }
    let webhook = std::env::var("LAYERTREE_STAGE_ALARM_WEBHOOK")
        .map_err(|_| "Set the dedicated staging alarm webhook.")?;
    endpoint(&webhook)?;
    let rpc_url = std::env::var("LAYERTREE_RPC_URL")
        .map_err(|_| "Set the test node URL in LAYERTREE_RPC_URL.")?;
    let mut core = Core {
        http: client()?,
        url: endpoint(&rpc_url)?,
        network: network.into(),
        mine,
    };
    if core.call("getblockchaininfo", json!([]))?["chain"] != network {
        return Err("The test node is on another network.".into());
    }
    let key_path = root.join("private/funding.key");
    if std::fs::metadata(&key_path)
        .map_err(|_| "Missing test funding key.")?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("The test funding key needs mode 0600.".into());
    }
    let secret =
        std::fs::read_to_string(key_path).map_err(|_| "Could not read the test funding key.")?;
    let key = SecretKey::from_slice(&unhex(secret.trim(), 32)?)
        .map_err(|_| "Invalid test funding key.")?;
    let secp = Secp256k1::new();
    let spk = ScriptBuf::new_p2tr(&secp, key.x_only_public_key(&secp).0, None);
    let coin = core.call(
        "gettxout",
        json!([funding.txid.to_string(), funding.vout, true]),
    )?;
    if coin["confirmations"].as_u64().unwrap_or(0) == 0 {
        return Err("Confirm the dedicated test funding coin first.".into());
    }
    let value =
        (coin["value"].as_f64().ok_or("Missing funding amount.")? * 100_000_000.0).round() as u64;
    if !(80_000..=1_000_000).contains(&value) || coin["scriptPubKey"]["hex"] != hex(spk.as_bytes())
    {
        return Err("Choose an 80,000–1,000,000 sat test coin paying the new staging key.".into());
    }
    let funding_coin = TxOut {
        value: Amount::from_sat(value),
        script_pubkey: spk.clone(),
    };
    let mut genesis = None;
    let mut world = common::World::with_funding(3, 128, None, funding, |secp, params, tree, k| {
        let root = tree.root().unwrap();
        let mut tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![input(funding)],
            output: vec![
                TxOut {
                    value: Amount::from_sat(root.sum),
                    script_pubkey: TreeOutput {
                        root_hash: root.hash,
                        unroll_key: k,
                    }
                    .script_pubkey(
                        secp,
                        &params.enclave_key,
                        params.unroll_delay,
                    ),
                },
                TxOut {
                    value: Amount::from_sat(value - root.sum - FEE),
                    script_pubkey: spk.clone(),
                },
            ],
        };
        sign(secp, &mut tx, &[funding_coin.clone()], 0, &key);
        let point = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        genesis = Some(tx);
        point
    });
    save(
        &root.join("stage.json"),
        &json!({"schema":"layertree.stage.v1","kind":"deliberately_broken_software_test","network":network,"tree_id":hex(&world.params().tree_id),"funding":funding.to_string(),"until":end}),
    )?;
    let store = root.join("mirror");
    std::fs::create_dir(&store).map_err(|_| "Could not create the test mirror.")?;
    atomic_file(
        &store.join(unroll_name(&world.genesis_unroll.tree_outpoint)),
        &world.genesis_unroll.encode(),
    )?;
    let genesis = genesis.unwrap();
    atomic_file(&root.join("genesis.tx"), &serialize(&genesis))?;
    let h = core.confirm(&genesis, end)?;
    let snapshot = root.join("genesis-snapshot.json");
    save(
        &snapshot,
        &rpc::snapshot(&world.enclave, Some(&world.genesis_unroll), Some(h), None),
    )?;
    let mut log = Writer::new(if network == "signet" {
        Network::Signet
    } else {
        Network::Regtest
    });
    log.push(
        Some(h),
        Body::tree_open(&world.params(), world.enclave.state().unwrap()),
    );
    atomic_file(&root.join("events.jsonl"), log.to_jsonl().as_bytes())?;
    let source = root.join("sources.json");
    save(&source, &json!({"directories":[store]}))?;
    let mut children = Children(Vec::new());
    for member in 0..3 {
        let dir = root.join(format!("member-{member}"));
        bootstrap::snapshot(
            &snapshot,
            &source,
            &dir,
            network,
            &rpc_url,
            h,
            member,
            "LAYERTREE_STAGE_ALARM_WEBHOOK",
        )?;
        let cfg = dir.join("follower.json");
        let mut config: Value = serde_json::from_slice(
            &std::fs::read(&cfg).map_err(|_| "Could not read staged configuration.")?,
        )
        .map_err(|_| "Invalid staged configuration.")?;
        config["poll_seconds"] = json!(5);
        config["missing_after"] = json!(1);
        save(&cfg, &config)?;
        let stdout = std::fs::File::create(dir.join("process.jsonl"))
            .map_err(|_| "Could not create the test process log.")?;
        let stderr = std::fs::File::create(dir.join("process.log"))
            .map_err(|_| "Could not create the test error log.")?;
        children.0.push(
            Command::new(follower)
                .arg("--config")
                .arg(cfg)
                .arg("--until")
                .arg(end.to_string())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .map_err(|_| "Could not start a staged follower.")?,
        );
    }
    loop {
        deadline(end)?;
        if (0..3).all(|member| {
            root.join(format!("member-{member}/state/status.json"))
                .exists()
        }) {
            break;
        }
        if children
            .0
            .iter_mut()
            .any(|c| c.try_wait().ok().flatten().is_some())
        {
            return Err("A staged follower failed its bootstrap.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut fee_point = OutPoint {
        txid: genesis.compute_txid(),
        vout: 1,
    };
    let mut fee_coin = genesis.output[1].clone();
    let mut evidence = Vec::new();
    for scenario in ["withheld", "mismatch", "theft"] {
        deadline(end)?;
        if scenario == "theft" {
            world.enclave.break_for_test();
        }
        let before = world.enclave.state().unwrap().clone();
        let (mut tx, mut prevouts, _) = world.build(&Op::Refresh);
        tx.input[1].previous_output = fee_point;
        prevouts[1] = fee_coin.clone();
        let steal = if scenario == "theft" { 5_000 } else { 0 };
        tx.output[0].value = Amount::from_sat(tx.output[0].value.to_sat() - steal);
        tx.output.push(TxOut {
            value: Amount::from_sat(fee_coin.value.to_sat() - FEE + steal),
            script_pubkey: spk.clone(),
        });
        let sigs = world.sign(&tx, &prevouts, &Op::Refresh);
        let signed = world
            .enclave
            .authorize(tx, &prevouts, &Op::Refresh, &sigs)
            .map_err(|e| format!("The test signer refused the staged step: {e:?}."))?;
        let mut tx = signed.tx.clone();
        sign(&secp, &mut tx, &prevouts, 1, &key);
        let txid = tx.compute_txid();
        atomic_file(&root.join(format!("{scenario}.tx")), &serialize(&tx))?;
        let rec_path = store.join(record_name(&signed.record.hash()));
        if scenario == "mismatch" {
            atomic_file(&rec_path, b"deliberately wrong test record")?;
        } else if scenario == "theft" {
            atomic_file(&rec_path, &signed.record.encode())?;
        }
        if let Some(pkg) = &signed.unroll {
            atomic_file(&store.join(unroll_name(&pkg.tree_outpoint)), &pkg.encode())?;
        }
        let height = core.confirm(&tx, end)?;
        let mut body = Body::transition(&Op::Refresh, &before, &signed.record, &txid);
        if let Body::Transition {
            observed_output_amount,
            ..
        } = &mut body
        {
            *observed_output_amount = Some(tx.output[0].value.to_sat());
        }
        log.push(Some(height), body);
        log.push(
            Some(height),
            Body::Block {
                spent: vec![before.outpoint.to_string()],
            },
        );
        atomic_file(&root.join("events.jsonl"), log.to_jsonl().as_bytes())?;
        if scenario == "withheld" {
            core.tick()?;
        }
        let result = match scenario {
            "withheld" => "RecordMissing",
            "mismatch" => "RecordNotAnchored",
            _ => "OutputMismatch",
        };
        println!(
            "{}",
            json!({"scenario":scenario,"txid":txid.to_string(),"height":height,"waiting_for":result})
        );
        let alarms = wait_for(root, end, &mut children, Some(txid), result)?;
        if scenario == "theft"
            && alarms.iter().any(|row| {
                row["observed_tip"]
                    .as_u64()
                    .is_none_or(|tip| tip > u64::from(height) + 1)
            })
        {
            return Err("The staged theft was detected more than one block after mining.".into());
        }
        evidence.push(
            json!({"scenario":scenario,"txid":txid.to_string(),"height":height,"alarms":alarms}),
        );
        save(&root.join("attack-evidence.json"), &evidence)?;
        if scenario != "theft" {
            atomic_file(&rec_path, &signed.record.encode())?;
            wait_for(root, end, &mut children, Some(txid), "ok")?;
        }
        fee_point = OutPoint {
            txid,
            vout: (tx.output.len() - 1) as u32,
        };
        fee_coin = tx.output.last().unwrap().clone();
    }
    println!(
        "{}",
        json!({"schema":"layertree.stage.result.v1","network":network,"root":root,"scenarios":evidence,"result":"passed"})
    );
    Ok(())
}
fn main() -> ExitCode {
    let result = (|| -> Result<(), String> {
        let mut args = std::env::args().skip(1);
        let action=args.next().ok_or("Use init DIRECTORY or run DIRECTORY OUTPOINT FOLLOWER --until UNIX --network signet|regtest [--mine-address ADDRESS].")?;
        let root = std::path::absolute(PathBuf::from(
            args.next().ok_or("Choose a new staging directory.")?,
        ))
        .map_err(|_| "Could not resolve the staging directory.")?;
        if action == "init" {
            if args.next().is_some() {
                return Err("Use init DIRECTORY.".into());
            }
            std::fs::create_dir(&root).map_err(|_| "Choose a new staging directory.")?;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Could not protect the test directory.")?;
            std::fs::create_dir(root.join("private"))
                .map_err(|_| "Could not create the test key directory.")?;
            let secp = Secp256k1::new();
            let key = SecretKey::new(&mut bitcoin::secp256k1::rand::thread_rng());
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(root.join("private/funding.key"))
                .map_err(|_| "Could not save the test funding key.")?;
            file.write_all(hex(&key.secret_bytes()).as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|_| "Could not save the test funding key.")?;
            let output = json!({"signet_address":Address::p2tr(&secp,key.x_only_public_key(&secp).0,None,bitcoin::Network::Signet).to_string(),"regtest_address":Address::p2tr(&secp,key.x_only_public_key(&secp).0,None,bitcoin::Network::Regtest).to_string()});
            save(&root.join("funding-address.json"), &output)?;
            println!("{output}");
            return Ok(());
        }
        if action != "run" {
            return Err("Use init or run.".into());
        }
        let point = OutPoint::from_str(
            &args
                .next()
                .ok_or("Choose the dedicated test funding outpoint.")?,
        )
        .map_err(|_| "Invalid test funding outpoint.")?;
        let follower = PathBuf::from(args.next().ok_or("Choose the follower binary.")?);
        let (mut until, mut network, mut mine) = (None, None, None);
        while let Some(arg) = args.next() {
            let value = args.next().ok_or("An option needs a value.")?;
            match arg.as_str() {
                "--until" => until = Some(value.parse().map_err(|_| "Invalid deadline.")?),
                "--network" => network = Some(value),
                "--mine-address" => mine = Some(value),
                _ => return Err("Unknown staging option.".into()),
            }
        }
        run(
            &root,
            point,
            &follower,
            until.ok_or("Choose --until UNIX.")?,
            &network.ok_or("Choose --network signet|regtest.")?,
            mine,
        )
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
