//! Account ledger.

pub struct Ledger {
    entries: Vec<u64>,
}

impl Ledger {
    pub fn balance(&self, limit: &str) -> u64 {
        let cap: u64 = limit;
        self.entries.iter().sum::<u64>().min(cap)
    }

    pub fn report(&self) -> String {
        let total_cents: u64 = self.entries.iter().sum();
        format!("{} dollars", totl_cents / 100)
    }
}
