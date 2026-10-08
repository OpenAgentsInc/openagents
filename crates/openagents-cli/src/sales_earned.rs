//! Earned sales: the owner's ledger, the once-only bell, and the reviewed
//! shared aggregate. The shared read needs no credential and carries no
//! lead, amount, or timing beyond a day boundary and a USD 100 floor.
use super::{Store, required};
use crate::{Args, Output};
use serde_json::Value;
use std::path::Path;

const USAGE: &str = "usage: openagents sales earned COMMAND --root DIR [--credential FILE] [--json]
  ledger                   Every service sale with its settlement, delivery, net, and why it is or is not earned (owner).
  ring                     Ring the Agora bell once for each newly earned sale; replays return nothing (owner).
  draft --through UNIX     Paul's shared aggregate and weekly update through that day; needs a seven-day lag (owner).
  approve --through UNIX --digest SHA256 --expires-at UNIX
                           Approve that exact aggregate for shared surfaces (owner).
  revoke --digest SHA256   Withdraw an approval (owner).
  shared                   What a shared surface may show now; no credential.";

pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(v) => v,
        Err(e) => return output.usage("sales earned", &e, USAGE),
    };
    match execute(&args) {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales earned", &e),
    }
}

fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    let [command] = args.positional() else {
        return Err(USAGE.into());
    };
    if !matches!(
        command.as_str(),
        "ledger" | "ring" | "draft" | "approve" | "revoke" | "shared"
    ) {
        return Err(USAGE.into());
    }
    if args.option_names().iter().any(|n| {
        !matches!(
            *n,
            "root" | "credential" | "through" | "digest" | "expires-at"
        )
    }) {
        return Err("unknown sales earned option".into());
    }
    required(&args, "root")?;
    if command != "shared" {
        required(&args, "credential")?;
    }
    match command.as_str() {
        "draft" => {
            required(&args, "through")?;
        }
        "approve" => {
            required(&args, "through")?;
            required(&args, "digest")?;
            required(&args, "expires-at")?;
        }
        "revoke" => {
            required(&args, "digest")?;
        }
        _ => {}
    }
    Ok(args)
}

fn unix(args: &Args, name: &str) -> Result<u64, String> {
    required(args, name)?
        .parse()
        .map_err(|_| format!("--{name} is a Unix time in seconds"))
}

fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let command = args.positional()[0].as_str();
    if command == "shared" {
        return serde_json::to_value(store.shared_aggregate()?).map_err(|e| e.to_string());
    }
    let owner = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    let value = match command {
        "ledger" => serde_json::to_value(store.earned_ledger(&owner)?),
        "ring" => serde_json::to_value(store.ring_earned(&owner)?),
        "draft" => {
            serde_json::to_value(store.shared_aggregate_draft(&owner, unix(args, "through")?)?)
        }
        "approve" => serde_json::to_value(store.approve_shared_aggregate(
            &owner,
            unix(args, "through")?,
            required(args, "digest")?,
            unix(args, "expires-at")?,
        )?),
        "revoke" => serde_json::to_value(serde_json::json!({
            "revoked": store.revoke_shared_aggregate(&owner, required(args, "digest")?)?
        })),
        _ => return Err(USAGE.into()),
    };
    value.map_err(|e| e.to_string())
}
