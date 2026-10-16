//! Local custodian administration. Remote signer connections are mounted
//! by the hosting process; this command line only initializes, revokes
//! callers, and advances a controller generation.
use crate::{
    Result,
    custodian::{Config, Custodian},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: sov-custody ACTION --state DIR --key-file KEY [options]
  init         --config CONFIG.json
  revoke       --caller PUBKEY
  generation   --agent PUBKEY";

fn read_secret(path: &Path) -> Result<secp256k1::SecretKey> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 256 {
        return Err("malformed: invalid key file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("not_admitted: key file must be readable only by its owner".into());
        }
    }
    std::fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "malformed: invalid key file".into())
}

pub fn run(args: Vec<String>) -> Result<Value> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(json!({"help":USAGE}));
    }
    let need = |name: &str| -> Result<String> {
        let index = args
            .iter()
            .position(|a| a == name)
            .ok_or_else(|| format!("malformed: missing {name}"))?;
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("malformed: missing value for {name}"))
    };
    let root = PathBuf::from(need("--state")?);
    let secret = read_secret(Path::new(&need("--key-file")?))?;
    match args[0].as_str() {
        "init" => {
            let bytes = workspace_host::artifact::read(Path::new(&need("--config")?), 1_048_576)?;
            let config: Config = serde_json::from_slice(&bytes).map_err(|_| "malformed: config")?;
            Custodian::initialize(root, secret, config)?;
            Ok(json!({"created":true}))
        }
        "revoke" => {
            Custodian::open(root, secret)?.revoke(&need("--caller")?)?;
            Ok(json!({"revoked":true}))
        }
        "generation" => Ok(
            json!({"generation":Custodian::open(root, secret)?.advance_generation(&need("--agent")?)?}),
        ),
        _ => Err(format!("malformed: {USAGE}")),
    }
}
