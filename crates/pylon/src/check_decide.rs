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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use nostr::pylon::{
    BeaconBook, Freshness, Lane, Outcome, RECEIPT_V, Receipt, Status, UnitKind, Units, Verdict,
    sha256_hex,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::check::Checker;
use crate::decide::Decider;
use crate::decide_client::{self, Reply};
use crate::identity::Identity;
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
    /// The generator seed, except for a private round.
    pub seed: Option<u64>,
    /// A private round's number (its items need the checker's secret too).
    pub private_round: Option<u64>,
    /// `private`, `public`, or `fresh`.
    pub items_from: String,
    /// The buyer key that sent the jobs and signed the receipt.
    pub buyer: String,
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

/// Where a round's items come from.
#[derive(Clone, Copy)]
pub enum Seed {
    /// A seed drawn from the operating system for this round.
    Fresh,
    /// A fixed seed on the public generator: reproducible, for calibration
    /// runs (`--seed`). Anyone can rebuild these items.
    Public(u64),
    /// Round `round` under the checker-private secret
    /// ([`items::checker_seed`]): nobody without the secret can rebuild the
    /// items, even knowing the round number.
    Private { secret: [u8; 32], round: u64 },
}

impl Seed {
    /// The generator seed.
    #[must_use]
    pub fn value(&self) -> u64 {
        match self {
            Self::Fresh => items::fresh_seed(),
            Self::Public(seed) => *seed,
            Self::Private { secret, round } => items::private_seed(secret, *round),
        }
    }

    fn source(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Public(_) => "public",
            Self::Private { .. } => "private",
        }
    }
}

/// How one round is run.
#[derive(Clone, Copy)]
pub struct Plan {
    pub seed: Seed,
    /// Send the round's jobs, and sign its receipt, with a new ephemeral
    /// buyer key instead of the checker's stored one, so a pylon cannot
    /// recognise the checker by its key. A pylon that admits only
    /// allowlisted keys refuses such a round.
    pub fresh_buyer: bool,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            seed: Seed::Fresh,
            fresh_buyer: false,
        }
    }
}

/// The next private round number in `home` (`checker-round`), counted up
/// and kept so a restarted checker never repeats a round.
///
/// # Errors
///
/// When the counter file cannot be read or written.
pub fn next_round(home: &Path) -> Result<u64, String> {
    let path = home.join(ROUND_FILE);
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => text
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("{} does not hold a round number", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e.to_string()),
    };
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    std::fs::write(&path, format!("{}\n", current + 1)).map_err(|e| e.to_string())?;
    Ok(current)
}

/// The private round counter's file in the pylon home.
pub const ROUND_FILE: &str = "checker-round";

/// Run one round against the pylon `pylon` (hex key), recomputing on
/// `reference`, and publish the receipt and verdict: items from a fresh
/// seed, sent with the checker's stored buyer key.
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
    round_with(checker, pylon, reference, limits, Plan::default()).await
}

/// [`round`] with the items and buyer key `plan` names.
///
/// # Errors
///
/// As [`round`].
pub async fn round_with(
    checker: &Checker,
    pylon: &str,
    reference: &dyn Decider,
    limits: &Limits,
    plan: Plan,
) -> Result<DecisionCheck, String> {
    let ephemeral;
    let checker = if plan.fresh_buyer {
        ephemeral = Checker {
            buyer: Identity::generate(),
            ..checker.clone()
        };
        &ephemeral
    } else {
        checker
    };
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
    let seed = plan.seed.value();
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
        // A pylon that is rate-limiting this buyer says so; the round
        // could not run, which is not the pylon answering wrong (#12190's
        // rule: a round the checker could not run is never a strike).
        if let Ok(Reply::Refused(code)) = &reply
            && code == "rate_limited"
        {
            return Err(format!(
                "the pylon is rate-limiting this buyer (item {} of {}); nothing was published, try again in a minute",
                answers.len() + 1,
                batch.len()
            ));
        }
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
        // A private round's seed stays with the checker.
        seed: match plan.seed {
            Seed::Private { .. } => None,
            _ => Some(seed),
        },
        private_round: match plan.seed {
            Seed::Private { round, .. } => Some(round),
            _ => None,
        },
        items_from: plan.seed.source().into(),
        buyer: checker.buyer.pubkey().to_string(),
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

/// How the continuous checker (`check decide --watch`) schedules rounds.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Seconds between rounds for one pylon (default 900).
    pub every: u64,
    /// Seconds before a struck pylon is checked again (default 3,600). A
    /// pass then is the `check-pass` the gateway needs to take it back.
    pub recheck_struck: u64,
    /// Rounds in flight at once (default 4).
    pub concurrency: usize,
    /// Only pylons asking to join this pool.
    pub pool: Option<String>,
    /// Only these pylons (hex keys), when set.
    pub only: Option<BTreeSet<String>>,
    /// A new ephemeral buyer key per round (default on).
    pub fresh_buyer: bool,
    /// Forget a pylon not seen for this long (default a day).
    pub forget_after: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            every: 900,
            recheck_struck: 3_600,
            concurrency: 4,
            pool: None,
            only: None,
            fresh_buyer: true,
            forget_after: 24 * 3_600,
        }
    }
}

