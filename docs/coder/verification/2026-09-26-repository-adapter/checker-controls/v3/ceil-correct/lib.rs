pub fn ceil_div(items: u64, capacity: u64) -> Option<u64> { if capacity == 0 { None } else { Some(items / capacity + u64::from(items % capacity != 0)) } }
