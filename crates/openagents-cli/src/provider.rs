//! `oa provider settle`: the settlement gate.
//!
//! A port of `packages/openagents-cli/src/provider-settlement.ts`, gate for
//! gate and message for message, so the same lease and the same closeout
//! receipt produce the same decision from either binary.
//!
//! What the gate is for: a lease is not an earning claim and a provider's own
//! submission is not a receipt. Only a NIP-LBR closeout receipt — one that
//! names this job and this provider, was not issued by the provider to itself,
//! carries a verification command and the evidence it produced, carries the
//! platform's own closeout, is content-addressable, landed inside the lease
//! window, and prices the job exactly as the lease did — earns anything.
//!
//! What it refuses to be: it moves no money and holds no key. A settled
//! decision is an accrual record. `payout_rail` is `not_connected` and
//! `custody` is `none`, and outbound payout stays on the MDK/Nexus bridge.
//!
//! Presence is not an input. [`settle_lease`] takes a lease and a closeout, so
//! a provider that never earns a closeout earns zero no matter how long it is
//! online.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A lease: the buyer's grant of one job to one provider at one price.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderLease {
    pub job_id: String,
    pub lane: String,
    pub provider: String,
    pub price_msats: f64,
    pub expires_at: String,
}

/// The public-safe fields of a NIP-LBR closeout receipt this gate reads.
///
/// A structural mirror of `LbrLaborCloseout`; the names are that module's
/// names, so a receipt produced there is accepted here without translation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LaborCloseoutReceipt {
    pub receipt_ref: String,
    pub request_id: String,
    pub requester_pubkey: String,
    pub provider_pubkey: String,
    pub quoted_amount_msats: f64,
    /// What was run to check the work. Empty means nothing checked it.
    pub verification_command_ref: String,
    /// The evidence that check produced.
    pub test_ref: String,
    /// The platform's own closeout. Settlement authority lives there.
    pub platform_closeout_ref: String,
    /// SHA-256 over the canonical projection.
    pub digest: String,
    pub settled_at: String,
}

/// Why a settlement did not happen. Each one is a distinct, nameable failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementRefusal {
    PriceNotPayable,
    NoCloseout,
    CloseoutJobMismatch,
    CloseoutProviderMismatch,
    SelfDealt,
    WorkNotVerified,
    NoSettlementAuthority,
    ReceiptNotAddressable,
    LeaseExpired,
    PriceMismatch,
}

impl SettlementRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PriceNotPayable => "price_not_payable",
            Self::NoCloseout => "no_closeout",
            Self::CloseoutJobMismatch => "closeout_job_mismatch",
            Self::CloseoutProviderMismatch => "closeout_provider_mismatch",
            Self::SelfDealt => "self_dealt",
            Self::WorkNotVerified => "work_not_verified",
            Self::NoSettlementAuthority => "no_settlement_authority",
            Self::ReceiptNotAddressable => "receipt_not_addressable",
            Self::LeaseExpired => "lease_expired",
            Self::PriceMismatch => "price_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettlementDecision {
    pub job_id: String,
    /// `settled` or `unsettled`.
    pub state: &'static str,
    /// What the verified job is owed. Zero on every path but a clean receipt.
    pub earned_msats: f64,
    pub reason: String,
    pub refusal: Option<SettlementRefusal>,
    pub receipt_ref: Option<String>,
}

