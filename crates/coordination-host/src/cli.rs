//! Local administration and signed event transport for the native coordinator.
use crate::{
    Result,
    book::{Coordinator, Policy},
};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
pub const USAGE: &str = "usage: coordination-host ACTION --state DIR --key-file FILE [--input FILE] [--sources FILE]\nActions: init, scope, inspect, put, operation, capture, sign, invoke, serve, recover, grant, frame, propose, claim, control, prepare, run, check, finding, integrate, background, poll.\nSource mounts use an owner-selected JSON map of names to existing workspace store directories.";
pub fn display(value: &Value) -> String {
    if let Some(help) = value["help"].as_str() {
        return help.into();
    }
    if value["error"].is_string() {
        return "The task request could not be completed.".into();
    }
    match value["status"].as_str() {
        Some("conflict") => "The task changed. Read it again before continuing.".into(),
        Some("refused") => "The task request could not be completed.".into(),
        Some("unknown") => "The result is not known. Check the task before retrying.".into(),
        _ => "Task request completed.".into(),
    }
}
fn key(path: &Path) -> Result<secp256k1::SecretKey> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 256 {
        return Err("invalid key file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("key file must be readable only by its owner".into());
        }
    }
    std::fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "invalid key file".into())
}
fn read(path: &str) -> Result<Value> {
    let bytes = workspace_host::artifact::read(Path::new(path), 12 * 1_048_576)?;
    contracts::parse_strict_bounded(&bytes, 12 * 1_048_576).map_err(|e| e.to_string())
}
pub fn run(args: Vec<String>) -> Result<Value> {
    if args.is_empty() || args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        return Ok(json!({"help":USAGE}));
    }
    let option = |name: &str| -> Result<String> {
        let i = args
            .iter()
            .position(|a| a == name)
            .ok_or_else(|| format!("missing {name}"))?;
        args.get(i + 1)
            .cloned()
            .ok_or_else(|| format!("missing value for {name}"))
    };
    let secret = key(Path::new(&option("--key-file")?))?;
    let root = PathBuf::from(option("--state")?);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    if args[0] == "init" {
        Coordinator::initialize(root, secret)?;
        return Ok(json!({"created":true}));
    }
    let mut host = Coordinator::open(root, secret)?;
    if args.iter().any(|a| a == "--sources") {
        let mounts: BTreeMap<String, PathBuf> =
            serde_json::from_value(read(&option("--sources")?)?).map_err(|_| "source mount map")?;
        if mounts.len() > 64 {
            return Err("too many source mounts".into());
        }
        for (name, path) in mounts {
            host = host.mount_source(name, workspace_host::host::Host::open(path, secret)?)?;
        }
    }
    let value = read(&option("--input")?)?;
    match args[0].as_str() {
        "scope" => {
            let policy: Policy = serde_json::from_value(value["policy"].clone())
                .map_err(|_| "native scope policy")?;
            let mut resources = Vec::new();
            for r in value["resources"]
                .as_array()
                .ok_or("source resource declarations")?
            {
                resources.push(host.resource(
                    r["id"].as_str().ok_or("resource name")?.into(),
                    r["source"].as_str().ok_or("source mount")?.into(),
                    r["reference"].clone(),
                    now,
                )?);
            }
            host.create_scope(
                policy,
                resources,
                value["targets"]
                    .as_array()
                    .ok_or("registered targets")?
                    .clone(),
                now,
            )
        }
        "inspect" => host.scope(&value),
        "put" => {
            host.attachments(
                &serde_json::from_value(
                    value
                        .get("attachments")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                )
                .map_err(|_| "artifact attachments")?,
            )?;
            host.put_local_json(&value["body"])
        }
        "operation" => match value["binding"].as_str() {
            Some("data") => host.data_operation(),
            Some("automation-data") => host.automation_operation(),
            Some("container") => host.container_operation(value["effects"].clone()),
            Some("git-change") => host.git_operation(value["resource"].as_str().ok_or("resource")?),
            _ => Err("unsupported native operation".into()),
        },
        "capture" => {
            let resource = host.resource(
                value["id"].as_str().ok_or("resource")?.into(),
                value["source"].as_str().ok_or("source mount")?.into(),
                value["reference"].clone(),
                now,
            )?;
            host.capture_resource(&resource, now)
        }
        "sign" => {
            let participant = key(Path::new(&option("--participant-key")?))?;
            Ok(
                json!({"event":workspace_host::host::request(&participant,host.authority(),value,now)?}),
            )
        }
        "invoke" => {
            let participant = key(Path::new(&option("--participant-key")?))?;
            Ok(
                json!({"event":crate::transport::invoke(&participant,host.authority(),&value["transport"],value["body"].clone(),value["request"].as_str().ok_or("request ID")?.into(),now)?}),
            )
        }
        "check" => {
            let checker = crate::checks::Checker::new(key(Path::new(&option("--checker-key")?))?);
            let event = checker.check(&host, &value["coordinator"], &value["claim"], now)?;
            host.record_check(&event, now)
        }
        action => {
            let event: Event = serde_json::from_value(value["event"].clone())
                .map_err(|_| "signed event required")?;
            let files: BTreeMap<String, Vec<u8>> = serde_json::from_value(
                value
                    .get("attachments")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            )
            .map_err(|_| "exact artifact attachments")?;
            match action {
                "recover" => {
                    let (event, files) = host.recover_controller(&event, now)?;
                    Ok(json!({"event":event,"attachments":files}))
                }
                "serve" => {
                    let (event, files) = host.serve(&event, &files, now)?;
                    Ok(json!({"event":event,"attachments":files}))
                }
                "grant" => host.control_grants(&event, now),
                "frame" => host.frame(
                    &event,
                    &contracts::jcs(&value["frame"]).map_err(|e| e.to_string())?,
                    now,
                ),
                "propose" => host.propose(&event, &files, now),
                "claim" => host.claim(&event, now),
                "control" => host.control(&event, now),
                "prepare" => Ok(json!({"event":host.prepare(&event,now)?})),
                "run" => {
                    let worker = crate::worker::Worker::open(
                        PathBuf::from(option("--worker-state")?),
                        key(Path::new(&option("--worker-key")?))?,
                        now,
                    )?;
                    let (result, files) = worker.run(&host, &event, now)?;
                    host.record_result(&result, &files, now)
                }
                "finding" => host.finding(&event, &files, now),
                "integrate" => host.integrate(&event, now, None),
                "background" => host.activate_background(&event, now),
                "poll" => host.poll_background(&event, now),
                _ => Err("unknown task command".into()),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn help_needs_no_account_or_key() {
        assert!(
            super::run(vec!["--help".into()]).unwrap()["help"]
                .as_str()
                .unwrap()
                .contains("coordination")
        );
    }
}
