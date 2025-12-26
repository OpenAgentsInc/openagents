//! Payee address book CLI commands.

use anyhow::Result;

use crate::storage::address_book::AddressBook;

pub fn list() -> Result<()> {
    let book = AddressBook::load()?;
    if book.entries.is_empty() {
        println!("No payees saved yet.");
        return Ok(());
    }

    println!("Saved Payees");
    println!("----------------------------");
    for entry in &book.entries {
        println!("  {} -> {}", entry.name, entry.address);
    }
    Ok(())
}

pub fn add(name: String, address: String) -> Result<()> {
    let mut book = AddressBook::load()?;
    book.add(name.clone(), address)?;
    book.save()?;
    println!("Added payee '{}'.", name);
    Ok(())
}

pub fn remove(name: String) -> Result<()> {
    let mut book = AddressBook::load()?;
    if !book.remove(&name) {
        anyhow::bail!("Payee '{}' not found.", name);
    }
    book.save()?;
    println!("Removed payee '{}'.", name);
    Ok(())
}
