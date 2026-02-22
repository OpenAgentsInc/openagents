//! Rust-native L402 primitives used by OpenAgents payment workflows.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Duration, Utc};

/// Parsed L402 challenge components from a `WWW-Authenticate` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L402Challenge {
    /// L402 macaroon.
    pub macaroon: String,
    /// L402 BOLT11 invoice.
    pub invoice: String,
}

/// Parser for `WWW-Authenticate` values containing an L402 challenge.
#[derive(Debug, Default, Clone, Copy)]
pub struct WwwAuthenticateParser;

impl WwwAuthenticateParser {
    /// Parse an L402 challenge from a header value.
    ///
    /// Returns `None` when the header does not include a valid
    /// `L402 macaroon="...", invoice="..."` challenge.
    #[must_use]
    pub fn parse_l402_challenge(&self, header: Option<&str>) -> Option<L402Challenge> {
        let raw = header?.trim();
        if raw.is_empty() {
            return None;
        }

        let lowered = raw.to_ascii_lowercase();
        let pos = lowered.find("l402")?;
        let after = &raw[pos + 4..];

        let mut macaroon: Option<String> = None;
        let mut invoice: Option<String> = None;

        for part in after.split(',') {
            let segment = part.trim();
            if segment.is_empty() {
                continue;
            }

            let Some((key_raw, value_raw)) = segment.split_once('=') else {
                continue;
            };
            let key = key_raw.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }

            let value = parse_quoted_value(value_raw.trim())?;
            if value.is_empty() {
                continue;
            }

            match key.as_str() {
                "macaroon" => macaroon = Some(value),
                "invoice" => invoice = Some(value),
                _ => {}
            }
        }

        let macaroon = macaroon?;
        let invoice = invoice?;

        Some(L402Challenge { macaroon, invoice })
    }
}

/// Minimal BOLT11 amount parser used for L402 cap enforcement.
#[derive(Debug, Clone, Copy)]
pub struct Bolt11;

impl Bolt11 {
    /// Parse amount from a BOLT11 invoice and return millisatoshis.
    ///
    /// Returns `None` if the invoice is amountless, malformed, contains an
    /// unsupported multiplier, or overflows 64-bit arithmetic.
    #[must_use]
    pub fn amount_msats(invoice: &str) -> Option<u64> {
        let invoice = invoice.trim().to_ascii_lowercase();
        let bytes = invoice.as_bytes();
        if bytes.len() < 5 {
            return None;
        }

        if !invoice.starts_with("ln") {
            return None;
        }

        if !bytes[2].is_ascii_lowercase() || !bytes[3].is_ascii_lowercase() {
            return None;
        }

        let mut idx = 4usize;
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }

        if idx == digits_start {
            return None;
        }

        let amount = invoice[digits_start..idx].parse::<u64>().ok()?;

        let mut multiplier: Option<u8> = None;
        if idx < bytes.len() && matches!(bytes[idx], b'm' | b'u' | b'n' | b'p') {
            multiplier = Some(bytes[idx]);
            idx += 1;
        }

        if idx >= bytes.len() || bytes[idx] != b'1' {
            return None;
        }

        match multiplier {
            Some(b'p') => {
                if amount % 10 != 0 {
                    return None;
                }
                Some(amount / 10)
            }
            Some(b'm') => amount.checked_mul(100_000_000),
            Some(b'u') => amount.checked_mul(100_000),
            Some(b'n') => amount.checked_mul(100),
            None => amount.checked_mul(100_000_000_000),
            _ => None,
        }
    }
}

/// Cached L402 credential value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L402CredentialValue {
    /// L402 macaroon.
    pub macaroon: String,
    /// L402 preimage.
    pub preimage: String,
    /// Credential expiry timestamp.
    pub expires_at: DateTime<Utc>,
}

/// In-memory credential cache with TTL semantics.
#[derive(Debug, Clone, Default)]
pub struct L402CredentialCache {
    entries: Arc<RwLock<HashMap<String, L402CredentialValue>>>,
}

impl L402CredentialCache {
    /// Create an empty credential cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a credential by host and scope.
    ///
    /// Expired credentials are evicted and treated as missing.
    #[must_use]
    pub fn get(&self, host: &str, scope: &str, now: DateTime<Utc>) -> Option<L402CredentialValue> {
        let key = credential_key(host, scope);

        {
            let lock = self.entries.read().ok()?;
            let row = lock.get(&key)?.clone();
            if row.expires_at > now {
                return Some(row);
            }
        }

        if let Ok(mut lock) = self.entries.write() {
            lock.remove(&key);
        }
        None
    }

    /// Store/update a credential with TTL in seconds.
    pub fn put(
        &self,
        host: &str,
        scope: &str,
        macaroon: &str,
        preimage: &str,
        ttl_seconds: i64,
        now: DateTime<Utc>,
    ) {
        let ttl_seconds = ttl_seconds.max(1);
        let expires_at = now + Duration::seconds(ttl_seconds);
        let key = credential_key(host, scope);
        let value = L402CredentialValue {
            macaroon: macaroon.to_string(),
            preimage: preimage.to_string(),
            expires_at,
        };

        if let Ok(mut lock) = self.entries.write() {
            lock.insert(key, value);
        }
    }

    /// Delete a credential by host and scope.
    pub fn delete(&self, host: &str, scope: &str) {
        let key = credential_key(host, scope);
        if let Ok(mut lock) = self.entries.write() {
            lock.remove(&key);
        }
    }

