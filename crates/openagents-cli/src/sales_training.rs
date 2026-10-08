//! Private synthetic scheduling and a reproducible local fixture adapter.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::training::{self, Model};
use serde_json::{Value, json};
use std::path::Path;
const USAGE: &str = "usage: openagents sales training COMMAND --root DIR --credential FILE [--json]
  personas                             Read labeled fictional buyer situations.
  script-source                        Read the exact zero-cost fixture source.
  schedule --input FILE                Retain an immutable bounded practice schedule.
  run-scripted --run ID                 Run the original synthetic fixture once.
  show --run ID                        Read partial evidence and original cost references.
  list --agent NAME [--after ID] [--limit N]
                                       Read at most 64 practices through current Paul authority.
FILE=- reads bounded JSON; file inputs must be private and unshared.
Scripted completion establishes no model quality, certification, contact permission,
customer record, revenue, payment, or sending authority.";
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(a) => a,
        Err(e) => return output.usage("sales training", &e, USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(e) => output.fail("sales training", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "personas" | "script-source" => &[],
        "schedule" => &["input"],
        "run-scripted" | "show" => &["run"],
        "list" => &["agent", "after", "limit"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential") && !extra.contains(n))
    {
        return Err("unknown sales training option".into());
    }
    required(&args, "root")?;
    required(&args, "credential")?;
    for name in extra.iter().filter(|n| !matches!(**n, "after" | "limit")) {
        required(&args, name)?;
    }
    Ok(args)
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let access = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    store.sales_agent_owner_view(&access)?;
    match args.positional()[0].as_str() {
        "personas" => serde_json::to_value(training::personas()).map_err(|e| e.to_string()),
        "script-source" => serde_json::to_value(
            training::Scripted::new(&format!("human:{}", access.principal())).source(),
        )
        .map_err(|e| e.to_string()),
        "schedule" => {
            let input: training::Schedule = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "invalid synthetic practice schedule")?;
            serde_json::to_value(store.schedule_sales_roleplay(&access, &input)?)
                .map_err(|e| e.to_string())
        }
        "run-scripted" => {
            let mut script = training::Scripted::new(&format!("human:{}", access.principal()));
            serde_json::to_value(store.run_sales_roleplay(
                &access,
                required(args, "run")?,
                &mut script,
            )?)
            .map_err(|e| e.to_string())
        }
        "show" => serde_json::to_value(store.sales_roleplay(&access, required(args, "run")?)?)
            .map_err(|e| e.to_string()),
        _ => {
            let paul = store.sales_agent_anchor(&access, required(args, "agent")?)?;
            let limit = args
                .option("limit")
                .unwrap_or("20")
                .parse()
                .map_err(|_| "invalid synthetic schedule limit")?;
            Ok(
                json!({"label":training::LABEL,"practices":store.sales_roleplay_schedule(&access,&paul,args.option("after"),limit)?}),
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_commands_require_explicit_private_authority_and_bounded_forms() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert!(parse(&words("personas --root private --credential owner")).is_ok());
        assert!(
            parse(&words(
                "run-scripted --root private --credential owner --run practice"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "schedule --root private --credential owner --input practice"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "run-scripted --root private --credential owner --run practice --send"
            ))
            .is_err()
        );
        assert!(parse(&words("run-scripted --run practice")).is_err());
    }
}