impl SettlementDecision {
    /// The document `--json` prints. Field for field the TypeScript shape,
    /// including the two constants that say this decision moved nothing.
    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "schema".into(),
            Value::String("openagents.provider_settlement.v1".into()),
        );
        map.insert("job_id".into(), Value::String(self.job_id.clone()));
        map.insert("state".into(), Value::String(self.state.into()));
        map.insert("earned_msats".into(), number(self.earned_msats));
        map.insert("reason".into(), Value::String(self.reason.clone()));
        if let Some(refusal) = self.refusal {
            map.insert("refusal".into(), Value::String(refusal.as_str().into()));
        }
        map.insert("payout_rail".into(), Value::String("not_connected".into()));
        map.insert("custody".into(), Value::String("none".into()));
        if let Some(receipt) = &self.receipt_ref {
            map.insert("receipt_ref".into(), Value::String(receipt.clone()));
        }
        Value::Object(map)
    }

    /// The lines the human mode prints, in the TypeScript order.
    pub fn human(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Job: {}", self.job_id),
            format!("Outcome: {}", self.state),
            format!("Earned: {} msats", render_number(self.earned_msats)),
        ];
        if let Some(refusal) = self.refusal {
            lines.push(format!("Refused: {}", refusal.as_str()));
        }
        if let Some(receipt) = &self.receipt_ref {
            lines.push(format!("Receipt: {}", receipt));
        }
        lines.push(self.reason.clone());
        lines.push(
            "Accrual only: this command holds no key, connects no payout rail, and moves nothing."
                .to_string(),
        );
        lines
    }
}

/// JSON has one number type and the TypeScript writes `1200`, not `1200.0`.
fn number(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 {
        return Value::from(value as i64);
    }
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// `NaN` reads as `NaN` in JavaScript, and a lease priced by a missing field
/// should say so rather than say `0`.
fn render_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.fract() == 0.0 && value.is_finite() {
        return format!("{}", value as i64);
    }
    format!("{}", value)
}

fn unsettled(job_id: &str, refusal: SettlementRefusal, reason: String) -> SettlementDecision {
    SettlementDecision {
        job_id: job_id.to_string(),
        state: "unsettled",
        earned_msats: 0.0,
        reason,
        refusal: Some(refusal),
        receipt_ref: None,
    }
}

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// A 32-byte hex hash, the only digest a receipt can be dereferenced by.
fn addressable(digest: &str) -> bool {
    digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit())
}

/// RFC 3339 to milliseconds, or `None` for anything unparseable.
///
/// Only the ordering of two instants matters here, so this reads the fields it
/// needs rather than pulling a date library into a crate that has none.
pub fn parse_time(value: &str) -> Option<i64> {
    let text = value.trim();
    if text.len() < 19 {
        return None;
    }
    let bytes = text.as_bytes();
    let digits = |from: usize, to: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[from..to]).ok()?.parse().ok()
    };
    if bytes[4] != b'-' || bytes[7] != b'-' || (bytes[10] != b'T' && bytes[10] != b' ') {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let year = digits(0, 4)?;
    let month = digits(5, 7)?;
    let day = digits(8, 10)?;
    let hour = digits(11, 13)?;
    let minute = digits(14, 16)?;
    let second = digits(17, 19)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let mut fraction_ms = 0i64;
    let mut cursor = 19;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        let mut place = 100;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            if place > 0 {
                fraction_ms += i64::from(bytes[cursor] - b'0') * place;
                place /= 10;
            }
            cursor += 1;
        }
    }

    // Offset, if the timestamp carries one. `Z` and a missing offset are both
    // read as UTC, which is what `Date.parse` does for an RFC 3339 instant.
    let mut offset_minutes = 0i64;
    if cursor < bytes.len() {
        match bytes[cursor] {
            b'Z' | b'z' => {}
            sign @ (b'+' | b'-') => {
                if bytes.len() < cursor + 6 || bytes[cursor + 3] != b':' {
                    return None;
                }
                let hours: i64 = std::str::from_utf8(&bytes[cursor + 1..cursor + 3])
                    .ok()?
                    .parse()
                    .ok()?;
                let minutes: i64 = std::str::from_utf8(&bytes[cursor + 4..cursor + 6])
                    .ok()?
                    .parse()
                    .ok()?;
                let total = hours * 60 + minutes;
                offset_minutes = if sign == b'+' { total } else { -total };
            }
            _ => return None,
        }
    }

    Some(
        (days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second) * 1_000
            + fraction_ms
            - offset_minutes * 60_000,
    )
}

