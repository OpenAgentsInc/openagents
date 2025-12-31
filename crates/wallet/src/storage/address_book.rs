//! Address book storage for frequently used payment destinations.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const ADDRESS_BOOK_ENV: &str = "OPENAGENTS_ADDRESS_BOOK";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddressEntry {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressBook {
    pub entries: Vec<AddressEntry>,
}

impl AddressBook {
    pub fn load() -> Result<Self> {
        let path = address_book_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read address book {}", path.display()))?;
        if contents.trim().is_empty() {
            return Ok(Self::default());
        }
        let book = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse address book {}", path.display()))?;
        Ok(book)
    }

    pub fn save(&self) -> Result<()> {
        let path = address_book_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let contents =
            serde_json::to_string_pretty(self).context("Failed to serialize address book")?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write address book {}", path.display()))?;
        Ok(())
    }

    pub fn add(&mut self, name: String, address: String) -> Result<()> {
        if self.entries.iter().any(|entry| entry.name == name) {
            anyhow::bail!("Payee '{}' already exists.", name);
        }

        self.entries.push(AddressEntry { name, address });
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.name != name);
        before != self.entries.len()
    }

    pub fn find(&self, name: &str) -> Option<&AddressEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }
}

fn address_book_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var(ADDRESS_BOOK_ENV) {
        return Ok(PathBuf::from(path));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".openagents").join("address_book.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ADDRESS_BOOK_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_add_remove_and_find_entries() {
        let _guard = ADDRESS_BOOK_ENV_LOCK.lock().unwrap();
        let temp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        let original = std::env::var(ADDRESS_BOOK_ENV).ok();

        unsafe {
            std::env::set_var(ADDRESS_BOOK_ENV, temp.path());
        }

        let mut book = AddressBook::load().unwrap();
        assert!(book.entries.is_empty());

        book.add("alice".to_string(), "lnbc1alice".to_string())
            .unwrap();
        book.save().unwrap();

        let book = AddressBook::load().unwrap();
        assert_eq!(book.entries.len(), 1);
        assert_eq!(book.find("alice").unwrap().address, "lnbc1alice");

        let mut book = book;
        assert!(book.remove("alice"));
        book.save().unwrap();

        let book = AddressBook::load().unwrap();
        assert!(book.entries.is_empty());

        if let Some(value) = original {
            unsafe {
                std::env::set_var(ADDRESS_BOOK_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(ADDRESS_BOOK_ENV);
            }
        }
    }
}