/// What the watcher knows about one pylon.
#[derive(Debug, Clone, Serialize)]
pub struct Watched {
    /// When its next round is due (Unix seconds).
    pub next: u64,
    /// When its beacon was last eligible.
    pub seen: u64,
    /// The last round's verdict, if a round finished.
    pub last: Option<Verdict>,
    /// Rounds attempted.
    pub rounds: u64,
}

/// When each pylon is due: new pylons at once, then every
/// [`WatchConfig::every`], struck pylons after
/// [`WatchConfig::recheck_struck`]. Pure, so tests can drive the clock.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    pub pylons: BTreeMap<String, Watched>,
}

impl Schedule {
    /// The pylons in `eligible` whose round is due at `at`, marking them
    /// seen; a pylon seen for the first time is due at once. Pylons not
    /// seen for [`WatchConfig::forget_after`] are forgotten.
    pub fn due(&mut self, eligible: &[String], at: u64, config: &WatchConfig) -> Vec<String> {
        let mut out = Vec::new();
        for pylon in eligible {
            let entry = self.pylons.entry(pylon.clone()).or_insert(Watched {
                next: at,
                seen: at,
                last: None,
                rounds: 0,
            });
            entry.seen = at;
            if entry.next <= at {
                out.push(pylon.clone());
            }
        }
        self.pylons
            .retain(|_, w| at.saturating_sub(w.seen) <= config.forget_after);
        out
    }

    /// Record a round on `pylon` that finished at `at` with `verdict`
    /// (`None` when the round could not run, which is retried after the
    /// usual interval and never counts against the pylon).
    pub fn record(&mut self, pylon: &str, verdict: Option<Verdict>, at: u64, config: &WatchConfig) {
        let entry = self.pylons.entry(pylon.to_string()).or_insert(Watched {
            next: at,
            seen: at,
            last: None,
            rounds: 0,
        });
        entry.rounds += 1;
        if verdict.is_some() {
            entry.last = verdict;
        }
        entry.next = at
            + match verdict {
                Some(Verdict::Fail) => config.recheck_struck,
                _ => config.every,
            };
    }
}

/// The pylons (hex keys) the watcher may check at `at`: a fresh beacon
/// that is online (an offline or draining pylon is left alone until it is
/// back), offers the decision lane with exactly the identity the reference
/// serves (`served`), and is in `config`'s pool and list when they are set.
#[must_use]
pub fn eligible(book: &BeaconBook, at: u64, served: &str, config: &WatchConfig) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for (_, beacon) in book.iter() {
        let ok = nostr::pylon::freshness(beacon, at) == Freshness::Fresh
            && beacon.status == Status::Online
            && beacon
                .serves(Lane::CjDecision)
                .is_some_and(|s| s.model == served)
            && config
                .pool
                .as_ref()
                .is_none_or(|pool| beacon.pools.iter().any(|p| p == pool))
            && config
                .only
                .as_ref()
                .is_none_or(|only| only.contains(&beacon.provider));
        if ok {
            out.insert(beacon.provider.clone());
        }
    }
    out.into_iter().collect()
}

/// Where a watcher's items come from.
#[derive(Clone, Copy)]
pub enum Items {
    /// The checker-private secret and its persisted round counter.
    Private([u8; 32]),
    /// Public seeds counting up from this one, for calibration runs.
    Public(u64),
}

/// One finished (or failed) round in a watcher pass.
#[derive(Debug, Clone, Serialize)]
pub struct Watch {
    /// The pylon's hex key.
    pub pylon: String,
    pub check: Option<DecisionCheck>,
    /// Why no round ran.
    pub error: Option<String>,
    /// When this pylon's next round is due.
    pub next: u64,
}

/// The continuous decision checker: one [`Watcher::pass`] reads the
/// relay's beacons and runs every due round, at most
/// [`WatchConfig::concurrency`] at once.
pub struct Watcher {
    pub checker: Checker,
    pub reference: Arc<dyn Decider>,
    pub limits: Limits,
    pub config: WatchConfig,
    pub items: Items,
    pub schedule: Schedule,
    next_public: u64,
}

impl Watcher {
    /// A watcher with an empty schedule.
    #[must_use]
    pub fn new(
        checker: Checker,
        reference: Arc<dyn Decider>,
        limits: Limits,
        config: WatchConfig,
        items: Items,
    ) -> Self {
        let next_public = match items {
            Items::Public(seed) => seed,
            Items::Private(_) => 0,
        };
        Self {
            checker,
            reference,
            limits,
            config,
            items,
            schedule: Schedule::default(),
            next_public,
        }
    }

