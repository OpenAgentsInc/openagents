//! Checking a decision pylon: fresh items, recompute, and a verdict.
//!
//! A round sends the pylon `limits.items` fresh decision items
//! ([`crate::items`], a new seed every round, so nothing can be cached or
//! looked up) as ordinary NIP-DEC jobs from the checker's buyer key. Only
//! after every answer is in does the checker draw which items it verifies
//! ([`verify::choose_verified`]), so the pylon cannot do real work on some
//! items and cheap work on others. It recomputes those items on its own
//! reference server running the same pinned model, scores the round
//! ([`verify::score`]), publishes the buyer's `3201` receipt for the
//! round, and signs a NIP-32 verdict on it with the checker key:
//!
//! - a passing round is `check-pass`;
//! - a strike (a large miss, a missing or inconsistent answer, or an
//!   answer naming another model) is `check-fail`, which drops the pylon
//!   from checked pools and holds its unpaid shares;
//! - a lost round (a small miss) is `check-inconclusive`.
//!
//! This is REAX's validator round on our rails: a receipt and a label
//! instead of a weight, and sats from revenue instead of emissions.

use std::time::Instant;

use nostr::pylon::{Lane, Outcome, RECEIPT_V, Receipt, UnitKind, Units, Verdict, sha256_hex};
use serde::Serialize;
use serde_json::{Value, json};

use crate::check::Checker;
use crate::decide::Decider;
use crate::decide_client::{self, Reply};
use crate::items;
use crate::verify::{self, Limits, Round};

/// One item of a round, as the checker saw it.
#[derive(Debug, Clone, Serialize)]
pub struct Asked {
    pub index: usize,
    pub ms: u64,
    pub verified: bool,
    /// The distance to the reference, for a verified item.
    pub distance: Option<f64>,
    /// Why the answer could not be used.
    pub problem: Option<String>,
}

/// One checked round.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionCheck {
    /// The pylon's `30200` address.
    pub pylon: String,
    /// What its beacon says it serves.
    pub identity: String,
    pub seed: u64,
    pub items: Vec<Asked>,
    pub round: Round,
    pub verdict: Verdict,
    pub method: String,
    pub receipt: Option<String>,
    pub label: Option<String>,
    pub error: Option<String>,
}

/// The verdict a round earns.
#[must_use]
pub fn verdict_for(round: &Round) -> Verdict {
    match round.verdict {
        verify::Verdict::Pass => Verdict::Pass,
        verify::Verdict::Strike => Verdict::Fail,
        verify::Verdict::Lost => Verdict::Inconclusive,
    }
}

/// Score answers against reference answers for the verified items: the
/// pure part of a round, for tests. `answers[i]` is the pylon's response
/// body for item `i` (or `None`), `references[i]` the reference's for a
/// verified item. `model` is the model the pylon's beacon names; an answer
/// naming another is unusable.
#[must_use]
pub fn score_answers(
    items: &[items::Item],
    answers: &[Option<Value>],
    references: &[(usize, Value)],
    model: &str,
    limits: &Limits,
) -> (Round, Vec<Option<f64>>, Vec<Option<String>>) {
    let mut distances = vec![None; items.len()];
    let mut problems: Vec<Option<String>> = vec![None; items.len()];
    for (i, answer) in answers.iter().enumerate() {
        match answer {
            None => problems[i] = Some("no answer".into()),
            Some(body) => {
                let served = body["model"].as_str().unwrap_or_default();
                if !served.is_empty() && served != model {
                    problems[i] = Some(format!("answered as `{served}`, not `{model}`"));
                } else if let Err(why) =
                    verify::consistent(&items[i].question, &body["answers"][&items[i].question_id])
                {
                    problems[i] = Some(why);
                }
            }
        }
    }
    let mut checked = Vec::new();
    for (i, reference) in references {
        let item = &items[*i];
        let d = match (
            problems[*i].is_none(),
            answers[*i].as_ref(),
            verify::consistent(&item.question, &reference["answers"][&item.question_id]),
        ) {
            (true, Some(body), Ok(q)) => {
                verify::consistent(&item.question, &body["answers"][&item.question_id])
                    .map_or(f64::INFINITY, |p| verify::distance(&p, &q))
            }
            _ => f64::INFINITY,
        };
        distances[*i] = Some(d);
        checked.push(d);
    }
    let mut round = verify::score(&checked, limits);
    // An unusable answer anywhere in the round is a strike, verified or
    // not: a pylon may not answer unchecked items with junk.
    if problems.iter().any(Option::is_some) {
        round.verdict = verify::Verdict::Strike;
    }
    (round, distances, problems)
}

