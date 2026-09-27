//! Stable identifiers for data records (spells, abilities, items, etc.).

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(pub String);

impl Id {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