    fn seed(&mut self) -> Result<Seed, String> {
        Ok(match self.items {
            Items::Private(secret) => Seed::Private {
                secret,
                round: next_round(&self.checker.home)?,
            },
            Items::Public(_) => {
                let seed = self.next_public;
                self.next_public = self.next_public.wrapping_add(1);
                Seed::Public(seed)
            }
        })
    }

    /// One pass at schedule time `at`: read the beacons (freshness is
    /// judged on the wall clock), run the due rounds, and record them.
    ///
    /// # Errors
    ///
    /// When the relay cannot be read or the round counter kept.
    pub async fn pass(&mut self, at: u64) -> Result<Vec<Watch>, String> {
        let reader = Identity::generate();
        let mut conn =
            crate::relay::connect(&self.checker.relay, &reader, crate::relay::LIFETIME).await?;
        let book = crate::client::beacons(&mut conn, None).await;
        let _ = conn.close().await;
        let book = book?;
        let served = self.reference.identity().advertised();
        let candidates = eligible(&book, crate::now(), &served, &self.config);
        let due = self.schedule.due(&candidates, at, &self.config);
        let mut out = Vec::new();
        let mut running = tokio::task::JoinSet::new();
        let limit = self.config.concurrency.max(1);
        for pylon in due {
            while running.len() >= limit {
                if let Some(Ok(done)) = running.join_next().await {
                    out.push(done);
                }
            }
            let plan = Plan {
                seed: self.seed()?,
                fresh_buyer: self.config.fresh_buyer,
            };
            let checker = self.checker.clone();
            let reference = Arc::clone(&self.reference);
            let limits = self.limits;
            running.spawn(async move {
                let result = round_with(&checker, &pylon, reference.as_ref(), &limits, plan).await;
                (pylon, result)
            });
        }
        while let Some(done) = running.join_next().await {
            if let Ok(done) = done {
                out.push(done);
            }
        }
        let mut watched = Vec::new();
        for (pylon, result) in out {
            let verdict = result.as_ref().ok().map(|c| c.verdict);
            self.schedule.record(&pylon, verdict, at, &self.config);
            let next = self.schedule.pylons.get(&pylon).map_or(at, |w| w.next);
            let (check, error) = match result {
                Ok(c) => (Some(c), None),
                Err(e) => (None, Some(e)),
            };
            watched.push(Watch {
                pylon,
                check,
                error,
                next,
            });
        }
        watched.sort_by(|a, b| a.pylon.cmp(&b.pylon));
        Ok(watched)
    }
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
                let keys = crate::verify::option_keys(q);
                let best = (0..p.len()).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap();
                let probs: serde_json::Map<String, Value> = keys
                    .iter()
                    .zip(p)
                    .map(|(k, v)| (k.clone(), json!(v)))
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

    #[test]
    fn a_new_pylon_is_due_at_once_then_after_the_interval() {
        let config = WatchConfig::default();
        let mut schedule = Schedule::default();
        let pylons = vec!["aa".to_string()];
        assert_eq!(schedule.due(&pylons, 1_000, &config), pylons);
        schedule.record("aa", Some(Verdict::Pass), 1_000, &config);
        assert!(schedule.due(&pylons, 1_000 + 899, &config).is_empty());
        assert_eq!(schedule.due(&pylons, 1_000 + 900, &config), pylons);
    }

    #[test]
    fn a_struck_pylon_is_checked_again_after_an_hour() {
        let config = WatchConfig::default();
        let mut schedule = Schedule::default();
        let pylons = vec!["bb".to_string()];
        assert_eq!(schedule.due(&pylons, 0, &config), pylons);
        schedule.record("bb", Some(Verdict::Fail), 0, &config);
        assert!(schedule.due(&pylons, 900, &config).is_empty());
        assert!(schedule.due(&pylons, 3_599, &config).is_empty());
        assert_eq!(schedule.due(&pylons, 3_600, &config), pylons);
        // A pass on the re-check puts it back on the usual interval.
        schedule.record("bb", Some(Verdict::Pass), 3_600, &config);
        assert_eq!(schedule.pylons["bb"].next, 4_500);
        assert_eq!(schedule.pylons["bb"].last, Some(Verdict::Pass));
    }

    #[test]
    fn an_absent_pylon_waits_and_a_long_gone_one_is_forgotten() {
        let config = WatchConfig::default();
        let mut schedule = Schedule::default();
        let one = vec!["cc".to_string()];
        schedule.due(&one, 0, &config);
        schedule.record("cc", Some(Verdict::Pass), 0, &config);
        // Offline or draining: not in the eligible list, so never due,
        // and due at once when it comes back after its interval.
        assert!(schedule.due(&[], 2_000, &config).is_empty());
        assert_eq!(schedule.due(&one, 2_100, &config), one);
        schedule.record("cc", None, 2_100, &config);
        assert_eq!(schedule.pylons["cc"].last, Some(Verdict::Pass));
        schedule.due(&[], 2_100 + config.forget_after + 1, &config);
        assert!(schedule.pylons.is_empty());
    }
}
