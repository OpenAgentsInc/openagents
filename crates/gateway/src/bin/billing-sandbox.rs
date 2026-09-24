//! `billing-sandbox` — the sandbox provider's operator half.
//!
//! The sandbox provider keeps a journal of provider-side events in
//! `billing-provider.jsonl` beside the registry. Emitting an event
//! writes it there — "the provider processed this" — and prints the
//! body plus the `x-openagents-billing-signature` header value that a
//! real provider's webhook would deliver. Delivering it is a second,
//! separate step, so a lost delivery is testable:
//!
//! ```text
//! # Provider side — record the event.
//! billing-sandbox --registry <dir> emit checkout-completed \
//!     --checkout cko_… --provider-ref ps_…
//! # Delivered side — sign and post the printed body to
//! # POST /v1/billing/webhook with the printed signature header, or
//! # skip the delivery to exercise reconciliation.
//! ```
//!
//! The HMAC secret comes from the same environment variable the
//! gateway reads — a name, never a file the command stores.

use std::path::PathBuf;
use std::process::ExitCode;

use tenancy::billing::Event;

/// The command's arguments.
struct Options {
    /// The registry directory holding `billing-provider.jsonl`.
    registry: PathBuf,
    /// The event kind — `checkout-completed`, `invoice-paid`, and so on.
    kind: String,
    /// Optional event fields, as `name=value` pairs.
    fields: Vec<(String, String)>,
    /// The environment variable holding the webhook secret.
    secret_env: String,
    /// Emit without journaling — print a signed body only.
    print_only: bool,
}

/// Parse the arguments.
fn options(args: &[String]) -> Result<Options, String> {
    let mut registry = None;
    let mut kind = None;
    let mut fields = Vec::new();
    let mut secret_env = "OPENAGENTS_BILLING_SECRET".to_string();
    let mut print_only = false;
    let mut args = args.iter().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--registry" => {
                registry = Some(
                    args.next()
                        .ok_or_else(|| "--registry names a directory".to_string())?
                        .into(),
                );
            }
            "--secret-env" => {
                secret_env = args
                    .next()
                    .ok_or_else(|| "--secret-env names a variable".to_string())?
                    .clone();
            }
            "--print" => print_only = true,
            "emit" => {
                kind = Some(
                    args.next()
                        .ok_or_else(|| "`emit` needs an event kind".to_string())?
                        .clone(),
                );
            }
            other if other.starts_with("--") => {
                let name = other.trim_start_matches('-');
                let value = args
                    .next()
                    .ok_or_else(|| format!("--{name} names a value"))?;
                fields.push((name.to_string(), value.clone()));
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Options {
        registry: registry.ok_or_else(|| "--registry is required".to_string())?,
        kind: kind.ok_or_else(|| "Name an event kind: `emit <kind>`".to_string())?,
        fields,
        secret_env,
        print_only,
    })
}

/// The current time as Unix seconds.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// A fresh event id — `evt_<hex>` through the book's own entropy.
fn event_id() -> Result<String, String> {
    tenancy::billing::fresh_ref()
        .map(|hex| format!("evt_{hex}"))
        .map_err(|_| "entropy unavailable".to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let options = match options(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("billing-sandbox: {error}");
            eprintln!(
                "usage: billing-sandbox --registry <dir> emit <kind> \
                 [--<field> <value>…] [--secret-env <name>] [--print]"
            );
            return ExitCode::from(2);
        }
    };
    let mut event = Event {
        provider: "sandbox".to_string(),
        id: match event_id() {
            Ok(id) => id,
            Err(error) => {
                eprintln!("billing-sandbox: entropy unavailable: {error}");
                return ExitCode::from(1);
            }
        },
        kind: options.kind.clone(),
        checkout: None,
        subscription: None,
        invoice: None,
        period: 0,
        amount: 0,
        currency: None,
        at_period_end: true,
        provider_ref: None,
        received: 0,
        applied: false,
        outcome: String::new(),
    };
    for (name, value) in &options.fields {
        match name.as_str() {
            "id" => event.id = value.clone(),
            "checkout" => event.checkout = Some(value.clone()),
            "subscription" => event.subscription = Some(value.clone()),
            "invoice" => event.invoice = Some(value.clone()),
            "period" => {
                event.period = match value.parse() {
                    Ok(period) => period,
                    Err(_) => {
                        eprintln!("billing-sandbox: --period takes a whole number");
                        return ExitCode::from(2);
                    }
                };
            }
            "amount" => {
                event.amount = match value.parse() {
                    Ok(amount) => amount,
                    Err(_) => {
                        eprintln!("billing-sandbox: --amount takes a whole number of millionths");
                        return ExitCode::from(2);
                    }
                };
            }
            "currency" => event.currency = Some(value.clone()),
            "at-period-end" => event.at_period_end = value == "true",
            "provider-ref" => event.provider_ref = Some(value.clone()),
            other => {
                eprintln!("billing-sandbox: unknown field `--{other}`");
                return ExitCode::from(2);
            }
        }
    }
    if !options.print_only
        && let Err(error) = gateway::billing::sandbox::emit(&options.registry, &event)
    {
        eprintln!("billing-sandbox: the provider journal refused the event: {error}");
        return ExitCode::from(1);
    }
    let body = match serde_json::to_vec(&event) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("billing-sandbox: {error}");
            return ExitCode::from(1);
        }
    };
    let timestamp = unix_now();
    let signature = match std::env::var(&options.secret_env) {
        Ok(secret) if !secret.is_empty() => gateway::billing::sign(&secret, timestamp, &body),
        _ => {
            eprintln!(
                "billing-sandbox: `{}` is unset — the event is journaled \
                 but cannot be signed for delivery",
                options.secret_env
            );
            String::new()
        }
    };
    let output = serde_json::json!({
        "journal": if options.print_only { "skipped" } else { "written" },
        "body": serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        "signature_header": format!("t={timestamp},v1={signature}"),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    ExitCode::SUCCESS
}
