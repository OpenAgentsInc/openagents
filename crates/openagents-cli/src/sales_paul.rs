//! Thin controls over Paul's canonical private controller.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::paul;
use serde_json::{Value, json};
use std::path::Path;
const USAGE: &str = "usage: openagents sales paul COMMAND --root DIR --credential FILE [--json]
  owner-view                              Read the original owner's queue without a reservation.
  propose-draft --requester DEVICE --request ID --input FILE
                                          Propose an original reviewed helper body; qualification applies.
  source                                  Read the bounded zero-cost native verification source.
  binding-check --input FILE               Read the exact private binding digest.
  configure --input FILE --approve SHA256  Approve the next binding as the current owner.
  pipeline --requester DEVICE --request ID Read current assigned records and original expense evidence.
  research --requester DEVICE --request ID --input FILE
                                          Read only exact reviewed claims for a current assignment.
  practice --requester DEVICE             Read original opaque practice and expense references.
No command grants model capacity, Coder execution, message sending, certification,
payment, or customer-data disclosure. Credential and input files must be private.";
pub(crate) fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|s| s == "help")
        || words.iter().any(|s| matches!(s.as_str(), "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(v) => v,
        Err(e) => return output.usage("sales paul", &e, USAGE),
    };
    match execute(&args) {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales paul", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "source" | "owner-view" => &[],
        "binding-check" => &["input"],
        "configure" => &["input", "approve"],
        "pipeline" => &["requester", "request"],
        "research" | "propose-draft" => &["requester", "request", "input"],
        "practice" => &["requester"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential") && !extra.contains(n))
    {
        return Err("unknown sales Paul option".into());
    }
    for n in ["root", "credential"]
        .into_iter()
        .chain(extra.iter().copied())
    {
        required(&args, n)?;
    }
    Ok(args)
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let owner = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    store.sales_agent_owner_view(&owner)?;
    match args.positional()[0].as_str() {
        "source" => Ok(json!({"source":paul::source(),"model_available":false})),
        "propose-draft" => {
            let request: paul::DraftRequest = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "malformed Paul reviewed draft request")?;
            serde_json::to_value(store.ask_paul_draft(
                required(args, "requester")?,
                required(args, "request")?,
                &request,
            )?)
            .map_err(|e| e.to_string())
        }
        "owner-view" => {
            serde_json::to_value(store.read_paul_pipeline(&owner)?).map_err(|e| e.to_string())
        }
        "binding-check" | "configure" => {
            let binding: paul::Binding = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "malformed Paul binding")?;
            let sha = binding.sha256()?;
            if args.positional()[0] == "configure" {
                store.configure_paul(&owner, &binding, required(args, "approve")?)?;
            }
            Ok(json!({"sha256":sha,"external_effects":false}))
        }
        "research" => {
            let request: paul::ResearchRequest =
                serde_json::from_slice(&super::agents::input(args)?)
                    .map_err(|_| "malformed Paul research request")?;
            serde_json::to_value(store.ask_paul_research(
                required(args, "requester")?,
                required(args, "request")?,
                &request,
            )?)
            .map_err(|e| e.to_string())
        }
        "practice" => serde_json::to_value(store.ask_paul_practice(required(args, "requester")?)?)
            .map_err(|e| e.to_string()),
        "pipeline" => serde_json::to_value(
            store.ask_paul_pipeline(required(args, "requester")?, required(args, "request")?)?,
        )
        .map_err(|e| e.to_string()),
        _ => Err(USAGE.into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paul_parser_requires_exact_owner_files_and_typed_controls() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert!(
            parse(&words(
                "pipeline --root private --credential owner --requester device --request one"
            ))
            .is_ok()
        );
        assert!(
            parse(&words(
                "configure --root private --credential owner --input binding --approve digest"
            ))
            .is_ok()
        );
        for s in [
            "pipeline --root private --credential owner --request one",
            "configure --root private --credential owner --input binding",
            "source --root private",
            "pipeline --root private --credential owner --requester device --request one --send true",
        ] {
            assert!(parse(&words(s)).is_err());
        }
    }
}
