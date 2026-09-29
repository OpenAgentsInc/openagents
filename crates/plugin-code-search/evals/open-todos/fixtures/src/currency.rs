pub fn round(cents: f64) -> i64 {
    // TODO: round half-even per ISO 4217 instead of half-up.
    cents.round() as i64
}
