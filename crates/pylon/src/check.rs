//! Victor's checker: known-answer canaries and redundant execution through
//! the normal job path, with NIP-32 verdicts (`openagents.pylon`) that
//! point at the receipts.
//!
//! A canary is an ordinary-looking job whose answer the checker knows. The
//! checker sends it as a buyer, publishes the receipt like any buyer, and
//! then signs a verdict on that receipt with a separate checker key, so the
//! label never comes from the receipt's buyer or provider. Each hardware
//! family has one pinned Gym suite of canaries ([`suites`]); a verdict's
//! method names the suite's digest, which is what the pylon league reads.
//!
//! Redundant execution sends one prompt to several pylons and compares
//! normalized answers: a pylon in a strict majority passes, one outside it
//! fails, and anything without a majority is inconclusive.
//!
//! Readers count verdicts only from checkers they trust ([`trusted`]): this
//! computer's own checker key and `OPENAGENTS_PYLON_CHECKERS`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nostr::domain::Event;
use nostr::pylon::{
    self, Beacon, CHECK_KIND, CHECK_NAMESPACE, Check, Family, Receipt, Record, Verdict,
    check_event, counted, parse_check, parse_receipt, sha256_hex, standings,
};
use serde::Serialize;
use serde_json::json;

use crate::client::{self, Ask};
use crate::identity::{Identity, hex_pubkey};
use crate::now;
use crate::relay::{self, LIFETIME};

/// The suite body version.
pub const SUITE_V: &str = "openagents.pylon-suite.v1";
/// How far back a reader counts verdicts by default.
pub const CHECK_WINDOW_SECS: u64 = 24 * 3_600;

/// One known-answer job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Canary {
    pub prompt: String,
    /// The word the answer must contain, after [`normalize`].
    pub expect: String,
}

/// A pinned spot-check suite for one service class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suite {
    pub v: String,
    pub id: String,
    pub family: Family,
    pub canaries: Vec<Canary>,
}

impl Suite {
    /// SHA-256 of the suite's canonical JSON: what a verdict's method and
    /// a `pylon-check` quest name.
    #[must_use]
    pub fn digest(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or_default();
        nostr::contracts::jcs(&value).map_or_else(|_| String::new(), |bytes| sha256_hex(&bytes))
    }

    /// The method text a canary verdict carries.
    #[must_use]
    pub fn method(&self) -> String {
        format!("canary exact-match suite:{} {}", self.digest(), self.id)
    }
}

fn canary(prompt: &str, expect: &str) -> Canary {
    Canary {
        prompt: prompt.into(),
        expect: expect.into(),
    }
}

/// The pinned suites, one per hardware family. Changing a canary changes
/// the digest, which starts a new league column and a new quest.
#[must_use]
pub fn suites() -> Vec<Suite> {
    let suite = |id: &str, family, canaries| Suite {
        v: SUITE_V.into(),
        id: id.into(),
        family,
        canaries,
    };
    vec![
        suite(
            "pylon-text-cpu-v1",
            Family::Cpu,
            vec![
                canary("What is 7 + 5? Reply with the number only.", "12"),
                canary(
                    "What is the capital of France? Reply with one word.",
                    "paris",
                ),
                canary(
                    "How many days are in a week? Reply with the number only.",
                    "7",
                ),
            ],
        ),
        suite(
            "pylon-text-gpu-v1",
            Family::Gpu,
            vec![
                canary("What is 9 times 8? Reply with the number only.", "72"),
                canary(
                    "What color is a clear daytime sky? Reply with one word.",
                    "blue",
                ),
                canary(
                    "How many legs does a spider have? Reply with the number only.",
                    "8",
                ),
            ],
        ),
        suite(
            "pylon-text-unified-memory-v1",
            Family::UnifiedMemory,
            vec![
                canary("What is 15 minus 6? Reply with the number only.", "9"),
                canary("What is the opposite of hot? Reply with one word.", "cold"),
                canary(
                    "How many sides does a triangle have? Reply with the number only.",
                    "3",
                ),
            ],
        ),
    ]
}

/// The pinned suite for a hardware family.
#[must_use]
pub fn suite_for(family: Family) -> Suite {
    suites()
        .into_iter()
        .find(|s| s.family == family)
        .unwrap_or_else(|| suites().remove(0))
}

/// Lowercase words of letters and digits.
#[must_use]
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A canary's verdict on `answer`: pass when one of its words is the
/// expected one, fail on any other answer, inconclusive with none.
#[must_use]
pub fn grade(canary: &Canary, answer: Option<&str>) -> Verdict {
    match answer {
        None => Verdict::Inconclusive,
        Some(text) if normalize(text).split(' ').any(|w| w == canary.expect) => Verdict::Pass,
        Some(_) => Verdict::Fail,
    }
}

