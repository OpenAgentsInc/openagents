//! Explicit commercial account custody over the existing Rust gateway client.
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::customer::{CredentialCommand, CredentialStatus, Store};
use serde_json::{Value, json};
use std::path::Path;
#[path = "customer_referral.rs"]
mod referrals;
#[cfg(test)]
pub(crate) fn tree_usage() -> &'static str {
    static USAGES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    USAGES.get_or_init(|| {
        let mut rows = vec![USAGE.lines().next().unwrap_or_default().to_owned()];
        rows.extend(
            USAGE
                .lines()
                .skip(1)
                .take_while(|line| line.is_empty() || line.starts_with(' '))
                .filter(|line| !line.starts_with("  referral COMMAND"))
                .map(str::to_owned),
        );
        for line in referrals::USAGE
            .lines()
            .skip(1)
            .take_while(|line| line.is_empty() || line.starts_with(' '))
        {
            rows.push(match line.strip_prefix("  ") {
                Some(row) if !row.starts_with(' ') => format!("  referral {row}"),
                _ => line.to_owned(),
            });
        }
        rows.join("\n")
    })
}
#[cfg(test)]
pub(crate) fn tree_effects() -> &'static [Declared] {
    static EFFECT: std::sync::OnceLock<Vec<Declared>> = std::sync::OnceLock::new();
    EFFECT.get_or_init(|| {
        let mut all = EFFECTS.to_vec();
        all.extend_from_slice(referrals::EFFECTS);
        all
    })
}
#[path = "customer_team.rs"]
mod team;
pub const USAGE: &str = "usage: openagents customer COMMAND --root DIR [OPTIONS]
  import --alias NAME --input FILE
        Import an immutable credential alias from a private file; prints no key.
  account --origin URL --alias NAME
        Read the authenticated account and workspace membership identifiers.
  select --origin URL --alias NAME --account ID --workspace ID --door NAME
        Bind the explicitly named commercial customer, payer, and resource.
  current
        Read stored selection and current server rights, or explicit unavailable state.
  commercial --product gateway|plugin
        Read the selected product's current native commercial attribution.
  history
        Read only the selected customer's historical purchase references.
  show --purchase ID
        Read one selected-customer quote, approval, receipt, and unresolved ceiling.
  quote --purchase ID --input FILE
        Freeze exact private decision input with current payer, price, and ceiling.
  approve --purchase ID --digest DIGEST
        Approve the exact reviewed quote after rechecking current server rights.
  invoke --purchase ID
        Invoke the approved purchase once; print its result and settlement state.
  reconcile --purchase ID [--receipt DIGEST]
        Read original settlement proof; never dispatch a purchase again.
  change --input FILE [--recovery-token FILE]
        Apply one private credential intent: sign-in, recover, rotate,
        revoke, or sign-out. Its ID prevents replay after uncertain responses.
  funding --input FILE
        Quote, approve, read, or reconcile an exact gateway funding intent.
        Issue creates a receiver invoice and never pays from a caller wallet.
  funding-history
        Read retained original funding references, including uncertain outcomes.
  credentials
        Read credential-operation references for the selected customer.
  inspect --operation ID
        Verify a retained once-issued credential after interruption; never replay
        the mutation or silently select its account.
  referral COMMAND [OPTIONS]
        Create a private referrer, rotate its public source link, or capture
        an explicitly consented introduction. Run referral --help for forms.
  team change --input FILE [--invitation FILE]
        Apply a reviewed team intent with private invitation custody.
  team members --workspace ID
        Read current team roles through the selected account.
  team switch --workspace ID --door NAME [--alias NAME]
        Switch future purchase context while preserving earlier payers.
  team inspect --operation ID
        Inspect the original team intent under current rights.

