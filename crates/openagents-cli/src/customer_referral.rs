//! Introduction commands use the selected customer's authenticated account.
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::customer::Store;
use jev::{ReferralCapture, ReferralKind};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

pub const USAGE: &str = "usage: openagents customer referral COMMAND --root DIR [OPTIONS]
  create --input FILE
        Create a stable referrer from {kind:person|agent|author|partner,label}.
  show --referrer ID
        Read a referrer managed by the current authenticated account.
  link --referrer ID
        Rotate earlier public links and return a shareable source URL.
  disable --referrer ID
        Disable current links while preserving captured source history.
  capture --input FILE
        Record {request,token,consent,consent_version} once for this account.
        Consent uses openagents.referral.consent.v1. Missing or declined
        attribution stays explicit; changed or competing capture is refused.
  source
        Read only the current account's private acquisition source.
  migrate --referrer ID --input FILE
        Offer management migration to the account in {account:ID}.
  accept --referrer ID
        Accept migration offered to the current authenticated account.

Select a customer first. FILE is a bounded private regular JSON file.
Links contain only random source lookup material. Source capture records an
introduction and grants no permanent attribution, commission, or payment right.
OpenAgents sales-agent identities are provisioned source-only by the operator.
Public wording and consent presentation require owner review before distribution.";
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("referral create", Effect::Publishes),
    Declared::computer("referral show", Effect::ReadOnly),
    Declared::computer("referral link", Effect::Publishes),
    Declared::computer("referral disable", Effect::Grants),
    Declared::computer("referral capture", Effect::Grants),
    Declared::computer("referral source", Effect::ReadOnly),
    Declared::computer("referral migrate", Effect::Grants),
    Declared::computer("referral accept", Effect::Grants),
];
fn required<'a>(args: &'a Args, name: &str) -> Result<&'a str, String> {
    args.option(name)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("--{name} is required"))
}
fn input<T: for<'de> Deserialize<'de>>(args: &Args) -> Result<T, String> {
    let bytes = Store::private_input(Path::new(required(args, "input")?), 4096)?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid bounded private referral input.".into())
}
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|v| matches!(v.as_str(), "--help" | "help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(v) => v,
        Err(e) => return output.usage("customer referral", &e, USAGE),
    };
    let Some(command) = args
        .positional()
        .first()
        .filter(|_| args.positional().len() == 1)
        .cloned()
    else {
        return output.usage("customer referral", "Select one referral command.", USAGE);
    };
    let allowed: &[&str] = match command.as_str() {
        "create" | "capture" => &["root", "input"],
        "show" | "link" | "disable" | "accept" => &["root", "referrer"],
        "migrate" => &["root", "referrer", "input"],
        "source" => &["root"],
        _ => return output.usage("customer referral", "Unknown referral command.", USAGE),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !allowed.contains(name))
        || allowed.iter().any(|name| required(&args, name).is_err())
    {
        return output.usage(
            "customer referral",
            "Use the declared options and private input files.",
            USAGE,
        );
    }
    match crate::runtime().block_on(execute(&args, &command)) {
        Ok(value) => {
            output.emit(&value, |v| serde_json::to_string(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("customer referral", &e),
    }
}
async fn execute(args: &Args, command: &str) -> Result<serde_json::Value, String> {
    let root = Path::new(required(args, "root")?);
    if !root.is_absolute() {
        return Err("Customer state needs an absolute private root.".into());
    }
    let store = Store::open(root)?;
    let selection = store.current_selection().await?;
    let client = store.client(&selection.origin, &selection.credential_alias)?;
    let account = client
        .account()
        .for_referrals_account(&selection.context.account);
    let err = |e: jev::Error| format!("Referral service refused the operation: {e}");
    let id = || required(args, "referrer");
    Ok(match command {
        "create" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Create {
                kind: ReferralKind,
                label: String,
            }
            let value: Create = input(args)?;
            json!(
                account
                    .create_referrer(value.kind, &value.label)
                    .await
                    .map_err(err)?
            )
        }
        "show" => json!(account.referrer(id()?).await.map_err(err)?),
        "link" => {
            let value = account.issue_referral_link(id()?).await.map_err(err)?;
            json!({"referrer":value.referrer,"url":format!("{}{}",selection.origin,value.path),"token":value.token})
        }
        "disable" => {
            account.disable_referral_links(id()?).await.map_err(err)?;
            json!({"disabled":true})
        }
        "source" => {
            let source = account.acquisition().await.map_err(err)?;
            if source
                .as_ref()
                .is_some_and(|s| s.account != selection.context.account)
            {
                return Err("Acquisition source belongs to another account.".into());
            }
            json!(source)
        }
        "capture" => {
            let capture: ReferralCapture = input(args)?;
            let source = account.capture_acquisition(&capture).await.map_err(err)?;
            if source.account != selection.context.account {
                return Err("Acquisition source belongs to another account.".into());
            }
            json!(source)
        }
        "migrate" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Migration {
                account: String,
            }
            let input: Migration = input(args)?;
            json!(
                account
                    .offer_referrer_migration(id()?, &input.account)
                    .await
                    .map_err(err)?
            )
        }
        "accept" => json!(
            account
                .accept_referrer_migration(id()?)
                .await
                .map_err(err)?
        ),
        _ => return Err("Unknown referral command.".into()),
    })
}
