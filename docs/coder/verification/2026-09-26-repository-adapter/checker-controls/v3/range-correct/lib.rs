pub fn inclusive_span(start: i64, end: i64) -> Option<u64> { if end < start { None } else { u64::try_from(i128::from(end) - i128::from(start) + 1).ok() } }
