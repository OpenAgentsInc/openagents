//! NIP-FI offline assertion policy and issuer-scoped deny-set primitives.
//!
//! This module does not supply JWT signature algorithms, fetch JWKS, authenticate
//! a NIP-42 handshake, register sessions, or protect HTTP routes. An embedding
//! service must provide the trusted [`OfflineVerifier`] and all those lifecycle
//! integrations before advertising NIP-FI. The verifier receives the exact
//! compact token parsed here; unverified claims never become an identity.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::read_state_snapshot::hex_bytes;

const MAX_TOKEN_BYTES: usize = 65_536;

/// Public denial classes, with no private claim or dependency detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    MissingEvidence,
    EvidenceRejected,
    AuthorizationDenied,
    AuthorizationUnavailable,
}

impl Denial {
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::MissingEvidence => 401,
            Self::EvidenceRejected | Self::AuthorizationDenied => 403,
            Self::AuthorizationUnavailable => 503,
        }
    }
    #[must_use]
    pub fn body(self) -> &'static str {
        match self {
            Self::MissingEvidence => "authentication required\n",
            Self::EvidenceRejected => "evidence rejected\n",
            Self::AuthorizationDenied => "authorization denied\n",
            Self::AuthorizationUnavailable => "authorization unavailable\n",
        }
    }
    #[must_use]
    pub fn nostr_text(self) -> &'static str {
        match self {
            Self::MissingEvidence => "auth-required: authentication required",
            Self::EvidenceRejected => "restricted: evidence rejected",
            Self::AuthorizationDenied => "restricted: authorization denied",
            Self::AuthorizationUnavailable => "restricted: authorization unavailable",
        }
    }
}

/// Algorithms a configured verifier may support. Symmetric and unsigned JWTs
/// have no representation. An allowlist does not implement an algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Rs256,
    Rs384,
    Rs512,
    Ps256,
    Ps384,
    Ps512,
    Es256,
    Es384,
    Es512,
    Es256K,
    EdDsa,
}

impl Algorithm {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "RS256" => Self::Rs256,
            "RS384" => Self::Rs384,
            "RS512" => Self::Rs512,
            "PS256" => Self::Ps256,
            "PS384" => Self::Ps384,
            "PS512" => Self::Ps512,
            "ES256" => Self::Es256,
            "ES384" => Self::Es384,
            "ES512" => Self::Es512,
            "ES256K" => Self::Es256K,
            "EdDSA" => Self::EdDsa,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub enum TokenClass {
    Dedicated,
    Access {
        marker_claim: String,
        resource_owner_values: Vec<String>,
        client_subject_values: Vec<String>,
        admit_client_subjects: bool,
        disjoint_subject_coordinates: bool,
    },
}

/// Private deployment policy. Never publish or log these fields.
#[derive(Clone)]
pub struct IssuerPolicy {
    pub issuer: String,
    pub token_class: TokenClass,
    pub algorithms: Vec<Algorithm>,
    pub maximum_assertion_age: f64,
    pub skew: f64,
    pub max_token_bytes: usize,
    pub max_jwks_keys: usize,
}

impl IssuerPolicy {
    fn validate(&self) -> Result<(), Denial> {
        if self.issuer.is_empty()
            || self.algorithms.is_empty()
            || !positive(self.maximum_assertion_age)
            || !nonnegative(self.skew)
            || self.max_token_bytes == 0
            || self.max_token_bytes > MAX_TOKEN_BYTES
            || self.max_jwks_keys == 0
        {
            return Err(Denial::AuthorizationUnavailable);
        }
        if let TokenClass::Access {
            marker_claim,
            resource_owner_values,
            client_subject_values,
            admit_client_subjects,
            disjoint_subject_coordinates,
        } = &self.token_class
            && (marker_claim.is_empty()
                || resource_owner_values.is_empty()
                || client_subject_values.is_empty()
                || resource_owner_values
                    .iter()
                    .chain(client_subject_values)
                    .any(String::is_empty)
                || resource_owner_values
                    .iter()
                    .any(|value| client_subject_values.contains(value))
                || (*admit_client_subjects && !*disjoint_subject_coordinates))
        {
            return Err(Denial::AuthorizationUnavailable);
        }
        Ok(())
    }
}

/// Immutable result of trusted Host-to-community resolution. A caller must
/// enforce globally distinct canonical audiences across its community map.
#[derive(Clone)]
pub struct ResolvedCommunity {
    pub expected_audience: String,
    pub authorized_issuers: Vec<String>,
}

/// The signature adapter's authenticated, issuer-bound snapshot receipt.
/// This is private execution state, not a Nostr event or a public receipt.
pub struct KeySnapshot {
    pub issuer: String,
    pub hard_deadline: f64,
}

/// Trusted offline cryptography boundary. Implementations must use only the
/// policy's authenticated JWKS snapshot, bound its key count before `kid`
/// selection, pin algorithm and key type, and verify the exact compact JWS.
/// They must never follow token-supplied URLs or contact an IdP at admission.
pub trait OfflineVerifier {
    fn verify(
        &self,
        compact_jws: &str,
        policy: &IssuerPolicy,
        algorithm: Algorithm,
        key_id: &str,
        now: f64,
    ) -> Result<KeySnapshot, Denial>;
}

/// Authenticated identity and deadlines, kept out of Debug and serialization.
pub struct VerifiedAssertion {
    issuer: String,
    subject: String,
    asserted_key: String,
    authority_deadline: f64,
}

impl VerifiedAssertion {
    pub fn identity(&self) -> (&str, &str) {
        (&self.issuer, &self.subject)
    }
    pub fn asserted_key(&self) -> &str {
        &self.asserted_key
    }