/// Redundant execution's verdicts, one per answer, and the majority answer
/// (normalized) when there is one.
#[must_use]
pub fn judge(answers: &[Option<String>]) -> (Vec<Verdict>, Option<String>) {
    let normalized: Vec<Option<String>> = answers
        .iter()
        .map(|a| a.as_deref().map(normalize).filter(|a| !a.is_empty()))
        .collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for answer in normalized.iter().flatten() {
        *counts.entry(answer.as_str()).or_default() += 1;
    }
    let majority = counts
        .iter()
        .find(|(_, n)| **n * 2 > answers.len())
        .map(|(a, _)| (*a).to_string());
    let verdicts = normalized
        .iter()
        .map(|answer| match (answer, &majority) {
            (None, _) | (_, None) => Verdict::Inconclusive,
            (Some(a), Some(m)) if a == m => Verdict::Pass,
            (Some(_), Some(_)) => Verdict::Fail,
        })
        .collect();
    (verdicts, majority)
}

/// The checkers this computer trusts: its own checker key (when one
/// exists in `home`) and the npubs or hex keys in
/// `OPENAGENTS_PYLON_CHECKERS`, comma-separated.
#[must_use]
pub fn trusted(home: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(own) = Identity::load_existing(&home.join("checker.key")) {
        out.insert(own.pubkey().to_string());
    }
    if let Ok(list) = std::env::var("OPENAGENTS_PYLON_CHECKERS") {
        out.extend(list.split(',').filter_map(|k| hex_pubkey(k.trim())));
    }
    out
}

/// One verdict the checker published.
#[derive(Debug, Clone, Serialize)]
pub struct Checked {
    /// The pylon's `30200` address.
    pub pylon: String,
    pub prompt: String,
    pub answer: Option<String>,
    pub verdict: Verdict,
    /// The receipt checked.
    pub receipt: Option<String>,
    /// The label, when one was published.
    pub label: Option<String>,
    pub method: String,
    pub error: Option<String>,
    /// The signed receipt and label, for an XP award.
    #[serde(skip)]
    pub events: Option<(Event, Event)>,
}

/// Who checks and where.
#[derive(Clone)]
pub struct Checker {
    pub relay: String,
    /// Signs the verdicts.
    pub checker: Identity,
    /// Sends the jobs and signs their receipts: never the checker key.
    pub buyer: Identity,
    pub home: PathBuf,
    pub wait: Duration,
}

impl Checker {
    async fn run(&self, pylon: &str, prompt: &str) -> Result<client::Answer, String> {
        client::ask(
            &self.buyer,
            &Ask {
                relay: self.relay.clone(),
                pylon: Some(pylon.into()),
                prompt: prompt.into(),
                wait: self.wait,
                publish_receipt: true,
                home: self.home.clone(),
                checkers: BTreeSet::new(),
            },
        )
        .await
    }

    async fn label(
        &self,
        verdict: Verdict,
        receipt: &Event,
        own: &str,
        method: &str,
    ) -> Result<Event, String> {
        let event = check_event(
            self.checker.signer(),
            verdict,
            receipt,
            &sha256_hex(own.as_bytes()),
            method,
            now(),
        )?;
        let mut conn = relay::connect(&self.relay, &self.checker, LIFETIME).await?;
        let published = relay::publish(&mut conn, &event).await;
        let _ = conn.close().await;
        published.map(|()| event)
    }

    /// Award NIP-XP for verified work: for the first passing canary in
    /// `checked`, publish the suite's quest (the checker is its referee)
    /// and one `pylon-check` award to the pylon, unless the pylon already
    /// holds that quest's award. XP is a level and never converts to sats.
    ///
    /// # Errors
    ///
    /// When the relay cannot be reached or the award doesn't verify.
    pub async fn award(&self, checked: &[Checked]) -> Result<Option<String>, String> {
        let Some((receipt, label)) = checked
            .iter()
            .filter(|c| c.verdict == Verdict::Pass)
            .find_map(|c| c.events.clone())
        else {
            return Ok(None);
        };
        let suite = parse_check(&label)?
            .suite()
            .map(str::to_string)
            .ok_or("only a canary on a pinned suite earns XP")?;
        let quest = sign(
            &self.checker,
            SEASON_OPENS,
            nostr::xp::quest(&quest_spec(&suite, self.checker.pubkey()))
                .map_err(|e| e.to_string())?,
        );
        let parts = nostr::xp::pylon_check_award(&quest, &receipt, &label, now())
            .map_err(|e| e.to_string())?;
        let award = sign(&self.checker, now(), parts);
        let key = nostr::xp::parse_award(&award)
            .map_err(|e| e.to_string())?
            .key;
        let provider = parse_receipt(&receipt, None)?.provider;
        let mut conn = relay::connect(&self.relay, &self.checker, LIFETIME).await?;
        let held = relay::query(
            &mut conn,
            "awards",
            &[json!({
                "kinds": [nostr::xp::AWARD_KIND],
                "authors": [self.checker.pubkey()],
                "#p": [provider],
            })],
        )
        .await?;
        if held
            .iter()
            .filter_map(|e| nostr::xp::parse_award(e).ok())
            .any(|a| a.key == key)
        {
            let _ = conn.close().await;
            return Ok(None);
        }
        relay::publish(&mut conn, &quest).await?;
        relay::publish(&mut conn, &award).await?;
        let _ = conn.close().await;
        Ok(Some(award.id))
    }

