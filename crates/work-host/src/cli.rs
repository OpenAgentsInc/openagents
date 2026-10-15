//! Local work authority commands. JSON inputs use the same authenticated API.
use crate::{Result, authority::Authority};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
pub fn run(args: Vec<String>) -> Result<Value> {
    run_configured(args, |_, _, _| Ok(None), None)
}
pub fn run_configured(
    mut args: Vec<String>,
    configure: impl FnOnce(
        &Authority,
        &secp256k1::SecretKey,
        u64,
    ) -> Result<Option<Box<dyn crate::market::Market>>>,
    github: Option<(Value, u64)>,
) -> Result<Value> {
    if args.first().is_some_and(|s| s == "issue") {
        args.remove(0);
    }
    if args.is_empty() || args.iter().any(|s| matches!(s.as_str(), "--help" | "-h")) {
        return Ok(json!({"help":USAGE}));
    }

    let action = args
        .first()
        .ok_or("usage: work ACTION --state DIR --key-file FILE [--input FILE]")?;
    let option = |name: &str| -> Result<String> {
        let index = args
            .iter()
            .position(|a| a == name)
            .ok_or_else(|| format!("missing {name}"))?;
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("missing value for {name}"))
    };
    let root = PathBuf::from(option("--state")?);
    let keyfile = PathBuf::from(option("--key-file")?);
    let secret = read_secret(&keyfile)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    if action == "init" {
        Authority::initialize(root, secret)?;
        return Ok(json!({"created":true}));
    }
    let mut host = Authority::open(root, secret)?;
    if let Some(market) = configure(&host, &secret, now)? {
        host = host.with_market(market);
    }

    if action == "scope" {
        let verifiers = if args.iter().any(|a| a == "--verifiers") {
            serde_json::from_slice(
                &std::fs::read(option("--verifiers")?).map_err(|e| e.to_string())?,
            )
            .map_err(|_| "verifier keys must be a JSON array")?
        } else {
            BTreeSet::new()
        };
        return host.create_scope(verifiers, now);
    }
    use std::io::Read;
    let mut input = Vec::new();
    std::fs::File::open(option("--input")?)
        .map_err(|e| e.to_string())?
        .take(12 * 1_048_576 + 1)
        .read_to_end(&mut input)
        .map_err(|e| e.to_string())?;
    if input.len() > 12 * 1_048_576 {
        return Err("work input exceeds its bound".into());
    }
    let value =
        contracts::parse_strict_bounded(&input, 12 * 1_048_576).map_err(|e| e.to_string())?;
    if action == "import-plan" {
        let (capture, observed) =
            github.ok_or("GitHub import needs a validated tracker snapshot")?;
        let reason = value["resolution"].as_str().map(|s| s.as_bytes().to_vec());
        return host.plan_github(
            &value["scope"],
            capture,
            value["snapshot"].clone(),
            value.get("expected").filter(|v| !v.is_null()).cloned(),
            reason,
            observed,
            now,
        );
    }

    if action == "plan" {
        let snapshot = host.store.put_json(&value["snapshot"])?;
        let command = json!({"v":"openagents.work-command.v1","requires":[],"issuer":host.signer.pubkey(),"command_id":workspace_host::artifact::token(),"scope":value["scope"],"work":value["work"],"action":if value["expected"].is_null(){"create"}else{"replace"},"expected":value["expected"],"snapshot":snapshot,"source":value["source"],"reason":value["reason"],"expires_at":now+300});
        nostr::work::validate(&command).map_err(|e| e.to_string())?;
        let event = workspace_host::host::request(&secret, host.signer.pubkey(), command, now)?;
        return Ok(
            json!({"event":event,"attachments":value.get("attachments").cloned().unwrap_or_else(||json!({}))}),
        );
    }
    if action == "verify-job" {
        let verifier = read_secret(&PathBuf::from(option("--verifier-key")?))?;
        let (event, files) = crate::checks::verify_job(
            &host,
            &verifier,
            value["job"].as_str().ok_or("job identity")?,
            now,
        )?;
        return host.submit_check(&event, &files, now);
    }
    let event: Event =
        serde_json::from_value(value["event"].clone()).map_err(|_| "signed event is required")?;
    match action.as_str() {
        "run-job" => {
            let worker = crate::execution::PureWorker::open(
                PathBuf::from(option("--worker-state")?),
                read_secret(&PathBuf::from(option("--worker-key")?))?,
                now,
            )?;
            let (result, files) = worker.run(&host, &event, now)?;
            host.record_worker_result(&result, &files, now)
        }
        "apply" => {
            let files: BTreeMap<String, Vec<u8>> =
                serde_json::from_value(value["attachments"].clone())
                    .map_err(|_| "attachment bytes are required")?;
            host.command(&event, &files, now)
        }
        "declare" => host.submit_declaration(&event, now),
        "check" => {
            let files = serde_json::from_value(value["attachments"].clone())
                .map_err(|_| "check attachments")?;
            host.submit_check(&event, &files, now)
        }
        "control" => host.control(&event, now),
        "history" => host.read_history(&event, now),
        "grant-execution" => host.execution_grant(&event, now),
        "prepare-job" => host.prepare_job(&event, now),
        "view" => host.define_view(&event, now),
        "page" => host.projection_read(&event, now),
        "attention" => host.attention(&event, now),
        _ => Err("unknown work command".into()),
    }
}

pub const USAGE: &str =
    "usage: openagents work [issue] ACTION --state DIR --key-file FILE [--input FILE]
  init             Create a private native work store.
  scope            Create a scope; --verifiers FILE lists admitted checker keys.
  plan             Prepare a conditional command from a proposed snapshot.
  apply            Save an exact signed conditional work command.
  declare          Retain signed evidence, delegation, or disposition.
  control          Change scope grants with a signed owner request.
  history          Read an admitted item's signed history.
  grant-execution  Admit a separate bounded pure execution grant.
  prepare-job      Prepare a frozen CJ request without dispatching it.
  run-job          Run the pure worker with --worker-key and --worker-state.
  verify-job       Run the independent checker with --verifier-key FILE.
  check            Retain a separate verifier's signed report.
  view / page      Create and read recipient-scoped WS views.
  attention        Read or dismiss your attention records.
  import-plan      Propose a copy from --tracker FILE --node ID.
  --labor-state DIR --labor-setup FILE mounts an existing admitted LAB book.
Input bundles contain event and attachments (digest-to-byte-array JSON).";
pub fn display(value: &Value) -> String {
    if let Some(help) = value["help"].as_str() {
        return help.into();
    }
    if value.get("error").is_some() {
        return "The work request could not be completed.".into();
    }
    match value["status"].as_str() {
        Some("accepted" | "duplicate") => "Saved work item.".into(),
        Some("conflict") => "The work item changed. Read it again before saving.".into(),
        Some("unknown") => "The result is not known. Check its history before retrying.".into(),
        Some("refused") => "The work request could not be completed.".into(),
        _ if value["created"] == true => "Created work store.".into(),
        _ if value["records"].is_array() => "Read work history.".into(),
        _ if value.get("request").is_some() || value.get("event").is_some() => {
            "Prepared work request.".into()
        }
        _ => "Work request completed.".into(),
    }
}

fn read_secret(path: &std::path::Path) -> Result<secp256k1::SecretKey> {
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
