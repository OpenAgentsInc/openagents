//! Native original-purchase commissions. Commands never invoke a guest or pay.
use crate::{Args, Output};
use commercial_accounts::commission::Native;
use pay_ledger::Ledger;
use std::path::{Path, PathBuf};

pub(crate) const USAGE:&str="usage: openagents pay commission COMMAND --config FILE --ledger FILE --customer-root DIR --purchase ID
  admit --cost-policy FILE --receiver-home DIR
                          Freeze current bilateral terms on the original approved
                          purchase before payment. Costs are explicitly declared.
  cost-qualify --admission ID --cost-policy FILE
                          Fill previously unknown costs under an immutable exact
                          policy. Original known costs and terms cannot change.
  reconcile --admission ID --receiver-home DIR
                          Verify original outcome and native inbound collection;
                          accrue once or retain unknown costs and delivery holds.
  refund-prepare --admission ID --request ID --amount-msat N --buyer-home DIR --expiry SECS
                          Authenticate buyer and merchant owner, retain preparation,
                          then issue an exact invoice on the original buyer node.
                          This command sends no funds. Unknown creation cannot remint.
  refund-reconcile --refund ID --receiver-home DIR
                          Verify the original merchant's exact outbound refund,
                          reverse the same obligation once, or retain unknown holds.
  report --admission ID
                          Read this original purchase's private liability, payout,
                          reversal, loss, and retained-remainder reconciliation.
  abuse-review --admission ID --input FILE --approve DIGEST
                          Hold, release, or reject this original commission under
                          private reviewed rules and current merchant-owner grants.
All paths are explicit private native stores. No payment, reexecution, FX, or
commercial rate is inferred. Existing native wallet commands dispatch owner-
authorized transfers separately. Add --json before pay for one JSON document.";
fn required(args: &Args, key: &str) -> Result<String, String> {
    args.option(key)
        .map(str::to_owned)
        .ok_or_else(|| format!("--{key} is required"))
}
fn wallet(path: &Path) -> Result<openagents_wallet::resident::RemoteWallet, String> {
    if !path.is_absolute() {
        return Err("An explicit resident wallet home is required.".into());
    }
    openagents_wallet::resident::RemoteWallet::probe(path)
        .ok_or("The selected native resident is unavailable.".into())
}
pub(crate) fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_none_or(|s| matches!(s.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let command = &words[0];
    if !matches!(
        command.as_str(),
        "admit"
            | "cost-qualify"
            | "reconcile"
            | "refund-prepare"
            | "refund-reconcile"
            | "report"
            | "abuse-review"
    ) {
        return output.usage("pay commission", "unknown command", USAGE);
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let allowed: Vec<&str> = [
            vec!["config", "ledger", "customer-root", "purchase"],
            match command.as_str() {
                "admit" => vec!["cost-policy", "receiver-home"],
                "cost-qualify" => vec!["admission", "cost-policy"],
                "reconcile" => vec!["admission", "receiver-home"],
                "refund-prepare" => vec![
                    "admission",
                    "request",
                    "amount-msat",
                    "buyer-home",
                    "expiry",
                ],
                "refund-reconcile" => vec!["refund", "receiver-home"],
                "abuse-review" => vec!["admission", "input", "approve"],
                _ => vec!["admission"],
            },
        ]
        .concat();
        let args = crate::argv::parse_command(&words[1..], "pay commission", &allowed, &[], 0, 0)?;
        if args
            .option_names()
            .iter()
            .any(|name| args.options(name).len() != 1)
        {
            return Err("Duplicate commission option.".into());
        }
        let config_path = PathBuf::from(required(&args, "config")?);
        let ledger_path = PathBuf::from(required(&args, "ledger")?);
        let root = PathBuf::from(required(&args, "customer-root")?);
        let purchase = required(&args, "purchase")?;
        let admission = args.option("admission");
        let refund = args.option("refund");
        let policy = args.option("cost-policy");
        let receiver = args.option("receiver-home");
        let buyer = args.option("buyer-home");
        let request = args.option("request");
        let amount = args.option("amount-msat");
        let expiry = args.option("expiry");

        let native = Native::open_private(&config_path)?;
        let mut ledger = Ledger::open_native(&ledger_path).map_err(|e| e.to_string())?;
        let store = coder::customer::Store::open(&root)?;
        let source = store.plugin_commission_source(&purchase, command == "admit")?;
        let now = crate::relay::unix_now();
        let id = || admission.ok_or("--admission is required".to_string());
        let receiver = || wallet(Path::new(receiver.ok_or("--receiver-home is required")?));
        match command.as_str() {
            "abuse-review" => serde_json::to_value(native.review_abuse(
                &mut ledger,
                &source,
                id()?,
                Path::new(args.option("input").ok_or("--input is required")?),
                args.option("approve").ok_or("--approve is required")?,
                now,
            )?)
            .map_err(|_| "Abuse review encoding failed.".into()),
            "admit" => serde_json::to_value(native.admit(
                &mut ledger,
                &source,
                Path::new(policy.ok_or("--cost-policy is required")?),
                &receiver()?,
                now,
            )?)
            .map_err(|_| "Admission encoding failed.".into()),
            "cost-qualify" => {
                native.qualify_costs(
                    &mut ledger,
                    &source,
                    id()?,
                    Path::new(policy.ok_or("--cost-policy is required")?),
                )?;
                serde_json::to_value(native.report(&mut ledger, &source, id()?)?)
                    .map_err(|_| "Report encoding failed.".into())
            }
            "reconcile" => serde_json::to_value(native.reconcile(
                &mut ledger,
                &source,
                id()?,
                &receiver()?,
                now,
            )?)
            .map_err(|_| "Reconciliation encoding failed.".into()),
            "refund-prepare" => serde_json::to_value(
                native.prepare_refund(
                    &mut ledger,
                    &source,
                    id()?,
                    request.ok_or("--request is required")?,
                    amount
                        .ok_or("--amount-msat is required")?
                        .parse()
                        .map_err(|_| "Invalid refund amount.")?,
                    &wallet(Path::new(buyer.ok_or("--buyer-home is required")?))?,
                    now,
                    expiry
                        .ok_or("--expiry is required")?
                        .parse()
                        .map_err(|_| "Invalid refund expiry.")?,
                )?,
            )
            .map_err(|_| "Refund preparation encoding failed.".into()),
            "refund-reconcile" => serde_json::to_value(native.reconcile_refund(
                &mut ledger,
                &source,
                refund.ok_or("--refund is required")?,
                &receiver()?,
                now,
            )?)
            .map_err(|_| "Refund reconciliation encoding failed.".into()),
            _ => serde_json::to_value(native.report(&mut ledger, &source, id()?)?)
                .map_err(|_| "Report encoding failed.".into()),
        }
    })();
    match result {
        Ok(value) => {
            output.line(&value, |v| v.to_string());
            0
        }
        Err(reason) => output.fail("pay commission", &reason),
    }
}
