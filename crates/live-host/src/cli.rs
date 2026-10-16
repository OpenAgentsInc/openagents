//! Local controller commands over the synthetic adapters. A production host
//! mounts its own broker, capture source, and device behind the same roles.
use crate::{
    Result,
    controller::{Adapters, Bundle, Controller, Policy},
    media::{LocalBroker, SyntheticDevice, SyntheticSource},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: live-host ACTION --state DIR --key-file KEY [options]
  init     --policy POLICY.json
  submit   --input BUNDLE
  status   --session ID
  device-operation   describe the installed input operation
  input-policy --input GRANTS.json   replace exact input approval grants
  serve --socket PATH (--document REGISTRATION.json | --synthetic)
  request --socket PATH --input REQUEST.json   send with the caller's signed event";

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
    #[cfg(unix)]
    if args[0] == "request" {
        let request = serde_json::from_value(json_file(&need("--input")?)?)
            .map_err(|_| "malformed: resident request")?;
        return serde_json::to_value(crate::resident::request(
            Path::new(&need("--socket")?),
            &request,
        )?)
        .map_err(|e| e.to_string());
    }
    let root = PathBuf::from(need("--state")?);
    let secret = read_secret(Path::new(&need("--key-file")?))?;
    if args[0] == "init" {
        let policy: Policy = serde_json::from_value(json_file(&need("--policy")?)?)
            .map_err(|_| "malformed: policy")?;
        Controller::initialize(root, secret, policy)?;
        return Ok(json!({"created":true}));
    }
    if args[0] == "serve" && !args.iter().any(|a| a == "--document" || a == "--synthetic") {
        return Err("not_admitted: select a document or synthetic profile".into());
    }
    let mut adapters = Adapters {
        broker: Box::new(LocalBroker::default()),
        source: Box::new(SyntheticSource::default()),
        device: Box::new(SyntheticDevice::default()),
    };
    if args.iter().any(|a| a == "--document") {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Registration {
            root: PathBuf,
            branch: String,
            path: String,
            owner: String,
        }
        let config: Registration = serde_json::from_value(json_file(&need("--document")?)?)
            .map_err(|_| "malformed: document registration")?;
        let document = crate::document::Document::register(
            &config.root,
            &config.branch,
            &config.path,
            config.owner,
        )?;
        adapters.source = Box::new(document.clone());
        adapters.device = Box::new(document);
    }
    let controller = Controller::open(root, secret, adapters)?;
    match args[0].as_str() {
        #[cfg(unix)]
        "serve" => {
            crate::resident::Server::bind(&controller, PathBuf::from(need("--socket")?))?
                .run(&std::sync::atomic::AtomicBool::new(false))?;
            Ok(json!({"stopped":true}))
        }
        "submit" => {
            let bundle: Bundle = serde_json::from_value(json_file(&need("--input")?)?)
                .map_err(|_| "malformed: bundle")?;
            controller.submit(&bundle, crate::now())
        }
        "status" => controller.status(&need("--session")?),
        "device-operation" => controller.device_operation(),
        "input-policy" => {
            let grants = serde_json::from_value(json_file(&need("--input")?)?)
                .map_err(|_| "malformed: input grants")?;
            Ok(json!({"rules":controller.set_input_grants(grants,crate::now())?}))
        }
        _ => Err(format!("malformed: {USAGE}")),
    }
}