    /// Pair with a separately authenticated, fresh NIP-42 or NIP-98 key.
    pub fn require_proven_key(&self, proven_key: &str) -> Result<(), Denial> {
        if proven_key == self.asserted_key {
            Ok(())
        } else {
            Err(Denial::AuthorizationDenied)
        }
    }

    /// Equality is expired. A connection cannot renew itself in band.
    pub fn session_deadline(
        &self,
        connection_time: f64,
        maximum_lifetime: f64,
    ) -> Result<f64, Denial> {
        if !nonnegative(connection_time) || !positive(maximum_lifetime) {
            return Err(Denial::AuthorizationUnavailable);
        }
        let limit = finite_add(connection_time, maximum_lifetime)?;
        let deadline = limit.min(self.authority_deadline);
        if connection_time >= deadline {
            return Err(Denial::EvidenceRejected);
        }
        Ok(deadline)
    }
}

/// Extract exactly one client-attached field. Duplicate/comma-combined fields,
/// padding, whitespace within the compact token, and URL transport are refused.
pub fn assertion_header<'a>(fields: &[&'a str]) -> Result<&'a str, Denial> {
    let value = match fields {
        [] => return Err(Denial::MissingEvidence),
        [value] => *value,
        _ => return Err(Denial::EvidenceRejected),
    };
    let (scheme, token) = value.split_once(' ').ok_or(Denial::EvidenceRejected)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.len() > MAX_TOKEN_BYTES
        || token
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b',')
    {
        return Err(Denial::EvidenceRejected);
    }
    Ok(token)
}