    /// Number of cached credentials.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().map_or(0, |lock| lock.len())
    }

    /// Returns `true` when no cached credentials remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn parse_quoted_value(raw: &str) -> Option<String> {
    if raw.len() < 2 {
        return None;
    }
    if !raw.starts_with('"') || !raw.ends_with('"') {
        return None;
    }

    Some(raw[1..raw.len() - 1].to_string())
}

fn credential_key(host: &str, scope: &str) -> String {
    format!("{}::{}", host.trim(), scope.trim())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{Bolt11, L402CredentialCache, WwwAuthenticateParser};

    #[test]
    fn www_authenticate_parser_extracts_macaroon_and_invoice() {
        let parser = WwwAuthenticateParser;
        let challenge = parser.parse_l402_challenge(Some("L402 macaroon=\"mac\", invoice=\"inv\""));

        assert!(challenge.is_some());
        let challenge = if let Some(value) = challenge {
            value
        } else {
            assert!(false, "missing challenge");
            return;
        };
        assert_eq!(challenge.macaroon, "mac");
        assert_eq!(challenge.invoice, "inv");
    }

    #[test]
    fn www_authenticate_parser_finds_l402_in_multi_scheme_header() {
        let parser = WwwAuthenticateParser;
        let challenge = parser.parse_l402_challenge(Some(
            "Bearer realm=\"x\", L402 macaroon=\"mac\", invoice=\"inv\"",
        ));

        assert!(challenge.is_some());
        let challenge = if let Some(value) = challenge {
            value
        } else {
            assert!(false, "missing challenge");
            return;
        };
        assert_eq!(challenge.macaroon, "mac");
        assert_eq!(challenge.invoice, "inv");
    }

    #[test]
    fn www_authenticate_parser_rejects_empty_missing_or_invalid_values() {
        let parser = WwwAuthenticateParser;

        assert!(parser.parse_l402_challenge(None).is_none());
        assert!(parser.parse_l402_challenge(Some("")).is_none());
        assert!(
            parser
                .parse_l402_challenge(Some("Bearer realm=\"x\""))
                .is_none()
        );
        assert!(
            parser
                .parse_l402_challenge(Some("L402 macaroon=\"\", invoice=\"inv\""))
                .is_none()
        );
        assert!(
            parser
                .parse_l402_challenge(Some("L402 macaroon=\"mac\""))
                .is_none()
        );
    }

    #[test]
    fn bolt11_amount_msats_parses_common_multipliers() {
        assert_eq!(Bolt11::amount_msats("lnbc420n1test"), Some(42_000));
        assert_eq!(Bolt11::amount_msats("LNBC1m1TEST"), Some(100_000_000));
        assert_eq!(Bolt11::amount_msats("lnbc2500u1test"), Some(2500 * 100_000));
        assert_eq!(Bolt11::amount_msats("lnbc1n1test"), Some(100));
        assert_eq!(Bolt11::amount_msats("lnbc10p1test"), Some(1));
        assert_eq!(Bolt11::amount_msats("lnbc1000p1test"), Some(100));
    }

    #[test]
    fn bolt11_amount_msats_returns_none_for_amountless_invoice() {
        assert_eq!(Bolt11::amount_msats("lnbc1amountless"), None);
    }

    #[test]
    fn bolt11_amount_msats_rejects_invalid_formats() {
        assert_eq!(Bolt11::amount_msats("not-an-invoice"), None);
        assert_eq!(Bolt11::amount_msats("lnbc10x1test"), None);
        assert_eq!(Bolt11::amount_msats("lnbc1p1test"), None);
    }

    #[test]
    fn bolt11_amount_msats_rejects_overflow() {
        let unit_msats = 100_000_000_000_u64;
        let digits = (u64::MAX / unit_msats).saturating_add(1);
        let invoice = format!("lnbc{digits}1test");

        assert_eq!(Bolt11::amount_msats(&invoice), None);
    }

    #[test]
    fn credential_cache_round_trips_values_and_expiry() {
        let cache = L402CredentialCache::new();
        let now = Utc::now();

        cache.put(
            "fake-l402.local",
            "demo.fake",
            "macaroon_abc",
            &"a".repeat(64),
            60,
            now,
        );

        let value = cache.get("fake-l402.local", "demo.fake", now + Duration::seconds(1));
        assert!(value.is_some());
        let value = if let Some(value) = value {
            value
        } else {
            assert!(false, "missing cached value");
            return;
        };
        assert_eq!(value.macaroon, "macaroon_abc");
        assert_eq!(value.preimage, "a".repeat(64));
        assert!(value.expires_at > now);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn expired_credentials_are_deleted_on_read() {
        let cache = L402CredentialCache::new();
        let now = Utc::now();

        cache.put(
            "fake-l402.local",
            "demo.fake",
            "macaroon_abc",
            &"b".repeat(64),
            1,
            now,
        );

        let expired = cache.get("fake-l402.local", "demo.fake", now + Duration::seconds(2));
        assert!(expired.is_none());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn delete_removes_credentials() {
        let cache = L402CredentialCache::new();
        let now = Utc::now();

        cache.put(
            "fake-l402.local",
            "demo.fake",
            "macaroon_abc",
            &"c".repeat(64),
            60,
            now,
        );
        assert_eq!(cache.len(), 1);

        cache.delete("fake-l402.local", "demo.fake");
        assert_eq!(cache.len(), 0);
        assert!(cache.get("fake-l402.local", "demo.fake", now).is_none());
    }
}
