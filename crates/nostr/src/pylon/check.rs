//! Check verdicts: NIP-32 labels (`1985`) in the `openagents.pylon`
//! namespace that point at one `3201` receipt and say whether a checker's
//! own run of that job agreed with the pylon's.
//!
//! A verdict is a checker's claim, as a receipt is a buyer's. A reader
//! counts one only from a checker on its trust list, never from the buyer
//! or provider of the receipt it names, and only once it has the receipt
//! the label points at. [`standings`] folds counted verdicts into each
//! pylon's standing: one counted `check-fail` marks the pylon failing for
//! as long as the reader keeps that verdict in its window, which is what a
//! pool policy with `exclude_failed` uses to drop the pylon from admission.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{CheckTotals, Receipt, hex64, single_tag};
use crate::domain::{Event, RelaySigner, Tag};

/// The NIP-32 label kind.
pub const CHECK_KIND: u16 = 1_985;
/// The label namespace (`L`) and mark every verdict carries.
pub const CHECK_NAMESPACE: &str = "openagents.pylon";
/// The longest label content, in bytes.
pub const MAX_CHECK_CONTENT: usize = 512;
/// The most labels one aggregate counts.
pub const MAX_CHECKS: usize = 65_536;

/// A checker's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}

impl Verdict {
    /// The `l` value: `check-pass`, `check-fail`, or `check-inconclusive`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "check-pass",
            Self::Fail => "check-fail",
            Self::Inconclusive => "check-inconclusive",
        }
    }

    /// The verdict an `l` value names.
    #[must_use]
    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "check-pass" => Some(Self::Pass),
            "check-fail" => Some(Self::Fail),
            "check-inconclusive" => Some(Self::Inconclusive),
            _ => None,
        }
    }
}

/// A verified verdict label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// The label's event ID.
    pub id: String,
    /// The signer.
    pub checker: String,
    pub verdict: Verdict,
    /// The `3201` receipt checked.
    pub receipt: String,
    /// The pylon key (`p`).
    pub provider: String,
    /// SHA-256 of the checker's own result plaintext (`x`).
    pub result_digest: String,
    /// The method, such as `canary exact-match suite:<digest>`.
    pub method: String,
    pub created_at: u64,
}

impl Check {
    /// The Gym suite digest the method names (`suite:<64 hex>`), if any.
    #[must_use]
    pub fn suite(&self) -> Option<&str> {
        self.method
            .split_whitespace()
            .filter_map(|word| word.strip_prefix("suite:"))
            .find(|digest| hex64(digest, "suite").is_ok())
    }
}

/// Sign a verdict on the receipt event `receipt` as a `1985` label.
/// `result_digest` is the SHA-256 of the checker's own result plaintext;
/// `method` is inert text naming how it checked.
///
/// # Errors
///
/// When the receipt doesn't verify, the checker is its buyer or provider,
/// or the method is empty or longer than 512 bytes.
pub fn check_event(
    checker: &RelaySigner,
    verdict: Verdict,
    receipt: &Event,
    result_digest: &str,
    method: &str,
    created_at: u64,
) -> Result<Event, String> {
    let body = super::parse_receipt(receipt, None)?;
    hex64(result_digest, "result digest")?;
    if method.is_empty() || method.len() > MAX_CHECK_CONTENT {
        return Err("a verdict's method is 1 to 512 bytes".into());
    }
    if checker.pubkey() == body.buyer || checker.pubkey() == body.provider {
        return Err("a receipt's buyer or provider can't check it".into());
    }
    let tags = vec![
        Tag::new(vec!["L".into(), CHECK_NAMESPACE.into()]),
        Tag::new(vec![
            "l".into(),
            verdict.label().into(),
            CHECK_NAMESPACE.into(),
        ]),
        Tag::new(vec!["e".into(), receipt.id.clone()]),
        Tag::new(vec!["p".into(), body.provider.clone()]),
        Tag::new(vec!["x".into(), result_digest.into()]),
    ];
    Ok(checker.sign(created_at, CHECK_KIND, tags, method.into()))
}

