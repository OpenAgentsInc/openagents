/// Return the least whole number of buckets that hold `items`.
/// A zero capacity is invalid. The function must cover every u64 input.
pub fn ceil_div(items: u64, capacity: u64) -> Option<u64> {
    if capacity == 0 {
        return None;
    }

    let quotient = items / capacity;
    let remainder = items % capacity;
    Some(quotient + u64::from(remainder != 0))
}

#[cfg(test)]
mod tests {
    use super::ceil_div;

    #[test]
    fn zero_capacity_is_invalid() {
        assert_eq!(ceil_div(0, 0), None);
        assert_eq!(ceil_div(u64::MAX, 0), None);
    }

    #[test]
    fn rounds_up_only_when_needed() {
        assert_eq!(ceil_div(0, 1), Some(0));
        assert_eq!(ceil_div(6, 3), Some(2));
        assert_eq!(ceil_div(7, 3), Some(3));
    }

    #[test]
    fn handles_u64_boundaries() {
        assert_eq!(ceil_div(u64::MAX, 1), Some(u64::MAX));
        assert_eq!(ceil_div(u64::MAX, u64::MAX), Some(1));
        assert_eq!(ceil_div(u64::MAX, 2), Some(u64::MAX / 2 + 1));
    }
}
