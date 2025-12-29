//! Payment address validation functions.

use super::error::WalletError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentDestinationType {
    LightningInvoice,
    SparkAddress,
    SparkInvoice,
    BitcoinAddress,
    Lnurl,
    LightningAddress,
}

#[derive(Debug, Clone)]
pub struct ValidatedDestination {
    pub original: String,
    pub destination_type: PaymentDestinationType,
    pub normalized: String,
}

pub fn validate_lightning_invoice(invoice: &str) -> Result<ValidatedDestination, WalletError> {
    let lower = invoice.to_lowercase();
    let trimmed = lower.trim();

    if trimmed.is_empty() {
        return Err(WalletError::InvalidLightningInvoice(
            "Invoice cannot be empty".to_string(),
        ));
    }

    let prefixes = ["lnbc", "lntb", "lnbcrt", "lntbs"];
    if !prefixes.iter().any(|p| trimmed.starts_with(p)) {
        return Err(WalletError::InvalidLightningInvoice(format!(
            "Must start with lnbc (mainnet), lntb (testnet), or lnbcrt (regtest), got: {}...",
            &trimmed[..trimmed.len().min(10)]
        )));
    }

    if trimmed.len() < 50 {
        return Err(WalletError::InvalidLightningInvoice(
            "Invoice too short to be valid".to_string(),
        ));
    }

    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(WalletError::InvalidLightningInvoice(
            "Invoice contains invalid characters".to_string(),
        ));
    }

    Ok(ValidatedDestination {
        original: invoice.to_string(),
        destination_type: PaymentDestinationType::LightningInvoice,
        normalized: trimmed.to_string(),
    })
}

pub fn validate_spark_address(address: &str) -> Result<ValidatedDestination, WalletError> {
    let trimmed = address.trim();

    if trimmed.is_empty() {
        return Err(WalletError::InvalidSparkAddress(
            "Address cannot be empty".to_string(),
        ));
    }

    let lower = trimmed.to_lowercase();
    let prefixes = ["sp1", "sprt1", "spt1"];
    if !prefixes.iter().any(|p| lower.starts_with(p)) {
        return Err(WalletError::InvalidSparkAddress(format!(
            "Must start with sp1 (mainnet), sprt1 (regtest), or spt1 (testnet), got: {}...",
            &trimmed[..trimmed.len().min(10)]
        )));
    }

    if trimmed.len() < 20 {
        return Err(WalletError::InvalidSparkAddress(
            "Address too short to be valid".to_string(),
        ));
    }

    Ok(ValidatedDestination {
        original: address.to_string(),
        destination_type: PaymentDestinationType::SparkAddress,
        normalized: trimmed.to_string(),
    })
}

pub fn validate_bitcoin_address(address: &str) -> Result<ValidatedDestination, WalletError> {
    let trimmed = address.trim();

    if trimmed.is_empty() {
        return Err(WalletError::InvalidBitcoinAddress(
            "Address cannot be empty".to_string(),
        ));
    }

    let is_segwit = trimmed.starts_with("bc1")
        || trimmed.starts_with("tb1")
        || trimmed.starts_with("bcrt1");

    let is_legacy = trimmed.starts_with('1')
        || trimmed.starts_with('3')
        || trimmed.starts_with('m')
        || trimmed.starts_with('n')
        || trimmed.starts_with('2');

    if !is_segwit && !is_legacy {
        return Err(WalletError::InvalidBitcoinAddress(format!(
            "Unrecognized address format. Expected bc1 (mainnet), tb1 (testnet), bcrt1 (regtest), or legacy format. Got: {}...",
            &trimmed[..trimmed.len().min(10)]
        )));
    }

    if is_segwit && trimmed.len() < 42 {
        return Err(WalletError::InvalidBitcoinAddress(
            "SegWit address too short".to_string(),
        ));
    }

    if is_legacy && (trimmed.len() < 26 || trimmed.len() > 35) {
        return Err(WalletError::InvalidBitcoinAddress(
            "Legacy address has invalid length".to_string(),
        ));
    }

    Ok(ValidatedDestination {
        original: address.to_string(),
        destination_type: PaymentDestinationType::BitcoinAddress,
        normalized: trimmed.to_string(),
    })
}

