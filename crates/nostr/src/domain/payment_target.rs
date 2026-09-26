//! NIP-A3 payment-target discovery. A declared address is not payment authority.
//!
//! Parsing and URI construction do not validate a network address, resolve a
//! recipient, open a link, create an invoice, or spend funds.

use super::{DomainError, Event};

/// One declared payment target, including types unknown to this client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentTarget {
    pub payment_type: String,
    pub address: String,
}

impl PaymentTarget {
    /// Build an inert URI for a separately approved user interaction.
    ///
    /// Escape the address as data, so a declaration cannot inject a query,
    /// fragment, authority, or another URI scheme. Unknown types use `payto`.
    #[must_use]
    pub fn uri(&self) -> String {
        let address = uri_component(&self.address);
        match self.payment_type.as_str() {
            "bitcoin" | "ethereum" => format!("{}:{address}", self.payment_type),
            _ => format!("payto://{}/{address}", uri_component(&self.payment_type)),
        }
    }
}

/// Extract all payment targets in their published order from kind 10133.
///
/// Unknown lowercase types are preserved. Network-specific address validation
/// remains a separate consumer decision, never inferred from this event.
///
/// # Errors
///
/// Returns an error for a different kind, malformed tag, empty value, or a
/// payment type containing uppercase, whitespace, or control characters.
pub fn open_payment_targets(event: &Event) -> Result<Vec<PaymentTarget>, DomainError> {
    if event.kind != 10_133 {
        return Err(invalid("payment targets use kind 10133"));
    }
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("payto"))
        .map(|tag| {
            let values = tag.as_slice();
            if values.len() != 3 {
                return Err(invalid("a payto tag has a type and address"));
            }
            let payment_type = &values[1];
            if payment_type.is_empty()
                || payment_type
                    .chars()
                    .any(|c| c.is_uppercase() || c.is_whitespace() || c.is_control())
            {
                return Err(invalid("a payto type is a nonempty lowercase identifier"));
            }
            if values[2].is_empty() || values[2].chars().any(char::is_control) {
                return Err(invalid(
                    "a payto address is nonempty and contains no control characters",
                ));
            }
            Ok(PaymentTarget {
                payment_type: payment_type.clone(),
                address: values[2].clone(),
            })
        })
        .collect()
}

fn uri_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut escaped = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            escaped.push(char::from(byte));
        } else {
            escaped.push('%');
            escaped.push(char::from(HEX[usize::from(byte >> 4)]));
            escaped.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    escaped
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn event(tags: Vec<Tag>, time: u64) -> Event {
        RelaySigner::from_secret_hex(&"42".repeat(32))
            .unwrap()
            .sign(time, 10_133, tags, String::new())
    }

    #[test]
    fn targets_preserve_unknown_types_and_escape_uris_as_data() {
        let event = event(
            vec![
                Tag::new(vec!["payto".into(), "bitcoin".into(), "bc1qexample".into()]),
                Tag::new(vec![
                    "payto".into(),
                    "lightning".into(),
                    "name@example.com".into(),
                ]),
                Tag::new(vec![
                    "payto".into(),
                    "newtype".into(),
                    "user/name?amount=9#fragment".into(),
                ]),
            ],
            1,
        );
        event.validate_structure().unwrap();
        let targets = open_payment_targets(&event).unwrap();
        assert_eq!(targets[0].uri(), "bitcoin:bc1qexample");
        assert_eq!(targets[1].uri(), "payto://lightning/name%40example.com");
        assert_eq!(
            targets[2].uri(),
            "payto://newtype/user%2Fname%3Famount%3D9%23fragment"
        );
        assert_eq!(targets[2].address, "user/name?amount=9#fragment");
        assert_eq!(event.class(), EventClass::Replaceable);
        assert_eq!(
            compare_replacement(&event, &self::event(vec![], 2)).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
    }

    #[test]
    fn malformed_targets_refuse_without_discarding_unknown_valid_types() {
        for values in [
            vec!["payto", "bitcoin"],
            vec!["payto", "Bitcoin", "addr"],
            vec!["payto", "", "addr"],
            vec!["payto", "bitcoin", ""],
            vec!["payto", "bitcoin", "addr", "extra"],
        ] {
            assert!(
                open_payment_targets(&event(
                    vec![Tag::new(values.into_iter().map(str::to_owned).collect())],
                    1
                ))
                .is_err()
            );
        }
        assert!(open_payment_targets(&event(vec![], 1)).unwrap().is_empty());
        let mut wrong = event(vec![], 1);
        wrong.kind = 1;
        assert!(open_payment_targets(&wrong).is_err());
    }
}
