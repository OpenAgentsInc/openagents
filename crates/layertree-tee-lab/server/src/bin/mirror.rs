//! A durable public-data mirror with a separate research receipt key.
use bitcoin::{
    key::Keypair,
    secp256k1::{Message, Secp256k1, SecretKey},
};
use layertree_signet_service::{request, response, response_bytes};
use layertree_tee_lab::{
    follow::{record_name, unroll_name},
    mirror::receipt_msg,
    rpc,
    rules::Record,
    unroll::UnrollPackage,
};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

fn store(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = root.join(name);
    let temporary = root.join(format!(".{}.writing", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&temporary)
        .map_err(|_| "cannot stage public data")?;
    file.write_all(bytes)
        .map_err(|_| "cannot write public data")?;
    file.sync_all().map_err(|_| "cannot sync public data")?;
    let result = match std::fs::hard_link(&temporary, &path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut previous = Vec::new();
            File::open(&path)
                .map_err(|_| "cannot read stored data")?
                .take((rpc::MAX_BODY + 1) as u64)
                .read_to_end(&mut previous)
                .map_err(|_| "cannot read stored data")?;
            if previous == bytes {
                Ok(())
            } else {
                Err("stored bytes differ".into())
            }
        }
        Err(_) => Err("cannot publish public data".into()),
    };
    std::fs::remove_file(temporary).map_err(|_| "cannot remove staging file")?;
    File::open(root)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "cannot sync directory")?;
    result
}
fn run() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() == 3 && args[1] == "keygen" {
        let secp = Secp256k1::new();
        let (secret, _) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&args[2])
            .map_err(|_| "mirror key already exists or cannot be created")?;
        file.write_all(rpc::hex(secret.as_ref()).as_bytes())
            .map_err(|_| "cannot save mirror key")?;
        file.sync_all().map_err(|_| "cannot sync mirror key")?;
        println!(
            "{}",
            Keypair::from_secret_key(&secp, &secret)
                .x_only_public_key()
                .0
        );
        return Ok(());
    }
    if args.len() != 4 {
        return Err("use ADDRESS DATA_DIRECTORY KEY_FILE".into());
    }
    let key_path = PathBuf::from(&args[3]);
    if std::fs::metadata(&key_path)
        .map_err(|_| "cannot read mirror key")?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("mirror key permissions must be 0600".into());
    }
    let key = SecretKey::from_slice(&rpc::bytes(
        std::fs::read_to_string(key_path)
            .map_err(|_| "cannot read mirror key")?
            .trim(),
    )?)
    .map_err(|_| "invalid mirror key")?;
    let secp = Secp256k1::new();
    let pair = Keypair::from_secret_key(&secp, &key);
    let root = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&root).map_err(|_| "cannot create mirror directory")?;
    let listener = TcpListener::bind(&args[1]).map_err(|_| "cannot listen")?;
    for socket in listener.incoming() {
        let Ok(mut socket) = socket else { continue };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .ok();
        socket
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .ok();
        let result = (|| -> Result<Option<Value>, String> {
            let input = request(&mut socket)?;
            if input.method == "POST" && input.path == "/store" {
                let payload: rpc::Stored =
                    serde_json::from_slice(&input.body).map_err(|_| "invalid stored payload")?;
                let record =
                    Record::decode(&rpc::bytes(&payload.record)?).map_err(|_| "invalid record")?;
                let unroll = payload
                    .unroll
                    .as_ref()
                    .map(|raw| {
                        UnrollPackage::decode(&rpc::bytes(raw)?)
                            .map_err(|_| "invalid unroll".to_string())
                    })
                    .transpose()?;
                if let Some(pkg) = &unroll {
                    store(&root, &unroll_name(&pkg.tree_outpoint), &pkg.encode())?;
                }
                store(&root, &record_name(&record.hash()), &record.encode())?;
                let signature = secp.sign_schnorr(
                    &Message::from_digest(receipt_msg(&record, unroll.as_ref())),
                    &pair,
                );
                return Ok(Some(
                    serde_json::to_value(rpc::Receipt {
                        mirror: pair.x_only_public_key().0.to_string(),
                        signature: signature.to_string(),
                    })
                    .expect("receipt"),
                ));
            }
            if input.method == "GET" {
                let name = input.path.strip_prefix('/').ok_or("invalid path")?;
                if name.len() > 180
                    || name.contains('/')
                    || name.contains("..")
                    || !(name.starts_with("record-") || name.starts_with("unroll-"))
                {
                    return Err("unknown public data path".into());
                }
                let mut bytes = Vec::new();
                File::open(root.join(name))
                    .map_err(|_| "public data not found")?
                    .take((rpc::MAX_BODY + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "cannot read public data")?;
                if bytes.len() > rpc::MAX_BODY {
                    return Err("public data is too large".into());
                }
                response_bytes(&mut socket, 200, "application/octet-stream", &bytes)?;
                return Ok(None);
            }
            Err("unknown mirror route".into())
        })();
        let (status, body) = match result {
            Ok(Some(value)) => (200, value),
            Ok(None) => continue,
            Err(error) => (400, json!({"error":error})),
        };
        let _ = response(&mut socket, status, &body);
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("signet mirror stopped: {error}");
        std::process::exit(1);
    }
}
