//! `tenant-money` — the operator's view and audited mutation path.
//!
//! Reads a `tenancy::money` ledger and prints each workspace account's
//! position: what was credited, what is still reserved, what settled,
//! what was refunded, what remains available and authorized to spend,
//! and the price versions the account has transacted under. Amounts are
//! integer millionths of the account's currency — the ledger's own
//! units, printed unscaled so a reader compares them with the price
//! schedule rather than a reformatted number.
//!
//! Each hold lists its phase, so an outstanding liability reads as
//! outstanding rather than vanishing from the view the way a settled
//! charge never vanishes either. `unknown` marks a completion the
//! ledger cannot yet account for; it is still owed until reconciled.
//!
//! Opening the ledger takes its exclusive lock and replays its chain —
//! the same open any writer performs, so a corrupt tail or a held lock
//! fails the read rather than printing numbers the file cannot vouch
//! for.
//! `--apply` records one privileged local mutation; `--json` reads funding,
//! policy, credit provenance, and an explicit accounting snapshot time.
//!
//! ```text
//! tenant-money --ledger PATH [--workspace NAME] [--json] [--apply MUTATION.json]
//! ```

use std::io::Read;
use std::path::Path;

use tenancy::money::{Ledger, Mutation, Phase};

fn usage() -> ! {
    eprintln!(
        "Usage:\n  tenant-money --ledger PATH [--workspace NAME] [--json] [--apply MUTATION.json]"
    );
    std::process::exit(2);
}

fn amount(value: Option<u64>) -> String {
    value.map_or("-".to_string(), |amount| amount.to_string())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut ledger_path = None;
    let mut workspace = None;
    let mut apply_path = None;
    let mut json = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ledger" => ledger_path = args.next(),
            "--workspace" => workspace = Some(args.next().unwrap_or_else(|| usage())),
            "--apply" => apply_path = Some(args.next().unwrap_or_else(|| usage())),
            "--json" => json = true,
            _ => usage(),
        }
    }
    let Some(path) = ledger_path else { usage() };
    let mut ledger = match Ledger::open(Path::new(&path)) {
        Ok(ledger) => ledger,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };

    // This is a privileged local operator action against the protected ledger,
    // never a route an inference caller or executor can invoke.
    if let Some(path) = apply_path {
        let applied = (|| -> Result<bool, String> {
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 1024 * 1024 {
                return Err("money mutation exceeds 1 MiB".into());
            }
            let mutation: Mutation = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if workspace
                .as_ref()
                .is_some_and(|name| name != &mutation.workspace)
            {
                return Err("mutation workspace differs from the selected workspace".into());
            }
            ledger.apply(mutation)
        })();
        match applied {
            Ok(changed) => eprintln!("mutation applied: {changed}"),
            Err(trouble) => {
                eprintln!("{trouble}");
                std::process::exit(1);
            }
        }
    }

    let names: Vec<String> = match workspace.as_deref() {
        Some(name) => vec![name.to_string()],
        None => ledger
            .workspaces()
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    };
    for name in names {
        if json {
            match ledger
                .statement(&name)
                .and_then(|statement| serde_json::to_string(&statement).map_err(|e| e.to_string()))
            {
                Ok(statement) => println!("{statement}"),
                Err(trouble) => {
                    eprintln!("{name}: {trouble}");
                    std::process::exit(1);
                }
            }
            continue;
        }
        let balance = match ledger.balance(&name) {
            Ok(balance) => balance,
            Err(trouble) => {
                eprintln!("{name}: {trouble}");
                continue;
            }
        };
        println!(
            "{name}\tcurrency {}\tcredited {}\treserved {}\tsettled {}\trefunded {}\tavailable {}\tspend-remaining {}\tprices {}",
            balance.currency,
            balance.credited,
            balance.reserved,
            balance.settled,
            balance.refunded,
            balance.available,
            balance.spend_remaining,
            balance.price_versions.join(",")
        );
        if !balance.funding_policy_versions.is_empty() {
            println!(
                "  funding\tpurchased {}\tpromotional {}\treversed {}\texpired {}\trestricted {}\toperator-loss {}\tuncovered-holds {}\twallet-liquidity unknown\tpolicies {}",
                balance.purchased_funding,
                balance.promotional_credit,
                balance.reversed_credit,
                balance.expired_credit,
                balance.restricted_credit,
                balance.operator_loss,
                balance.uncovered_holds,
                balance.funding_policy_versions.join(",")
            );
        }
        for (attempt, hold) in ledger.holds(&name) {
            let phase = match hold.phase {
                Phase::Held => "held",
                Phase::Unknown => "unknown",
                Phase::Settled => "settled",
                Phase::Released => "released",
            };
            println!(
                "  {phase}\t{attempt}\treserved {}\tretail {}\trefunded {}\tprovider {}\thosting {}\tprice {}\treceipt {}",
                hold.reserved,
                amount(hold.retail_charge),
                hold.refunded,
                amount(hold.provider_cost),
                amount(hold.hosting_cost),
                hold.price.version,
                hold.receipt.as_deref().unwrap_or("-"),
            );
        }
    }
}
