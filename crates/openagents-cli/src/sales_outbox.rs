//! Exact owner decisions and single-use SMTP dispatch through the native sales book.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::{email, outbox};
use serde_json::{Value, json};
use std::{io::Read, path::Path, sync::atomic::AtomicBool};
pub(super) const USAGE: &str =
    "usage: openagents sales outbox COMMAND --root DIR --credential FILE [--json]
  view                          Read the owner's exact approval subjects and attempt states.
  propose --input FILE --mailbox-key FILE
                                Reserve counts and freeze recipient, content, and authority.
  apply --input FILE [--mailbox-key FILE]
                                Approve an exact subject, record owner sends, pause, or review restart.
  fixture --proposal ID --subject-sha256 SHA --input FILE --mailbox-key FILE
                                Consume one fixture approval with bounded synthetic evidence.
  dispatch --proposal ID --subject-sha256 SHA --mailbox-key FILE
                                Consume one live approval through the qualified SMTP adapter.
FILE=- reads bounded JSON from stdin. File inputs and mailbox keys must be private
regular files. Only the owner can approve and dispatch. SMTP needs current measured
certification, cost attribution, consent, suppression, a qualified reply handler,
and a current owner activation. Missing prerequisites refuse before SMTP contact.
Attachments are bounded private UTF-8 text files; unsupported formats refuse.
Approval pins exact MIME bytes and attachment sources. The native host rechecks
current authority before credentials, envelope recipients, and body disclosure.
A provider's acceptance does not prove delivery. An uncertain or interrupted
attempt consumes its reservation and never authorizes automatic resend.
Fixture activity does not qualify a live operating week or increase real caps.";
fn input(args: &Args) -> Result<Vec<u8>, String> {
    let path = required(args, "input")?;
    let mut bytes = vec![];
    if path == "-" {
        std::io::stdin()
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "outbox input read failed")?;
    } else {
        bytes = coder::task::sales::privacy::read_command(Path::new(path))?;
    }
    if bytes.len() > 32 * 1024 {
        return Err("outbox input exceeds its bound".into());
    }
    Ok(bytes)
}
struct MissingCredentials;
impl email::MailboxCredentials for MissingCredentials {
    fn load(&self, _account: &str) -> Result<email::MailboxSecret, String> {
        Err("host mailbox credential unavailable".into())
    }
}
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
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
            "propose" | "apply" => &["input", "mailbox-key"],
            "fixture" => &["proposal", "subject-sha256", "input", "mailbox-key"],
            "dispatch" => &["proposal", "subject-sha256", "mailbox-key"],
            _ => return Err(USAGE.into()),
        };
        if args
            .option_names()
            .iter()
            .any(|n| !matches!(*n, "root" | "credential") && !allowed.contains(n))
        {
            return Err("unknown sales outbox option".into());
        }
        let mut store = Store::open(Path::new(required(&args, "root")?))?;
        let access = store.authenticate(&Store::read_credential(Path::new(required(
            &args,
            "credential",
        )?))?)?;
        if command == "view" {
            return store.sales_outbox_view(&access);
        }
        let keys: Box<dyn email::MailboxCredentials> =
            if let Some(path) = args.option("mailbox-key") {
                let view = store.email_view(&access)?;
                let current = view["current"]
                    .as_str()
                    .ok_or("email configuration unavailable")?;
                let account = view["configurations"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|r| r["sha256"].as_str() == Some(current)))
                    .and_then(|r| r["config"]["credential_account"].as_str())
                    .ok_or("email account unavailable")?;
                Box::new(email::FileAccount::new(account, Path::new(path))?)
            } else {
                if command != "apply" {
                    return Err("outbox operation requires a protected mailbox-key source".into());
                }
                Box::new(MissingCredentials)
            };
        match command {
            "propose" => {
                let proposal: outbox::Proposal = serde_json::from_slice(&input(&args)?)
                    .map_err(|_| "outbox proposal is malformed")?;
                let subject = store.propose_sales_outbox(&access, proposal, keys.as_ref())?;
                Ok(
                    json!({"subject_sha256":subject.sha256()?,"subject":subject,"outbound_authority":false}),
                )
            }
            "apply" => Ok(
                json!({"revision":store.apply_sales_outbox(&access,&input(&args)?,keys.as_ref())?,"outbound_authority":false}),
            ),
            "fixture" => {
                let mut transport = email::FakeTransport {
                    result: input(&args)?,
                    calls: 0,
                };
                let record = store.dispatch_sales_outbox_fixture(
                    &access,
                    required(&args, "proposal")?,
                    required(&args, "subject-sha256")?,
                    keys.as_ref(),
                    &mut transport,
                    &AtomicBool::new(false),
                )?;
                Ok(json!({"record":record,"fixture_calls":transport.calls,"live_send":false}))
            }
            _ => {
                let admission = store.admit_sales_outbox_smtp(
                    &access,
                    required(&args, "proposal")?,
                    required(&args, "subject-sha256")?,
                    keys.as_ref(),
                )?;
                drop(store);
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "SMTP runtime unavailable")?;
                serde_json::to_value(
                    runtime.block_on(admission.execute(keys.as_ref(), &AtomicBool::new(false)))?,
                )
                .map_err(|_| "outbox observation serialization failed".into())
            }
        }
    })();
    match result {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(error) => output.fail("sales outbox", &error),
    }
}
