//! Pointer and length checks for guest linear memory.
//!
//! Lengths are unsigned. A null pointer with a nonzero length is invalid.
//! The output allocation must not overlap the input the host still owns.

/// A pointer and length the host may read.
///
/// # Errors
///
/// Returns a reason the range is outside `memory_len` or overflows.
pub fn check_range(ptr: u32, len: u32, memory_len: usize) -> Result<(), &'static str> {
    if ptr == 0 && len != 0 {
        return Err("null with nonzero length");
    }
    let end = ptr.checked_add(len).ok_or("pointer length overflow")?;
    if usize::try_from(end).ok().is_none_or(|end| end > memory_len) {
        return Err("pointer outside memory");
    }
    Ok(())
}

/// Whether two occupied ranges share a byte.
#[must_use]
pub fn overlaps(left: u32, left_len: u32, right: u32, right_len: u32) -> bool {
    if left_len == 0 || right_len == 0 {
        return false;
    }
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_with_length_and_overlap_are_refused() {
        assert_eq!(
            check_range(0, 4, 64).unwrap_err(),
            "null with nonzero length"
        );
        assert!(check_range(0, 0, 64).is_ok());
        assert!(check_range(60, 8, 64).is_err());
        assert!(overlaps(100, 10, 105, 5));
        assert!(!overlaps(100, 10, 110, 1));
    }
}
