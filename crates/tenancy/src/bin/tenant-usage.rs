//! `tenant-usage` — the operator's view of the quota ledger.
//!
//! Reads the registry and the ledger beside it, then prints each
//! tenant's position: what the budget charged today, what is still
//! held, and what recovery orphaned. Units are resources — requests,
//! questions, input bytes — never money; nothing here prices anything.
//!
//! Opening the ledger is also its recovery step: a reservation whose
//! deadline passed unsettled is written `orphaned` here as it would be
//! on any other open, so the numbers include the expired rather than
//! silently freeing them.
//!
//! ```text
//! tenant-usage --registry DIR [--tenant NAME]
//! ```

use std::path::Path;

use tenancy::{Registry, quota};

fn usage() -> ! {
    eprintln!("Usage:\n  tenant-usage --registry DIR [--tenant NAME]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut registry_dir = None;
    let mut tenant = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--registry" => registry_dir = args.next(),
            "--tenant" => tenant = args.next(),
            _ => usage(),
        }
    }
    let Some(dir) = registry_dir else { usage() };
    let dir = Path::new(&dir);

    let registry = match Registry::open(dir) {
        Ok(registry) => registry,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };
    let ledger = match quota::Ledger::open(dir) {
        Ok(ledger) => ledger,
        Err(trouble) => {
            eprintln!("{trouble}");
            std::process::exit(1);
        }
    };

    let names: Vec<&str> = match tenant.as_deref() {
        Some(name) => vec![name],
        None => registry
            .manifest()
            .tenants
            .keys()
            .map(String::as_str)
            .collect(),
    };
    for name in names {
        let usage = ledger.usage(name);
        println!(
            "{name}\tsettled {}\tquestions {}\tinput-bytes {}\toutstanding {}\torphaned {}",
            usage.settled_today,
            usage.questions_today,
            usage.input_bytes_today,
            usage.outstanding,
            usage.orphaned_today,
        );
        for held in ledger.outstanding(name) {
            println!(
                "  held\t{}\tattempt {}\t{} questions\t{} bytes\texpires {}",
                held.request,
                held.attempt,
                held.units.questions,
                held.units.input_bytes,
                held.expires_unix,
            );
        }
    }
}