/// Verify a `1985` verdict label on its own: signature, namespace, exactly
/// one verdict, receipt, pylon, and digest, and bounded content. Whether
/// the checker is trusted and the receipt agrees is [`bind_check`]'s.
///
/// # Errors
///
/// Names the first rule the label breaks.
pub fn parse_check(event: &Event) -> Result<Check, String> {
    if event.kind != CHECK_KIND {
        return Err(format!("expected kind {CHECK_KIND}, got {}", event.kind));
    }
    event
        .validate_crypto()
        .map_err(|e| format!("bad signature: {e}"))?;
    if single_tag(event, "L")? != CHECK_NAMESPACE {
        return Err("not an openagents.pylon label".into());
    }
    let marks: Vec<&Tag> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some("l"))
        .collect();
    let [mark] = marks.as_slice() else {
        return Err("a verdict carries exactly one `l` tag".into());
    };
    if mark.0.get(2).map(String::as_str) != Some(CHECK_NAMESPACE) {
        return Err("the `l` tag's mark is not openagents.pylon".into());
    }
    let verdict = mark
        .value()
        .and_then(Verdict::from_label)
        .ok_or("unknown verdict")?;
    let receipt = single_tag(event, "e")?;
    hex64(receipt, "e")?;
    let provider = single_tag(event, "p")?;
    hex64(provider, "p")?;
    let result_digest = single_tag(event, "x")?;
    hex64(result_digest, "x")?;
    if event.content.is_empty() || event.content.len() > MAX_CHECK_CONTENT {
        return Err("a verdict's content is 1 to 512 bytes".into());
    }
    if event.pubkey == provider {
        return Err("a pylon can't check itself".into());
    }
    Ok(Check {
        id: event.id.clone(),
        checker: event.pubkey.clone(),
        verdict,
        receipt: receipt.into(),
        provider: provider.into(),
        result_digest: result_digest.into(),
        method: event.content.clone(),
        created_at: event.created_at,
    })
}

/// Check a verdict against the verified receipt it names.
///
/// # Errors
///
/// When the pylon differs, or the checker is the receipt's buyer or
/// provider.
pub fn bind_check(check: &Check, receipt: &Receipt) -> Result<(), String> {
    if check.provider != receipt.provider {
        return Err("the verdict names another pylon than its receipt".into());
    }
    if check.checker == receipt.buyer || check.checker == receipt.provider {
        return Err("a receipt's buyer or provider can't check it".into());
    }
    Ok(())
}

/// A pylon's standing from the verdicts a reader counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// No counted verdict.
    #[default]
    Unchecked,
    /// At least one pass and no fail: the pylon shows a sigil.
    Passing,
    /// At least one fail: the pylon leaves admission where the pool
    /// excludes failed pylons, and never shows a sigil.
    Failing,
}

/// One pylon's counted verdicts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub standing: Standing,
    pub totals: CheckTotals,
}

/// Which verdicts count: from a checker in `checkers`, naming a receipt in
/// `receipts` (by event ID) that it binds to, first per `(checker,
/// receipt)` by `created_at` then ID. Returns the counted checks, oldest
/// first.
#[must_use]
pub fn counted<'a>(
    checks: impl IntoIterator<Item = &'a Check>,
    receipts: &BTreeMap<String, Receipt>,
    checkers: &BTreeSet<String>,
) -> Vec<&'a Check> {
    let mut sorted: Vec<&Check> = checks
        .into_iter()
        .filter(|c| checkers.contains(&c.checker))
        .filter(|c| {
            receipts
                .get(&c.receipt)
                .is_some_and(|r| bind_check(c, r).is_ok())
        })
        .collect();
    sorted.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    let mut seen = BTreeSet::new();
    sorted.retain(|c| seen.insert((c.checker.clone(), c.receipt.clone())));
    sorted
}

/// Each pylon's standing (by `30200` address) from counted verdicts.
#[must_use]
pub fn standings<'a>(
    counted: impl IntoIterator<Item = &'a Check>,
    receipts: &BTreeMap<String, Receipt>,
) -> BTreeMap<String, Record> {
    let mut out: BTreeMap<String, Record> = BTreeMap::new();
    for check in counted {
        let Some(receipt) = receipts.get(&check.receipt) else {
            continue;
        };
        let record = out.entry(receipt.address()).or_default();
        match check.verdict {
            Verdict::Pass => record.totals.pass += 1,
            Verdict::Fail => record.totals.fail += 1,
            Verdict::Inconclusive => record.totals.inconclusive += 1,
        }
        record.standing = if record.totals.fail > 0 {
            Standing::Failing
        } else if record.totals.pass > 0 {
            Standing::Passing
        } else {
            Standing::Unchecked
        };
    }
    out
}