/// Days since 1970-01-01, by Howard Hinnant's civil-from-days inverse.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Decide what one leased job earns.
///
/// The gates run in the order a reader would check them by hand, so a refusal
/// names the first thing that is actually wrong rather than the last.
pub fn settle_lease(
    lease: &ProviderLease,
    closeout: Option<&LaborCloseoutReceipt>,
) -> SettlementDecision {
    if !lease.price_msats.is_finite() || lease.price_msats <= 0.0 {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::PriceNotPayable,
            format!(
                "The lease prices this job at {} msats, so there is nothing to settle.",
                render_number(lease.price_msats)
            ),
        );
    }

    let Some(closeout) = closeout else {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::NoCloseout,
            "No closeout receipt covers this job. A lease is not an earning claim and a submission \
             is not a receipt, so this earns nothing."
                .to_string(),
        );
    };

    if closeout.request_id != lease.job_id {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::CloseoutJobMismatch,
            format!(
                "The receipt closes out job {}, not the leased job {}.",
                closeout.request_id, lease.job_id
            ),
        );
    }

    if closeout.provider_pubkey != lease.provider {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::CloseoutProviderMismatch,
            format!(
                "The receipt credits provider {}, but the lease is held by {}.",
                closeout.provider_pubkey, lease.provider
            ),
        );
    }

    if closeout.requester_pubkey == closeout.provider_pubkey {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::SelfDealt,
            "The receipt names the same key as requester and provider. A provider cannot buy its \
             own work into an earning."
                .to_string(),
        );
    }

    if blank(&closeout.verification_command_ref) || blank(&closeout.test_ref) {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::WorkNotVerified,
            "The receipt carries no verification command and evidence pair, so nothing checked \
             this work. Unverified work earns nothing."
                .to_string(),
        );
    }

    if blank(&closeout.platform_closeout_ref) {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::NoSettlementAuthority,
            "The receipt carries no platform closeout ref. Settlement authority stays in the \
             platform receipt systems; the relay is only transport."
                .to_string(),
        );
    }

    if !addressable(&closeout.digest) {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::ReceiptNotAddressable,
            "The receipt digest is not a 32-byte hex hash, so the receipt cannot be dereferenced \
             and re-verified."
                .to_string(),
        );
    }

    let expires_at = parse_time(&lease.expires_at);
    let settled_at = parse_time(&closeout.settled_at);
    let (Some(expires_at), Some(settled_at)) = (expires_at, settled_at) else {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::LeaseExpired,
            "The lease window could not be read, so the closeout cannot be placed inside it."
                .to_string(),
        );
    };
    if settled_at > expires_at {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::LeaseExpired,
            format!(
                "The job closed out at {}, after the lease expired at {}.",
                closeout.settled_at, lease.expires_at
            ),
        );
    }

    if closeout.quoted_amount_msats != lease.price_msats {
        return unsettled(
            &lease.job_id,
            SettlementRefusal::PriceMismatch,
            format!(
                "The receipt quotes {} msats but the lease priced the job at {} msats.",
                render_number(closeout.quoted_amount_msats),
                render_number(lease.price_msats)
            ),
        );
    }

    SettlementDecision {
        job_id: lease.job_id.clone(),
        state: "settled",
        earned_msats: closeout.quoted_amount_msats,
        reason: format!(
            "Verified by {} with evidence {}, closed out by {}. Accrued, not paid: no payout rail \
             is connected.",
            closeout.verification_command_ref, closeout.test_ref, closeout.platform_closeout_ref
        ),
        refusal: None,
        receipt_ref: Some(closeout.receipt_ref.clone()),
    }
}