/// Run one round against the pylon `pylon` (hex key), recomputing on
/// `reference`, and publish the receipt and verdict.
///
/// # Errors
///
/// When the pylon has no decision service, the reference serves another
/// artifact, or the relay cannot be reached.
pub async fn round(
    checker: &Checker,
    pylon: &str,
    reference: &dyn Decider,
    limits: &Limits,
) -> Result<DecisionCheck, String> {
    let beacon = checker.beacon(pylon).await?;
    let service = beacon
        .serves(Lane::CjDecision)
        .ok_or("the pylon's beacon offers no decision service")?
        .clone();
    if !reference.healthy().await {
        return Err("the reference decision server does not answer".into());
    }
    let mine = reference.identity().advertised();
    if mine != service.model {
        return Err(format!(
            "the reference serves `{mine}`, but the pylon serves `{}`; a check needs the same artifact",
            service.model
        ));
    }
    let model = service
        .model
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string();
    let seed = items::fresh_seed();
    let batch = items::items(seed, u32::try_from(limits.items).unwrap_or(8));
    let started = crate::now();
    let mut answers = Vec::new();
    let mut ms = Vec::new();
    for item in &batch {
        let at = Instant::now();
        let reply = decide_client::ask(
            &checker.buyer,
            &checker.relay,
            pylon,
            &model,
            &item.state,
            &item.questions(),
            checker.wait,
        )
        .await;
        ms.push(at.elapsed().as_millis() as u64);
        answers.push(match reply {
            Ok(Reply::Answered(body)) => Some(body),
            _ => None,
        });
    }
    let finished = crate::now();
    // Drawn only now, after every answer is in.
    let verified = verify::choose_verified(batch.len(), limits.verified);
    let mut references = Vec::new();
    for &i in &verified {
        let item = &batch[i];
        match reference.decide(&item.state, &item.questions()).await {
            Ok(body) => references.push((i, body)),
            Err(refusal) => {
                return Err(format!(
                    "the reference refused item {i} ({}); nothing was published",
                    refusal.code
                ));
            }
        }
    }
    let (round, distances, problems) = score_answers(&batch, &answers, &references, &model, limits);
    let verdict = verdict_for(&round);
    let method = format!(
        "decision-recompute {} verified:{}/{} mean:{} max:{} limits:{}/{}/{}",
        service.model,
        round.verified,
        batch.len(),
        finite(round.mean),
        finite(round.worst),
        limits.mean,
        limits.strike,
        limits.item
    )
    .chars()
    .take(512)
    .collect::<String>();
    let mut out = DecisionCheck {
        pylon: beacon.address(),
        identity: service.model.clone(),
        seed,
        items: (0..batch.len())
            .map(|i| Asked {
                index: i,
                ms: ms[i],
                verified: verified.contains(&i),
                distance: distances[i],
                problem: problems[i].clone(),
            })
            .collect(),
        round,
        verdict,
        method: method.clone(),
        receipt: None,
        label: None,
        error: None,
    };
    let items_json = json!(batch.iter().map(|i| i.body(&model)).collect::<Vec<_>>());
    let receipt = Receipt {
        v: RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: checker.buyer.pubkey().to_string(),
        provider: beacon.provider.clone(),
        pylon: beacon.pylon.clone(),
        lane: Lane::CjDecision,
        capability: service.capability.clone(),
        request: sha256_hex(format!("{seed:016x}:{pylon}").as_bytes()),
        request_digest: sha256_hex(items_json.to_string().as_bytes()),
        result_digest: Some(sha256_hex(json!(answers).to_string().as_bytes())),
        started_at: started,
        finished_at: finished.max(started),
        units: Units {
            kind: UnitKind::Jobs,
            count: batch.len() as u64,
        },
        outcome: if answers.iter().all(Option::is_some) {
            Outcome::Accepted
        } else {
            Outcome::Failed
        },
        payment: None,
    };
    let own = json!(
        references
            .iter()
            .map(|(i, r)| json!([i, r["answers"]]))
            .collect::<Vec<_>>()
    )
    .to_string();
    match checker.publish_receipt(&receipt).await {
        Ok(event) => {
            out.receipt = Some(event.id.clone());
            match checker.verdict_on(verdict, &event, &own, &method).await {
                Ok(label) => out.label = Some(label.id),
                Err(e) => out.error = Some(e),
            }
        }
        Err(e) => out.error = Some(e),
    }
    Ok(out)
}

