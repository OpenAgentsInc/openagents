//! A dependency-free workload for fresh environment verification.

/// Sum comma-separated unsigned amounts, refusing bad input and overflow.
pub fn total(input: &str) -> Result<u64, &'static str> {
    if input.trim().is_empty() {
        return Err("Enter at least one amount.");
    }
    input.split(',').try_fold(0u64, |sum, item| {
        let amount = item.trim().parse::<u64>().map_err(|_| "Invalid amount.")?;
        sum.checked_add(amount).ok_or("The total is too large.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_multiple_amounts() {
        assert_eq!(total("1000, 42, 9001"), Ok(10043));
        assert_eq!(total("0"), Ok(0));
    }

    #[test]
    fn rejects_missing_or_invalid_amounts() {
        for input in ["", " ", "1,", "1,,2", "-1", "two", "1.5"] {
            assert!(total(input).is_err(), "{input}");
        }
    }

    #[test]
    fn detects_overflow_instead_of_wrapping() {
        assert_eq!(total("18446744073709551615"), Ok(u64::MAX));
        assert!(total("18446744073709551615,1").is_err());
    }
}
