//! Admit repair context from the exact buyer review and preceding submission.
//! The target, input, lock, and execution grant remain separate frozen pins.
use crate::{
    Result,
    book::{Book, Final},
    records::{Record, ReviewDecision},
    terms::Use,
};
use nostr::contracts::{self, ArtifactRef};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Exact plaintext already supplied under the order's rights policy.
pub type Bytes = BTreeMap<String, Vec<u8>>;
fn read<'a>(bytes: &'a Bytes, reference: &ArtifactRef) -> Result<&'a [u8]> {
    let raw = bytes
        .get(&reference.digest)
        .ok_or("repair context bytes are unavailable")?;
    contracts::check_artifact_bytes(reference, raw).map_err(|e| e.to_string())?;
    Ok(raw)
}

/// Check a target-supported repair without admitting another source,
/// instruction, recipient, or requirement. The first bounded representation
/// profile permits exact source bytes only; transformations are refused.
pub fn admit_rework(
    book: &Book,
    number: u32,
    review_digest: &str,
    initial: &Value,
    proposed: &Value,
    evidence: &BTreeMap<String, Value>,
    bytes: &Bytes,
    now: u64,
) -> Result<()> {
    let terms = &book.terms;
    if number == 0
        || number > u32::from(terms.max_reworks)
        || now > terms.due(number)
        || !matches!(book.final_state(), Final::Open)
        || book
            .entries()
            .iter()
            .any(|e| matches!(e.record, Record::Cancellation(_)))
        || !terms
            .rights
            .permits(terms, Use::PerformAndReview, &terms.worker, &[])
    {
        return Err("the order does not authorize this repair".into());
    }
    let previous = book
        .entries()
        .iter()
        .find_map(|e| match &e.record {
            Record::Submission(s) if s.number + 1 == number => Some((e.digest.as_str(), s)),
            _ => None,
        })
        .ok_or("the previous submission is unavailable")?;
    if book
        .entries()
        .iter()
        .any(|e| matches!(&e.record,Record::Submission(s) if s.number>=number))
    {
        return Err("the repair already has a submission".into());
    }
    let review = book
        .entries()
        .iter()
        .find(|e| e.digest == review_digest)
        .ok_or("the buyer review is unavailable")?;
    if !matches!(&review.record,Record::Review(r) if r.issuer==terms.buyer && r.decision==ReviewDecision::RequestRework && r.submission==previous.0)
    {
        return Err("the repair does not name the buyer's exact rework request".into());
    }
    let original = contracts::parse_context(initial).map_err(|e| e.to_string())?;
    let repair = contracts::parse_context(proposed).map_err(|e| e.to_string())?;
    if original.recipient != terms.worker
        || repair.recipient != original.recipient
        || repair.policy != original.policy
        || proposed["task"] != initial["task"]
        || repair.omissions != original.omissions
        || repair.coverage != original.coverage
        || !repair.entries.starts_with(&original.entries)
        || repair.entries.len() == original.entries.len()
    {
        return Err("repair context changes the original context contract".into());
    }
    let mut allowed: BTreeSet<String> = previous
        .1
        .deliverables
        .iter()
        .map(|d| d.digest.clone())
        .collect();
    allowed.extend(previous.1.executions.iter().cloned());
    allowed.extend(previous.1.run_evidence.iter().cloned());
    allowed.insert(review_digest.into());
    let mut seen = BTreeSet::new();
    let mut review_seen = false;
    let mut total = 0usize;
    for entry in &repair.entries[original.entries.len()..] {
        let descriptor = evidence
            .get(&entry.evidence)
            .ok_or("repair evidence descriptor is unavailable")?;
        let parsed = contracts::parse_evidence(descriptor).map_err(|e| e.to_string())?;
        let origin = contracts::parse_artifact(&descriptor["source"]["observation"])
            .map_err(|e| e.to_string())?;
        if parsed.id != entry.evidence
            || parsed.task != initial["task"].as_str().unwrap_or_default()
            || parsed.recipients != [terms.worker.clone()]
            || parsed.transform.is_some()
            || !parsed.derived_from.is_empty()
            || !parsed.capture.complete
            || parsed.capture.omitted_bytes != Some(0)
            || descriptor["source"]
                != json!({"kind":"document","identity":terms.order,"version":origin.digest,"observation":descriptor["content"]})
            || parsed.content != entry.representation
            || origin != entry.representation
            || !allowed.contains(&origin.digest)
            || !entry.mandatory
            || !seen.insert(origin.digest.clone())
        {
            return Err("repair context introduces unauthenticated or changed evidence".into());
        }
        let raw = read(bytes, &entry.representation)?;
        total = total.saturating_add(raw.len());
        if total > 1_048_576 {
            return Err("repair context exceeds its byte limit".into());
        }
        if origin.digest == review_digest {
            let expected =
                contracts::jcs(&serde_json::to_value(&review.record).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if raw != expected {
                return Err("repair review bytes differ".into());
            }
            review_seen = true;
        }
    }
    if !review_seen {
        return Err("repair context omits the authorizing review".into());
    }
    Ok(())
}