fn finite(x: f64) -> String {
    if x.is_finite() {
        format!("{x:.4}")
    } else {
        "inf".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_answer(item: &items::Item, p: &[f64]) -> Value {
        let q = &item.question;
        let answer = match q["type"].as_str() {
            Some("noul") => json!({"type": "noul", "noul": p[0]}),
            Some("choice") => {
                let keys: Vec<&String> = q["criteria"].as_object().unwrap().keys().collect();
                let best = (0..p.len()).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap();
                let probs: serde_json::Map<String, Value> = keys
                    .iter()
                    .zip(p)
                    .map(|(k, v)| ((*k).clone(), json!(v)))
                    .collect();
                json!({"type": "choice", "choice": keys[best], "confidence": p[best], "probabilities": probs})
            }
            _ => {
                let probs: serde_json::Map<String, Value> = p
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i.to_string(), json!(v)))
                    .collect();
                let score: f64 = p.iter().enumerate().map(|(i, v)| i as f64 * v).sum();
                json!({"type": "score", "score": score, "confidence": 0.5, "probabilities": probs})
            }
        };
        json!({"model": "clef-flash", "answers": {"q": answer}})
    }

    fn options(item: &items::Item) -> usize {
        match item.question["type"].as_str() {
            Some("choice") => item.question["criteria"].as_object().unwrap().len(),
            Some("score") => item.question["criteria"].as_array().unwrap().len(),
            _ => 2,
        }
    }

    fn profile(n: usize, tilt: f64) -> Vec<f64> {
        let raw: Vec<f64> = (0..n).map(|i| (-(i as f64) * tilt).exp()).collect();
        let z: f64 = raw.iter().sum();
        raw.into_iter().map(|x| x / z).collect()
    }

    #[test]
    fn the_same_answers_pass_and_other_answers_strike() {
        let batch = items::items(9, 8);
        let reference: Vec<Value> = batch
            .iter()
            .map(|i| item_answer(i, &profile(options(i), 0.7)))
            .collect();
        let refs: Vec<(usize, Value)> = (0..8).map(|i| (i, reference[i].clone())).collect();
        let same: Vec<Option<Value>> = reference.iter().cloned().map(Some).collect();
        let (round, ..) = score_answers(&batch, &same, &refs, "clef-flash", &verify::CLEF_FLASH);
        assert_eq!(verdict_for(&round), Verdict::Pass);

        let other: Vec<Option<Value>> = batch
            .iter()
            .map(|i| Some(item_answer(i, &profile(options(i), 1.6))))
            .collect();
        let (round, ..) = score_answers(&batch, &other, &refs, "clef-flash", &verify::CLEF_FLASH);
        assert_eq!(verdict_for(&round), Verdict::Fail);
    }

    #[test]
    fn junk_on_an_unverified_item_still_strikes() {
        let batch = items::items(10, 8);
        let reference: Vec<Value> = batch
            .iter()
            .map(|i| item_answer(i, &profile(options(i), 0.7)))
            .collect();
        let refs: Vec<(usize, Value)> = (0..3).map(|i| (i, reference[i].clone())).collect();
        let mut answers: Vec<Option<Value>> = reference.iter().cloned().map(Some).collect();
        answers[6] = None;
        let (round, _, problems) =
            score_answers(&batch, &answers, &refs, "clef-flash", &verify::CLEF_FLASH);
        assert_eq!(verdict_for(&round), Verdict::Fail);
        assert!(problems[6].is_some());
    }

    #[test]
    fn an_answer_naming_another_model_strikes() {
        let batch = items::items(11, 8);
        let reference: Vec<Value> = batch
            .iter()
            .map(|i| item_answer(i, &profile(options(i), 0.7)))
            .collect();
        let refs: Vec<(usize, Value)> = (0..8).map(|i| (i, reference[i].clone())).collect();
        let mut answers: Vec<Option<Value>> = reference.iter().cloned().map(Some).collect();
        answers[2].as_mut().unwrap()["model"] = json!("jev-1.13.0");
        let (round, ..) = score_answers(&batch, &answers, &refs, "clef-flash", &verify::CLEF_FLASH);
        assert_eq!(verdict_for(&round), Verdict::Fail);
    }
}
