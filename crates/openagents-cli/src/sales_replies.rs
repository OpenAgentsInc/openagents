//! Untrusted inbox imports and owner review through the canonical private sales book.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::{meetings, outbox, replies};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Read, path::Path};
pub(super) const USAGE: &str = "usage: openagents sales replies COMMAND --root DIR --credential FILE [--json]
  view                          Read bounded private reply records and explicit source provenance.
  ingest --input FILE            Import an untrusted reply; contact safety runs before classification.
  review --input FILE            Record one exact owner classification; grants no sending authority.
  qualify --expires-at UNIX      Run and retain the native scratch-injection safety suite.
  revoke --qualification SHA     Revoke the current handler qualification and pause outbound work.
  booking --input FILE          Prepare a human meeting from an exact owner-reviewed interested reply.
  follow-up --lead LEAD --mode fixture|live
                                Retain a native no-response plan with real-week spacing.
FILE=- reads bounded JSON from stdin. Files must be private regular files.
Imported quotes, links, and attachment metadata never select tools or permissions.
Owner imports are untrusted statements and cannot prove provider delivery.
Automatic mailbox polling and independent inbound provider receipts are unavailable.
Handler qualification measures native safety, not model quality or mailbox health.
A follow-up plan grants no dispatch authority; sending needs current permission,
certification, suppression, remaining message counts, and exact owner approval.";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    reply: String,
    input_sha256: String,
    expected_revision: u64,
    label: replies::Label,
    review_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Booking {
    reply: String,
    input_sha256: String,
    handler_qualification_sha256: String,
    meeting: meetings::ProposalInput,
}
fn input(args: &Args) -> Result<Vec<u8>, String> {
    let name = required(args, "input")?;
    let mut bytes = vec![];
    if name == "-" {
        std::io::stdin()
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "reply input read failed")?;
    } else {
        bytes = coder::task::sales::privacy::read_command(Path::new(name))?;
    }
    if bytes.len() > 32 * 1024 {
        return Err("reply input exceeds 32 KiB".into());
    }
    Ok(bytes)
}
fn value<T: Serialize>(input: T) -> Result<Value, String> {
    serde_json::to_value(input).map_err(|_| "reply result serialization failed".into())
}
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let result = (|| -> Result<Value, String> {
        let args = Args::parse(words, &[])?;
        if args.positional().len() != 1 {
            return Err(USAGE.into());
        }
        let command = args.positional()[0].as_str();
        let allowed: &[&str] = match command {
            "view" => &[],
            "ingest" | "review" | "booking" => &["input"],
            "qualify" => &["expires-at"],
            "revoke" => &["qualification"],
            "follow-up" => &["lead", "mode"],
            _ => return Err(USAGE.into()),
        };
        if args
            .option_names()
            .iter()
            .any(|n| !matches!(*n, "root" | "credential") && !allowed.contains(n))
        {
            return Err("unknown sales replies option".into());
        }
        let mut store = Store::open(Path::new(required(&args, "root")?))?;
        let owner = store.authenticate(&Store::read_credential(Path::new(required(
            &args,
            "credential",
        )?))?)?;
        match command {
            "view" => store.sales_replies_view(&owner),
            "ingest" => value(store.ingest_sales_reply(
                &owner,
                serde_json::from_slice(&input(&args)?).map_err(|_| "reply import is malformed")?,
            )?),
            "review" => {
                let r: Review = serde_json::from_slice(&input(&args)?)
                    .map_err(|_| "reply review is malformed")?;
                value(store.review_sales_reply(
                    &owner,
                    &r.reply,
                    &r.input_sha256,
                    r.expected_revision,
                    r.label,
                    &r.review_sha256,
                )?)
            }
            "booking" => {
                let b: Booking = serde_json::from_slice(&input(&args)?)
                    .map_err(|_| "reply booking is malformed")?;
                value(store.propose_sales_meeting_from_reply(
                    &owner,
                    &b.reply,
                    &b.input_sha256,
                    &b.handler_qualification_sha256,
                    &b.meeting,
                )?)
            }
            "qualify" => {
                let receipt = store.qualify_sales_reply_handler(
                    &owner,
                    required(&args, "expires-at")?
                        .parse()
                        .map_err(|_| "reply qualification expiry is malformed")?,
                )?;
                Ok(
                    json!({"qualification_sha256": receipt.sha256()?, "qualification": receipt, "outbound_authority": false}),
                )
            }
            "revoke" => {
                store
                    .revoke_sales_reply_qualification(&owner, required(&args, "qualification")?)?;
                Ok(json!({"outbound_authority":false,"paused":true}))
            }
            _ => {
                let mode = match required(&args, "mode")? {
                    "fixture" => outbox::Mode::Fixture,
                    "live" => outbox::Mode::Live,
                    _ => return Err("follow-up mode must be fixture or live".into()),
                };
                let plan = store.plan_sales_follow_up(&owner, required(&args, "lead")?, mode)?;
                Ok(json!({"reference":plan.artifact()?,"plan":plan,"outbound_authority":false}))
            }
        }
    })();
    match result {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales replies", &e),
    }
}
