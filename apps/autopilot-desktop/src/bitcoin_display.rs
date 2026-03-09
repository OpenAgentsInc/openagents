pub const BITCOIN_SYMBOL: &str = "\u{20BF}";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitcoinAmountDisplayMode {
    Integer,
    LegacyBtc,
}

impl BitcoinAmountDisplayMode {
    pub const fn toggle(self) -> Self {
        match self {
            Self::Integer => Self::LegacyBtc,
            Self::LegacyBtc => Self::Integer,
        }
    }

    pub const fn button_label(self) -> &'static str {
        match self {
            Self::Integer => "INTEGER (\u{20BF})",
            Self::LegacyBtc => "LEGACY (BTC)",
        }
    }
}

pub fn format_bitcoin_amount<T: std::fmt::Display>(value: T) -> String {
    format!("{BITCOIN_SYMBOL}{value}")
}

pub fn format_sats_amount(sats: u64) -> String {
    format_bitcoin_amount(sats)
}

pub fn format_btc_amount_from_sats(sats: u64) -> String {
    format_bitcoin_amount(format!("{:.8}", sats as f64 / 100_000_000.0))
}

pub fn format_mission_control_amount(sats: u64, mode: BitcoinAmountDisplayMode) -> String {
    match mode {
        BitcoinAmountDisplayMode::Integer => {
            format!("{BITCOIN_SYMBOL} {}", format_grouped_integer(sats))
        }
        BitcoinAmountDisplayMode::LegacyBtc => format!("{:.8} BTC", sats as f64 / 100_000_000.0),
    }
}

fn format_grouped_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(' ');
        }
        grouped.push(ch);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::{BitcoinAmountDisplayMode, format_grouped_integer, format_mission_control_amount};

    #[test]
    fn formats_grouped_integer_bip177_amounts() {
        assert_eq!(format_grouped_integer(0), "0");
        assert_eq!(format_grouped_integer(12), "12");
        assert_eq!(format_grouped_integer(2_698_437), "2 698 437");
    }

    #[test]
    fn formats_mission_control_amounts_for_both_display_modes() {
        assert_eq!(
            format_mission_control_amount(2_698_437, BitcoinAmountDisplayMode::Integer),
            "\u{20BF} 2 698 437"
        );
        assert_eq!(
            format_mission_control_amount(2_698_437, BitcoinAmountDisplayMode::LegacyBtc),
            "0.02698437 BTC"
        );
    }
}
