//! Local session-host commands and a resident socket client. Offline commands
//! refuse while a resident owns the engines; a new process cannot adopt an
//! earlier instance's native sessions.
use crate::{
    Result,
    engine::LoopEngine,
    host::{Binding, Bindings, Bundle, Policy, SessionHost},
};
use env_host::{allocator::Allocator, local::LocalDirectories};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "usage: session-host ACTION --state DIR --key-file KEY [options]
  init       --policy POLICY.json
  policy     --policy POLICY.json   replace owner/controller sets with the host key
  admit      --engine loop-mid-turn|loop-boundary
  submit     --input BUNDLE [--env-state DIR --env-key KEY --env-root DIR
             --attachment ID --fingerprint DIGEST --grant GRANT.json
             --coordinator-state DIR --coordinator-key-file KEY]
  observe    --session ID
  status     --session ID
  export     --session ID
  serve      --socket PATH [the same ENV and COORD options as submit]
  request    --socket PATH --input REQUEST.json (no host key required)";

struct NoBindings;
impl Bindings for NoBindings {
    fn check(&self, _: &str, _: &str, _: u64) -> Result<()> {
        Err("not_admitted: no environment allocator is mounted".into())
    }
}

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
    let option = |name: &str| -> Option<String> {
        let index = args.iter().position(|a| a == name)?;
        args.get(index + 1).cloned()
    };
    let need = |name: &str| option(name).ok_or_else(|| format!("malformed: missing {name}"));
    #[cfg(unix)]
    if args[0] == "request" {
        let bytes = workspace_host::artifact::read(
            Path::new(&need("--input")?),
            crate::resident::MAX_FRAME,
        )?;
        let value = nostr::contracts::parse_strict_bounded(&bytes, crate::resident::MAX_FRAME)
            .map_err(|e| e.to_string())?;
        let request: crate::resident::Request =
            serde_json::from_value(value).map_err(|_| "malformed: transport request")?;
        return serde_json::to_value(crate::resident::request(
            Path::new(&need("--socket")?),
            &request,
        )?)
        .map_err(|e| e.to_string());
    }
    let root = PathBuf::from(need("--state")?);
    let secret = read_secret(Path::new(&need("--key-file")?))?;
    let now = crate::now();
    if args[0] == "init" {
        let policy: Policy = serde_json::from_value(json_file(&need("--policy")?)?)
            .map_err(|_| "malformed: policy")?;
        SessionHost::initialize(root, secret, policy)?;
        return Ok(json!({"created":true}));
    }
    let bindings: Box<dyn Bindings> = match option("--env-state") {
        Some(dir) => {
            let key = read_secret(Path::new(&need("--env-key")?))?;
            let provider = LocalDirectories::new(PathBuf::from(need("--env-root")?))?;
            let mut allocator = Allocator::open(PathBuf::from(dir), key, Box::new(provider))?;
            if let Some(root) = option("--coordinator-state") {
                allocator =
                    allocator.with_claims(Box::new(coordination_host::book::Coordinator::open(
                        PathBuf::from(root),
                        read_secret(Path::new(&need("--coordinator-key-file")?))?,
                    )?));
            }
            Box::new(allocator)
        }
        None => Box::new(NoBindings),
    };
    #[cfg(unix)]
    let _owner = if args[0] != "serve" {
        Some(crate::resident::claim(&root)?)
    } else {
        None
    };
    let host = SessionHost::open(
        root,
        secret,
        vec![
            Box::new(LoopEngine::mid_turn()),
            Box::new(LoopEngine::boundary()),
        ],
        bindings,
    )?;
    match args[0].as_str() {
        #[cfg(unix)]
        "serve" => {
            let mut server =
                crate::resident::Server::bind(&host, PathBuf::from(need("--socket")?))?;
            server.run(&std::sync::atomic::AtomicBool::new(false))?;
            Ok(json!({"stopped":true}))
        }
        "policy" => {
            let policy = serde_json::from_value(json_file(&need("--policy")?)?)
                .map_err(|_| "malformed: policy")?;
            host.set_policy(policy)?;
            Ok(json!({"updated":true}))
        }
        "admit" => host.admit_adapter(&need("--engine")?, now),
        "submit" => {
            let bundle: Bundle = serde_json::from_value(json_file(&need("--input")?)?)
                .map_err(|_| "malformed: bundle")?;
            let binding = match option("--attachment") {
                Some(attachment) => Some(Binding {
                    attachment,
                    fingerprint: need("--fingerprint")?,
                    grant: json_file(&need("--grant")?)?,
                }),
                None => None,
            };
            host.submit(&bundle, binding.as_ref(), now)
        }
        "observe" => host.observe(&need("--session")?, now),
        "status" => host.status(&need("--session")?),
        "export" => host.export(&need("--session")?, now),
        _ => Err(format!("malformed: {USAGE}")),
    }
}
