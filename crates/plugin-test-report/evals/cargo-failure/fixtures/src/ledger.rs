pub fn debit(balance: i64, amount: i64) -> Result<i64, &'static str> {
    Ok(balance - amount)
}