/// Verify class, crypto, exact community binding, identity, and time policy.
///
/// Unknown and community-unauthorized issuers fail before the adapter is called,
/// so their key-source availability cannot leak through the public denial.
pub fn verify_assertion(
    compact_jws: &str,
    community: Option<&ResolvedCommunity>,
    policies: &[IssuerPolicy],
    now: f64,
    verifier: &impl OfflineVerifier,
) -> Result<VerifiedAssertion, Denial> {
    let community = community.ok_or(Denial::AuthorizationUnavailable)?;
    if community.expected_audience.is_empty()
        || community.authorized_issuers.is_empty()
        || !nonnegative(now)
    {
        return Err(Denial::AuthorizationUnavailable);
    }
    if compact_jws.len() > MAX_TOKEN_BYTES {
        return Err(Denial::EvidenceRejected);
    }
    let parts = compact_jws.split('.').collect::<Vec<_>>();
    let [header, claims, signature] = parts.as_slice() else {
        return Err(Denial::EvidenceRejected);
    };
    let header = json_segment(header)?;
    let claims = json_segment(claims)?;
    if base64url(signature)?.is_empty() {
        return Err(Denial::EvidenceRejected);
    }
    let issuer = string(&claims, "iss")?;
    let matching = policies
        .iter()
        .filter(|policy| policy.issuer == issuer)
        .collect::<Vec<_>>();
    let [policy] = matching.as_slice() else {
        return Err(Denial::EvidenceRejected);
    };
    if !community
        .authorized_issuers
        .iter()
        .any(|allowed| allowed == issuer)
    {
        return Err(Denial::EvidenceRejected);
    }
    policy.validate()?;
    if compact_jws.len() > policy.max_token_bytes {
        return Err(Denial::EvidenceRejected);
    }
    validate_class(&header, &claims, &policy.token_class)?;
    let algorithm = Algorithm::parse(string(&header, "alg")?).ok_or(Denial::EvidenceRejected)?;
    if !policy.algorithms.contains(&algorithm)
        || header.get("crit").is_some()
        || header.get("b64").is_some()
    {
        return Err(Denial::EvidenceRejected);
    }
    let key_id = string(&header, "kid")?;
    if key_id.len() > 256 {
        return Err(Denial::EvidenceRejected);
    }
    let snapshot = verifier.verify(compact_jws, policy, algorithm, key_id, now)?;
    if snapshot.issuer != policy.issuer
        || !snapshot.hard_deadline.is_finite()
        || now >= snapshot.hard_deadline
    {
        return Err(Denial::AuthorizationUnavailable);
    }
    let audience = claims.get("aud").ok_or(Denial::EvidenceRejected)?;
    let audience_matches = match audience {
        Value::String(value) => value == &community.expected_audience,
        Value::Array(values) => {
            !values.is_empty()
                && values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|value| !value.is_empty()))
                && values
                    .iter()
                    .any(|value| value.as_str() == Some(&community.expected_audience))
        }
        _ => false,
    };
    if !audience_matches {
        return Err(Denial::EvidenceRejected);
    }
    let issued_at = number(&claims, "iat")?;
    let expires_at = number(&claims, "exp")?;
    let latest_issue = finite_add(now, policy.skew)?;
    let age_deadline = finite_add(issued_at, policy.maximum_assertion_age)?;
    if now >= expires_at
        || issued_at > latest_issue
        || now >= age_deadline
        || claims.get("nbf").is_some_and(|value| {
            value
                .as_f64()
                .is_none_or(|nbf| !nbf.is_finite() || nbf > latest_issue)
        })
    {
        return Err(Denial::EvidenceRejected);
    }
    let subject = string(&claims, "sub")?;
    let key = string(&claims, "nostr_pubkey")?;
    hex_bytes::<32>(key).map_err(|_| Denial::EvidenceRejected)?;
    Ok(VerifiedAssertion {
        issuer: issuer.into(),
        subject: subject.into(),
        asserted_key: key.into(),
        authority_deadline: expires_at.min(age_deadline).min(snapshot.hard_deadline),
    })
}

fn validate_class(header: &Value, claims: &Value, class: &TokenClass) -> Result<(), Denial> {
    match class {
        TokenClass::Dedicated if string(header, "typ")? == "nip-fi+jwt" => Ok(()),
        TokenClass::Access {
            marker_claim,
            resource_owner_values,
            client_subject_values,
            admit_client_subjects,
            ..
        } if string(header, "typ")? == "at+jwt" => {
            string(claims, "client_id")?;
            let marker = string(claims, marker_claim)?;
            if resource_owner_values.iter().any(|value| value == marker)
                || (*admit_client_subjects
                    && client_subject_values.iter().any(|value| value == marker))
            {
                Ok(())
            } else {
                Err(Denial::EvidenceRejected)
            }
        }
        _ => Err(Denial::EvidenceRejected),
    }
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Denial> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(Denial::EvidenceRejected)
}
fn number(value: &Value, key: &str) -> Result<f64, Denial> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or(Denial::EvidenceRejected)
}
fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn finite_add(left: f64, right: f64) -> Result<f64, Denial> {
    let value = left + right;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Denial::EvidenceRejected)
    }
}
fn json_segment(segment: &str) -> Result<Value, Denial> {
    let value = crate::contracts::parse_strict(&base64url(segment)?)
        .map_err(|_| Denial::EvidenceRejected)?;
    if !value.is_object() {
        return Err(Denial::EvidenceRejected);
    }
    Ok(value)
}
fn base64url(text: &str) -> Result<Vec<u8>, Denial> {
    if text.is_empty() || text.len() % 4 == 1 {
        return Err(Denial::EvidenceRejected);
    }
    let mut output = Vec::with_capacity(text.len() * 3 / 4);
    let mut bits: u32 = 0;
    let mut count = 0;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(Denial::EvidenceRejected),
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    if bits != 0 {
        return Err(Denial::EvidenceRejected);
    }
    Ok(output)
}

