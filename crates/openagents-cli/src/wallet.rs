//! `openagents wallet`: the person's wallet on this computer, answered in
//! plain words. A balance is one sentence ("Your balance is ₿12,000
//! (0.00012000 BTC).") and an address is one line ("Your address: …"). No
//! answer names a node, a network, a chain server, channels, liquidity, or
//! millisatoshis; those belong to the x402 receiver under
//! `openagents x402 node`. `--json` keeps machine fields.

use bitcoin_amount::Format;
use serde_json::{Value, json};

use crate::Output;
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents wallet COMMAND
  balance                 Your balance, in one plain sentence.
  address                 An address to receive bitcoin at.
Amounts show as ₿12,345 (BIP 177), with BTC beside them; set
OPENAGENTS_AMOUNT_FORMAT=btc to show BTC first. Add --json before `wallet`
for one JSON document.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("balance", Effect::ReadOnly, "wallet"),
    Declared::screen("address", Effect::ReadOnly, "wallet"),
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

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, _rest)) = words.split_first() else {
        return output.usage("wallet", "a command is required", USAGE);
    };
    let format = Format::from_env();
    let result = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return 0;
        }
        "balance" => balance().map(|sats| {
            (
                json!({ "balance_sats": sats }),
                Box::new(move |_: &Value| balance_line(sats, format))
                    as Box<dyn FnOnce(&Value) -> String>,
            )
        }),
        "address" => address().map(|address| {
            let line = address_line(&address);
            (
                json!({ "address": address }),
                Box::new(move |_: &Value| line) as Box<dyn FnOnce(&Value) -> String>,
            )
        }),
        other => return output.usage("wallet", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok((value, render)) => {
            output.emit(&value, render);
            0
        }
        Err(message) => output.fail("wallet", &message),
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

/// What a failed read says: plain, with no internals.
const UNREADABLE: &str = "The wallet could not be read right now. Try again in a minute.";

fn balance() -> Result<u64, String> {
    let (wallet, _) = crate::x402::open_wallet().map_err(|_| UNREADABLE.to_owned())?;
    let balance =
        openagents_wallet::LightningWallet::balance(&wallet).map_err(|_| UNREADABLE.to_owned())?;
    Ok(balance
        .lightning_total_sats
        .saturating_add(balance.onchain_total_sats))
}

fn address() -> Result<String, String> {
    let (wallet, _) = crate::x402::open_wallet().map_err(|_| UNREADABLE.to_owned())?;
    openagents_wallet::LightningWallet::funding_address(&wallet).map_err(|_| UNREADABLE.to_owned())
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
    fn an_address_is_one_plain_line() {
        let text = address_line("sp1qexample");
        assert_eq!(text, "Your address: sp1qexample");
        plain(&text);
        plain(UNREADABLE);
        plain(USAGE);
    }
}