// ---------------------------------------------------------------------------
// reading the two documents
// ---------------------------------------------------------------------------

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn count(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// Read a lease document.
///
/// The four fields the gate needs are required; a lease missing one is a typo
/// the reader would rather hear about now than as a mysterious refusal.
pub fn decode_lease(value: &Value, path: &str) -> Result<ProviderLease, String> {
    if !value.is_object() {
        return Err(format!("The lease at {path} is not a JSON object."));
    }
    let missing: Vec<&str> = ["job_id", "lane", "provider", "expires_at"]
        .into_iter()
        .filter(|field| match value.get(*field).and_then(Value::as_str) {
            Some(text) => text.is_empty(),
            None => true,
        })
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "The lease at {path} is missing {}.",
            missing.join(", ")
        ));
    }
    Ok(ProviderLease {
        job_id: text(value, "job_id"),
        lane: text(value, "lane"),
        provider: text(value, "provider"),
        price_msats: count(value, "price_msats"),
        expires_at: text(value, "expires_at"),
    })
}

/// Read a closeout receipt.
///
/// Absent fields become empty strings rather than an error: the gate already
/// has a named refusal for each of them, and a receipt missing its
/// verification refs should be refused as unverified work, not as a bad file.
pub fn decode_closeout(value: &Value, path: &str) -> Result<LaborCloseoutReceipt, String> {
    if !value.is_object() {
        return Err(format!("The closeout at {path} is not a JSON object."));
    }
    Ok(LaborCloseoutReceipt {
        receipt_ref: text(value, "receiptRef"),
        request_id: text(value, "requestId"),
        requester_pubkey: text(value, "requesterPubkey"),
        provider_pubkey: text(value, "providerPubkey"),
        quoted_amount_msats: count(value, "quotedAmountMsats"),
        verification_command_ref: text(value, "verificationCommandRef"),
        test_ref: text(value, "testRef"),
        platform_closeout_ref: text(value, "platformCloseoutRef"),
        digest: text(value, "digest"),
        settled_at: text(value, "settled_at"),
    })
}