pub fn validate_lnurl(lnurl: &str) -> Result<ValidatedDestination, WalletError> {
    let trimmed = lnurl.trim();

    if trimmed.is_empty() {
        return Err(WalletError::InvalidLnurl(
            "LNURL cannot be empty".to_string(),
        ));
    }

    let lower = trimmed.to_lowercase();

    if lower.starts_with("lnurl") {
        if lower.len() < 20 {
            return Err(WalletError::InvalidLnurl(
                "LNURL too short to be valid".to_string(),
            ));
        }

        return Ok(ValidatedDestination {
            original: lnurl.to_string(),
            destination_type: PaymentDestinationType::Lnurl,
            normalized: lower,
        });
    }

    Err(WalletError::InvalidLnurl(format!(
        "Must start with 'lnurl', got: {}...",
        &trimmed[..trimmed.len().min(10)]
    )))
}

pub fn validate_lightning_address(address: &str) -> Result<ValidatedDestination, WalletError> {
    let trimmed = address.trim();

    if trimmed.is_empty() {
        return Err(WalletError::InvalidLnurl(
            "Lightning address cannot be empty".to_string(),
        ));
    }

    let parts: Vec<&str> = trimmed.split('@').collect();
    if parts.len() != 2 {
        return Err(WalletError::InvalidLnurl(format!(
            "Lightning address must be in format user@domain, got: {}",
            trimmed
        )));
    }

    let user = parts[0];
    let domain = parts[1];

    if user.is_empty() {
        return Err(WalletError::InvalidLnurl(
            "Lightning address username cannot be empty".to_string(),
        ));
    }

    if domain.is_empty() || !domain.contains('.') {
        return Err(WalletError::InvalidLnurl(
            "Lightning address domain must be valid (e.g., example.com)".to_string(),
        ));
    }

    Ok(ValidatedDestination {
        original: address.to_string(),
        destination_type: PaymentDestinationType::LightningAddress,
        normalized: trimmed.to_lowercase(),
    })
}

pub fn validate_amount(amount: u64) -> Result<(), WalletError> {
    if amount == 0 {
        return Err(WalletError::InvalidAmount(
            "Amount must be greater than 0 sats".to_string(),
        ));
    }

    const MAX_SATS: u64 = 21_000_000 * 100_000_000;
    if amount > MAX_SATS {
        return Err(WalletError::InvalidAmount(format!(
            "Amount {} exceeds maximum possible (21M BTC)",
            amount
        )));
    }

    Ok(())
}

pub fn validate_amount_with_limit(amount: u64, limit: Option<u64>) -> Result<(), WalletError> {
    validate_amount(amount)?;

    if let Some(max) = limit {
        if amount > max {
            return Err(WalletError::AmountExceedsLimit { amount, limit: max });
        }
    }

    Ok(())
}

