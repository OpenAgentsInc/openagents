//! Payee address book CLI commands.

#![allow(dead_code)]

use anyhow::Result;
use colored::Colorize;

use crate::storage::address_book::AddressBook;
use super::error::{WalletError, format_error_with_hint};
use super::validation::{detect_and_validate_destination, PaymentDestinationType};

fn validate_payee_name(name: &str) -> Result<(), WalletError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(WalletError::Other("Payee name cannot be empty".to_string()));
    }

    if trimmed.len() > 50 {
        return Err(WalletError::Other(
            "Payee name too long (max 50 characters)".to_string(),
        ));
    }

    if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ') {
        return Err(WalletError::Other(
            "Payee name can only contain letters, numbers, spaces, hyphens, and underscores".to_string(),
        ));
    }

    Ok(())
}

fn format_destination_type(dest_type: &PaymentDestinationType) -> &'static str {
    match dest_type {
        PaymentDestinationType::LightningInvoice => "Lightning Invoice",
        PaymentDestinationType::SparkAddress => "Spark Address",
        PaymentDestinationType::SparkInvoice => "Spark Invoice",
        PaymentDestinationType::BitcoinAddress => "Bitcoin Address",
        PaymentDestinationType::Lnurl => "LNURL",
        PaymentDestinationType::LightningAddress => "Lightning Address",
    }
}

pub fn list() -> Result<()> {
    let book = AddressBook::load()?;
    if book.entries.is_empty() {
        println!("No payees saved yet.");
        println!();
        println!(
            "{}: Use 'openagents wallet payee add <name> <address>' to save a payment destination.",
            "Hint".cyan()
        );
        return Ok(());
    }

    println!("{}", "Saved Payees".bold());
    println!("────────────────────────────────────────");

    for entry in &book.entries {
        let type_hint = match detect_and_validate_destination(&entry.address) {
            Ok(validated) => format!(" ({})", format_destination_type(&validated.destination_type)),
            Err(_) => String::new(),
        };
        println!("  {} → {}{}", entry.name.green(), entry.address, type_hint.dimmed());
    }

    println!();
    println!("{} payee(s) saved", book.entries.len());
    Ok(())
}

pub fn add(name: String, address: String) -> Result<()> {
    if let Err(e) = validate_payee_name(&name) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    let validated = match detect_and_validate_destination(&address) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", format_error_with_hint(&e));
            return Err(e.into());
        }
    };

    let mut book = AddressBook::load()?;

    if book.find(&name).is_some() {
        let error = WalletError::PayeeAlreadyExists(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        return Err(error.into());
    }

    book.add(name.clone(), validated.normalized.clone())?;
    book.save()?;

    println!("{} Added payee '{}'", "✓".green(), name);
    println!(
        "  {} {}",
        "Type:".dimmed(),
        format_destination_type(&validated.destination_type)
    );
    println!("  {} {}", "Address:".dimmed(), validated.normalized);
    Ok(())
}

pub fn remove(name: String) -> Result<()> {
    let mut book = AddressBook::load()?;

    let entry = book.find(&name).cloned();
    if entry.is_none() {
        let error = WalletError::PayeeNotFound(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        return Err(error.into());
    }

    book.remove(&name);
    book.save()?;

    println!("{} Removed payee '{}'", "✓".green(), name);

    if let Some(entry) = entry {
        println!("  {} {}", "Was:".dimmed(), entry.address);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_payee_name_valid() {
        assert!(validate_payee_name("alice").is_ok());
        assert!(validate_payee_name("Bob Smith").is_ok());
        assert!(validate_payee_name("payment-1").is_ok());
        assert!(validate_payee_name("test_payee").is_ok());
    }

    #[test]
    fn test_validate_payee_name_empty() {
        assert!(validate_payee_name("").is_err());
        assert!(validate_payee_name("   ").is_err());
    }

    #[test]
    fn test_validate_payee_name_too_long() {
        let long_name = "a".repeat(51);
        assert!(validate_payee_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_payee_name_invalid_chars() {
        assert!(validate_payee_name("alice!").is_err());
        assert!(validate_payee_name("bob@home").is_err());
        assert!(validate_payee_name("test#1").is_err());
    }
}
