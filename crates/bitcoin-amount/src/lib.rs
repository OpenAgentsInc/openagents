//! Bitcoin amounts as OpenAgents shows and reads them.
//!
//! Every amount is a `u64` count of base units: what was called a satoshi,
//! and what [BIP 177](https://bips.dev/177/) calls one bitcoin. No floating
//! point is used anywhere; the legacy decimal form is built and parsed from
//! integer digits.
//!
//! Two display formats exist:
//!
//! - [`Format::Bip177`], the default: integer base units with the ₿ symbol
//!   before them, grouped by thousands: `₿12,345`. Its spoken form, for
//!   accessibility, is `12,345 bitcoin`.
//! - [`Format::LegacyBtc`]: the legacy BTC currency code, 1 BTC =
//!   100,000,000 base units, with eight decimals: `0.00012345 BTC`.
//!
//! Machine-readable names are not display: protocol fields such as BOLT11's
//! `amount_msat`, LNURL's `minSendable`/`maxSendable` (msat), x402's
//! `amount` with `asset: "BTC"`, and Rust identifiers ending in `_sats`
//! keep their names and units. See `docs/breez/amounts.md`.

use std::fmt;

/// Base units in one legacy BTC.
pub const BASE_UNITS_PER_BTC: u64 = 100_000_000;
/// The most base units that can ever exist: 21,000,000 BTC. Amounts above it
/// are refused when parsed.
pub const MAX_SUPPLY: u64 = 21_000_000 * BASE_UNITS_PER_BTC;
/// The BIP 177 symbol.
pub const SYMBOL: &str = "₿";

