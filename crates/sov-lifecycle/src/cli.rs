//! Local coordinator commands. Signed inputs use the same API as remote calls.
use crate::{
    Result,
    coordinator::{Bundle, Config, Coordinator},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: sov-lifecycle ACTION --state DIR --key-file KEY [options]
  init     --config CONFIG.json
  submit   --input BUNDLE
  status   --activation ID
  expire";

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

fn json_file(path: &str) -> Result<Value> {
    let bytes = workspace_host::artifact::read(Path::new(path), 8 * 1_048_576)?;
    nostr::contracts::parse_strict_bounded(&bytes, 8 * 1_048_576).map_err(|e| e.to_string())
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
    if args[0] == "init" {
        let config: Config = serde_json::from_value(json_file(&need("--config")?)?)
            .map_err(|_| "malformed: config")?;
        Coordinator::initialize(root, secret, config)?;
        return Ok(json!({"created":true}));
    }
    let coordinator = Coordinator::open(root, secret)?;
    match args[0].as_str() {
        "submit" => {
            let bundle: Bundle = serde_json::from_value(json_file(&need("--input")?)?)
                .map_err(|_| "malformed: bundle")?;
            coordinator.submit(&bundle, crate::now())
        }
        "status" => coordinator.status(&need("--activation")?),
        "expire" => Ok(json!({"expired":coordinator.expire(crate::now())?})),
        _ => Err(format!("malformed: {USAGE}")),
    }
}
