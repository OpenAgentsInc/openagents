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
//! (`link`, sealed to a one-time key after the owner approves on the phone),
//! from the recovery words (`restore`, typed without echo), or is made here
//! (`create`, for a person or agent with no phone). Only `create` shows the
//! words, once: on a terminal after which the person confirms they wrote them
//! down, or printed with `--show-words` when asked for explicitly.

use std::io::{BufRead, IsTerminal, Write};
use std::time::Duration;

use bitcoin_amount::Format;
use openagents_spark::computer;
use openagents_spark::model::{
    Destination, Node, PayFailure, PaymentRow, QuoteFailure, SendRequest,
};
use openagents_spark::seed::Seed;
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents wallet COMMAND [OPTIONS]
  create [--show-words]   Start a new wallet on this computer. It shows your
                          12 recovery words once: write them down, they are
                          the only way back to the wallet. Off a terminal or
                          with --json, add --show-words to print them.
  balance                 Your balance, in one plain sentence.
  address                 Your address to get paid at.
  receive [--amount SATS] [--bitcoin]
                          A payment request to share: a Lightning invoice,
                          for SATS if given; --bitcoin gives a Bitcoin
                          address to send to instead.
  send TO [--amount SATS] [--yes]
                          Pay TO: a Lightning invoice, a Lightning address,
                          or a Spark or Bitcoin address. Shows the amount
                          and fee and asks before paying; --yes pays
                          without asking.
  history [--limit N]     Your recent payments, newest first, with the time
                          and any fee (N is 1 to 200; default 10).
  link [--replace] [--wait SECONDS]
                          Use your phone's wallet on this computer. Your
                          phone asks you to approve and shows a code to
                          check against this screen (10 to 3600 seconds;
                          default 600).
  restore [--replace]     Use the wallet for your recovery words, typed
                          without showing them.
This is the same wallet as the OpenAgents app on your phone: one balance on
every device. Amounts are whole sats, written ₿12,345 (BIP 177); a balance
shows BTC beside it. Set OPENAGENTS_AMOUNT_FORMAT=btc to show BTC first and
type amounts in BTC. `openagents x402 fetch` buys paid calls from this wallet.
Add --json before `wallet` for one JSON document.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("create", Effect::Secret, "wallet"),
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

const SWITCHES: &[&str] = &["bitcoin", "yes", "replace", "show-words"];
/// How many payments `history` lists unless asked.
const HISTORY: u32 = 10;
/// How long `link` waits for the phone unless asked.
const LINK_WAIT: u64 = 600;

