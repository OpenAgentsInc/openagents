//! `coder host spend`: ask the owner's phone to pay, and read the answers.

use std::path::Path;
use std::time::Duration;

use coder_access::RelayPolicy;
use coder_access::host::Host;
use coder_access::spend::{Context, Purpose, Receipt, Settlement};

use super::{Ask, Book, DEFAULT_TTL};

pub const USAGE: &str = "usage: coder host spend COMMAND [OPTIONS]
  request --invoice BOLT11 [--purpose x402_purchase|labor_payment|tip|transfer]
          [--fee-max-msat N] [--task ID] [--title TEXT] [--resource URI]
          [--note TEXT] [--ttl SECS] [--id HEX] [--wait SECS] [--json]
      Ask the phone that gave this computer a spend grant to pay a mainnet
      invoice. The owner approves or denies it on the phone; nothing pays
      without that tap. The fee ceiling is the lower of --fee-max-msat and
      the grant's for the amount. With --wait, wait up to SECS for the answer and print
      the receipt (a paid receipt carries the preimage); the exit code is 0
      only when it was paid.
  list [--json]              Every request this computer holds, newest first.
  show --request ID [--json] One request's receipt.
Every command also takes --state DIR (the access store, default
~/.openagents/coder-access).";

/// Run `coder host spend ARGS` against the access store at `state`.
pub fn run(args: &[String], state: &Path) -> u8 {
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return 2;
    };
    let mut options = match Flags::parse(rest) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("coder host spend: {message}\n\n{USAGE}");
            return 2;
        }
    };
    let state = options
        .take("--state")
        .map_or_else(|| state.to_path_buf(), Into::into);
    let result = match command.as_str() {
        "request" => request(&state, &mut options),
        "list" => list(&state, &mut options),
        "show" => show(&state, &mut options),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            return 0;
        }
        _ => Err(Usage(format!("unknown command `{command}`"))),
    };
    match result {
        Ok(code) => code,
        Err(Usage(message)) => {
            eprintln!("coder host spend: {message}\n\n{USAGE}");
            2
        }
    }
}

struct Usage(String);

struct Flags {
    values: Vec<(String, String)>,
    json: bool,
}

impl Flags {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut flags = Self {
            values: vec![],
            json: false,
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg == "--json" {
                flags.json = true;
            } else if arg.starts_with("--") {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                if flags.values.iter().any(|(name, _)| name == arg) {
                    return Err(format!("{arg} is given twice"));
                }
                flags.values.push((arg.clone(), value.clone()));
            } else {
                return Err(format!("unexpected argument `{arg}`"));
            }
        }
        Ok(flags)
    }

    fn take(&mut self, name: &str) -> Option<String> {
        let at = self.values.iter().position(|(n, _)| n == name)?;
        Some(self.values.remove(at).1)
    }

    fn number(&mut self, name: &str, default: u64) -> Result<u64, Usage> {
        self.take(name).map_or(Ok(default), |text| {
            text.parse()
                .map_err(|_| Usage(format!("{name} takes a whole number")))
        })
    }

    fn finish(&self) -> Result<(), Usage> {
        match self.values.first() {
            Some((name, _)) => Err(Usage(format!("{name} does not apply to this command"))),
            None => Ok(()),
        }
    }
}

fn now() -> u64 {
    coder_access::unix_time().unwrap_or_default()
}

