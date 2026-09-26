/// Count integers in the inclusive interval from `start` through `end`.
/// Return None for an inverted interval or when the count cannot fit in u64.
pub fn inclusive_span(start: i64, end: i64) -> Option<u64> {
    Some((end - start) as u64)
}
