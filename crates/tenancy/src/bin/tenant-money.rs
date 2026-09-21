//! `tenant-money` — the operator's view of the monetary ledger.
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
//!
//! ```text
//! tenant-money --ledger PATH [--workspace NAME]
//! ```

use std::path::Path;

use tenancy::money::{Ledger, Phase};

fn usage() -> ! {
    eprintln!("usage:\n  tenant-money --ledger PATH [--workspace NAME]");
    std::process::exit(2);
}

fn amount(value: Option<u64>) -> String {
    value.map_or("-".to_string(), |amount| amount.to_string())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut ledger_path = None;
    let mut workspace = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ledger" => ledger_path = args.next(),
            "--workspace" => workspace = args.next(),
            _ => usage(),
        }
    }
    let Some(path) = ledger_path else { usage() };
    let ledger = match Ledger::open(Path::new(&path)) {
        Ok(ledger) => ledger,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };

    let names: Vec<String> = match workspace.as_deref() {
        Some(name) => vec![name.to_string()],
        None => ledger
            .workspaces()
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    };
    for name in names {
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
