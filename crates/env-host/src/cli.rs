//! Local allocator commands. Signed inputs use the same API as remote calls.
use crate::{
    Result,
    allocator::{Allocator, Bundle, MaterializeSpec, Policy},
    local::LocalDirectories,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: env-host ACTION --state DIR --key-file KEY [options]
  init         --policy POLICY.json      create allocator state
  submit       --provider-root DIR --input BUNDLE   admit a signed request, control,
               attach, participant admission, attachment control, or fencing
  materialize  --provider-root DIR --lease ID --input SPEC
  reconcile    --provider-root DIR --lease ID
  cleanup      --provider-root DIR --lease ID
  expire       --provider-root DIR
  status       --provider-root DIR --lease ID
  leases       --provider-root DIR
  check        --provider-root DIR --attachment ID --fingerprint DIGEST
  run          --provider-root DIR --coordinator-state DIR --coordinator-key-file KEY
               --worker-state DIR --worker-key-file KEY --input BUNDLE
  bootstrap-binding --provider-root DIR --input CONFIG
  transport    --provider-root DIR       show the exact controller endpoint
  invoke       --input REQUEST          sign with the caller key; no state needed
  serve|recover --provider-root DIR --input BUNDLE";

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
    let option = |name: &str| -> Result<String> {
        let index = args
            .iter()
            .position(|a| a == name)
            .ok_or_else(|| format!("malformed: missing {name}"))?;
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("malformed: missing value for {name}"))
    };
    let action = args[0].as_str();
    let secret = read_secret(Path::new(&option("--key-file")?))?;
    if action == "invoke" {
        let input = json_file(&option("--input")?)?;
        return serde_json::to_value(crate::transport::invoke(
            &secret,
            input["allocator"].as_str().ok_or("allocator")?,
            &input["binding"],
            input["body"].clone(),
            input["request"].as_str().ok_or("request")?.into(),
            crate::now(),
        )?)
        .map_err(|e| e.to_string());
    }
    let root = PathBuf::from(option("--state")?);
    if action == "init" {
        let policy: Policy = serde_json::from_value(json_file(&option("--policy")?)?)
            .map_err(|_| "malformed: policy")?;
        Allocator::initialize(root, secret, policy)?;
        return Ok(json!({"created":true}));
    }
    let provider = LocalDirectories::new(PathBuf::from(option("--provider-root")?))?;
    let mut allocator = Allocator::open(root, secret, Box::new(provider))?;
    if args.iter().any(|a| a == "--coordinator-state") {
        allocator = allocator.with_claims(Box::new(coordination_host::book::Coordinator::open(
            PathBuf::from(option("--coordinator-state")?),
            read_secret(Path::new(&option("--coordinator-key-file")?))?,
        )?));
    }
    let now = crate::now();
    match action {
        "run" => {
            let bundle: Bundle = serde_json::from_value(json_file(&option("--input")?)?)
                .map_err(|_| "malformed: worker bundle")?;
            let coordinator = coordination_host::book::Coordinator::open(
                PathBuf::from(option("--coordinator-state")?),
                read_secret(Path::new(&option("--coordinator-key-file")?))?,
            )?;
            let worker = coordination_host::worker::Worker::open(
                PathBuf::from(option("--worker-state")?),
                read_secret(Path::new(&option("--worker-key-file")?))?,
                now,
            )?;
            let (event, artifacts) =
                worker.run_in_environment(&coordinator, &bundle.event, now, &allocator)?;
            Ok(json!({"event":event,"artifacts":artifacts}))
        }
        "bootstrap-binding" => {
            let input = json_file(&option("--input")?)?;
            let admission: nostr::domain::Event =
                serde_json::from_value(input["admission"].clone())
                    .map_err(|_| "bootstrap allowance")?;
            allocator.bootstrap_binding(
                &input["claim"],
                &admission,
                input["request"].as_str().ok_or("bootstrap request")?,
            )
        }
        "transport" => allocator.transport(),
        "serve" | "recover" => {
            let bundle: Bundle = serde_json::from_value(json_file(&option("--input")?)?)
                .map_err(|_| "malformed: bundle")?;
            let (event, artifacts) = if action == "recover" {
                allocator.recover(&bundle.event, now)?
            } else {
                allocator.serve(&bundle.event, &bundle.artifacts, &bundle.declarations, now)?
            };
            Ok(json!({"event":event,"artifacts":artifacts}))
        }
        "submit" => {
            let bundle: Bundle = serde_json::from_value(json_file(&option("--input")?)?)
                .map_err(|_| "malformed: bundle")?;
            allocator.submit(&bundle, now)
        }
        "materialize" => {
            let spec: MaterializeSpec = serde_json::from_value(json_file(&option("--input")?)?)
                .map_err(|_| "malformed: materialization spec")?;
            allocator.materialize(&option("--lease")?, &spec, now)
        }
        "reconcile" => allocator.reconcile(&option("--lease")?, now),
        "cleanup" => allocator.cleanup(&option("--lease")?, now),
        "expire" => Ok(json!({"draining":allocator.expire(now)?})),
        "status" => allocator.status(&option("--lease")?),
        "leases" => Ok(json!({"leases":allocator.leases()?})),
        "check" => {
            allocator.check_dispatch(&option("--attachment")?, &option("--fingerprint")?, now)?;
            Ok(json!({"dispatch":"admitted"}))
        }
        _ => Err(format!("malformed: {USAGE}")),
    }
}