    /// Run the pinned suite for the pylon `pylon` (a hex key) through the
    /// normal job path and publish one verdict per canary.
    ///
    /// # Errors
    ///
    /// When the relay cannot be reached or the pylon has no fresh beacon.
    pub async fn canaries(&self, pylon: &str) -> Result<Vec<Checked>, String> {
        let beacon = self.beacon(pylon).await?;
        let suite = suite_for(beacon.class.family);
        let method = suite.method();
        let mut out = Vec::new();
        for canary in &suite.canaries {
            let answer = self.run(pylon, &canary.prompt).await?;
            let verdict = grade(canary, answer.text.as_deref());
            out.push(
                self.publish(&beacon, answer, verdict, &canary.expect, &method)
                    .await,
            );
        }
        Ok(out)
    }

    /// Send `prompt` to each pylon in `pylons` (hex keys) and publish a
    /// verdict for each from the majority answer.
    ///
    /// # Errors
    ///
    /// When the relay cannot be reached or a pylon has no fresh beacon.
    pub async fn redundant(&self, prompt: &str, pylons: &[String]) -> Result<Vec<Checked>, String> {
        let mut runs = Vec::new();
        for pylon in pylons {
            let beacon = self.beacon(pylon).await?;
            runs.push((beacon, self.run(pylon, prompt).await?));
        }
        let texts: Vec<Option<String>> = runs.iter().map(|(_, a)| a.text.clone()).collect();
        let (verdicts, majority) = judge(&texts);
        let method = format!("redundant-{} exact-match", pylons.len());
        let own = majority.unwrap_or_default();
        let mut out = Vec::new();
        for ((beacon, answer), verdict) in runs.into_iter().zip(verdicts) {
            out.push(self.publish(&beacon, answer, verdict, &own, &method).await);
        }
        Ok(out)
    }

    async fn publish(
        &self,
        beacon: &Beacon,
        answer: client::Answer,
        verdict: Verdict,
        own: &str,
        method: &str,
    ) -> Checked {
        let mut checked = Checked {
            pylon: beacon.address(),
            prompt: answer.prompt.clone(),
            answer: answer.text.clone(),
            verdict,
            receipt: answer.receipt.clone(),
            label: None,
            method: method.into(),
            error: answer.receipt_error.clone(),
            events: None,
        };
        if let Some(receipt) = &answer.receipt_event {
            match self.label(verdict, receipt, own, method).await {
                Ok(label) => {
                    checked.label = Some(label.id.clone());
                    checked.events = Some((receipt.clone(), label));
                }
                Err(e) => checked.error = Some(e),
            }
        }
        checked
    }

    async fn beacon(&self, pylon: &str) -> Result<Beacon, String> {
        let mut conn = relay::connect(&self.relay, &self.buyer, LIFETIME).await?;
        let book = client::beacons(&mut conn, Some(&[pylon.to_string()])).await?;
        let _ = conn.close().await;
        book.iter()
            .map(|(_, b)| b.clone())
            .max_by_key(|b| b.observed_at)
            .ok_or_else(|| format!("no beacon from pylon {pylon}"))
    }
}

/// When the pylon XP season opens, Unix seconds (2026-09-21).
pub const SEASON_OPENS: u64 = 1_790_000_000;
/// When it closes (2027-12-31).
pub const SEASON_CLOSES: u64 = 1_830_297_600;
/// XP for a pylon's first verified job on a suite.
pub const XP_PER_SUITE: u64 = 25;

/// The `pylon-check` quest for the suite with `digest`, refereed by
/// `checker`: one award per pylon per suite version.
#[must_use]
pub fn quest_spec(digest: &str, checker: &str) -> serde_json::Value {
    json!({
        "id": format!("pylon.verified.{}", &digest[..16]),
        "version": 1,
        "season": {"id": "pylon-v1", "opens_at": SEASON_OPENS, "closes_at": SEASON_CLOSES},
        "title": "Pass a pinned Gym suite canary",
        "objective": "Answer a known-answer job from the pinned suite correctly through the normal job path.",
        "acceptance": {"rule": "pylon-check", "suite": digest, "checker": checker, "max_awards": 10_000},
        "reference": null,
        "award": {"provider": XP_PER_SUITE},
    })
}

