//! `openagents wallet`: the person's Spark wallet on this computer, the same
//! wallet as the phone's (owner decision, 2026-10-02, `docs/breez/README.md`),
//! answered in plain words.
//!
//! A balance is one sentence ("Your balance is ₿12,000 (0.00012000 BTC).")
//! and an address is one line ("Your address: …"). No answer names a node,
//! a network, a chain server, channels, liquidity, or millisatoshis; the
//! x402 receiver's Lightning node is separate, under `openagents x402 node`.
//! `--json` keeps machine fields.
//!
//! The wallet and its seed live in `~/.openagents/spark`
//! (`openagents_spark::computer`). The seed arrives from the phone
//! (`link`, sealed to a one-time key after the owner approves on the phone)
//! or from the recovery words (`restore`, typed without echo). No command
//! prints the seed or the words.

use std::io::{BufRead, IsTerminal, Write};
use std::time::Duration;

use bitcoin_amount::Format;
use openagents_spark::computer;
use openagents_spark::model::{Destination, Node, PaymentRow, QuoteFailure, SendRequest};
use openagents_spark::seed::Seed;
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents wallet COMMAND [OPTIONS]
  balance                 Your balance, in one plain sentence.
  address                 Your address to get paid at.
  receive [--amount N] [--bitcoin]
                          A payment request to share: a Lightning invoice,
                          for N if given; --bitcoin gives a Bitcoin address
                          to send to instead.
  send TO [--amount N] [--yes]
                          Pay TO: a Lightning invoice, a Lightning address,
                          or a Spark or Bitcoin address. Shows the amount
                          and fee and asks before paying; --yes pays
                          without asking.
  history [--limit N]     Your recent payments, newest first.
  link [--replace] [--wait SECONDS]
                          Use your phone's wallet on this computer. Your
                          phone asks you to approve and shows a code to
                          check against this screen (default wait 600).
  restore [--replace]     Use the wallet for your recovery words, typed
                          without showing them.
This is the same wallet as the OpenAgents app on your phone: one balance on
every device. Amounts show as ₿12,345 (BIP 177), with BTC beside them; set
OPENAGENTS_AMOUNT_FORMAT=btc to show BTC first and type amounts in BTC.
Add --json before `wallet` for one JSON document.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("balance", Effect::ReadOnly, "wallet"),
    Declared::screen("address", Effect::ReadOnly, "wallet"),
    Declared::screen("receive", Effect::LocalWrite, "wallet"),
    Declared::screen("send", Effect::Spends, "wallet"),
    Declared::screen("history", Effect::ReadOnly, "wallet"),
    Declared::screen("link", Effect::Secret, "wallet"),
    Declared::screen("restore", Effect::Secret, "wallet"),
];

/// Words a wallet answer never contains: the technical detail the owner
/// called noise (#10170's `wallet info` printed all of them).
#[cfg(test)]
pub(crate) const TECHNICAL: &[&str] = &[
    "node",
    "testnet",
    "signet",
    "regtest",
    "esplora",
    "channel",
    "liquidity",
    "inbound",
    "outbound",
    "msat",
    "ldk",
    "lsp",
];

const SWITCHES: &[&str] = &["bitcoin", "yes", "replace"];
/// How many payments `history` lists unless asked.
const HISTORY: u32 = 10;
/// How long `link` waits for the phone unless asked.
const LINK_WAIT: u64 = 600;

type Render = Box<dyn FnOnce(&Value) -> String>;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("wallet", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("wallet", &message, USAGE),
    };
    let format = Format::from_env();
    let result: Result<(Value, Render), Failure> = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        "balance" => balance().map(|sats| {
            (
                json!({ "balance_sats": sats }),
                Box::new(move |_: &Value| balance_line(sats, format)) as Render,
            )
        }),
        "address" => address().map(|address| {
            let line = address_line(&address);
            (
                json!({ "address": address }),
                Box::new(move |_: &Value| line) as Render,
            )
        }),
        "receive" => receive(&args, format),
        "send" => send(output, &args, format),
        "history" => history(&args, format),
        "link" => link(output, &args),
        "restore" => restore(&args),
        other => return output.usage("wallet", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok((value, render)) => {
            output.emit(&value, render);
            0
        }
        Err(Failure::Usage(message)) => output.usage("wallet", &message, USAGE),
        Err(Failure::Plain(message)) => output.fail("wallet", &message),
    }
}

