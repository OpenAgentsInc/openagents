//! Standalone local operations. Keys and store mounts are operator-selected.
use crate::{
    Result,
    host::{Automation, Policy},
};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
pub const USAGE: &str = "usage: automation-host ACTION --state DIR --key-file FILE --coordinator-state DIR --coordinator-key-file FILE [--input FILE] [--json]\nActions: init, policy, put, inspect, admit, tick, advance, run, result, check, control, sign, transport, invoke, serve, recover, source-init, source-binding, evaluator-init, evaluator-binding, evaluate.\nWorker actions need --worker-state and --worker-key-file. Checking needs --checker-key-file. Source watches also need --sources, --source-state, and --source-key-file. Source/evaluator initialization and binding commands use their own state/key. Continuation evaluation also needs --evaluation-state and --checker-key-file.";
fn key(path: &Path) -> Result<secp256k1::SecretKey> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 256 {
        return Err("invalid key file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("key file must be readable only by its owner".into());
        }
    }
    std::fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "invalid key file".into())
}
fn read(path: &Path) -> Result<Value> {
    contracts::parse_strict_bounded(
        &workspace_host::artifact::read(path, 12 * 1048576)?,
        12 * 1048576,
    )
    .map_err(|e| e.to_string())
}
fn files(value: &Value) -> Result<BTreeMap<String, Vec<u8>>> {
    let Some(map) = value.get("artifacts") else {
        return Ok(BTreeMap::new());
    };
    let mut result = BTreeMap::new();
    for (digest, body) in map.as_object().ok_or("artifact map")? {
        let bytes = contracts::jcs(body).map_err(|e| e.to_string())?;
        if contracts::digest_bytes(&bytes) != *digest {
            return Err("artifact bytes differ".into());
        }
        result.insert(digest.clone(), bytes);
    }
    Ok(result)
}
pub fn run(args: Vec<String>) -> Result<Value> {
    if args.is_empty() || args.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        return Ok(json!({"help":USAGE}));
    }
    let option = |name: &str| -> Result<String> {
        let i = args
            .iter()
            .position(|s| s == name)
            .ok_or_else(|| format!("missing {name}"))?;
        args.get(i + 1)
            .cloned()
            .ok_or_else(|| format!("missing value for {name}"))
    };
    let secret = key(Path::new(&option("--key-file")?))?;
    let root = PathBuf::from(option("--state")?);
    if args[0] == "evaluator-init" {
        let policy = serde_json::from_value(read(Path::new(&option("--input")?))?)
            .map_err(|_| "evaluation policy")?;
        let evaluator = crate::evaluator::Evaluator::initialize(root, secret, policy)?;
        return Ok(json!({"created":true,"binding":evaluator.binding()?}));
    }
    if args[0] == "evaluator-binding" {
        let evaluator = crate::evaluator::Evaluator::open(root, secret)?;
        let artifacts = evaluator
            .binding_files()?
            .into_iter()
            .map(|(d, b)| Ok((d, contracts::parse_strict(&b).map_err(|e| e.to_string())?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        return Ok(json!({"binding":evaluator.binding()?,"artifacts":artifacts}));
    }
    if args[0] == "source-init" {
        let policy = serde_json::from_value(read(Path::new(&option("--input")?))?)
            .map_err(|_| "source policy")?;
        let source = crate::source::Source::initialize(root, secret, policy)?;
        return Ok(json!({"created":true,"binding":source.binding()?}));
    }
    if args[0] == "source-binding" {
        let source = crate::source::Source::open(root, secret)?;
        let artifacts = source
            .binding_files()?
            .into_iter()
            .map(|(d, b)| Ok((d, contracts::parse_strict(&b).map_err(|e| e.to_string())?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        return Ok(json!({"binding":source.binding()?,"artifacts":artifacts}));
    }
    let coordinator_key = key(Path::new(&option("--coordinator-key-file")?))?;
    let mut coordinator = coordination_host::book::Coordinator::open(
        PathBuf::from(option("--coordinator-state")?),
        coordinator_key,
    )?;
    if args.iter().any(|a| a == "--sources") {
        let mounts: BTreeMap<String, PathBuf> =
            serde_json::from_value(read(Path::new(&option("--sources")?))?)
                .map_err(|_| "workspace mount map")?;
        if mounts.len() > 64 {
            return Err("workspace mount count".into());
        }
        for (name, path) in mounts {
            coordinator = coordinator.mount_source(
                name,
                workspace_host::host::Host::open(path, coordinator_key)?,
            )?;
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    if args[0] == "transport" {
        return Automation::open(root, secret, coordinator)?.transport();
    }
    if args[0] == "policy" {
        return Automation::open(root, secret, coordinator)?.policy();
    }
    let input = read(Path::new(&option("--input")?))?;
    if args[0] == "init" {
        let policy: Policy = serde_json::from_value(input).map_err(|_| "automation policy")?;
        let host = Automation::initialize(root, secret, policy, coordinator)?;
        return Ok(json!({"created":true,"policy":host.policy()?}));
    }
    if args[0] == "sign" {
        let recipient = input["recipient"].as_str().ok_or("recipient")?;
        return serde_json::to_value(workspace_host::host::request(
            &secret,
            recipient,
            input["body"].clone(),
            now,
        )?)
        .map_err(|e| e.to_string());
    }
    let mut host = Automation::open(root, secret, coordinator)?;
    if args.iter().any(|a| a == "--source-state") {
        host = host.mount_source(crate::source::Source::open(
            PathBuf::from(option("--source-state")?),
            key(Path::new(&option("--source-key-file")?))?,
        )?)?;
    }
    if args.iter().any(|a| a == "--evaluation-state") {
        host = host.mount_evaluator(crate::evaluator::Evaluator::open(
            PathBuf::from(option("--evaluation-state")?),
            key(Path::new(&option("--checker-key-file")?))?,
        )?)?;
    }
    let reference = &input["plan"];
    match args[0].as_str() {
        "put" => host.put_local_json(&input),
        "invoke" => serde_json::to_value(coordination_host::transport::invoke(
            &key(Path::new(&option("--participant-key-file")?))?,
            host.authority(),
            &host.transport()?,
            input["body"].clone(),
            input["request"].as_str().ok_or("request")?.into(),
            now,
        )?)
        .map_err(|e| e.to_string()),
        "serve" | "recover" => {
            let event: Event =
                serde_json::from_value(input["event"].clone()).map_err(|_| "controller request")?;
            let (event, artifacts) = if args[0] == "recover" {
                host.recover_controller(&event, now)?
            } else {
                host.serve(&event, &files(&input)?, now)?
            };
            let artifacts = artifacts
                .into_iter()
                .map(|(d, b)| Ok((d, contracts::parse_strict(&b).map_err(|e| e.to_string())?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            Ok(json!({"event":event,"artifacts":artifacts}))
        }
        "inspect" => host.inspect(reference),
        "admit" => host.admit(
            &serde_json::from_value::<Event>(input["event"].clone()).map_err(|_| "owner event")?,
            &files(&input)?,
            now,
        ),
        "tick" => host.tick(reference, now),
        "advance" => Ok(json!({"event":host.advance(reference,now)?})),
        "control" => host.control(
            &serde_json::from_value::<Event>(input["event"].clone())
                .map_err(|_| "owner control")?,
            now,
        ),
        "result" => host.record_result(
            reference,
            &serde_json::from_value::<Event>(input["event"].clone())
                .map_err(|_| "worker result")?,
            &files(&input)?,
            now,
        ),
        "run" => {
            let worker = coordination_host::worker::Worker::open(
                PathBuf::from(option("--worker-state")?),
                key(Path::new(&option("--worker-key-file")?))?,
                now,
            )?;
            let (event, artifacts) = worker.run_gated(
                &host.coordinator,
                &serde_json::from_value::<Event>(input["event"].clone())
                    .map_err(|_| "worker request")?,
                now,
                &host,
            )?;
            let artifacts = artifacts
                .into_iter()
                .map(|(d, b)| Ok((d, contracts::parse_strict(&b).map_err(|e| e.to_string())?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            Ok(json!({"event":event,"artifacts":artifacts}))
        }
        "evaluate" => host.evaluate(reference, now),
        "check" => host.check(
            reference,
            &coordination_host::checks::Checker::new(key(Path::new(&option(
                "--checker-key-file",
            )?))?),
            now,
        ),
        _ => Err(USAGE.into()),
    }
}
pub fn display(value: &Value) -> String {
    if let Some(help) = value["help"].as_str() {
        return help.into();
    }
    if value["error"].is_string() {
        return "The scheduled task could not be updated.".into();
    }
    match value["status"].as_str() {
        Some("unknown") => "The result is not known. Check the task before retrying.".into(),
        _ => "Scheduled task request completed.".into(),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn help_requires_no_keys() {
        assert!(
            super::run(vec!["--help".into()]).unwrap()["help"]
                .as_str()
                .unwrap()
                .contains("automation-host")
        );
    }
}
