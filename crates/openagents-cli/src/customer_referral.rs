//! Introduction commands use the selected customer's authenticated account.
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::customer::Store;
use jev::{AttributionProposal, ReferralCapture, ReferralKind};
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
  lineage --referrer ID
        Read authorized management successors, which create no earnings right.
  policy [--digest DIGEST]
        Read the operator-published attribution terms and exact policy digest.
  attribution
        Read the current customer's private decisions and retained binding.
  propose --input FILE
        Consent to {request,policy_digest,introduction,referrer,evidence,reason,
        consent,expected_decision}. An introduction is captured_source,
        early_agreement, preexisting_customer, missing_evidence, or correction.
        Evidence contains opaque {reference,digest} pairs. A correction pins
        the current decision digest and preserves earlier decisions.
  confirm --input FILE
        As the referrer manager, confirm {customer,decision} explicitly.
        Early agreements and reviewed corrections require both parties.
  workspace --workspace ID
        Read a workspace relationship as its current owner or admin.
  adopt --workspace ID --input FILE
        As owner, attach your accepted relationship using {decision:DIGEST}.

Select a customer first. FILE is a bounded private regular JSON file.
Links contain only random source lookup material. Source capture records an
introduction and grants no commission or payment right. Permanent attribution
requires separate consent to published terms. Missing or competing evidence
stays in review. Team creation and ownership transfer retain the original
relationship; current signing keys and wallet destinations do not replace it.
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
    Declared::computer("referral lineage", Effect::ReadOnly),
    Declared::computer("referral policy", Effect::ReadOnly),
    Declared::computer("referral attribution", Effect::ReadOnly),
    Declared::computer("referral propose", Effect::Grants),
    Declared::computer("referral confirm", Effect::Grants),
    Declared::computer("referral workspace", Effect::ReadOnly),
    Declared::computer("referral adopt", Effect::Grants),
];
fn required<'a>(args: &'a Args, name: &str) -> Result<&'a str, String> {
    args.option(name)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("--{name} is required"))
}
fn input<T: for<'de> Deserialize<'de>>(args: &Args) -> Result<T, String> {
    let bytes = Store::private_input(Path::new(required(args, "input")?), 16 * 1024)?;
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
        "create" | "capture" | "propose" | "confirm" => &["root", "input"],
        "show" | "link" | "disable" | "accept" | "lineage" => &["root", "referrer"],
        "migrate" => &["root", "referrer", "input"],
        "source" | "attribution" => &["root"],
        "policy" => &["root", "digest"],
        "workspace" => &["root", "workspace"],
        "adopt" => &["root", "workspace", "input"],
        _ => return output.usage("customer referral", "Unknown referral command.", USAGE),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !allowed.contains(name))
        || allowed
            .iter()
            .filter(|name| command != "policy" || **name != "digest")
            .any(|name| required(&args, name).is_err())
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
        "lineage" => json!(account.referrer_successors(id()?).await.map_err(err)?),
        "policy" => json!(match args.option("digest") {
            Some(digest) => account
                .attribution_policy_version(digest)
                .await
                .map_err(err)?,
            None => account.attribution_policy().await.map_err(err)?,
        }),
        "attribution" => {
            let view = account.attribution().await.map_err(err)?;
            if view
                .as_ref()
                .is_some_and(|v| v.customer != selection.context.account)
            {
                return Err("Attribution belongs to another account.".into());
            }
            json!(view)
        }
        "propose" => {
            let proposal: AttributionProposal = input(args)?;
            let view = account.propose_attribution(&proposal).await.map_err(err)?;
            if view.customer != selection.context.account {
                return Err("Attribution belongs to another account.".into());
            }
            json!(view)
        }
        "confirm" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Confirm {
                customer: String,
                decision: String,
            }
            let input: Confirm = input(args)?;
            let view = account
                .confirm_attribution(&input.customer, &input.decision)
                .await
                .map_err(err)?;
            if view.customer != input.customer {
                return Err("Confirmation belongs to another account.".into());
            }
            json!(view)
        }
        "workspace" => {
            let workspace = required(args, "workspace")?;
            let view = account
                .workspace_attribution(workspace)
                .await
                .map_err(err)?;
            if view.as_ref().is_some_and(|v| v.workspace != workspace) {
                return Err("Attribution belongs to another workspace.".into());
            }
            json!(view)
        }
        "adopt" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Adopt {
                decision: String,
            }
            let input: Adopt = input(args)?;
            let workspace = required(args, "workspace")?;
            let view = account
                .adopt_workspace_attribution(workspace, &input.decision)
                .await
                .map_err(err)?;
            if view.workspace != workspace || view.binding.customer != selection.context.account {
                return Err("Attribution belongs to another account or workspace.".into());
            }
            json!(view)
        }
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