enum Failure {
    /// The command line was wrong.
    Usage(String),
    /// What went wrong, in plain words.
    Plain(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Plain(message)
    }
}

/// The balance as a person reads it: the chosen format, then the other.
pub(crate) fn balance_line(sats: u64, format: Format) -> String {
    format!(
        "Your balance is {} ({}).",
        format.show(sats),
        format.other().show(sats)
    )
}

/// An address to pay, alone on its line.
pub(crate) fn address_line(address: &str) -> String {
    format!("Your address: {address}")
}

/// This computer's wallet, open and synced.
fn opened() -> Result<openagents_spark::spark::SparkNode, Failure> {
    let node = computer::open(&computer::home()).map_err(Failure::Plain)?;
    node.sync().map_err(|_| Failure::Plain(UNREADABLE.into()))?;
    Ok(node)
}

/// What a failed read says: plain, with no internals.
const UNREADABLE: &str =
    "The wallet could not be read right now. Check the connection and try again in a minute.";

fn balance() -> Result<u64, Failure> {
    opened()?
        .balance()
        .map_err(|_| Failure::Plain(UNREADABLE.into()))
}

fn address() -> Result<String, Failure> {
    let node = computer::open(&computer::home()).map_err(Failure::Plain)?;
    node.spark_address()
        .map_err(|_| Failure::Plain(UNREADABLE.into()))
}

fn amount(args: &Args, format: Format) -> Result<Option<u64>, Failure> {
    match args.option("amount") {
        None => Ok(None),
        Some(text) => bitcoin_amount::parse(text, format)
            .map_err(|error| Failure::Usage(error.message(format))),
    }
}

fn receive(args: &Args, format: Format) -> Result<(Value, Render), Failure> {
    let amount = amount(args, format)?;
    let node = computer::open(&computer::home()).map_err(Failure::Plain)?;
    if args.switch("bitcoin") {
        if amount.is_some() {
            return Err(Failure::Usage(
                "--amount is for a Lightning invoice; a Bitcoin address takes any amount".into(),
            ));
        }
        let address = node
            .bitcoin_address()
            .map_err(|_| Failure::Plain(UNREADABLE.into()))?;
        let line = format!(
            "Send bitcoin to: {address}\nIt reaches your balance after the Bitcoin network confirms it."
        );
        return Ok((
            json!({ "bitcoin_address": address }),
            Box::new(move |_: &Value| line),
        ));
    }
    let invoice = node
        .invoice(amount, "OpenAgents")
        .map_err(|_| Failure::Plain(UNREADABLE.into()))?;
    let line = match amount {
        Some(sats) => format!("Ask to be paid {} with: {invoice}", format.show(sats)),
        None => format!("Ask to be paid any amount with: {invoice}"),
    };
    Ok((
        json!({ "invoice": invoice, "amount_sats": amount }),
        Box::new(move |_: &Value| line),
    ))
}

/// Who a payment goes to, for one line.
fn recipient(destination: &Destination) -> String {
    match destination {
        Destination::LightningAddress(address) => address.clone(),
        Destination::Lightning(text) | Destination::Spark(text) | Destination::Bitcoin(text) => {
            shorten(text)
        }
    }
}

fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 28 {
        return text.to_owned();
    }
    let head: String = chars[..14].iter().collect();
    let tail: String = chars[chars.len() - 10..].iter().collect();
    format!("{head}…{tail}")
}

/// The line that asks before paying.
pub(crate) fn confirm_line(to: &str, amount: u64, fee: u64, format: Format) -> String {
    format!(
        "Send {} to {to}? The fee is {}, {} in all.",
        format.show(amount),
        format.show(fee),
        format.show(amount.saturating_add(fee))
    )
}

