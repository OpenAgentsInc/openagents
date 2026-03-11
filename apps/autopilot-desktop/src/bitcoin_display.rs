pub const BITCOIN_SYMBOL: &str = "\u{20BF}";

pub fn format_bitcoin_amount<T: std::fmt::Display>(value: T) -> String {
    format!("{BITCOIN_SYMBOL}{value}")
}

pub fn format_sats_amount(sats: u64) -> String {
    format_bitcoin_amount(sats)
}

pub fn format_btc_amount_from_sats(sats: u64) -> String {
    format_bitcoin_amount(format!("{:.8}", sats as f64 / 100_000_000.0))
}

pub fn format_mission_control_amount(sats: u64) -> String {
    format!("{BITCOIN_SYMBOL} {}", format_grouped_integer(sats))
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
    use super::{format_grouped_integer, format_mission_control_amount};

    #[test]
    fn formats_grouped_integer_bip177_amounts() {
        assert_eq!(format_grouped_integer(0), "0");
        assert_eq!(format_grouped_integer(12), "12");
        assert_eq!(format_grouped_integer(2_698_437), "2 698 437");
    }

    #[test]
    fn formats_mission_control_amounts_as_grouped_integer_sats() {
        assert_eq!(
            format_mission_control_amount(2_698_437),
            "\u{20BF} 2 698 437"
        );
    }
}
