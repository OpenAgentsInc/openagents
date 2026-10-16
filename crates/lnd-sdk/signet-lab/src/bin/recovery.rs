//! Fresh signet-only seed, watch-only, and SCB qualification; no secret output.
use lnd_sdk::{
    Client,
    macaroon::{Macaroon, Role},
    proto::lnrpc,
    service::LightningRail,
    transport::Config,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tonic::transport::Channel;
use zeroize::{Zeroize, Zeroizing};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn port(node: &str) -> Result<u16> {
    match node {
        "signer" => Ok(10059),
        "watch" => Ok(10069),
        "peer" => Ok(10079),
        _ => Err("Unknown research node.".into()),
    }
}
fn read<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn save(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = path.as_ref();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    let bytes = Zeroizing::new(serde_json::to_vec_pretty(value)?);
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::File::open(path.parent().ok_or("Missing parent directory.")?)?.sync_all()?;
    Ok(())
}
fn cli(node: &str, args: &[&str]) -> Result<Value> {
    let output = Command::new("timeout")
        .args(["45s", "docker"])
        .args([
            "exec",
            &format!("oa-lnd-signet-11862-{node}"),
            "lncli",
            "--lnddir=/data",
            "--network=signet",
            &format!("--rpcserver=127.0.0.1:{}", port(node)?),
        ])
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err("The research node refused the command.".into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
async fn once<T>(
    call: impl std::future::Future<Output = std::result::Result<tonic::Response<T>, tonic::Status>>,
    message: &'static str,
) -> Result<T> {
    let response = tokio::time::timeout(Duration::from_secs(30), call)
        .await
        .map_err(|_| message)?;
    Ok(response.map_err(|_| message)?.into_inner())
}

async fn bootstrap(
    root: &Path,
    node: &str,
) -> Result<lnrpc::wallet_unlocker_client::WalletUnlockerClient<Channel>> {
    let certificate = rustls_pemfile::certs(&mut std::io::Cursor::new(fs::read(
        root.join(node).join("tls.cert"),
    )?))
    .next()
    .ok_or("Missing research certificate.")??;
    let mut config = Config::new(
        format!("https://127.0.0.1:{}", port(node)?),
        Sha256::digest(certificate.as_ref()).into(),
    );
    config.timeout = Duration::from_secs(30);
    Ok(lnd_sdk::transport::wallet_unlocker(&config).await?)
}
async fn client(root: &Path, node: &str) -> Result<Client> {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join(node);
    let certificate =
        rustls_pemfile::certs(&mut std::io::Cursor::new(fs::read(path.join("tls.cert"))?))
            .next()
            .ok_or("Missing research certificate.")??;
    let config = Config::new(
        format!("https://127.0.0.1:{}", port(node)?),
        Sha256::digest(certificate.as_ref()).into(),
    );
    let macaroon = path.join("data/chain/bitcoin/signet/admin.macaroon");
    fs::set_permissions(&macaroon, fs::Permissions::from_mode(0o600))?;
    let client = Client::connect(&config, Macaroon::from_file(Role::Ops, 0, &macaroon)?).await?;
    if client.network() != "signet" {
        return Err("The research node must use signet.".into());
    }
    Ok(client)
}
async fn initialize_seed(root: &Path, node: &str, restore: bool) -> Result<Value> {
    if node != "signer" && node != "peer" {
        return Err("Only seed-holding research nodes can use this operation.".into());
    }
    let private = root.join("private");
    let seed_path = private.join(format!("{node}-seed.json"));
    let mut rpc = bootstrap(root, node).await?;
    if !seed_path.exists() {
        if restore {
            return Err("The original private test seed is missing.".into());
        }
        let mut generated = once(
            rpc.gen_seed(lnrpc::GenSeedRequest::default()),
            "The research node refused seed generation.",
        )
        .await?;
        generated.enciphered_seed.zeroize();
        let words = Zeroizing::new(generated.cipher_seed_mnemonic);
        save(&seed_path, &*words)?;
    }
    let words = Zeroizing::new(read::<Vec<String>>(&seed_path)?);
    let password = Zeroizing::new(fs::read(private.join(format!("{node}-password")))?);
    let marker = root.join(if restore {
        format!("{node}-seed-restore-started.json")
    } else {
        format!("{node}-init-started.json")
    });
    save(marker, &json!({"at_unix":now(),"restore":restore}))?;
    let request = lnrpc::InitWalletRequest {
        wallet_password: password.to_vec(),
        cipher_seed_mnemonic: words.to_vec(),
        recovery_window: if restore { 100 } else { 0 },
        ..Default::default()
    };
    let mut response = once(
        rpc.init_wallet(request),
        "Wallet initialization is uncertain; inspect the original research node before retrying.",
    )
    .await?;
    response.admin_macaroon.zeroize();
    Ok(json!({"node":node,"seed_restored":restore,"private_seed_printed":false}))
}
async fn initialize_watch(root: &Path, restore: bool) -> Result<Value> {
    let accounts_path = root.join("public-accounts.json");
    if !accounts_path.exists() {
        if restore {
            return Err("The original public account set is missing.".into());
        }
        let exported = cli("signer", &["wallet", "accounts", "list"])?;
        save(&accounts_path, &exported)?;
    }
    let exported: Value = read(&accounts_path)?;
    let mut accounts = Vec::new();
    for account in exported["accounts"]
        .as_array()
        .ok_or("Missing signer accounts.")?
    {
        let Some(xpub) = account["extended_public_key"]
            .as_str()
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let path = account["derivation_path"]
            .as_str()
            .ok_or("Missing public account path.")?;
        let indices: Vec<u32> = path
            .strip_prefix("m/")
            .ok_or("Invalid public account path.")?
            .split('/')
            .map(|index| {
                index
                    .strip_suffix('\'')
                    .ok_or("Invalid public account path.")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid public account path.")
            })
            .collect::<std::result::Result<_, _>>()?;
        if indices.len() != 3 {
            return Err("Unexpected public account depth.".into());
        }
        accounts.push(lnrpc::WatchOnlyAccount {
            purpose: indices[0],
            coin_type: indices[1],
            account: indices[2],
            xpub: xpub.into(),
        });
    }
    if accounts.len() < 259 {
        return Err("The public account set is incomplete.".into());
    }
    let count = accounts.len();
    let metadata: Value = read(root.join("setup.json"))?;
    let birthday = metadata["created_at_unix"]
        .as_u64()
        .ok_or("Missing original wallet birthday.")?
        .saturating_sub(7200);
    let password = Zeroizing::new(fs::read(root.join("private/watch-password"))?);
    let marker = if restore {
        "watch-restore-started.json"
    } else {
        "watch-init-started.json"
    };
    save(
        root.join(marker),
        &json!({"at_unix":now(),"restore":restore,"public_accounts":count}),
    )?;
    let mut rpc = bootstrap(root, "watch").await?;
    let mut response=once(rpc.init_wallet(lnrpc::InitWalletRequest{wallet_password:password.to_vec(),
        watch_only:Some(lnrpc::WatchOnly{master_key_birthday_timestamp:birthday,accounts,..Default::default()}),
        recovery_window:if restore{100}else{0},..Default::default()}),
        "Watch-only initialization is uncertain; inspect the original research node before retrying.").await?;
    response.admin_macaroon.zeroize();
    Ok(
        json!({"node":"watch","watch_restored":restore,"restored_scb":false,"public_accounts":count,"private_keys_imported":false}),
    )
}
async fn inspect(root: &Path) -> Result<Value> {
    let signer = client(root, "signer").await?;
    let watch = client(root, "watch").await?;
    let peer = client(root, "peer").await?;
    if signer.node_id() != watch.node_id() || watch.node_id() == peer.node_id() {
        return Err("Research identities do not match the isolated signer arrangement.".into());
    }
    let accounts = cli("watch", &["wallet", "accounts", "list"])?;
    let list = accounts["accounts"]
        .as_array()
        .ok_or("Missing watch-only accounts.")?;
    if list.len() < 259 || !list.iter().all(|a| a["watch_only"] == true) {
        return Err("The research watcher contains a non-watch-only account.".into());
    }
    let mut nodes = Vec::new();
    for (name, node) in [("signer", signer), ("watch", watch), ("peer", peer)] {
        let info = node.info().await?;
        nodes.push(json!({"node":name,"identity":node.node_id(),"network":node.network(),"synced":info.synced_to_chain,"height":info.block_height,"version":info.version}));
    }
    let result =
        json!({"nodes":nodes,"watch_only_accounts":list.len(),"same_signer_identity":true});
    let output = root.join("identity-before-funding.json");
    if output.exists() {
        let original: Value = read(&output)?;
        let before = original["nodes"]
            .as_array()
            .ok_or("Missing original identities.")?;
        let after = result["nodes"]
            .as_array()
            .ok_or("Missing current identities.")?;
        if before.len() != after.len()
            || before.iter().zip(after).any(|(a, b)| {
                a["node"] != b["node"]
                    || a["identity"] != b["identity"]
                    || a["network"] != b["network"]
            })
        {
            return Err("The original research identities changed.".into());
        }
    } else {
        save(output, &result)?;
    }
    Ok(result)
}
async fn addresses(root: &Path) -> Result<Value> {
    inspect(root).await?;
    let output = root.join("funding-addresses.json");
    if output.exists() {
        return read(output);
    }
    for node in ["watch", "peer"] {
        let info = client(root, node).await?.info().await?;
        if !info.synced_to_chain {
            return Err("The research wallet is still syncing.".into());
        }
    }
    let watch = cli("watch", &["newaddress", "p2wkh"])?;
    let peer = cli("peer", &["newaddress", "p2wkh"])?;
    let value = json!({"network":"signet","watch":watch["address"],"peer":peer["address"]});
    save(output, &value)?;
    Ok(value)
}
fn docker(action: &str, node: &str) -> Result<()> {
    port(node)?;
    if !matches!(action, "pause" | "unpause") {
        return Err("Unsupported research process operation.".into());
    }
    let status = Command::new("docker")
        .args([action, &format!("oa-lnd-signet-11862-{node}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err("Could not change the research signer state.".into());
    }
    Ok(())
}
async fn payment_case(
    root: &Path,
    label: &str,
    payer: &Client,
    receiver: &Client,
    amount: u64,
    pause: bool,
) -> Result<Value> {
    use openagents_wallet::Proof;
    let directory = root.join("private").join(label);
    fs::create_dir_all(&directory)?;
    let invoice_path = directory.join("invoice.json");
    let invoice: openagents_wallet::IssuedInvoice = if invoice_path.exists() {
        read(&invoice_path)?
    } else {
        let hints = receiver.private_route_hints().await?;
        if hints.is_empty() {
            return Err("The research private channel is not ready.".into());
        }
        let description: [u8; 32] = Sha256::digest(format!("signet recovery 11862 {label}")).into();
        let invoice = receiver
            .create_invoice_with_hints(amount, description, 3600, hints)
            .await?;
        save(&invoice_path, &invoice)?;
        invoice
    };
    let hash: [u8; 32] = hex::decode(&invoice.payment_hash)?
        .try_into()
        .map_err(|_| "Invalid original payment hash.")?;
    let proof_path = directory.join("proof.json");
    let marker = directory.join("send-started.json");
    let started = std::time::Instant::now();
    let proof: Proof = if proof_path.exists() {
        read(&proof_path)?
    } else {
        let immediate = if marker.exists() {
            None
        } else {
            save(
                &marker,
                &json!({"payment_hash":invoice.payment_hash,"amount_msat":amount,"fee_cap_msat":5000,"pause_signer":pause,"at_unix":now()}),
            )?;
            if pause {
                // A separate, bounded guard resumes this exact test container
                // even if the caller exits while its payment response is lost.
                Command::new("sh")
                    .args([
                        "-c",
                        "sleep 8; docker unpause oa-lnd-signet-11862-signer >/dev/null 2>&1",
                    ])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()?;
                docker("pause", "signer")?;
                let result = payer
                    .send_payment(&invoice.bolt11, 5000, Duration::from_secs(3))
                    .await;
                let observed = receiver.received_msat(hash).await;
                let resumed = docker("unpause", "signer");
                resumed?;
                if !matches!(
                    result,
                    Err(lnd_sdk::Error::Timeout | lnd_sdk::Error::InFlight)
                ) || observed? != None
                {
                    return Err(
                        "The signer-pause result did not establish an unresolved unpaid request."
                            .into(),
                    );
                }
                save(
                    directory.join("pause-observation.json"),
                    &json!({"result":"unresolved","received_while_paused":false,"same_hash_tracked_after_resume":true}),
                )?;
                None
            } else {
                Some(
                    payer
                        .send_payment(&invoice.bolt11, 5000, Duration::from_secs(30))
                        .await?,
                )
            }
        };
        let proof = if let Some(proof) = immediate {
            proof
        } else {
            let deadline = std::time::Instant::now() + Duration::from_secs(120);
            loop {
                let p = payer.track(hash).await?;
                match lnrpc::payment::PaymentStatus::try_from(p.status) {
                    Ok(lnrpc::payment::PaymentStatus::Succeeded) => {
                        break Proof {
                            payment_hash: p.payment_hash,
                            preimage: p.payment_preimage,
                            amount_msat: u64::try_from(p.value_msat)?,
                            fee_msat: u64::try_from(p.fee_msat)?,
                            bolt11: invoice.bolt11.clone(),
                        };
                    }
                    Ok(lnrpc::payment::PaymentStatus::Failed) => {
                        return Err(
                            "The original research payment failed; no replacement was sent.".into(),
                        );
                    }
                    _ => {}
                }
                if std::time::Instant::now() >= deadline {
                    return Err(
                        "The original research payment is still unresolved; track its saved hash."
                            .into(),
                    );
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        };
        save(&proof_path, &proof)?;
        proof
    };
    let preimage = Zeroizing::new(hex::decode(&proof.preimage)?);
    if proof.payment_hash != invoice.payment_hash
        || proof.bolt11 != invoice.bolt11
        || proof.amount_msat != amount
        || proof.fee_msat > 5000
        || preimage.len() != 32
        || Sha256::digest(&*preimage).as_slice() != hash
    {
        return Err("The original payment proof does not match its invoice.".into());
    }
    if pause && !directory.join("pause-observation.json").exists() {
        return Err(
            "The original payment was recovered, but its signer-pause observation is missing."
                .into(),
        );
    }
    if receiver.received_msat(hash).await? != Some(amount) {
        return Err("The receiver did not settle the exact original amount.".into());
    }
    let result = json!({"case":label,"payment_hash":invoice.payment_hash,"amount_msat":amount,"fee_msat":proof.fee_msat,"sender":payer.node_id(),"receiver":receiver.node_id(),
        "signer_pause":pause,"original_hash_only":true,"elapsed_ms":started.elapsed().as_millis()});
    let result_path = directory.join("public-result.json");
    if !result_path.exists() {
        save(&result_path, &result)?;
    }
    read(result_path)
}
async fn exercise(root: &Path) -> Result<Value> {
    inspect(root).await?;
    let watch = client(root, "watch").await?;
    let peer = client(root, "peer").await?;
    let channels = cli("watch", &["listchannels"])?;
    let list = channels["channels"]
        .as_array()
        .ok_or("Missing research channel.")?;
    if list.len() != 1
        || list[0]["active"] != true
        || list[0]["private"] != true
        || list[0]["commitment_type"] != "ANCHORS"
        || list[0]["capacity"] != "20000"
    {
        return Err("The confirmed private research channel is not ready.".into());
    }
    let backup = watch.channel_backup().await?;
    if backup.channels != 1 {
        return Err("Unexpected research backup channel count.".into());
    }
    let backup_path = root.join("private/channel.backup");
    if !backup_path.exists() {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&backup_path)?;
        file.write_all(backup.encrypted_bytes())?;
        file.sync_all()?;
    }
    fs::copy(&backup_path, root.join("watch/research-channel.backup"))?;
    cli(
        "watch",
        &[
            "verifychanbackup",
            "--multi_file",
            "/data/research-channel.backup",
        ],
    )?;
    let outbound = payment_case(root, "outbound", &watch, &peer, 1_001_001, false).await?;
    let inbound = payment_case(root, "inbound", &peer, &watch, 500_001, false).await?;
    let outage = payment_case(root, "signer-pause", &watch, &peer, 1_000_000, true).await?;
    let result = json!({"network":"signet","channel_point":list[0]["channel_point"],"capacity_sat":20000,"private":true,
        "payments":[outbound,inbound,outage],"backup":{"channels":backup.channels,"sha256":hex::encode(Sha256::digest(fs::read(&backup_path)?)),"encrypted":true,"verified_by_lnd":true},
        "watch_only_accounts":259,"signer_identity":watch.node_id()});
    let output = root.join("exercise-result.json");
    if !output.exists() {
        save(&output, &result)?;
    }
    Ok(result)
}

async fn restore_backup(root: &Path) -> Result<Value> {
    inspect(root).await?;
    if cli("watch", &["state"])?["state"] != "SERVER_ACTIVE"
        || cli("watch", &["getinfo"])?["synced_to_chain"] != true
    {
        return Err(
            "Wait for the restored watcher to finish starting before applying its backup.".into(),
        );
    }
    let result: Value = read(root.join("exercise-result.json"))?;
    let bytes = fs::read(root.join("private/channel.backup"))?;
    let digest = hex::encode(Sha256::digest(&bytes));
    if result["backup"]["sha256"] != digest || result["backup"]["channels"] != 1 {
        return Err("The original research backup changed.".into());
    }
    fs::copy(
        root.join("private/channel.backup"),
        root.join("watch/research-channel.backup"),
    )?;
    cli(
        "watch",
        &[
            "verifychanbackup",
            "--multi_file",
            "/data/research-channel.backup",
        ],
    )?;
    save(
        root.join("scb-restore-started.json"),
        &json!({"at_unix":now(),"backup_sha256":digest,"channel_point":result["channel_point"]}),
    )?;
    cli(
        "watch",
        &[
            "restorechanbackup",
            "--multi_file",
            "/data/research-channel.backup",
        ],
    )?;
    let result = json!({"scb_restore_submitted":true,"backup_sha256":digest,"private_keys_imported_into_watch":false});
    save(root.join("scb-restore-result.json"), &result)?;
    Ok(result)
}
async fn restore_first_channel_key(root: &Path) -> Result<Value> {
    let request: Value = read(root.join("anchor-key-repair-request.json"))?;
    let exercise: Value = read(root.join("exercise-result.json"))?;
    if request["channel_point"] != exercise["channel_point"]
        || request["key_family"] != 0
        || request["key_index"] != 0
        || read::<Value>(root.join("scb-restore-result.json"))?["scb_restore_submitted"] != true
    {
        return Err("The public key repair does not match the original recovered channel.".into());
    }
    let watch = client(root, "watch").await?;
    let signer = client(root, "signer").await?;
    if watch.node_id() != signer.node_id()
        || exercise["signer_identity"] != watch.node_id()
        || !cli("watch", &["listchannels"])?["channels"]
            .as_array()
            .ok_or("Missing research channels.")?
            .is_empty()
    {
        return Err("The public key repair requires the original restored watcher.".into());
    }
    let accounts = cli("watch", &["wallet", "accounts", "list"])?;
    let accounts = accounts["accounts"]
        .as_array()
        .ok_or("Missing public accounts.")?;
    if accounts.len() < 259 || !accounts.iter().all(|a| a["watch_only"] == true) {
        return Err("The restored watcher must still contain only public accounts.".into());
    }
    let account = accounts
        .iter()
        .find(|a| a["derivation_path"] == "m/1017'/1'/0'")
        .ok_or("Missing the original multisig account.")?;
    if account["external_key_count"] != 0 {
        return Err("Inspect the previously indexed public keys before another repair.".into());
    }
    let expected = request["public_key"]
        .as_str()
        .ok_or("Missing the original public key.")?;
    // DeriveKey does not store an address. The restored first channel's zero
    // locator requires that address for RPCKeyRing's public-key lookup.
    save(root.join("anchor-key-repair-started.json"), &request)?;
    watch
        .index_signet_research_first_channel_key(
            hex::decode(expected)?
                .try_into()
                .map_err(|_| "Invalid original public key.")?,
        )
        .await?;
    let result = json!({"channel_point":request["channel_point"], "public_key":expected,
        "key_family":0, "key_index":0, "public_address_indexed":true,
        "private_keys_imported":false});
    save(root.join("anchor-key-repair-result.json"), &result)?;
    Ok(result)
}
#[tokio::main]
async fn main() {
    let result = async {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 {
            return Err(
                "Use ROOT init-signer|init-peer|init-watch|restore-signer|restore-watch|restore-backup|restore-first-channel-key|inspect|addresses|exercise."
                    .into(),
            );
        }
        let root = PathBuf::from(&args[0]).canonicalize()?;
        if root.file_name().and_then(|s| s.to_str()) != Some("lnd-signet-recovery-11862") {
            return Err("Refusing a non-research wallet directory.".into());
        }
        let value = match args[1].as_str() {
            "init-signer" => initialize_seed(&root, "signer", false).await?,
            "init-peer" => initialize_seed(&root, "peer", false).await?,
            "init-watch" => initialize_watch(&root, false).await?,
            "restore-signer" => initialize_seed(&root, "signer", true).await?,
            "restore-watch" => initialize_watch(&root, true).await?,
            "inspect" => inspect(&root).await?,
            "addresses" => addresses(&root).await?,
            "exercise" => exercise(&root).await?,
            "restore-backup" => restore_backup(&root).await?,
            "restore-first-channel-key" => restore_first_channel_key(&root).await?,
            _ => return Err("Unknown research command.".into()),
        };
        println!("{value}");
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    }
    .await;
    if let Err(error) = result {
        eprintln!("Research check stopped: {error}");
        std::process::exit(1);
    }
}
