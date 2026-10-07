//! Thin local commands over the host's canonical reviewed claims register.
use super::{Store, required};
use crate::{Args, Output};
use coder::task::sales::claims::{ClaimInput, DraftPin, Pin, SourceInput};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
const USAGE: &str = "usage: openagents sales claims COMMAND --root DIR --credential FILE [--json]
  source --input FILE                 Review exact current source bytes.
  review --input FILE                 Review a versioned claim.
  read --input FILE --release COMMIT   Revalidate an array of 1 to 8 claim pins.
  draft --input FILE --draft ID --release COMMIT
                                      Compose an array of exact reviewed draft pins.
  validate --draft ID --release COMMIT Revalidate the entire retained draft.
  withdraw --input FILE               Record an irreversible revision withdrawal.
  history [--after N] [--limit N]      Read at most 100 private decisions.
FILE=- reads bounded JSON from stdin. This command grants no sending,
execution, purchase, publication, or customer-data disclosure authority.";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Withdrawal {
    pin: Pin,
    source: bool,
    reference: String,
}
pub(super) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|v| matches!(v.as_str(), "--help" | "help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(args) => args,
        Err(e) => return output.usage("sales claims", &e, USAGE),
    };
    match execute(&args) {
        Ok(value) => {
            output.emit(&value, |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            0
        }
        Err(e) => output.fail("sales claims", &e),
    }
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    if args.positional().len() != 1 {
        return Err(USAGE.into());
    }
    let extra: &[&str] = match args.positional()[0].as_str() {
        "source" | "review" | "withdraw" => &["input"],
        "read" => &["input", "release"],
        "draft" => &["input", "draft", "release"],
        "validate" => &["draft", "release"],
        "history" => &["after", "limit"],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !matches!(*name, "root" | "credential") && !extra.contains(name))
    {
        return Err("unknown claims option".into());
    }
    required(&args, "root")?;
    required(&args, "credential")?;
    if args.positional()[0] != "history" {
        for name in extra {
            required(&args, name)?;
        }
    } else {
        let limit: usize = args
            .option("limit")
            .unwrap_or("50")
            .parse()
            .map_err(|_| "invalid claims history limit")?;
        if !(1..=100).contains(&limit) {
            return Err("claims history limit must be 1 to 100".into());
        }
        args.option("after")
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| "invalid claims history cursor")?;
    }
    Ok(args)
}
fn input<T: for<'a> Deserialize<'a>>(args: &Args) -> Result<T, String> {
    let path = required(args, "input")?;
    let mut bytes = Vec::new();
    if path == "-" {
        std::io::stdin()
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    } else {
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(32 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    }
    if bytes.len() > 32 * 1024 {
        return Err("claims input exceeds 32 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "malformed claims input".into())
}
fn execute(args: &Args) -> Result<Value, String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let access = store.authenticate(&Store::read_credential(Path::new(required(
        args,
        "credential",
    )?))?)?;
    let value = match args.positional()[0].as_str() {
        "source" => {
            serde_json::to_value(store.review_claim_source(&access, input::<SourceInput>(args)?)?)
        }
        "review" => serde_json::to_value(store.review_claim(&access, input::<ClaimInput>(args)?)?),
        "read" => serde_json::to_value(store.current_claims(
            &access,
            &input::<Vec<Pin>>(args)?,
            required(args, "release")?,
        )?),
        "draft" => serde_json::to_value(store.compose_claim_draft(
            &access,
            required(args, "draft")?,
            input::<Vec<DraftPin>>(args)?,
            required(args, "release")?,
        )?),
        "validate" => serde_json::to_value(store.validate_claim_draft(
            &access,
            required(args, "draft")?,
            required(args, "release")?,
        )?),
        "withdraw" => {
            let request = input::<Withdrawal>(args)?;
            store.withdraw_claim_revision(
                &access,
                &request.pin,
                request.source,
                &request.reference,
            )?;
            return Ok(json!({"withdrawn":request.pin,"source":request.source}));
        }
        "history" => serde_json::to_value(
            store.claim_history(
                &access,
                args.option("after")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| "invalid claims cursor")?,
                args.option("limit")
                    .unwrap_or("50")
                    .parse()
                    .map_err(|_| "invalid claims limit")?,
            )?,
        ),
        _ => return Err(USAGE.into()),
    };
    value.map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn words(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }
    #[test]
    fn bounded_private_claim_consumer_has_no_fallback_root_or_secret_output() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let credential = dir.path().join("owner-token");
        let mut store = Store::open(&root).unwrap();
        store.initialize("owner", &credential).unwrap();
        let secret = Store::read_credential(&credential).unwrap();
        drop(store);
        let file = dir.path().join("pins.json");
        std::fs::write(&file, b"[{\"id\":\"missing\",\"revision\":1}]").unwrap();
        let args = parse(&words(&[
            "read",
            "--root",
            root.to_str().unwrap(),
            "--credential",
            credential.to_str().unwrap(),
            "--input",
            file.to_str().unwrap(),
            "--release",
            &"a".repeat(40),
        ]))
        .unwrap();
        let result = execute(&args).unwrap();
        assert_eq!(result[0]["verdict"]["reason"]["reason"], "unknown_claim");
        assert!(!result.to_string().contains(&secret));
        assert!(!result.to_string().contains(root.to_str().unwrap()));
        assert!(
            parse(&words(&[
                "read",
                "--credential",
                credential.to_str().unwrap(),
                "--input",
                file.to_str().unwrap(),
                "--release",
                &"a".repeat(40)
            ]))
            .is_err()
        );
        assert!(
            parse(&words(&[
                "history",
                "--root",
                root.to_str().unwrap(),
                "--credential",
                credential.to_str().unwrap(),
                "--limit",
                "101"
            ]))
            .is_err()
        );
        assert!(
            parse(&words(&[
                "validate",
                "--root",
                root.to_str().unwrap(),
                "--credential",
                credential.to_str().unwrap(),
                "--draft",
                "d",
                "--release",
                &"a".repeat(40),
                "--send",
                "yes"
            ]))
            .is_err()
        );
        std::fs::write(&file, vec![b'x'; 32 * 1024 + 1]).unwrap();
        assert!(execute(&args).unwrap_err().contains("32 KiB"));
    }
}
