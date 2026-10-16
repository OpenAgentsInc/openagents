//! Treasury administration. Wallets, the earnings ledger, and top-up
//! resources are mounted by the hosting process, never from ambient files.
use crate::{
    Result,
    treasury::{Config, Treasury},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: sov-treasury init --state DIR --key-file KEY --config CONFIG.json";

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
    if args.first().map(String::as_str) != Some("init")
        || args.iter().any(|a| a == "--help" || a == "-h")
    {
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
    let bytes = workspace_host::artifact::read(Path::new(&need("--config")?), 1_048_576)?;
    let config: Config = serde_json::from_slice(&bytes).map_err(|_| "malformed: config")?;
    Treasury::initialize(
        PathBuf::from(need("--state")?),
        read_secret(Path::new(&need("--key-file")?))?,
        config,
    )?;
    Ok(json!({"created":true}))
}