fn sign(identity: &Identity, at: u64, parts: nostr::kb::Unsigned) -> Event {
    identity
        .signer()
        .sign(at, parts.kind, parts.tags, parts.content)
}

/// The verdicts a reader counts and the receipts they bind to.
#[derive(Debug, Clone, Default)]
pub struct Verdicts {
    /// Receipts by event ID.
    pub receipts: BTreeMap<String, Receipt>,
    /// Counted checks, oldest first.
    pub checks: Vec<Check>,
}

impl Verdicts {
    /// From fetched labels and receipts, counting only `checkers`.
    #[must_use]
    pub fn new(labels: &[Event], receipts: &[Event], checkers: &BTreeSet<String>) -> Self {
        let receipts: BTreeMap<String, Receipt> = receipts
            .iter()
            .filter_map(|e| parse_receipt(e, None).ok().map(|r| (e.id.clone(), r)))
            .collect();
        let parsed: Vec<Check> = labels.iter().filter_map(|e| parse_check(e).ok()).collect();
        let checks = counted(&parsed, &receipts, checkers)
            .into_iter()
            .cloned()
            .collect();
        Self { receipts, checks }
    }

    /// Each pylon's standing by `30200` address.
    #[must_use]
    pub fn standings(&self) -> BTreeMap<String, Record> {
        standings(&self.checks, &self.receipts)
    }
}

/// Fetch the verdicts `checkers` published since `since`, and the receipts
/// they name.
///
/// # Errors
///
/// When the relay cannot be read.
pub async fn fetch(
    conn: &mut nostr_transport::Connection,
    checkers: &BTreeSet<String>,
    since: u64,
) -> Result<(Vec<Event>, Vec<Event>), String> {
    if checkers.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let labels = relay::query(
        conn,
        "checks",
        &[json!({
            "kinds": [CHECK_KIND],
            "authors": checkers,
            "#L": [CHECK_NAMESPACE],
            "since": since,
            "limit": 4_096,
        })],
    )
    .await?;
    let ids: BTreeSet<String> = labels
        .iter()
        .filter_map(|e| parse_check(e).ok().map(|c| c.receipt))
        .collect();
    let ids: Vec<String> = ids.into_iter().collect();
    let mut receipts = Vec::new();
    for chunk in ids.chunks(500) {
        receipts.extend(
            relay::query(
                conn,
                "checked-receipts",
                &[json!({"kinds": [pylon::RECEIPT_KIND], "ids": chunk})],
            )
            .await?,
        );
    }
    Ok((labels, receipts))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_suites_are_pinned() {
        let digests: Vec<String> = suites().iter().map(Suite::digest).collect();
        assert_eq!(digests.len(), 3);
        assert!(digests.iter().all(|d| d.len() == 64));
        assert_eq!(suite_for(Family::Gpu).digest(), digests[1]);
        assert!(suite_for(Family::Cpu).method().contains(&digests[0]));
        // An echoing pylon never passes a canary by repeating the prompt.
        for suite in suites() {
            for canary in &suite.canaries {
                let echo: String = canary.prompt.chars().rev().collect();
                assert_eq!(
                    grade(canary, Some(&echo)),
                    Verdict::Fail,
                    "{}",
                    canary.prompt
                );
                assert_eq!(grade(canary, Some(&canary.expect)), Verdict::Pass);
            }
        }
    }

    #[test]
    fn grading_and_redundant_judgment() {
        let c = canary("What is 7 + 5?", "12");
        assert_eq!(grade(&c, Some("12.")), Verdict::Pass);
        assert_eq!(grade(&c, Some("The answer is 12")), Verdict::Pass);
        assert_eq!(grade(&c, Some("13")), Verdict::Fail);
        assert_eq!(grade(&c, None), Verdict::Inconclusive);

        let some = |t: &str| Some(t.to_string());
        let (v, m) = judge(&[some("Blue."), some("blue"), some("green")]);
        assert_eq!(v, [Verdict::Pass, Verdict::Pass, Verdict::Fail]);
        assert_eq!(m.as_deref(), Some("blue"));
        let (v, m) = judge(&[some("blue"), some("green")]);
        assert_eq!(v, [Verdict::Inconclusive, Verdict::Inconclusive]);
        assert!(m.is_none());
        let (v, _) = judge(&[some("blue"), some("blue"), None]);
        assert_eq!(v, [Verdict::Pass, Verdict::Pass, Verdict::Inconclusive]);
    }
}
