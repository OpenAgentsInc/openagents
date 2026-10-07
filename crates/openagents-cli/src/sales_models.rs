//! Owner control of canonical local sales expenses, without model execution.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::expenses;
use serde_json::{Value, json};
use std::path::Path;
const USAGE: &str = "usage: openagents sales models COMMAND --root DIR --credential FILE [--json]
  policy-check --input FILE                 Check a finite expense policy and its digest.
  policy --input FILE --approve SHA256      Record the exact owner-approved policy.
  settle --reservation ID --input FILE     Reconcile estimates separately from provider billing.
  show --reservation ID                    Read the original expense receipt after retirement.
  history [--after ID] [--limit N]          Read at most 100 owner-only expense receipts.
All operations require the current owner credential and explicit private root.
FILE=- reads bounded JSON from stdin; files must be private and unshared.
These commands do not execute a model, grant contact authority, or certify results.
Unknown usage preserves its original liability and stops new floor work.";
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
        Err(e) => return output.usage("sales models", &e, USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(e) => output.fail("sales models", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "policy-check" => &["input"],
        "policy" => &["input", "approve"],
        "settle" => &["reservation", "input"],
        "show" => &["reservation"],
        "history" => &["after", "limit"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential") && !extra.contains(n))
    {
        return Err("unknown sales models option".into());
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
    // This owner read also fences policy-check and empty history.
    store.sales_agent_owner_view(&access)?;
    match args.positional()[0].as_str() {
        "policy" | "policy-check" => {
            let p: expenses::Policy = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "malformed sales model policy")?;
            let sha = p.sha256()?;
            if args.positional()[0] == "policy" {
                store.publish_sales_model_policy(&access, &p, required(args, "approve")?)?;
            }
            Ok(json!({"sha256":sha,"model_execution":false,"outbound_authority":false}))
        }
        "settle" => {
            let settlement: expenses::Settlement =
                serde_json::from_slice(&super::agents::input(args)?)
                    .map_err(|_| "malformed sales model settlement")?;
            serde_json::to_value(store.settle_sales_model(
                &access,
                required(args, "reservation")?,
                &settlement,
            )?)
            .map_err(|e| e.to_string())
        }
        "show" => serde_json::to_value(
            store.sales_model_reservation(&access, required(args, "reservation")?)?,
        )
        .map_err(|e| e.to_string()),
        _ => {
            let limit = args
                .option("limit")
                .unwrap_or("25")
                .parse::<usize>()
                .map_err(|_| "invalid sales model history limit")?;
            serde_json::to_value(store.sales_model_reservations(
                &access,
                args.option("after"),
                limit,
            )?)
            .map_err(|e| e.to_string())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expense_operations_require_explicit_custody_and_exact_flags() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert!(
            parse(&words(
                "history --root private --credential owner --limit 2"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "policy --root private --credential owner --input p --approve digest"
            ))
            .is_ok()
        );
        for s in [
            "history --credential owner",
            "policy --root private --credential owner --input p",
            "show --root private --credential owner",
            "history --root private --credential owner --api-key secret",
            "reserve --root private --credential owner",
        ] {
            assert!(parse(&words(s)).is_err());
        }
    }
}