fn send(output: &Output, args: &Args, format: Format) -> Result<(Value, Render), Failure> {
    let Some(to) = args.positional().first().cloned() else {
        return Err(Failure::Usage(
            "send needs who to pay: an invoice or an address".into(),
        ));
    };
    let amount = amount(args, format)?;
    let node = opened()?;
    let quote = match node.quote(&SendRequest {
        input: to,
        amount_sats: amount,
        comment: None,
        format,
    }) {
        Ok(quote) => quote,
        Err(QuoteFailure::NeedsAmount(ask)) => {
            return Err(Failure::Usage(format!("{} Add --amount N.", ask.message)));
        }
        Err(QuoteFailure::Refused(message)) => return Err(Failure::Plain(message)),
    };
    let who = recipient(&quote.destination);
    let question = confirm_line(&who, quote.amount_sats, quote.fee_sats, format);
    if !args.switch("yes") {
        if output.json() || !std::io::stdin().is_terminal() {
            return Err(Failure::Plain(format!(
                "{question} Nothing was sent. Run it again with --yes to send."
            )));
        }
        eprint!("{question} Type yes to send: ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().lock().read_line(&mut answer);
        if !answer.trim().eq_ignore_ascii_case("yes") && !answer.trim().eq_ignore_ascii_case("y") {
            return Err(Failure::Plain("Nothing was sent.".into()));
        }
    }
    let key = uuid::Uuid::new_v4().to_string();
    let paid = node.pay(quote.id, &key).map_err(Failure::Plain)?;
    let row = paid.row;
    let line = match row.status.as_str() {
        "completed" => format!("Sent {} to {who}.", format.show(row.amount_sats)),
        "pending" => format!(
            "Sending {} to {who}. It is on its way; `openagents wallet history` shows when it lands.",
            format.show(row.amount_sats)
        ),
        _ => format!("The payment to {who} did not go through. Nothing was sent."),
    };
    let line = match paid.message {
        Some(message) => format!("{line}\nThey said: {message}"),
        None => line,
    };
    Ok((
        json!({
            "payment": row.id,
            "status": row.status,
            "amount_sats": row.amount_sats,
            "fee_sats": row.fee_sats,
        }),
        Box::new(move |_: &Value| line),
    ))
}

/// One payment, for one line.
pub(crate) fn payment_line(row: &PaymentRow, format: Format) -> String {
    let amount = format.show_signed(row.amount_sats, row.received);
    let what = if row.received { "Received" } else { "Sent" };
    let when = date(row.at);
    let status = match row.status.as_str() {
        "pending" => " (on its way)",
        "failed" => " (did not go through)",
        _ => "",
    };
    format!("{when}  {amount:>14}  {what}{status}")
}

/// `2026-10-02` for Unix seconds, in UTC.
fn date(at: u64) -> String {
    let days = i64::try_from(at / 86_400).unwrap_or(0);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn history(args: &Args, format: Format) -> Result<(Value, Render), Failure> {
    let limit = match args.option("limit") {
        Some(text) => text
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=200).contains(n))
            .ok_or_else(|| Failure::Usage("--limit takes 1 to 200".into()))?,
        None => HISTORY,
    };
    let rows = opened()?
        .payments(limit)
        .map_err(|_| Failure::Plain(UNREADABLE.into()))?;
    let value = json!({
        "payments": rows.iter().map(|row| json!({
            "id": row.id,
            "received": row.received,
            "amount_sats": row.amount_sats,
            "fee_sats": row.fee_sats,
            "status": row.status,
            "at": row.at,
        })).collect::<Vec<_>>(),
    });
    let text = if rows.is_empty() {
        "No payments yet.".to_owned()
    } else {
        rows.iter()
            .map(|row| payment_line(row, format))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok((value, Box::new(move |_: &Value| text)))
}

/// Keep `seed` as this computer's wallet and say so, with the balance when
/// Spark answers.
fn adopt(seed: &Seed, replace: bool) -> Result<(Value, Render), Failure> {
    let home = computer::home();
    computer::save_seed(&home, seed, replace).map_err(Failure::Plain)?;
    let balance = computer::open(&home)
        .ok()
        .and_then(|node| node.sync().ok().and_then(|()| node.balance().ok()));
    let format = Format::from_env();
    let line = match balance {
        Some(sats) => format!(
            "Your wallet is on this computer now. {}",
            balance_line(sats, format)
        ),
        None => "Your wallet is on this computer now.".to_owned(),
    };
    Ok((
        json!({ "linked": true, "balance_sats": balance }),
        Box::new(move |_: &Value| line),
    ))
}

fn restore(args: &Args) -> Result<(Value, Render), Failure> {
    let home = computer::home();
    if computer::has_seed(&home) && !args.switch("replace") {
        return Err(Failure::Plain(
            "This computer already has a wallet. Run the command again with --replace to use another one; the current one's recovery words are the only way back to it.".into(),
        ));
    }
    let words = if std::io::stdin().is_terminal() {
        eprint!("Type your 12 or 24 recovery words, then press Enter (they won't show): ");
        let _ = std::io::stderr().flush();
        let words = read_hidden().map_err(|_| {
            Failure::Plain("The recovery words could not be read from this terminal.".into())
        })?;
        eprintln!();
        words
    } else {
        let mut words = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut words)
            .map_err(|_| Failure::Plain("The recovery words could not be read.".into()))?;
        words
    };
    let entropy = openagents_spark::seed::restore_entropy(&words).map_err(Failure::Plain)?;
    let entropy = unhex(&entropy)
        .ok_or_else(|| Failure::Plain("The recovery words can't be read.".into()))?;
    let seed = Seed::from_entropy(entropy).map_err(Failure::Plain)?;
    adopt(&seed, args.switch("replace"))
}

