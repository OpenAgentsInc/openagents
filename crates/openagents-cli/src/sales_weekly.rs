//! Explicit owner operations over private sources. This publishes nothing.
use crate::{Args, Output};
use coder::task::sales::Store;
use gym::sales_weekly as weekly;
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;

fn private_input(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    private_fs::nofollow(&mut options);
    let file = options
        .open(path)
        .map_err(|_| "private review input is unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "private review input metadata is unavailable")?;
    #[cfg(unix)]
    let private = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o077 == 0
    };
    #[cfg(windows)]
    let private = private_fs::is_private(&file).unwrap_or(false);
    if !meta.is_file() || !private {
        return Err("review input must be a private regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private review input read failed")?;
    if bytes.len() > maximum {
        return Err("private review input exceeds bound".into());
    }
    Ok(bytes)
}
pub fn run(output: &Output, words: &[String]) -> u8 {
    let args = match parse(words) {
        Ok(args) => args,
        Err(message) => return output.usage("sales", &message, super::sales::USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| serde_json::to_string(v).unwrap_or_default());
            0
        }
        Err(message) => output.fail("sales", &message),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 || !matches!(args.positional()[0].as_str(), "weekly" | "review")
    {
        return Err(super::sales::USAGE.into());
    }
    let reviewed = args.positional()[0] == "review";
    let allowed = if reviewed {
        vec![
            "root",
            "credential",
            "input",
            "evidence-root",
            "output",
            "report",
            "review",
        ]
    } else {
        vec!["root", "credential", "input", "evidence-root", "output"]
    };
    if args
        .option_names()
        .iter()
        .any(|name| !allowed.contains(name))
    {
        return Err("unknown weekly review option".into());
    }
    for flag in &allowed {
        if args.option(flag).is_none_or(str::is_empty) {
            return Err(format!("--{flag} is required"));
        }
    }
    Ok(args)
}
fn execute(args: &Args) -> Result<Value, String> {
    let reviewed = args.positional()[0] == "review";
    let path = |flag: &str| Path::new(args.option(flag).unwrap());
    let mut store = Store::open(path("root"))?;
    let access = store.authenticate(&Store::read_credential(path("credential"))?)?;
    store.authorize_funnel_snapshots(&access, &[])?;
    let now = coder::task::sales::unix_now();
    let checked = weekly::rebuild(
        path("evidence-root"),
        &private_input(path("input"), 1024 * 1024)?,
        now,
    )?;
    if checked.manifest.owner != access.principal() {
        return Err("weekly review owner differs from the authenticated human".into());
    }
    let bytes = if reviewed {
        let report = private_input(path("report"), 8 * 1024 * 1024)?;
        let retained: Value =
            serde_json::from_slice(&report).map_err(|_| "malformed private weekly report")?;
        if retained != serde_json::to_value(&checked).map_err(|_| "weekly serialization failed")? {
            return Err("reviewed report differs from the current rechecked weekly sources".into());
        }
        let review: weekly::Review =
            serde_json::from_slice(&private_input(path("review"), 32 * 1024)?)
                .map_err(|_| "malformed exact owner review")?;
        serde_json::to_vec_pretty(&weekly::project(&report, &review, now)?)
            .map_err(|_| "weekly aggregate serialization failed")?
    } else {
        serde_json::to_vec_pretty(&checked).map_err(|_| "weekly report serialization failed")?
    };
    let digest = store.funnel_write(&access, &checked.sources, path("output"), &bytes)?;
    Ok(json!({"sha256":digest,"reviewed_aggregate":reviewed,"published":false}))
}
