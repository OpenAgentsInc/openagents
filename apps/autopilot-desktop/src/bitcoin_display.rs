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