/// A line from the terminal with echo off.
fn read_hidden() -> std::io::Result<String> {
    use std::os::fd::AsRawFd;
    let stdin = std::io::stdin();
    let fd = stdin.as_raw_fd();
    // SAFETY: `termios` is plain data that `tcgetattr` fills in for `fd`.
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is this process's stdin, a terminal, and `saved` is valid.
    if unsafe { libc::tcgetattr(fd, &mut saved) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut quiet = saved;
    quiet.c_lflag &= !libc::ECHO;
    quiet.c_lflag |= libc::ECHONL;
    // SAFETY: as above; `quiet` is `saved` with echo turned off.
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &quiet) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut line = String::new();
    let read = stdin.lock().read_line(&mut line);
    // SAFETY: restores the settings read above.
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &saved) };
    read.map(|_| line)
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// This computer's name, as the phone shows it.
fn computer_name() -> String {
    let mut buffer = [0_u8; 256];
    // SAFETY: gethostname writes at most `buffer.len()` bytes into `buffer`.
    let status = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    let name = if status == 0 {
        let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
        String::from_utf8_lossy(&buffer[..end]).into_owned()
    } else {
        String::new()
    };
    let name = name.strip_suffix(".local").unwrap_or(&name);
    let name: String = name.chars().filter(|c| !c.is_control()).take(48).collect();
    if name.is_empty() {
        "this computer".to_owned()
    } else {
        name
    }
}