/// In-memory issuer partitions. The caller atomically combines insertion with
/// command replay admission and session cancellation; this object does neither.
#[derive(Default)]
pub struct DenySet {
    entries: BTreeMap<String, BTreeMap<String, f64>>,
}

impl DenySet {
    /// Insert before closing sessions. Capacity failure changes no live hold;
    /// active holds only extend, including when a past-until command arrives.
    pub fn apply(
        &mut self,
        policy: &IssuerPolicy,
        key: &str,
        until: f64,
        now: f64,
        issuer_capacity: usize,
    ) -> Result<(), Denial> {
        policy.validate()?;
        hex_bytes::<32>(key).map_err(|_| Denial::EvidenceRejected)?;
        let ceiling = finite_add(finite_add(now, policy.skew)?, policy.maximum_assertion_age)?;
        if !nonnegative(now) || !until.is_finite() || until > ceiling {
            return Err(Denial::EvidenceRejected);
        }
        if issuer_capacity == 0 {
            return Err(Denial::AuthorizationUnavailable);
        }
        let entries = self.entries.entry(policy.issuer.clone()).or_default();
        entries.retain(|_, expiry| now < *expiry);
        if let Some(existing) = entries.get_mut(key) {
            *existing = existing.max(until);
            return Ok(());
        }
        if until <= now {
            return Ok(());
        }
        if entries.len() >= issuer_capacity {
            return Err(Denial::AuthorizationUnavailable);
        }
        entries.insert(key.into(), until);
        Ok(())
    }

