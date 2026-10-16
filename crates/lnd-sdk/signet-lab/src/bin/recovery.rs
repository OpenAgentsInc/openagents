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
    let output = Command::new("docker")
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
    let backups = if restore {
        Some(lnrpc::ChanBackupSnapshot {
            multi_chan_backup: Some(lnrpc::MultiChanBackup {
                multi_chan_backup: fs::read(root.join("private/channel.backup"))?,
                chan_points: vec![],
            }),
            ..Default::default()
        })
    } else {
        None
    };
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
        recovery_window:if restore{100}else{0},channel_backups:backups,..Default::default()}),
        "Watch-only initialization is uncertain; inspect the original research node before retrying.").await?;
    response.admin_macaroon.zeroize();
    Ok(
        json!({"node":"watch","restored_scb":restore,"public_accounts":count,"private_keys_imported":false}),
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
#[tokio::main]
async fn main() {
    let result = async {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 {
            return Err(
                "Use ROOT init-signer|init-peer|init-watch|restore-signer|restore-watch|inspect."
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