fn request(state: &Path, flags: &mut Flags) -> Result<u8, Usage> {
    let payment = flags
        .take("--invoice")
        .ok_or_else(|| Usage("--invoice is required".into()))?;
    let purpose = match flags.take("--purpose") {
        Some(text) => {
            Purpose::parse(&text).ok_or_else(|| Usage(format!("unknown purpose `{text}`")))?
        }
        None => Purpose::X402Purchase,
    };
    let ask = Ask {
        payment,
        fee_max_msat: flags
            .take("--fee-max-msat")
            .map(|text| {
                text.parse()
                    .map_err(|_| Usage("--fee-max-msat takes a whole number".into()))
            })
            .transpose()?,
        purpose,
        context: Context {
            task: flags.take("--task"),
            title: flags.take("--title"),
            resource: flags.take("--resource"),
            note: flags.take("--note"),
        },
        ttl: flags.number("--ttl", DEFAULT_TTL)?,
        id: flags.take("--id"),
    };
    let wait = flags.number("--wait", 0)?;
    flags.finish()?;
    let host = match retry(|| Host::new(state, RelayPolicy::Production).public_key()) {
        Ok(host) => host,
        Err(error) => {
            eprintln!("coder host spend: this computer's host key is unavailable: {error}");
            return Ok(1);
        }
    };
    let book = Book::open(state);
    let request = match book.request(&host, &ask, now()) {
        Ok(request) => request,
        Err(refused) => {
            eprintln!("coder host spend: {refused}");
            return Ok(1);
        }
    };
    if wait == 0 {
        if flags.json {
            print_json(&serde_json::json!({ "request": request }));
        } else {
            println!(
                "asked the phone to pay {} msat; request {}; expires at {}",
                request.amount_msat, request.request, request.expires_at
            );
        }
        return Ok(0);
    }
    if !flags.json {
        eprintln!(
            "asked the phone to pay {} msat (request {}); waiting for the owner's answer",
            request.amount_msat, request.request
        );
    }
    let receipt = match book.wait(&request.request, Duration::from_secs(wait), now) {
        Ok(receipt) => receipt,
        Err(refused) => {
            eprintln!("coder host spend: {refused}");
            return Ok(1);
        }
    };
    report(&request.request, receipt.as_ref(), flags.json);
    Ok(u8::from(
        receipt.is_none_or(|receipt| receipt.outcome != Settlement::Paid),
    ))
}

fn report(request: &str, receipt: Option<&Receipt>, json: bool) {
    if json {
        print_json(&serde_json::json!({ "request": request, "receipt": receipt }));
        return;
    }
    match receipt {
        None => println!("no answer yet for request {request}"),
        Some(receipt) => match receipt.outcome {
            Settlement::Paid => println!(
                "paid {} msat, fee {} msat; preimage {}",
                receipt.amount_msat.unwrap_or_default(),
                receipt
                    .fees_msat
                    .map_or_else(|| "unknown".into(), |fee| fee.to_string()),
                receipt.proof.as_deref().unwrap_or_default()
            ),
            Settlement::Refused => println!(
                "not paid: {}",
                receipt.code.map_or("refused", |code| code.describe())
            ),
            Settlement::Pending => println!("the payment is pending on the phone"),
            Settlement::Unknown => println!("the phone could not tell whether it paid"),
        },
    }
}

fn list(state: &Path, flags: &mut Flags) -> Result<u8, Usage> {
    flags.finish()?;
    let entries = match Book::open(state).entries(now()) {
        Ok(entries) => entries,
        Err(refused) => {
            eprintln!("coder host spend: {refused}");
            return Ok(1);
        }
    };
    if flags.json {
        print_json(&serde_json::json!(entries));
        return Ok(0);
    }
    for entry in entries {
        let answer = match &entry.receipt {
            None => "waiting".to_owned(),
            Some(receipt) => match (receipt.outcome, receipt.code) {
                (Settlement::Refused, Some(code)) => format!("refused ({code:?})"),
                (outcome, _) => format!("{outcome:?}").to_lowercase(),
            },
        };
        println!(
            "{} {} msat {} {}",
            entry.request.request,
            entry.request.amount_msat,
            entry.request.purpose.as_str(),
            answer
        );
    }
    Ok(0)
}

fn show(state: &Path, flags: &mut Flags) -> Result<u8, Usage> {
    let request = flags
        .take("--request")
        .ok_or_else(|| Usage("--request is required".into()))?;
    flags.finish()?;
    match Book::open(state).receipt(&request, now()) {
        Ok(receipt) => {
            report(&request, receipt.as_ref(), flags.json);
            Ok(0)
        }
        Err(refused) => {
            eprintln!("coder host spend: {refused}");
            Ok(1)
        }
    }
}

fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn retry<T>(mut operation: impl FnMut() -> coder_access::Result<T>) -> coder_access::Result<T> {
    let started = std::time::Instant::now();
    loop {
        match operation() {
            Err(error)
                if error.code == coder_access::Code::Conflict
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}
