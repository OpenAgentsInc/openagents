//! Local owner admission and assigned-only native sales records.
use super::{Store, required};
use crate::{Args, Output};
use serde_json::Value;
use std::io::Read;
use std::path::Path;
const USAGE: &str = "usage: openagents sales agents COMMAND --root DIR --credential FILE [--json]
  anchor --agent NAME            Read current native key, role, and charter pins (owner).
  owner                         Read versioned policy and manual certificate references (owner).
  owner-apply --input FILE [--new-credential FILE]
                                Apply an owner command; assignments require a new credential.
  policy-check --input FILE      Check and digest an explicit policy without recording it.
  read                          Read the current credential's assigned lead fields.
  apply --input FILE             Update only granted fields or propose a private draft.
  memory                        Read opaque references and fixed nonidentifying summary fields.
Owner commands require the current human owner's credential. read, apply, and memory
require an owner-issued assigned-agent credential; human tokens cannot substitute.
FILE=- reads bounded JSON from stdin. File inputs must be private regular files.
Manual certificates are owner-recorded references, without measured qualification.
These operations grant no sending, execution, payment, publication, or cross-lead access.";
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|v| matches!(v.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(v) => v,
        Err(e) => return output.usage("sales agents", &e, USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(e) => output.fail("sales agents", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "anchor" => &["agent"],
        "owner" | "read" | "memory" => &[],
        "owner-apply" => &["input", "new-credential"],
        "apply" | "policy-check" => &["input"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !matches!(*name, "root" | "credential") && !extra.contains(name))
    {
        return Err("unknown sales agents option".into());
    }
    required(&args, "root")?;
    required(&args, "credential")?;
    if extra.contains(&"input") {
        required(&args, "input")?;
    }
    if extra.contains(&"agent") {
        required(&args, "agent")?;
    }
    Ok(args)
}
pub(super) fn input(args: &Args) -> Result<Vec<u8>, String> {
    input_bound(args, 32 * 1024)
}
pub(super) fn input_bound(args: &Args, limit: u64) -> Result<Vec<u8>, String> {
    let path = required(args, "input")?;
    let mut bytes = Vec::new();
    if path == "-" {
        std::io::stdin()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    } else {
        let path = Path::new(path);
        let meta =
            std::fs::symlink_metadata(path).map_err(|_| "sales agent input is unavailable")?;
        if !meta.is_file() || meta.len() > limit {
            return Err("sales agent input must be a bounded private regular file".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            if meta.mode() & 0o077 != 0 || meta.nlink() != 1 {
                return Err("sales agent input must be private and unshared".into());
            }
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        private_fs::nofollow(&mut options);
        let file = options
            .open(path)
            .map_err(|_| "sales agent input is unavailable")?;
        let held = file
            .metadata()
            .map_err(|_| "sales agent input is unavailable")?;
        if !held.is_file() || held.len() > limit {
            return Err("sales agent input must be a bounded private regular file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.dev() != held.dev()
                || meta.ino() != held.ino()
                || held.mode() & 0o077 != 0
                || held.nlink() != 1
            {
                return Err("sales agent input custody changed".into());
            }
        }
        #[cfg(windows)]
        if !private_fs::is_private(&file).map_err(|e| e.to_string())? {
            return Err("sales agent input must be private".into());
        }
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    }
    if bytes.len() as u64 > limit {
        return Err("sales agent input exceeds 32 KiB".into());
    }
    Ok(bytes)
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let secret = Store::read_credential(Path::new(required(args, "credential")?))?;
    match args.positional()[0].as_str() {
        "read" | "memory" | "apply" => {
            let access = store.authenticate_sales_agent(&secret)?;
            match args.positional()[0].as_str() {
                "read" => serde_json::to_value(store.read_sales_agent(&access)?),
                "memory" => serde_json::to_value(store.sales_agent_memory(&access)?),
                _ => serde_json::to_value(store.apply_sales_agent(&access, &input(args)?)?),
            }
            .map_err(|e| e.to_string())
        }
        _ => {
            let access = store.authenticate(&secret)?;
            match args.positional()[0].as_str() {
                "anchor" => serde_json::to_value(
                    store.sales_agent_anchor(&access, required(args, "agent")?)?,
                ),
                "owner" => serde_json::to_value(store.sales_agent_owner_view(&access)?),
                "policy-check" => {
                    // Owner authentication is rechecked by the bounded owner read.
                    store.sales_agent_owner_view(&access)?;
                    let policy: coder::task::sales::agents::Policy =
                        serde_json::from_slice(&input(args)?)
                            .map_err(|_| "malformed sales policy")?;
                    Ok(serde_json::json!({"sha256": policy.sha256()?, "outbound_authority":false}))
                }
                _ => serde_json::to_value(store.apply_sales_agent_owner(
                    &access,
                    &input(args)?,
                    args.option("new-credential").map(Path::new),
                )?),
            }
            .map_err(|e| e.to_string())
        }
    }
}
