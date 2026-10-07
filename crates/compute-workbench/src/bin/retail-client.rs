//! Native selected-client controls. No credential is accepted as a flag value.
use clap::{Parser, Subcommand};
use compute_workbench::retail::{Client, Result, read_task};
use route_contract::Digest;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    about = "Review, fund, and control one explicit retail service",
    version
)]
struct Args {
    /// Explicit private configuration; no HOME or environment fallback.
    #[arg(long)]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Account,
    Capacity,
    /// Request an invoice. Payment happens in the customer's separate wallet.
    TopUp {
        #[arg(long)]
        idempotency: String,
        #[arg(long)]
        amount_sats: u64,
    },
    TopUpStatus {
        purchase: String,
    },
    /// Review public source, exact checks, charge lines, and disclosure.
    Quote {
        #[arg(long)]
        idempotency: String,
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        provider_key: PathBuf,
    },
    /// Confirm only this exact retained review and explicit service custody.
    Confirm {
        #[arg(long)]
        review: String,
        #[arg(long)]
        provider_key: PathBuf,
        #[arg(long)]
        service_custody: bool,
    },
    Executions {
        #[arg(long)]
        after: Option<String>,
    },
    Reconnect {
        execution: String,
    },
    Progress {
        execution: String,
    },
    Cancel {
        execution: String,
    },
    Artifact {
        execution: String,
        name: String,
    },
    Receipt {
        execution: String,
    },
}
fn run(args: Args) -> Result<String> {
    let mut client = Client::from_file(&args.config)?;
    Ok(match args.command {
        Command::Account => client.account()?.lines(),
        Command::Capacity => serde_json::to_string_pretty(&client.capacity()?)?,
        Command::TopUp {
            idempotency,
            amount_sats,
        } => {
            let p = client.top_up(&idempotency, amount_sats)?;
            format!(
                "Purchase {}: {}\nAmount {}\nPay the exact invoice in your separately selected wallet; this client does not pay it.\nInvoice {}\nPayment hash {}\nExpires at {}\nPayment creates compute credits, not execution authority. Credits cannot be withdrawn as Lightning.",
                p.purchase,
                p.state,
                compute_workbench::credits(p.amount_msat),
                p.invoice,
                p.payment_hash,
                p.expires_at
            )
        }
        Command::TopUpStatus { purchase } => {
            serde_json::to_string_pretty(&client.top_up_status(&purchase)?)?
        }
        Command::Quote {
            idempotency,
            task,
            provider_key,
        } => client
            .quote(&idempotency, read_task(&task)?, &provider_key)?
            .lines(),
        Command::Confirm {
            review,
            provider_key,
            service_custody,
        } => serde_json::to_string_pretty(&client.confirm(
            &Digest::try_from(review).map_err(|_| {
                compute_workbench::retail::Error::Refused("review requires a canonical digest")
            })?,
            &provider_key,
            service_custody,
        )?)?,
        Command::Executions { after } => {
            serde_json::to_string_pretty(&client.executions(after.as_deref())?)?
        }
        Command::Reconnect { execution } => {
            serde_json::to_string_pretty(&client.reconnect(&execution)?)?
        }
        Command::Progress { execution } => {
            serde_json::to_string_pretty(&client.progress(&execution)?)?
        }
        Command::Cancel { execution } => {
            let result = client.cancel(&execution)?;
            format!(
                "{}\nStop request is separate from executor acknowledgment, sandbox deletion, and final settlement. Read receipt to reconcile those states.",
                serde_json::to_string_pretty(&result)?
            )
        }
        Command::Artifact { execution, name } => {
            serde_json::to_string_pretty(&client.artifact(&execution, &name)?)?
        }
        Command::Receipt { execution } => client.receipt(&execution)?.lines(),
    })
}
fn main() {
    match run(Args::parse()) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