DIR must be an explicit absolute private directory. Credentials, recovery
material, and decision requests come from private regular files, never secret
flags. change reads a CredentialCommand JSON document with id, origin, account,
credential_alias, and action. Recovery uses credential_alias:null and a separate
recovery token; other changes require the existing account alias. Issued aliases
are immutable and require a separate select. Unknown outcomes retain liability
and require inspection or restored access; a new ID is not retry authority.
Device pairing, a wallet, and provider login confer no commercial membership.";
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("import", Effect::Secret),
    Declared::computer("account", Effect::ReadOnly),
    Declared::computer("select", Effect::LocalWrite),
    Declared::computer("current", Effect::ReadOnly),
    Declared::computer("commercial", Effect::ReadOnly),
    Declared::computer("history", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("quote", Effect::LocalWrite),
    Declared::computer("approve", Effect::Grants),
    Declared::computer("invoke", Effect::Spends),
    Declared::computer("reconcile", Effect::LocalWrite),
    Declared::computer("change", Effect::Secret),
    Declared::computer("funding", Effect::Grants),
    Declared::computer("funding-history", Effect::ReadOnly),
    Declared::computer("credentials", Effect::ReadOnly),
    Declared::computer("inspect", Effect::Secret),
    Declared::computer("team change", Effect::Secret),
    Declared::computer("team members", Effect::ReadOnly),
    Declared::computer("team switch", Effect::LocalWrite),
    Declared::computer("team inspect", Effect::ReadOnly),
];
fn required<'a>(args: &'a Args, flag: &str) -> Result<&'a str, String> {
    args.option(flag)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("--{flag} is required"))
}
fn parse(words: &[String]) -> Result<Args, String> {
    let args = Args::parse(words, &[])?;
    let positions = args.positional();
    if positions.len() != 1 {
        return Err("Select one customer command.".into());
    }
    let allowed: &[&str] = match positions[0].as_str() {
        "import" => &["root", "alias", "input"],
        "account" => &["root", "origin", "alias"],
        "select" => &["root", "origin", "alias", "account", "workspace", "door"],
        "current" | "history" | "credentials" | "funding-history" => &["root"],
        "commercial" => &["root", "product"],
        "show" | "invoke" => &["root", "purchase"],
        "quote" => &["root", "purchase", "input"],
        "approve" => &["root", "purchase", "digest"],
        "reconcile" => &["root", "purchase", "receipt"],
        "change" => &["root", "input", "recovery-token"],
        "funding" => &["root", "input"],
        "inspect" => &["root", "operation"],
        _ => return Err("Unknown customer command.".into()),
    };
    if args.option_names().iter().any(|n| !allowed.contains(n)) {
        return Err("Unknown customer option; secret values must come from private files.".into());
    }
    for name in allowed
        .iter()
        .filter(|v| !matches!(**v, "receipt" | "recovery-token"))
    {
        required(&args, name)?;
    }
    if !Path::new(required(&args, "root")?).is_absolute() {
        return Err("Customer state needs an absolute private root.".into());
    }
    Ok(args)
}
fn secret(path: &str) -> Result<jev::ApiKey, String> {
    let bytes = Store::private_input(Path::new(path), 4096)?;
    let text = String::from_utf8(bytes).map_err(|_| "Invalid private credential input.")?;
    // Permit a file's single trailing newline, not embedded or leading whitespace.
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(&text);
    Ok(jev::ApiKey::new(text))
}
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.first().is_some_and(|word| word == "referral") {
        return referrals::run(output, &words[1..]);
    }
    if words.first().is_some_and(|word| word == "team") {
        return team::run(output, &words[1..]);
    }
    if words
        .first()
        .is_some_and(|v| matches!(v.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match parse(words) {
        Ok(v) => v,
        Err(e) => return output.usage("customer", &e, USAGE),
    };
    match crate::runtime().block_on(execute(&args)) {
        Ok((value, success)) => {
            output.emit(&value, |v| serde_json::to_string(v).unwrap_or_default());
            if success { 0 } else { 1 }
        }
        Err(e) => output.fail("customer", &e),
    }
}
async fn execute(args: &Args) -> Result<(Value, bool), String> {
    let mut store = Store::open(Path::new(required(args, "root")?))?;
    let command = args.positional()[0].as_str();
    let value = match command {
        "commercial" => {
            let product = match required(args, "product")? {
                "gateway" => receipts::purchase::CommercialProduct::Gateway,
                "plugin" => receipts::purchase::CommercialProduct::Plugin,
                _ => return Err("Select a supported native commercial product.".into()),
            };
            json!({"commercial":store.commercial_selection(product).await?})
        }
        "import" => {
            store
                .import_credential(required(args, "alias")?, &secret(required(args, "input")?)?)?;
            json!({"alias":required(args,"alias")?,"imported":true})
        }
        "account" => {
            let details = store
                .client(required(args, "origin")?, required(args, "alias")?)?
                .account()
                .details()
                .await
                .map_err(|_| "Authenticated account membership is unavailable.")?;
            json!({"account":details.account.id,"workspaces":details.workspaces.iter().map(|w|json!({"id":w.id,"tenant":w.tenant,"role":w.role,"kind":w.kind})).collect::<Vec<_>>()})
        }
        "select" => serde_json::to_value(
            store
                .select_account(
                    required(args, "origin")?,
                    required(args, "alias")?,
                    required(args, "account")?,
                    required(args, "workspace")?,
                    required(args, "door")?,
                )
                .await?,
        )
        .map_err(|_| "Customer selection encoding failed.")?,
        "current" => match store.current_selection().await {
            Ok(current) => {
                json!({"status":"available","stored":store.selected(),"current":current})
            }
            Err(_) => {
                return Ok((
                    json!({"status":"unavailable","stored":store.selected(),"history_retained":true}),
                    false,
                ));
            }
        },
        "history" => json!({"purchases":store.history()}),
        "funding-history" => json!({"funding":store.funding_history()}),
        "funding" => {
            let request: jev::DecisionFundingRequest = serde_json::from_slice(
                &Store::private_input(Path::new(required(args, "input")?), 32 * 1024)?,
            )
            .map_err(|_| "Invalid private funding intent.")?;
            serde_json::to_value(store.decision_funding(&request).await?)
                .map_err(|_| "Funding response encoding failed.")?
        }
        "show" => serde_json::to_value(store.show(required(args, "purchase")?)?)
            .map_err(|_| "Purchase view encoding failed.")?,
        "quote" => {
            let body = serde_json::from_slice(&Store::private_input(
                Path::new(required(args, "input")?),
                64 * 1024,
            )?)
            .map_err(|_| "Invalid private decision input.")?;
            serde_json::to_value(
                store
                    .create_quote(required(args, "purchase")?, body, now()?)
                    .await?,
            )
            .map_err(|_| "Quote encoding failed.")?
        }
        "approve" => serde_json::to_value(
            store
                .approve_quote(
                    required(args, "purchase")?,
                    required(args, "digest")?,
                    now()?,
                )
                .await?,
        )
        .map_err(|_| "Approval encoding failed.")?,
        "invoke" => {
            let (view, response) = store.invoke(required(args, "purchase")?, now()?).await?;
            json!({"purchase":view,"result":{"model":response.model,"answers":response.answers_value(),"usage":response.usage}})
        }
        "reconcile" => serde_json::to_value(
            store
                .reconcile(required(args, "purchase")?, args.option("receipt"))
                .await?,
        )
        .map_err(|_| "Reconciliation encoding failed.")?,
        "change" => {
            let command: CredentialCommand = serde_json::from_slice(&Store::private_input(
                Path::new(required(args, "input")?),
                32 * 1024,
            )?)
            .map_err(|_| "Invalid private credential intent.")?;
            let token = args.option("recovery-token").map(secret).transpose()?;
            let view = store.change_credential(command, token).await?;
            let applied = view.status == CredentialStatus::Applied;
            return Ok((
                serde_json::to_value(view).map_err(|_| "Credential view encoding failed.")?,
                applied,
            ));
        }
        "credentials" => json!({"operations":store.credential_history()}),
        "inspect" => {
            store
                .inspect_credential(required(args, "operation")?)
                .await?
        }
        _ => unreachable!(),
    };
    Ok((value, true))
}
fn now() -> Result<u64, String> {
    let value = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Customer clock is unavailable.")?
        .as_millis();
    u64::try_from(value).map_err(|_| "Customer clock exceeds its bound.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secrets_and_implicit_subjects_cannot_enter_command_flags() {
        let args = |v: &[&str]| v.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert!(
            parse(&args(&[
                "import",
                "--root",
                "/private-fixture",
                "--alias",
                "key",
                "--input",
                "private.key"
            ]))
            .is_ok()
        );
        for v in [
            vec![
                "import",
                "--root",
                "/private-fixture",
                "--alias",
                "key",
                "--token",
                "oak_private",
            ],
            vec![
                "select",
                "--root",
                "/private-fixture",
                "--origin",
                "https://fixture.invalid",
                "--alias",
                "key",
                "--workspace",
                "w",
                "--door",
                "d",
            ],
            vec!["history", "--root", "relative"],
            vec![
                "history",
                "--root",
                "/private-fixture",
                "--alias",
                "another-account",
            ],
        ] {
            assert!(parse(&args(&v)).is_err());
        }
    }
}
