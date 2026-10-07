//! Explicit private operator commands; no default wallet or training engine.

use clap::{Parser, Subcommand};
use contribution_service::{Host, load_config};
use openagents_wallet::resident::RemoteWallet;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(about = "Verify one protected checkpoint obligation and its exact central funding")]
struct Args {
    #[arg(long)]
    config: PathBuf,
    /// An explicitly admitted resident wallet home. No home-directory fallback.
    #[arg(long)]
    wallet_home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Recompute protected acceptance and known costs without issuing an invoice.
    Assess,
    /// Retain one exact invoice intent, then ask the configured central receiver.
    Prepare,
    /// Look up only the original invoice and accrue its funded liability once.
    Reconcile,
    /// Read local funding, central liability, and exact payout attempts.
    Statement,
    /// Retain a new private REV-25 reporting snapshot after funded acceptance.
    Finance {
        #[arg(long)]
        output: PathBuf,
    },
}
fn run(args: Args) -> Result<serde_json::Value, String> {
    let mut host = Host::open(load_config(&args.config)?)?;
    if matches!(args.command, Command::Statement) {
        return host.statement();
    }
    let home = args
        .wallet_home
        .filter(|p| p.is_absolute())
        .ok_or("an explicit absolute resident wallet home is required")?;
    let wallet = RemoteWallet::probe(&home)
        .ok_or("the explicitly selected central resident is unavailable")?;
    let now = unix_seconds()?;
    let result = match args.command {
        Command::Assess => serde_json::to_value(host.assess(&wallet, now)?),
        Command::Prepare => serde_json::to_value(host.prepare(&wallet, now)?),
        Command::Reconcile => serde_json::to_value(host.reconcile(&wallet, unix_seconds)?),
        Command::Finance { output } => {
            serde_json::to_value(host.export_finance(&wallet, now, &output)?)
        }
        Command::Statement => unreachable!(),
    };
    result.map_err(|_| "contribution result cannot be encoded".into())
}
fn unix_seconds() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is unavailable")?
            .as_secs(),
    )
    .map_err(|_| "system time overflow".into())
}
fn main() {
    match run(Args::parse()) {
        Ok(value) => println!("{value}"),
        Err(reason) => {
            eprintln!("{reason}");
            std::process::exit(1);
        }
    }
}