type Render = Box<dyn FnOnce(&Value) -> String>;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("wallet", "a command is required", USAGE);
    };
    if rest.first().is_some_and(|word| word == "--help") {
        if let Some(usage) = crate::argv::command_usage("wallet", command, USAGE) {
            println!("{usage}");
            return 0;
        }
    }
    let args = match Args::parse(rest, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("wallet", &message, USAGE),
    };
    let format = Format::from_env();
    let takes = match command.as_str() {
        "send" => 1,
        _ => 0,
    };
    if args.positional().len() > takes {
        let extra = &args.positional()[takes];
        return output.usage(
            "wallet",
            &format!("`{command}` doesn't take `{extra}`"),
            USAGE,
        );
    }
    let note = !output.json() && std::io::stderr().is_terminal();
    let result: Result<(Value, Render), Failure> = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        "create" => create(output, &args),
        "balance" => balance(note).map(|sats| {
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
        "send" => send(output, &args, format, note),
        "history" => history(&args, format, note),
        "link" => link(output, &args),
        "restore" => restore(&args, note),
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

/// This computer's wallet, open and synced. With `note`, a line on the
/// terminal says so while it syncs, which takes seconds.
fn opened(note: bool) -> Result<openagents_spark::spark::SparkNode, Failure> {
    let _syncing = Syncing::show(note);
    let node = computer::open(&computer::home()).map_err(Failure::Plain)?;
    node.sync().map_err(|_| Failure::Plain(UNREADABLE.into()))?;
    Ok(node)
}

/// "Syncing your wallet…" on the terminal until dropped.
struct Syncing(bool);

impl Syncing {
    fn show(note: bool) -> Self {
        if note {
            eprint!("Syncing your wallet…");
            let _ = std::io::stderr().flush();
        }
        Self(note)
    }
}

impl Drop for Syncing {
    fn drop(&mut self) {
        if self.0 {
            eprint!("\r\x1b[K");
            let _ = std::io::stderr().flush();
        }
    }
}

/// What a failed read says: plain, with no internals.
const UNREADABLE: &str =
    "The wallet could not be read right now. Check the connection and try again in a minute.";

fn balance(note: bool) -> Result<u64, Failure> {
    opened(note)?
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
            .map_err(|error| Failure::Usage(amount_message(error, format))),
    }
}

/// Why a typed amount was refused, naming the unit a command line takes.
fn amount_message(error: bitcoin_amount::ParseError, format: Format) -> String {
    use bitcoin_amount::ParseError;
    match (error, format) {
        (ParseError::Invalid | ParseError::TooPrecise, Format::Bip177) => {
            "Enter a whole number of sats, such as 1000 (₿1,000). To type BTC instead, set OPENAGENTS_AMOUNT_FORMAT=btc.".into()
        }
        (error, format) => error.message(format),
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

/// Why a quoted payment can't be sent from a balance, before asking.
pub(crate) fn short_of(balance: u64, amount: u64, fee: u64, format: Format) -> Option<String> {
    let total = amount.saturating_add(fee);
    (balance < total).then(|| {
        format!(
            "Your balance is {}; this needs {} with its fee. Nothing was sent.",
            format.show(balance),
            format.show(total)
        )
    })
}

fn send(
    output: &Output,
    args: &Args,
    format: Format,
    note: bool,
) -> Result<(Value, Render), Failure> {
    let Some(to) = args.positional().first().cloned() else {
        return Err(Failure::Usage(
            "send needs who to pay: an invoice or an address".into(),
        ));
    };
    let amount = amount(args, format)?;
    let node = opened(note)?;
    let quote = match node.quote(&SendRequest {
        input: to.clone(),
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
    if let Destination::Spark(address) = &quote.destination
        && node.spark_address().ok().as_deref() == Some(address.as_str())
    {
        return Err(Failure::Plain(
            "That is this wallet's own address. Nothing was sent.".into(),
        ));
    }
    if let Ok(balance) = node.balance()
        && let Some(short) = short_of(balance, quote.amount_sats, quote.fee_sats, format)
    {
        return Err(Failure::Plain(short));
    }
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
    // A send whose outcome stayed unknown left its key: the same send
    // reuses it, so the wallet returns that payment instead of paying twice.
    let home = computer::home();
    let what = format!("{to}\n{}", quote.amount_sats);
    let (key, repeated) = computer::send_key(&home, &what).map_err(Failure::Plain)?;
    let paid = match node.pay(quote.id, &key) {
        Ok(paid) => paid,
        Err(failure) => {
            if !failure.outcome_unknown() {
                let _ = computer::send_settled(&home, &what);
            }
            return Err(Failure::Plain(pay_failure_line(&failure)));
        }
    };
    let _ = computer::send_settled(&home, &what);
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
    let line = if repeated {
        format!(
            "{line}\nThis repeats a send whose outcome was unknown, so the wallet reported that payment rather than paying again."
        )
    } else {
        line
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

/// Why a send did not complete, in words that never claim "nothing was
/// sent" when the payment may have gone through.
fn pay_failure_line(failure: &PayFailure) -> String {
    match failure {
        PayFailure::NotSent(message) => format!("{message} Nothing was sent."),
        PayFailure::Unknown(message) => format!(
            "{message} `openagents wallet history` shows whether it went through. Running the same send again is safe: it reuses this attempt's key, so the wallet can't pay twice."
        ),
    }
}

/// One payment, for one line: when (UTC), the amount, and what happened,
/// with the fee of a payment sent.
pub(crate) fn payment_line(row: &PaymentRow, format: Format) -> String {
    let amount = format.show_signed(row.amount_sats, row.received);
    let what = if row.received { "Received" } else { "Sent" };
    let when = format!("{} {} UTC", date(row.at), clock(row.at));
    let fee = if !row.received && row.fee_sats > 0 {
        format!(", fee {}", format.show(row.fee_sats))
    } else {
        String::new()
    };
    let status = match row.status.as_str() {
        "pending" => " (on its way)",
        "failed" => " (did not go through)",
        _ => "",
    };
    format!("{when}  {amount:>14}  {what}{fee}{status}")
}

/// `14:05` for Unix seconds, in UTC.
fn clock(at: u64) -> String {
    let minutes = (at % 86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

pub(crate) use crate::out::date;

fn history(args: &Args, format: Format, note: bool) -> Result<(Value, Render), Failure> {
    let limit = match args.option("limit") {
        Some(text) => text
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=200).contains(n))
            .ok_or_else(|| Failure::Usage("--limit takes 1 to 200".into()))?,
        None => HISTORY,
    };
    let rows = opened(note)?
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
fn adopt(seed: &Seed, replace: bool, note: bool) -> Result<(Value, Render), Failure> {
    let home = computer::home();
    computer::save_seed(&home, seed, replace).map_err(Failure::Plain)?;
    let syncing = Syncing::show(note);
    let balance = computer::open(&home)
        .ok()
        .and_then(|node| node.sync().ok().and_then(|()| node.balance().ok()));
    drop(syncing);
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

/// How `create` may show the new wallet's recovery words.
#[derive(Debug, PartialEq, Eq)]
enum Showing {
    /// On the terminal, then ask the person to confirm they wrote them down.
    Terminal,
    /// In the answer, because `--show-words` asked for it.
    Printed,
}

/// Whether `create` may show the words: on a terminal, or printed only when
/// `--show-words` asks, since anyone who reads them can spend.
fn showing(json: bool, terminal: bool, show_words: bool) -> Result<Showing, String> {
    match (json, terminal, show_words) {
        (_, _, true) if json || !terminal => Ok(Showing::Printed),
        (false, true, _) => Ok(Showing::Terminal),
        _ => Err("Creating a wallet shows its 12 recovery words once. Run it on a terminal, or add --show-words to print them here; anyone who sees them can spend from the wallet.".into()),
    }
}

/// The words, numbered, four to a line.
fn numbered(words: &[&str]) -> String {
    words
        .chunks(4)
        .enumerate()
        .map(|(row, chunk)| {
            chunk
                .iter()
                .enumerate()
                .map(|(column, word)| format!("{:>2}. {word:<10}", row * 4 + column + 1))
                .collect::<Vec<_>>()
                .join(" ")
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a new wallet says it is ready to do.
const CREATED: &str = "Your new wallet is ready on this computer. `openagents wallet address` shows where to get paid, and `openagents wallet receive` gives a payment request.";

fn create(output: &Output, args: &Args) -> Result<(Value, Render), Failure> {
    let home = computer::home();
    if computer::has_seed(&home) {
        return Err(Failure::Plain(
            "This computer already has a wallet; `openagents wallet balance` shows it. To use another one, run `openagents wallet restore --replace` with its recovery words.".into(),
        ));
    }
    let terminal = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let how =
        showing(output.json(), terminal, args.switch("show-words")).map_err(Failure::Usage)?;
    let seed = Seed::generate().map_err(Failure::Plain)?;
    let words = numbered(&seed.words());
    if how == Showing::Terminal {
        eprintln!(
            "Your recovery words. Write them down in order and keep them somewhere safe:\nthey are the only way back to this wallet if this computer is lost, and\nanyone who has them can spend from it.\n\n{words}\n"
        );
        eprint!("Type saved when you have written them down: ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().lock().read_line(&mut answer);
        if !answer.trim().eq_ignore_ascii_case("saved") {
            return Err(Failure::Plain(
                "No wallet was created. Run `openagents wallet create` again when you can write the words down.".into(),
            ));
        }
    }
    computer::save_seed(&home, &seed, false).map_err(Failure::Plain)?;
    let printed = (how == Showing::Printed).then(|| seed.mnemonic.clone());
    let line = match &printed {
        Some(_) => format!(
            "{CREATED}\nYour recovery words, shown only now. Write them down; anyone who has them can spend from the wallet:\n{words}"
        ),
        None => CREATED.to_owned(),
    };
    Ok((
        json!({ "created": true, "recovery_words": printed }),
        Box::new(move |_: &Value| line),
    ))
}

fn restore(args: &Args, note: bool) -> Result<(Value, Render), Failure> {
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
    adopt(&seed, args.switch("replace"), note)
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
            adopt(&seed, replace, !output.json() && std::io::stderr().is_terminal())
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
    fn a_send_that_may_have_paid_never_says_nothing_was_sent() {
        let unknown = pay_failure_line(&PayFailure::Unknown(
            "The wallet lost track of this payment while sending it (Network error: reset), so it may have gone through.".into(),
        ));
        assert!(
            !unknown.to_lowercase().contains("nothing was sent"),
            "{unknown}"
        );
        assert!(unknown.contains("openagents wallet history"), "{unknown}");
        assert!(unknown.contains("can't pay twice"), "{unknown}");
        let refused = pay_failure_line(&PayFailure::NotSent(
            "The wallet doesn't hold enough to pay this and its fee.".into(),
        ));
        assert!(refused.ends_with("Nothing was sent."), "{refused}");
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
    fn create_shows_the_words_on_a_terminal_or_only_when_asked() {
        assert_eq!(showing(false, true, false), Ok(Showing::Terminal));
        assert_eq!(showing(false, false, true), Ok(Showing::Printed));
        assert_eq!(showing(true, true, true), Ok(Showing::Printed));
        let refused = showing(false, false, false).unwrap_err();
        assert!(refused.contains("--show-words"), "{refused}");
        assert!(showing(true, true, false).is_err());
        let words = numbered(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"]);
        assert_eq!(words.lines().count(), 3);
        assert!(words.starts_with(" 1. a"), "{words}");
        assert!(words.contains("12. l"), "{words}");
        plain(CREATED);
    }

    #[test]
    fn a_send_the_balance_cannot_cover_is_refused_before_asking() {
        let short = short_of(0, 10, 0, Format::Bip177).expect("short");
        assert_eq!(
            short,
            "Your balance is ₿0; this needs ₿10 with its fee. Nothing was sent."
        );
        assert!(short_of(11, 10, 1, Format::Bip177).is_none());
        assert!(short_of(10, 10, 1, Format::Bip177).is_some());
    }

    #[test]
    fn amounts_name_their_unit() {
        let text = amount_message(bitcoin_amount::ParseError::Invalid, Format::Bip177);
        assert!(text.contains("whole number of sats"), "{text}");
        assert!(!text.contains("base units"), "{text}");
        let sent = PaymentRow {
            id: "p2".into(),
            received: false,
            amount_sats: 26,
            fee_sats: 1,
            method: "Lightning".into(),
            status: "completed".into(),
            at: 1_790_899_200 + 14 * 3600 + 5 * 60,
        };
        let line = payment_line(&sent, Format::Bip177);
        assert!(line.starts_with("2026-10-02 14:05 UTC"), "{line}");
        assert!(line.ends_with("Sent, fee ₿1"), "{line}");
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
