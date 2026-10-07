//! Owner-only mailbox configuration and preflight. This command sends no email.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::email::{FileAccount, Message};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
pub(super) const USAGE: &str =
    "usage: openagents sales email COMMAND --root DIR --credential FILE [--json]
  view                          Read owner-declared mailbox configuration and evidence.
  apply --input FILE             Configure or revoke a versioned mailbox handle.
  check --input FILE --mailbox-key FILE
                                Prepare a message with an explicit private file account key.
  evidence --input FILE --message-sha256 SHA
                                Map bounded provider evidence; acceptance is not delivery.
FILE=- reads bounded JSON from stdin. File inputs must be private regular files.
Provider credentials are opaque UTF-8 values of 1 to 2,048 bytes with no newline
or NUL, in a private file. SMTP passwords and OAuth/API tokens keep their actual
format. No credential is printed or created. The host can inject a sealed source
whose existing AccountKeys authority key encrypts the arbitrary provider secret.
Only the owner can configure a mailbox. DNS, identity, and unsubscribe evidence
is owner-declared, not independent verification. Fixture checks advertise no
live mailbox and grant no dispatch or campaign authority.";
fn input(args: &Args) -> Result<Vec<u8>, String> {
    let path = required(args, "input")?;
    let bytes = if path == "-" {
        let mut bytes = vec![];
        std::io::stdin()
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "email input read failed")?;
        bytes
    } else {
        coder::task::sales::privacy::read_command(Path::new(path))?
    };
    if bytes.len() > 32 * 1024 {
        return Err("email input exceeds its bound".into());
    }
    Ok(bytes)
}
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "--help" | "-h" | "help"))
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
            "apply" => &["input"],
            "check" => &["input", "mailbox-key"],
            "evidence" => &["input", "message-sha256"],
            _ => return Err(USAGE.into()),
        };
        if args
            .option_names()
            .iter()
            .any(|n| !matches!(*n, "root" | "credential") && !allowed.contains(n))
        {
            return Err("unknown sales email option".into());
        }
        let mut store = Store::open(Path::new(required(&args, "root")?))?;
        let access = store.authenticate(&Store::read_credential(Path::new(required(
            &args,
            "credential",
        )?))?)?;
        match command {
            "view" => store.email_view(&access),
            "apply" => Ok(
                json!({"revision":store.apply_email(&access,&input(&args)?)?,"outbound_authority":false,"live_sender_qualified":false}),
            ),
            "check" => {
                let message: Message = serde_json::from_slice(&input(&args)?)
                    .map_err(|_| "email message is malformed")?;
                let view = store.email_view(&access)?;
                let account = view["configurations"]
                    .as_array()
                    .and_then(|rows| {
                        rows.iter()
                            .find(|r| r["sha256"].as_str() == Some(message.config_sha256.as_str()))
                    })
                    .and_then(|r| r["config"]["credential_account"].as_str())
                    .ok_or("email configuration is unavailable")?;
                let keys = FileAccount::new(account, Path::new(required(&args, "mailbox-key")?))?;
                Ok(store.prepare_email(&access, message, &keys)?.view())
            }
            _ => serde_json::to_value(store.email_provider_evidence(
                &access,
                &input(&args)?,
                required(&args, "message-sha256")?,
            )?)
            .map_err(|_| "email evidence serialization failed".into()),
        }
    })();
    match result {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales email", &e),
    }
}