pub fn detect_and_validate_destination(
    destination: &str,
) -> Result<ValidatedDestination, WalletError> {
    let trimmed = destination.trim();
    let lower = trimmed.to_lowercase();

    if lower.starts_with("lnbc")
        || lower.starts_with("lntb")
        || lower.starts_with("lnbcrt")
        || lower.starts_with("lntbs")
    {
        return validate_lightning_invoice(trimmed);
    }

    if lower.starts_with("sp1") || lower.starts_with("sprt1") || lower.starts_with("spt1") {
        return validate_spark_address(trimmed);
    }

    if lower.starts_with("bc1")
        || lower.starts_with("tb1")
        || lower.starts_with("bcrt1")
        || lower.starts_with('1')
        || lower.starts_with('3')
        || lower.starts_with('m')
        || lower.starts_with('n')
        || lower.starts_with('2')
    {
        return validate_bitcoin_address(trimmed);
    }

    if lower.starts_with("lnurl") {
        return validate_lnurl(trimmed);
    }

    if trimmed.contains('@') && !trimmed.starts_with('@') {
        return validate_lightning_address(trimmed);
    }

    Err(WalletError::InvalidDestination(format!(
        "Could not determine destination type for '{}'. Expected Lightning invoice (lnbc...), \
         Spark address (sp1...), Bitcoin address (bc1...), LNURL (lnurl...), or Lightning address (user@domain).",
        if trimmed.len() > 30 {
            format!("{}...", &trimmed[..30])
        } else {
            trimmed.to_string()
        }
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_lightning_invoice_mainnet() {
        let invoice = "lnbc1500n1pj9qjz4pp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxxatsyp3k7enxv4jsxqzpu";
        let result = validate_lightning_invoice(invoice);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::LightningInvoice);
    }

    #[test]
    fn test_validate_lightning_invoice_regtest() {
        let invoice = "lnbcrt1500n1pj9qjz4pp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5xysxx";
        let result = validate_lightning_invoice(invoice);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_lightning_invoice_empty() {
        let result = validate_lightning_invoice("");
        assert!(matches!(result, Err(WalletError::InvalidLightningInvoice(_))));
    }

    #[test]
    fn test_validate_lightning_invoice_wrong_prefix() {
        let result = validate_lightning_invoice("bitcoin:bc1qtest");
        assert!(matches!(result, Err(WalletError::InvalidLightningInvoice(_))));
    }

    #[test]
    fn test_validate_spark_address() {
        let result = validate_spark_address("sp1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::SparkAddress);
    }

    #[test]
    fn test_validate_spark_address_regtest() {
        let result = validate_spark_address("sprt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_spark_address_invalid() {
        let result = validate_spark_address("bc1qtest");
        assert!(matches!(result, Err(WalletError::InvalidSparkAddress(_))));
    }

    #[test]
    fn test_validate_bitcoin_address_segwit_mainnet() {
        let result = validate_bitcoin_address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::BitcoinAddress);
    }

    #[test]
    fn test_validate_bitcoin_address_segwit_testnet() {
        let result = validate_bitcoin_address("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bitcoin_address_segwit_regtest() {
        let result = validate_bitcoin_address("bcrt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3xueyj");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bitcoin_address_invalid() {
        let result = validate_bitcoin_address("lnbc1test");
        assert!(matches!(result, Err(WalletError::InvalidBitcoinAddress(_))));
    }

    #[test]
    fn test_validate_lnurl() {
        let result = validate_lnurl("lnurl1dp68gurn8ghj7um9wfmxjcm99e3k7mf0v9cxj");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::Lnurl);
    }

    #[test]
    fn test_validate_lnurl_invalid() {
        let result = validate_lnurl("http://example.com");
        assert!(matches!(result, Err(WalletError::InvalidLnurl(_))));
    }

    #[test]
    fn test_validate_lightning_address() {
        let result = validate_lightning_address("user@example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::LightningAddress);
    }

    #[test]
    fn test_validate_lightning_address_invalid_no_domain() {
        let result = validate_lightning_address("user@");
        assert!(matches!(result, Err(WalletError::InvalidLnurl(_))));
    }

    #[test]
    fn test_validate_lightning_address_invalid_no_user() {
        let result = validate_lightning_address("@example.com");
        assert!(matches!(result, Err(WalletError::InvalidLnurl(_))));
    }

    #[test]
    fn test_validate_amount_zero() {
        let result = validate_amount(0);
        assert!(matches!(result, Err(WalletError::InvalidAmount(_))));
    }

    #[test]
    fn test_validate_amount_valid() {
        assert!(validate_amount(1000).is_ok());
        assert!(validate_amount(1).is_ok());
    }

    #[test]
    fn test_validate_amount_with_limit() {
        assert!(validate_amount_with_limit(500, Some(1000)).is_ok());
        let result = validate_amount_with_limit(2000, Some(1000));
        assert!(matches!(result, Err(WalletError::AmountExceedsLimit { .. })));
    }

    #[test]
    fn test_detect_destination_lightning() {
        let result = detect_and_validate_destination("lnbc1500n1pj9qjz4pp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypqdq5x");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::LightningInvoice);
    }

    #[test]
    fn test_detect_destination_spark() {
        let result = detect_and_validate_destination("sp1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::SparkAddress);
    }

    #[test]
    fn test_detect_destination_bitcoin() {
        let result = detect_and_validate_destination("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::BitcoinAddress);
    }

    #[test]
    fn test_detect_destination_lightning_address() {
        let result = detect_and_validate_destination("alice@pay.example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().destination_type, PaymentDestinationType::LightningAddress);
    }

    #[test]
    fn test_detect_destination_unknown() {
        let result = detect_and_validate_destination("random-string-12345");
        assert!(matches!(result, Err(WalletError::InvalidDestination(_))));
    }
}
