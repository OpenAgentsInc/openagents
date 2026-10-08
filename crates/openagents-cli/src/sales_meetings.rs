//! Bounded private meeting proposals and named human decisions.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::meetings;
use serde_json::Value;
use std::path::Path;
const USAGE: &str = "usage: openagents sales meetings COMMAND --root DIR --credential FILE [--json]
  slot --input FILE --expected-version N    Publish explicit finite availability as the owner.
  slots                                   Read current slots under assigned native agent access.
  queue                                   Read opaque meeting references under assigned native agent access.
  list                                    Read only current owner/named-human private briefs.
  propose --input FILE                    Prepare the owner's bounded private brief.
  recommend --meeting ID --revision N --slot ID --slot-version N
                                          Recommend a published slot under assigned agent access.
  confirm --meeting ID --revision N --approve SHA256 --input FILE
                                          Confirm the exact proposal as owner; input is a JSON reference string.
  accept --meeting ID --revision N --approve SHA256 --input FILE
                                          Accept only as the named human; input is a JSON reference string.
  decline --meeting ID --revision N --approve SHA256 --input FILE
                                          Decline only as the named human; input is a JSON reference string.
  show --meeting ID                       Read the owner's or named human's bounded brief.
Files must be private and unshared. These commands grant no calendar, mailbox,
payment, outbound, or broader lead authority and record no earned revenue.";
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
        Err(e) => return output.usage("sales meetings", &e, USAGE),
    };
    match execute(&args) {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales meetings", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "slot" => &["input", "expected-version"],
        "slots" | "queue" | "list" => &[],
        "propose" => &["input"],
        "recommend" => &["meeting", "revision", "slot", "slot-version"],
        "confirm" | "accept" | "decline" => &["meeting", "revision", "approve", "input"],
        "show" => &["meeting"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|n| !matches!(*n, "root" | "credential") && !extra.contains(n))
    {
        return Err("unknown sales meetings option".into());
    }
    for n in ["root", "credential"]
        .into_iter()
        .chain(extra.iter().copied())
    {
        required(&args, n)?;
    }
    Ok(args)
}
fn number(args: &Args, name: &str) -> Result<u64, String> {
    required(args, name)?
        .parse()
        .map_err(|_| format!("invalid meeting {name}"))
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let secret = Store::read_credential(Path::new(required(args, "credential")?))?;
    let command = args.positional()[0].as_str();
    if matches!(command, "slots" | "queue" | "recommend") {
        let access = store.authenticate_sales_agent(&secret)?;
        return if command == "slots" {
            serde_json::to_value(store.sales_meeting_slots(&access)?)
        } else if command == "queue" {
            serde_json::to_value(store.sales_agent_meetings(&access)?)
        } else {
            serde_json::to_value(store.recommend_sales_meeting_slot(
                &access,
                required(args, "meeting")?,
                number(args, "revision")?,
                required(args, "slot")?,
                number(args, "slot-version")?,
            )?)
        }
        .map_err(|e| e.to_string());
    }
    let access = store.authenticate(&secret)?;
    let result = match command {
        "list" => serde_json::to_value(store.sales_meetings(&access, None, 100)?),
        "slot" => {
            let slot: meetings::Slot = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "malformed meeting slot")?;
            serde_json::to_value(store.publish_meeting_slot(
                &access,
                &slot,
                number(args, "expected-version")?,
            )?)
        }
        "propose" => {
            let proposal: meetings::ProposalInput =
                serde_json::from_slice(&super::agents::input(args)?)
                    .map_err(|_| "malformed meeting proposal")?;
            serde_json::to_value(store.propose_sales_meeting(&access, &proposal)?)
        }
        "confirm" | "accept" | "decline" => {
            let reference: String = serde_json::from_slice(&super::agents::input(args)?)
                .map_err(|_| "meeting input must be a JSON reference string")?;
            let id = required(args, "meeting")?;
            let rev = number(args, "revision")?;
            let sha = required(args, "approve")?;
            let result = if command == "confirm" {
                store.confirm_sales_meeting(&access, id, rev, sha, &reference)?
            } else {
                store.decide_sales_meeting(
                    &access,
                    id,
                    rev,
                    sha,
                    command == "accept",
                    &reference,
                )?
            };
            serde_json::to_value(result)
        }
        _ => serde_json::to_value(store.sales_meeting(&access, required(args, "meeting")?)?),
    };
    result.map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meeting_commands_require_explicit_authority_and_exact_revision() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        for s in [
            "slots --root private --credential agent",
            "show --root private --credential human --meeting m",
            "confirm --root private --credential owner --meeting m --revision 2 --approve hash --input private",
        ] {
            assert!(parse(&words(s)).is_ok());
        }
        for s in [
            "slots --root private",
            "confirm --root private --credential owner --meeting m --revision 2 --input private",
            "accept --root private --credential human --meeting m --approve hash --input private",
            "slots --root private --credential agent --calendar calendar",
            "book --root private --credential agent",
        ] {
            assert!(parse(&words(s)).is_err());
        }
    }
}
