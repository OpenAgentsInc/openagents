//! Canonical contact reductions and explicit owner-recorded business admission.
use super::{Store, required};
use crate::{Args, Output};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
pub(super) const USAGE: &str =
    "usage: openagents sales privacy COMMAND --root DIR --credential FILE [--json]
  view                          Read owner-only policy, opaque suppression, and copy cleanup.
  apply --input FILE             Record a versioned owner contact/privacy command.
  check --lead LEAD --channel email
                                Check current business-contact admission; grants no send.
  prune                         Apply due native retention and registered copy cleanup.
FILE=- reads bounded JSON from stdin. File inputs must be private regular files.
Owner-recorded requested contact or accepted introduction is attributable human
evidence, not independent attestation. An active human writer may record immediate
opt_out, including ambiguity, after permission expires. No command restores opt-out.
Generic sales model disclosure and relay sync have no qualified recipient adapter.
Removed native copies do not prove deletion of unmanaged captures or relay history.";
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
            "view" | "prune" => &[],
            "apply" => &["input"],
            "check" => &["lead", "channel"],
            _ => return Err(USAGE.into()),
        };
        if args
            .option_names()
            .iter()
            .any(|n| !matches!(*n, "root" | "credential") && !allowed.contains(n))
        {
            return Err("unknown sales privacy option".into());
        }
        let mut store = Store::open(Path::new(required(&args, "root")?))?;
        let access = store.authenticate(&Store::read_credential(Path::new(required(
            &args,
            "credential",
        )?))?)?;
        match command {
            "view" | "prune" => store.sales_privacy_view(&access),
            "check" => store.sales_contact_check(
                &access,
                required(&args, "lead")?,
                required(&args, "channel")?,
            ),
            _ => {
                let input = required(&args, "input")?;
                let mut bytes = vec![];
                if input == "-" {
                    std::io::stdin()
                        .take(32 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| "privacy input read failed")?;
                } else {
                    bytes = coder::task::sales::privacy::read_command(Path::new(input))?;
                }
                if bytes.len() > 32 * 1024 {
                    return Err("sales privacy input exceeds 32 KiB".into());
                }
                Ok(
                    json!({"revision":store.apply_sales_privacy(&access,&bytes)?,"outbound_authority":false}),
                )
            }
        }
    })();
    match result {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string_pretty(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("sales privacy", &e),
    }
}
