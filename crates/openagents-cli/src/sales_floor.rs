//! The owner's private floor report, escalations, and weekly draft. Reads only.
use super::{Store, required};
use crate::{Args, Output};
use serde_json::Value;
use std::path::Path;

const USAGE: &str = "usage: openagents sales floor COMMAND --root DIR --credential FILE [--json]
  report        Recompute stage, assignment, message, delivery, meeting, certification, and cost counts.
  escalations   List open complaints, pauses, suspensions, authentication, budget, and adversary findings.
  weekly-draft  Paul's weekly aggregate draft for owner review; it publishes nothing.
Unknown values are labeled unknown. These commands read the owner's private records only.";

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
        Err(e) => return output.usage("sales floor", &e, USAGE),
    };
    match execute(&args) {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales floor", &e),
    }
}

fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1
        || !matches!(
            args.positional()[0].as_str(),
            "report" | "escalations" | "weekly-draft"
        )
    {
        return Err(USAGE.into());
    }
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential"))
    {
        return Err("unknown sales floor option".into());
    }
    required(&args, "root")?;
    required(&args, "credential")?;
    Ok(args)
}

fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let access = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    let value = match args.positional()[0].as_str() {
        "report" => serde_json::to_value(store.floor_report(&access)?),
        "escalations" => serde_json::to_value(store.floor_escalations(&access)?),
        _ => serde_json::to_value(store.floor_weekly_draft(&access)?),
    };
    value.map_err(|_| "floor report serialization failed".to_string())
}