/// Read a JSON file, or say which one could not be read.
pub fn read_json_file(path: &str, label: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| format!("The {label} file at {path} could not be read as JSON."))?;
    serde_json::from_str(&text)
        .map_err(|_| format!("The {label} file at {path} could not be read as JSON."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lease() -> ProviderLease {
        ProviderLease {
            job_id: "job-1".into(),
            lane: "coding".into(),
            provider: "provider-key".into(),
            price_msats: 1_200.0,
            expires_at: "2026-08-26T12:00:00Z".into(),
        }
    }

    fn clean_closeout() -> LaborCloseoutReceipt {
        LaborCloseoutReceipt {
            receipt_ref: "lbr-closeout:job-1:".to_string() + &"a".repeat(64),
            request_id: "job-1".into(),
            requester_pubkey: "buyer-key".into(),
            provider_pubkey: "provider-key".into(),
            quoted_amount_msats: 1_200.0,
            verification_command_ref: "cmd:mix test".into(),
            test_ref: "evidence:run-9".into(),
            platform_closeout_ref: "platform:closeout-3".into(),
            digest: "a".repeat(64),
            settled_at: "2026-08-26T11:00:00Z".into(),
        }
    }

    #[test]
    fn a_clean_receipt_settles_for_the_quoted_amount() {
        let decision = settle_lease(&lease(), Some(&clean_closeout()));
        assert_eq!(decision.state, "settled");
        assert_eq!(decision.earned_msats, 1_200.0);
        assert!(decision.refusal.is_none());
        let json = decision.to_json();
        assert_eq!(json["payout_rail"], "not_connected");
        assert_eq!(json["custody"], "none");
        assert_eq!(json["earned_msats"], 1_200);
    }

    #[test]
    fn a_lease_without_a_receipt_earns_nothing() {
        let decision = settle_lease(&lease(), None);
        assert_eq!(decision.state, "unsettled");
        assert_eq!(decision.earned_msats, 0.0);
        assert_eq!(decision.refusal, Some(SettlementRefusal::NoCloseout));
    }

    #[test]
    fn each_gate_names_the_first_thing_wrong() {
        // The gates are ordered, so a receipt broken in two ways reports the
        // earlier break. Each case below breaks exactly one.
        type Break = Box<dyn Fn(&mut LaborCloseoutReceipt)>;
        let cases: Vec<(SettlementRefusal, Break)> = vec![
            (
                SettlementRefusal::CloseoutJobMismatch,
                Box::new(|c: &mut LaborCloseoutReceipt| c.request_id = "job-2".into()),
            ),
            (
                SettlementRefusal::CloseoutProviderMismatch,
                Box::new(|c: &mut LaborCloseoutReceipt| c.provider_pubkey = "someone".into()),
            ),
            (
                SettlementRefusal::WorkNotVerified,
                Box::new(|c: &mut LaborCloseoutReceipt| c.test_ref = String::new()),
            ),
            (
                SettlementRefusal::NoSettlementAuthority,
                Box::new(|c: &mut LaborCloseoutReceipt| c.platform_closeout_ref = String::new()),
            ),
            (
                SettlementRefusal::ReceiptNotAddressable,
                Box::new(|c: &mut LaborCloseoutReceipt| c.digest = "short".into()),
            ),
            (
                SettlementRefusal::LeaseExpired,
                Box::new(|c: &mut LaborCloseoutReceipt| {
                    c.settled_at = "2026-08-26T13:00:00Z".into()
                }),
            ),
            (
                SettlementRefusal::PriceMismatch,
                Box::new(|c: &mut LaborCloseoutReceipt| c.quoted_amount_msats = 900.0),
            ),
        ];
        for (expected, break_it) in cases {
            let mut closeout = clean_closeout();
            break_it(&mut closeout);
            let decision = settle_lease(&lease(), Some(&closeout));
            assert_eq!(
                decision.refusal,
                Some(expected),
                "expected {}",
                expected.as_str()
            );
            assert_eq!(decision.earned_msats, 0.0);
        }
    }

    #[test]
    fn a_self_dealt_receipt_earns_nothing() {
        let mut closeout = clean_closeout();
        closeout.requester_pubkey = closeout.provider_pubkey.clone();
        let decision = settle_lease(&lease(), Some(&closeout));
        assert_eq!(decision.refusal, Some(SettlementRefusal::SelfDealt));
    }

    #[test]
    fn a_lease_priced_at_zero_has_nothing_to_settle() {
        let mut lease = lease();
        lease.price_msats = 0.0;
        let decision = settle_lease(&lease, Some(&clean_closeout()));
        assert_eq!(decision.refusal, Some(SettlementRefusal::PriceNotPayable));
        assert!(decision.reason.contains("0 msats"));
    }

    #[test]
    fn a_lease_missing_a_required_field_is_named() {
        let value = serde_json::json!({ "job_id": "job-1", "lane": "coding" });
        let error = decode_lease(&value, "/tmp/lease.json").unwrap_err();
        assert_eq!(
            error,
            "The lease at /tmp/lease.json is missing provider, expires_at."
        );
    }

    #[test]
    fn timestamps_order_the_way_date_parse_does() {
        assert!(
            parse_time("2026-08-26T11:00:00Z").unwrap()
                < parse_time("2026-08-26T12:00:00Z").unwrap()
        );
        // An offset moves the instant, so 13:00+02:00 is before 12:00Z.
        assert!(
            parse_time("2026-08-26T13:00:00+02:00").unwrap()
                < parse_time("2026-08-26T12:00:00Z").unwrap()
        );
        assert_eq!(
            parse_time("2026-08-26T11:00:00.500Z").unwrap()
                - parse_time("2026-08-26T11:00:00Z").unwrap(),
            500
        );
        assert!(parse_time("not a time").is_none());
        assert!(parse_time("").is_none());
    }
}
