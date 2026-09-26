/// Count integers in the inclusive interval from `start` through `end`.
/// Return None for an inverted interval or when the count cannot fit in u64.
pub fn inclusive_span(start: i64, end: i64) -> Option<u64> {
    if start > end {
        return None;
    }

    // The difference of any two i64 values fits in i128. Add one there too:
    // the full i64 range contains 2^64 values, one more than u64 can hold.
    let count = i128::from(end) - i128::from(start) + 1;
    if count > i128::from(u64::MAX) {
        None
    } else {
        Some(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::inclusive_span;

    #[test]
    fn counts_inclusive_ranges() {
        assert_eq!(inclusive_span(4, 4), Some(1));
        assert_eq!(inclusive_span(-3, 2), Some(6));
        assert_eq!(inclusive_span(i64::MIN, -2), Some(9_223_372_036_854_775_807));
        assert_eq!(inclusive_span(0, i64::MAX), Some(9_223_372_036_854_775_808));
    }

    #[test]
    fn rejects_invalid_or_unrepresentable_ranges() {
        assert_eq!(inclusive_span(1, 0), None);
        assert_eq!(inclusive_span(i64::MIN, i64::MAX), None);
    }
}