    #[must_use]
    pub fn denies(&self, issuer: &str, key: &str, now: f64) -> bool {
        !nonnegative(now)
            || self
                .entries
                .get(issuer)
                .and_then(|entries| entries.get(key))
                .is_some_and(|until| now < *until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    struct TestVerifier {
        calls: Cell<usize>,
    }
    impl OfflineVerifier for TestVerifier {
        fn verify(
            &self,
            _: &str,
            policy: &IssuerPolicy,
            _: Algorithm,
            _: &str,
            _: f64,
        ) -> Result<KeySnapshot, Denial> {
            self.calls.set(self.calls.get() + 1);
            Ok(KeySnapshot {
                issuer: policy.issuer.clone(),
                hard_deadline: 190.0,
            })
        }
    }
    fn policy() -> IssuerPolicy {
        IssuerPolicy {
            issuer: "https://issuer.example".into(),
            token_class: TokenClass::Dedicated,
            algorithms: vec![Algorithm::Es256],
            maximum_assertion_age: 100.0,
            skew: 0.0,
            max_token_bytes: 4096,
            max_jwks_keys: 8,
        }
    }
    fn community() -> ResolvedCommunity {
        ResolvedCommunity {
            expected_audience: "https://relay.example".into(),
            authorized_issuers: vec![policy().issuer],
        }
    }
    fn encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        let mut bits = 0_u32;
        let mut count = 0;
        for byte in bytes {
            bits = (bits << 8) | u32::from(*byte);
            count += 8;
            while count >= 6 {
                count -= 6;
                out.push(ALPHABET[((bits >> count) & 63) as usize] as char);
                bits &= (1 << count) - 1;
            }
        }
        if count > 0 {
            out.push(ALPHABET[(bits << (6 - count)) as usize] as char);
        }
        out
    }
    fn token(header: Value, claims: Value) -> String {
        format!(
            "{}.{}.AQ",
            encode(&serde_json::to_vec(&header).unwrap()),
            encode(&serde_json::to_vec(&claims).unwrap())
        )
    }
    fn claims() -> Value {
        json!({"iss":policy().issuer,"sub":"opaque-id","aud":community().expected_audience,"nostr_pubkey":"11".repeat(32),"iat":100,"exp":200})
    }
    fn header() -> Value {
        json!({"typ":"nip-fi+jwt","alg":"ES256","kid":"current"})
    }

    #[test]
    fn unauthorized_issuer_does_not_touch_key_dependency() {
        let verifier = TestVerifier {
            calls: Cell::new(0),
        };
        let mut community = community();
        community.authorized_issuers = vec!["https://other.example".into()];
        assert!(matches!(
            verify_assertion(
                &token(header(), claims()),
                Some(&community),
                &[policy()],
                150.0,
                &verifier
            ),
            Err(Denial::EvidenceRejected)
        ));
        assert_eq!(verifier.calls.get(), 0);
        assert!(matches!(
            verify_assertion(
                &token(header(), claims()),
                None,
                &[policy()],
                150.0,
                &verifier
            ),
            Err(Denial::AuthorizationUnavailable)
        ));
        assert_eq!(verifier.calls.get(), 0);
    }

    #[test]
    fn policy_pairs_exact_key_and_caps_session_by_all_deadlines() {
        let verifier = TestVerifier {
            calls: Cell::new(0),
        };
        let assertion = verify_assertion(
            &token(header(), claims()),
            Some(&community()),
            &[policy()],
            150.0,
            &verifier,
        )
        .unwrap();
        assert_eq!(
            assertion.identity(),
            ("https://issuer.example", "opaque-id")
        );
        assertion.require_proven_key(&"11".repeat(32)).unwrap();
        assert_eq!(
            assertion.require_proven_key(&"22".repeat(32)),
            Err(Denial::AuthorizationDenied)
        );
        assert_eq!(assertion.session_deadline(150.0, 100.0), Ok(190.0));
        assert_eq!(assertion.session_deadline(150.0, 10.0), Ok(160.0));
        for (field, value) in [
            ("exp", json!(150)),
            ("iat", json!(151)),
            ("nbf", json!(151)),
            ("aud", json!("https://other.example")),
            ("nostr_pubkey", json!("nothex")),
        ] {
            let mut changed = claims();
            changed[field] = value;
            assert!(
                verify_assertion(
                    &token(header(), changed),
                    Some(&community()),
                    &[policy()],
                    150.0,
                    &verifier
                )
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn access_class_requires_marker_and_never_falls_back() {
        let verifier = TestVerifier {
            calls: Cell::new(0),
        };
        let mut policy = policy();
        policy.token_class = TokenClass::Access {
            marker_claim: "subject_type".into(),
            resource_owner_values: vec!["user".into()],
            client_subject_values: vec!["client".into()],
            admit_client_subjects: false,
            disjoint_subject_coordinates: false,
        };
        let mut h = header();
        h["typ"] = json!("at+jwt");
        let mut c = claims();
        c["client_id"] = json!("application");
        c["subject_type"] = json!("user");
        verify_assertion(
            &token(h.clone(), c.clone()),
            Some(&community()),
            &[policy.clone()],
            150.0,
            &verifier,
        )
        .unwrap();
        c["subject_type"] = json!("client");
        assert!(
            verify_assertion(
                &token(h, c),
                Some(&community()),
                &[policy.clone()],
                150.0,
                &verifier
            )
            .is_err()
        );
        assert!(
            verify_assertion(
                &token(header(), claims()),
                Some(&community()),
                &[policy],
                150.0,
                &verifier
            )
            .is_err()
        );
        let mut h = header();
        h["alg"] = json!("HS256");
        assert!(
            verify_assertion(
                &token(h, claims()),
                Some(&community()),
                &[super::tests::policy()],
                150.0,
                &verifier
            )
            .is_err()
        );
    }

    #[test]
    fn deny_partitions_preserve_longer_holds_and_fail_without_cross_issuer_effects() {
        let mut set = DenySet::default();
        let policy = policy();
        let key = "11".repeat(32);
        let other = "22".repeat(32);
        set.apply(&policy, &key, 190.0, 150.0, 1).unwrap();
        set.apply(&policy, &key, 140.0, 150.0, 1).unwrap();
        assert!(set.denies(&policy.issuer, &key, 180.0));
        assert_eq!(
            set.apply(&policy, &other, 190.0, 150.0, 1),
            Err(Denial::AuthorizationUnavailable)
        );
        let mut second = policy.clone();
        second.issuer = "https://second.example".into();
        set.apply(&second, &other, 190.0, 150.0, 1).unwrap();
        assert!(!set.denies(&policy.issuer, &key, 190.0));
        assert!(set.apply(&policy, &other, 251.0, 150.0, 1).is_err());
    }

    #[test]
    fn transport_rejects_combined_headers_and_noncanonical_base64() {
        assert_eq!(assertion_header(&[]), Err(Denial::MissingEvidence));
        assert_eq!(
            assertion_header(&["Bearer a.b.c", "Bearer a.b.c"]),
            Err(Denial::EvidenceRejected)
        );
        assert_eq!(
            assertion_header(&["Bearer a.b.c,foo"]),
            Err(Denial::EvidenceRejected)
        );
        assert!(base64url("AR").is_err());
        assert!(base64url("AQ==").is_err());
        assert_eq!(base64url("AQ").unwrap(), [1]);
    }
}
