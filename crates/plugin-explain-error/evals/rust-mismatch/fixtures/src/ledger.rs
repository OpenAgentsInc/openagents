//! Account ledger.

pub struct Ledger {
    entries: Vec<u64>,
}

impl Ledger {
    pub fn spending_cap(&self, limit: &str) -> u64 {
        let cap: u64 = limit;
        self.entries.iter().sum::<u64>().min(cap)
    }
}