fn link(output: &Output, args: &Args) -> Result<(Value, Render), Failure> {
    let home = computer::home();
    let replace = args.switch("replace");
    if computer::has_seed(&home) && !replace {
        return Err(Failure::Plain(
            "This computer already has a wallet. Run the command again with --replace to use your phone's instead.".into(),
        ));
    }
    let wait = match args.option("wait") {
        Some(text) => text
            .parse::<u64>()
            .ok()
            .filter(|n| (10..=3600).contains(n))
            .ok_or_else(|| Failure::Usage("--wait takes 10 to 3600 seconds".into()))?,
        None => LINK_WAIT,
    };
    let state = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| Failure::Plain("HOME is not set.".into()))?
        .join(".openagents/coder-access");
    let host_runs = coder_access::host::Host::new(&state, coder_access::RelayPolicy::Production)
        .public_key()
        .is_ok();
    if !host_runs {
        return Err(Failure::Plain(
            "Your phone reaches this computer through its OpenAgents host, and this computer has none yet. Connect it to your phone first (`openagents connect`), or use `openagents wallet restore` to type your recovery words.".into(),
        ));
    }
    let requester = openagents_spark::link::Requester::new().map_err(Failure::Plain)?;
    let book = coder_host::wallet_link::Book::open(&state);
    let name = computer_name();
    let id = book
        .ask(&requester.public_hex(), &name, wait, now())
        .map_err(|_| Failure::Plain("The request for your phone could not be saved.".into()))?;
    let code = requester.code();
    if output.json() {
        println!(
            "{}",
            json!({ "waiting": true, "computer": name, "code": code })
        );
    } else {
        eprintln!(
            "Open the OpenAgents app on your phone. It will ask to use your wallet on this computer.\nApprove it only if it shows the code {code}."
        );
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(wait);
    let answer = loop {
        match book.answer(&id, now()) {
            Ok(Some(answer)) => break answer,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_secs(1));
            }
            Ok(None) => break coder_host::wallet_link::Answer::Expired,
            Err(_) => {
                let _ = book.forget(&id);
                return Err(Failure::Plain(
                    "The request for your phone could not be read.".into(),
                ));
            }
        }
    };
    let _ = book.forget(&id);
    match answer {
        coder_host::wallet_link::Answer::Sealed(sealed) => {
            let sealed: openagents_spark::link::Sealed = serde_json::to_value(&sealed)
                .and_then(serde_json::from_value)
                .map_err(|_| Failure::Plain("The phone's reply could not be read.".into()))?;
            let seed = requester.open(&sealed).map_err(Failure::Plain)?;
            adopt(&seed, replace)
        }
        coder_host::wallet_link::Answer::Declined => Err(Failure::Plain(
            "Your phone declined. This computer's wallet is unchanged.".into(),
        )),
        coder_host::wallet_link::Answer::Expired => Err(Failure::Plain(
            "Your phone didn't answer in time. Open the OpenAgents app on your phone with this computer connected, then run `openagents wallet link` again.".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) {
        let lower = text.to_lowercase();
        for word in TECHNICAL {
            assert!(!lower.contains(word), "{text:?} names {word}");
        }
    }

    #[test]
    fn a_balance_is_one_plain_sentence() {
        let text = balance_line(12_000, Format::Bip177);
        assert_eq!(text, "Your balance is ₿12,000 (0.00012000 BTC).");
        plain(&text);
        assert_eq!(
            balance_line(12_000, Format::LegacyBtc),
            "Your balance is 0.00012000 BTC (₿12,000)."
        );
        assert_eq!(
            balance_line(0, Format::Bip177),
            "Your balance is ₿0 (0.00000000 BTC)."
        );
    }

    #[test]
    fn every_answer_is_plain() {
        let text = address_line("spark1qexample");
        assert_eq!(text, "Your address: spark1qexample");
        plain(&text);
        plain(UNREADABLE);
        plain(USAGE);
        plain(computer::NOT_SET_UP);
        let ask = confirm_line("alice@example.com", 1_000, 3, Format::Bip177);
        assert_eq!(
            ask,
            "Send ₿1,000 to alice@example.com? The fee is ₿3, ₿1,003 in all."
        );
        plain(&ask);
        let row = PaymentRow {
            id: "p1".into(),
            received: true,
            amount_sats: 2_100,
            fee_sats: 0,
            method: "Lightning".into(),
            status: "pending".into(),
            at: 1_790_899_200,
        };
        let line = payment_line(&row, Format::Bip177);
        assert!(line.starts_with("2026-10-02"), "{line}");
        assert!(line.contains("+₿2,100"), "{line}");
        assert!(line.ends_with("Received (on its way)"), "{line}");
        plain(&line);
        // The history names no method: Lightning, Spark, or a deposit look
        // the same to the person.
        assert!(!line.contains("Lightning"));
    }

    #[test]
    fn long_requests_are_shortened_for_one_line() {
        assert_eq!(shorten("alice@example.com"), "alice@example.com");
        let invoice = "lnbc10u1pjexampleexampleexampleexampleexample";
        let short = shorten(invoice);
        assert!(short.starts_with("lnbc10u1pjexam") && short.contains('…'));
        assert!(short.chars().count() < invoice.len());
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
    }
}