/// How amounts are shown and typed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Format {
    /// Integer base units: `₿12,345`.
    #[default]
    Bip177,
    /// Legacy decimal BTC: `0.00012345 BTC`.
    LegacyBtc,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Bip177, Format::LegacyBtc];

    /// The stored and wire name: `bip177` or `btc`.
    pub fn id(self) -> &'static str {
        match self {
            Format::Bip177 => "bip177",
            Format::LegacyBtc => "btc",
        }
    }

    /// Read a stored or wire name; anything else is `None`.
    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim() {
            "bip177" => Some(Format::Bip177),
            "btc" => Some(Format::LegacyBtc),
            _ => None,
        }
    }

    /// What a settings control calls the format.
    pub fn label(self) -> &'static str {
        match self {
            Format::Bip177 => "₿ bitcoin (BIP 177)",
            Format::LegacyBtc => "BTC (legacy)",
        }
    }

    /// The unit an amount field names: "₿" or "BTC".
    pub fn unit(self) -> &'static str {
        match self {
            Format::Bip177 => SYMBOL,
            Format::LegacyBtc => "BTC",
        }
    }

    /// Typed amounts take a decimal point (legacy BTC only).
    pub fn decimal_entry(self) -> bool {
        self == Format::LegacyBtc
    }

    /// The other format, for the transitional dual display.
    pub fn other(self) -> Self {
        match self {
            Format::Bip177 => Format::LegacyBtc,
            Format::LegacyBtc => Format::Bip177,
        }
    }

    /// `amount` for the screen: `₿12,345` or `0.00012345 BTC`.
    pub fn show(self, amount: u64) -> String {
        match self {
            Format::Bip177 => format!("{SYMBOL}{}", group(amount)),
            Format::LegacyBtc => format!("{} BTC", btc_decimal(amount)),
        }
    }

    /// `amount` with a sign: `+₿1,000`, `-0.00001000 BTC`.
    pub fn show_signed(self, amount: u64, incoming: bool) -> String {
        format!("{}{}", if incoming { "+" } else { "-" }, self.show(amount))
    }

    /// `amount` as a screen reader should say it: `12,345 bitcoin` or
    /// `0.00012345 BTC`.
    pub fn spoken(self, amount: u64) -> String {
        match self {
            Format::Bip177 => format!("{} bitcoin", group(amount)),
            Format::LegacyBtc => format!("{} BTC", btc_decimal(amount)),
        }
    }

    /// Read an amount a person typed in this format. See [`parse`].
    pub fn parse(self, text: &str) -> Result<Option<u64>, ParseError> {
        parse(text, self)
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Thousands-grouped digits: `12,345`.
pub fn group(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// Legacy BTC digits with eight decimals and a grouped whole part:
/// `0.00012345`, `21,000,000.00000000`.
pub fn btc_decimal(value: u64) -> String {
    format!(
        "{}.{:08}",
        group(value / BASE_UNITS_PER_BTC),
        value % BASE_UNITS_PER_BTC
    )
}

/// Whole base units in `msat` millisatoshis, rounded down (a maximum).
pub fn from_msat_floor(msat: u64) -> u64 {
    msat / 1000
}

/// Whole base units in `msat` millisatoshis, rounded up (a minimum).
pub fn from_msat_ceil(msat: u64) -> u64 {
    msat.div_ceil(1000)
}

/// Why a typed amount was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Zero or less.
    Zero,
    /// Not a number in the format: letters, a stray sign, a second point,
    /// or a decimal point in BIP 177 mode.
    Invalid,
    /// More than eight decimals of BTC: smaller than one base unit.
    TooPrecise,
    /// Above 21,000,000 BTC, or too large to hold.
    TooLarge,
}

impl ParseError {
    /// What the screen tells the person, in the format they type in.
    pub fn message(self, format: Format) -> String {
        match (self, format) {
            (ParseError::Zero, _) => "Enter an amount above zero.".into(),
            (ParseError::TooLarge, _) => {
                "That is more bitcoin than will ever exist (21,000,000 BTC).".into()
            }
            (ParseError::Invalid | ParseError::TooPrecise, Format::Bip177) => {
                "Enter the amount in whole bitcoin base units, such as ₿1,000.".into()
            }
            (ParseError::Invalid, Format::LegacyBtc) => {
                "Enter the amount in BTC, such as 0.00001.".into()
            }
            (ParseError::TooPrecise, Format::LegacyBtc) => {
                "BTC amounts have at most eight decimals.".into()
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(Format::Bip177))
    }
}

impl std::error::Error for ParseError {}

/// Read an amount a person typed, as base units. Empty (or only a unit)
/// means none. Spaces, `_`, and `,` group digits in either format.
///
/// - [`Format::Bip177`] takes whole base units, with an optional leading `₿`
///   or a trailing `bitcoin`/`bitcoins`: `1000`, `₿1,000`, `1,000 bitcoin`.
/// - [`Format::LegacyBtc`] takes decimal BTC with at most eight decimals and
///   an optional trailing `BTC`: `0.00001`, `1.5 BTC`, `.25`.
///
/// Zero, anything above [`MAX_SUPPLY`], and malformed text are refused.
pub fn parse(text: &str, format: Format) -> Result<Option<u64>, ParseError> {
    let mut rest = text.trim();
    match format {
        Format::Bip177 => {
            rest = rest.strip_prefix(SYMBOL).unwrap_or(rest).trim_start();
            let lower = rest.to_ascii_lowercase();
            for unit in ["bitcoins", "bitcoin"] {
                if lower.ends_with(unit) {
                    rest = rest[..rest.len() - unit.len()].trim_end();
                    break;
                }
            }
        }
        Format::LegacyBtc => {
            let cut = rest.len().saturating_sub(3);
            if rest.is_char_boundary(cut) && rest[cut..].eq_ignore_ascii_case("btc") {
                rest = rest[..cut].trim_end();
            }
        }
    }
    let cleaned: String = rest
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' '))
        .collect();
    if cleaned.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = match (format, cleaned.split_once('.')) {
        (Format::Bip177, Some(_)) => return Err(ParseError::Invalid),
        (_, None) => (cleaned.as_str(), ""),
        (Format::LegacyBtc, Some((whole, fraction))) => (whole, fraction),
    };
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !digits(fraction) || (whole.is_empty() && fraction.is_empty()) {
        return Err(ParseError::Invalid);
    }
    let amount = match format {
        Format::Bip177 => decimal(whole)?,
        Format::LegacyBtc => {
            if fraction.len() > 8 {
                // Trailing zeros past the eighth place are exact.
                if fraction[8..].bytes().any(|b| b != b'0') {
                    return Err(ParseError::TooPrecise);
                }
            }
            let fraction = &fraction[..fraction.len().min(8)];
            let whole = if whole.is_empty() { 0 } else { decimal(whole)? };
            let mut units = if fraction.is_empty() { 0 } else { decimal(fraction)? };
            for _ in fraction.len()..8 {
                units *= 10;
            }
            whole
                .checked_mul(BASE_UNITS_PER_BTC)
                .and_then(|base| base.checked_add(units))
                .ok_or(ParseError::TooLarge)?
        }
    };
    match amount {
        0 => Err(ParseError::Zero),
        amount if amount > MAX_SUPPLY => Err(ParseError::TooLarge),
        amount => Ok(Some(amount)),
    }
}

/// ASCII digits as a `u64`; overflow is [`ParseError::TooLarge`].
fn decimal(digits: &str) -> Result<u64, ParseError> {
    digits.bytes().try_fold(0u64, |value, byte| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
            .ok_or(ParseError::TooLarge)
    })
}

#[cfg(test)]
mod tests;
