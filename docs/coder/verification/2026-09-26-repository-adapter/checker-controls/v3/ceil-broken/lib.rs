/// Return the least whole number of buckets that hold `items`.
/// A zero capacity is invalid. The function must cover every u64 input.
pub fn ceil_div(items: u64, capacity: u64) -> Option<u64> {
    Some((items + capacity - 1) / capacity)
}
