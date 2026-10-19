//! The community decision lane (#12190): contributor pylons, admitted by
//! trusted checkers' verdicts, checked on their real answers.
//!
//! **Two kinds of pylon.** Operator pylons are the keys in
//! `decisions.pylons` (`DECISION_PYLONS`): asked exactly as before, in the
//! request's order. Contributor pylons are every other fresh beacon that
//! advertises `<key>:pylon/decision` on the `cj-decision` lane and serves
//! the pinned Clef-Flash ([`pinned_identity`]). Contributors exist only
//! when `decisions.checkers` names at least one trusted checker; with no
//! checkers the dispatcher is what it was (every pylon in
//! `decisions.pylons`, or every pylon when that is empty, is an operator
//! pylon).
//!
//! **Admission** ([`admission`]). A contributor is asked only with a
//! trusted checker's `check-pass` (NIP-32, `openagents.pylon`, on a
//! `cj-decision` receipt) from the last 24 hours, no trusted `check-fail`
//! after it, and no strike of ours after it. Three strikes in 24 hours
//! keep it out until a check-pass newer than the third.
//!
//! **The community lane** ([`Lane`]). Contributors are asked only for a
//! request on the community lane: `X-Decision-Lane: community`, or a
//! caller key whose holder opted in (`tenancy::keys::Key::community`).
//! There the contributors go first, then the request's own order.
//! Without it a contributor is never asked.
//!
//! **What a pylon receives** ([`job`]). `model`, `state`, `questions`,
//! and an opaque request id hashed from the caller's idempotency key and
//! the pylon: never an account, key, or caller identity.
//!
//! **Checking real answers.** Every contributor answer must agree with its
//! own probabilities ([`consistent`]), else it is a strike and the next
//! door is asked. One answer in twenty (`decisions.sample`) is asked again
//! at the reference (`decisions.reference`, a Psionic Clef-Flash
//! `/v1/systemone` on the same pinned artifact) after the caller has its
//! answer, and each question's centred-log distance ([`distances`],
//! `pylon::verify`) goes into the pylon's window ([`Window`]): an item
//! above 0.5, or a mean above 0.15 over its last eight sampled answers,
//! is a strike; a mean above 0.05 is logged as a miss. A reference that
//! fails is never a strike. Hidden canaries (`decisions.canaries`, made by
//! `openagents pylon canary make`) go out now and then (`canary_rate`) as
//! ordinary jobs and are scored the same way against their stored vectors.
//!
//! **Strikes** ([`Strikes`]). A strike is one JSON line in
//! `<registry>/decisions/strikes.jsonl` (read back at start, so it
//! survives a restart), one line in the day's decision log, and, when the
//! gateway holds a checker key (`<registry>/decisions/checker.key`), a
//! `check-fail` label on the struck job's receipt. [`read_strikes`] is the
//! feed the earnings ledger reads to forfeit unpaid shares.
//!
//! **Fairness** ([`Fairness`]). One NIP-OA owner (or one pylon key, when
//! unlinked) may hold at most `owner_share` (default 40 %) of the
//! community-lane decisions in flight, and always at least one.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::HeaderMap;
use pylon::verify::{self, Limits};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The request header that names the lane.
pub const LANE_HEADER: &str = "x-decision-lane";

/// A day, in seconds.
pub const DAY: u64 = 86_400;

/// The strikes file under `<registry>/decisions`.
pub const STRIKES_FILE: &str = "strikes.jsonl";

/// Which pylons a decision may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Operator pylons only (today's behavior).
    Default,
    /// Checked contributor pylons first, then the request's order.
    Community,
}

impl Lane {
    /// The lane `X-Decision-Lane` names: `community`, or `default` to
    /// keep a community key's decision off contributor pylons.
    #[must_use]
    pub fn from_header(headers: &HeaderMap) -> Option<Self> {
        let value = headers.get(LANE_HEADER)?.to_str().ok()?.trim();
        if value.eq_ignore_ascii_case("community") {
            Some(Self::Community)
        } else if value.eq_ignore_ascii_case("default") {
            Some(Self::Default)
        } else {
            None
        }
    }
}

/// Whether a beacon's served identity is the pinned Clef-Flash: the model
/// `clef-flash` at the pinned file's digest (`sha256:…`) or the pinned
/// tensors' model id (`crates/pylon` `setup`).
#[must_use]
pub fn pinned_identity(identity: &str) -> bool {
    let Some((model, digest)) = identity.split_once('@') else {
        return false;
    };
    model == "clef-flash"
        && (digest == format!("sha256:{}", pylon::setup::CLEF_FLASH_SHA256)
            || digest == pylon::setup::CLEF_FLASH_MODEL_ID)
}

/// A pylon's newest trusted verdicts, Unix seconds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Checks {
    pub pass: Option<u64>,
    pub fail: Option<u64>,
}

/// The newest counted `check-pass` and `check-fail` per pylon key, from
/// verdicts on decision (`cj-decision`) receipts only.
#[must_use]
pub fn checks(verdicts: &pylon::check::Verdicts) -> HashMap<String, Checks> {
    let mut out: HashMap<String, Checks> = HashMap::new();
    for check in &verdicts.checks {
        let decision = verdicts
            .receipts
            .get(&check.receipt)
            .is_some_and(|r| r.lane == nostr::pylon::Lane::CjDecision);
        if !decision {
            continue;
        }
        let entry = out.entry(check.provider.clone()).or_default();
        let slot = match check.verdict {
            nostr::pylon::Verdict::Pass => &mut entry.pass,
            nostr::pylon::Verdict::Fail => &mut entry.fail,
            nostr::pylon::Verdict::Inconclusive => continue,
        };
        *slot = Some(slot.map_or(check.created_at, |at| at.max(check.created_at)));
    }
    out
}

/// Why a contributor pylon is or is not asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    Admitted,
    /// No trusted check-pass.
    Unchecked,
    /// Its newest check-pass is older than 24 hours.
    Stale,
    /// A trusted check-fail at or after its newest check-pass.
    Failed,
    /// A strike at or after its newest check-pass.
    Struck,
    /// Three strikes in 24 hours, the third at or after its newest
    /// check-pass.
    ThreeStrikes,
}

/// Whether a contributor with `checks` and our `strikes` (Unix seconds,
/// any order) is admitted at `now`.
#[must_use]
pub fn admission(checks: Checks, strikes: &[u64], now: u64) -> Admission {
    let Some(pass) = checks.pass else {
        return Admission::Unchecked;
    };
    if now.saturating_sub(pass) > DAY {
        return Admission::Stale;
    }
    if checks.fail.is_some_and(|fail| fail >= pass) {
        return Admission::Failed;
    }
    let mut times: Vec<u64> = strikes.to_vec();
    times.sort_unstable();
    let Some(&last) = times.last() else {
        return Admission::Admitted;
    };
    if last < pass {
        return Admission::Admitted;
    }
    // A third strike within 24 hours of the two before it.
    let three = times
        .windows(3)
        .any(|w| w[2] >= pass && w[2].saturating_sub(w[0]) <= DAY);
    if three {
        Admission::ThreeStrikes
    } else {
        Admission::Struck
    }
}

/// The most community-lane decisions one owner may hold when `in_flight`
/// are in flight (this one counted): `share` of them, at least one.
#[must_use]
pub fn owner_cap(share: f64, in_flight: usize) -> usize {
    let share = if share.is_finite() {
        share.clamp(0.0, 1.0)
    } else {
        0.4
    };
    ((share * in_flight as f64).ceil() as usize).max(1)
}

/// Community-lane decisions in flight, in all and per owner.
#[derive(Debug, Default)]
pub struct Fairness {
    in_flight: AtomicUsize,
    by_owner: Mutex<HashMap<String, usize>>,
}

/// A community-lane decision in flight, counted until dropped.
pub struct InLane<'a>(&'a Fairness);

impl Drop for InLane<'_> {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

/// One owner's contributor job in flight, counted until dropped.
pub struct ForOwner<'a> {
    fairness: &'a Fairness,
    owner: String,
}

impl Drop for ForOwner<'_> {
    fn drop(&mut self) {
        if let Ok(mut map) = self.fairness.by_owner.lock()
            && let Some(count) = map.get_mut(&self.owner)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(&self.owner);
            }
        }
    }
}

impl Fairness {
    /// Count a community-lane decision.
    pub fn enter(&self) -> InLane<'_> {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        InLane(self)
    }

    /// Community-lane decisions in flight.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Relaxed)
    }

    /// Hold a job for `owner`, unless it already holds its share.
    pub fn try_owner(&self, owner: &str, share: f64) -> Option<ForOwner<'_>> {
        let cap = owner_cap(share, self.in_flight().max(1));
        let mut map = self.by_owner.lock().ok()?;
        let held = map.entry(owner.to_owned()).or_default();
        if *held >= cap {
            return None;
        }
        *held += 1;
        Some(ForOwner {
            fairness: self,
            owner: owner.to_owned(),
        })
    }
}

/// What a window of sampled answers came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scored {
    /// Within the limits (`mean` once the window is full).
    Fine { mean: Option<f64> },
    /// The window's mean is above [`Limits::mean`]: logged, no strike.
    Miss { mean: f64 },
    /// An item above [`Limits::item`] (`item`), or the window's mean
    /// above [`Limits::strike`] (`mean`).
    Strike { reason: &'static str, value: f64 },
}

/// One pylon's last [`Limits::items`] sampled distances.
#[derive(Debug, Default, Clone)]
pub struct Window(VecDeque<f64>);

impl Window {
    /// Add `distances` (one per answered question) and score. A strike
    /// empties the window, so a pylon admitted again starts fresh.
    pub fn score(&mut self, distances: &[f64], limits: &Limits) -> Scored {
        if let Some(&worst) = distances
            .iter()
            .find(|d| !d.is_finite() || **d > limits.item)
        {
            self.0.clear();
            return Scored::Strike {
                reason: "item",
                value: worst,
            };
        }
        for &d in distances {
            self.0.push_back(d);
            while self.0.len() > limits.items.max(1) {
                self.0.pop_front();
            }
        }
        if self.0.len() < limits.items.max(1) {
            return Scored::Fine { mean: None };
        }
        let mean = self.0.iter().sum::<f64>() / self.0.len() as f64;
        if mean > limits.strike {
            self.0.clear();
            Scored::Strike {
                reason: "mean",
                value: mean,
            }
        } else if mean > limits.mean {
            Scored::Miss { mean }
        } else {
            Scored::Fine { mean: Some(mean) }
        }
    }
}

/// Why two answers could not be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// The pylon's answer disagrees with its own probabilities: a strike.
    Inconsistent(String),
    /// The reference's answer is unusable: never a strike.
    Reference(String),
}

/// Whether every answer agrees with its own probabilities
/// (`pylon::verify::consistent`): the choice is its most likely option, a
/// score its expected level.
///
/// # Errors
///
/// The first question whose answer does not.
pub fn consistent(questions: &Map<String, Value>, response: &Value) -> Result<(), String> {
    for (id, question) in questions {
        verify::consistent(question, &response["answers"][id])
            .map_err(|why| format!("question `{id}`: {why}"))?;
    }
    Ok(())
}

/// Each question's centred-log distance between `answer` and `reference`
/// (`pylon::verify::distance`).
///
/// # Errors
///
/// [`Mismatch`]: the reference's answer is checked first, so a broken
/// reference never strikes.
pub fn distances(
    questions: &Map<String, Value>,
    answer: &Value,
    reference: &Value,
) -> Result<Vec<f64>, Mismatch> {
    questions
        .iter()
        .map(|(id, question)| {
            let q = verify::consistent(question, &reference["answers"][id])
                .map_err(|why| Mismatch::Reference(format!("question `{id}`: {why}")))?;
            let p = verify::consistent(question, &answer["answers"][id])
                .map_err(|why| Mismatch::Inconsistent(format!("question `{id}`: {why}")))?;
            Ok(verify::distance(&p, &q))
        })
        .collect()
}

/// The job a pylon receives.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    /// Exactly `model`, `state`, and `questions`.
    pub body: Value,
    /// The job's request id: `dec-` and 32 hex digits hashed from the
    /// caller's idempotency key (or a random id) and the pylon key, the
    /// same on a retry and meaningless to the pylon.
    pub request: String,
}

/// The job for one pylon. Nothing about the caller goes in: no account,
/// key, header, or the caller's own idempotency key.
#[must_use]
pub fn job(
    model: &str,
    state: &Value,
    questions: &Map<String, Value>,
    request: &str,
    pylon: &str,
) -> Job {
    let digest = nostr::pylon::sha256_hex(format!("{request}\n{pylon}").as_bytes());
    Job {
        body: json!({"model": model, "state": state, "questions": questions}),
        request: format!("dec-{}", &digest[..32]),
    }
}

/// Check a job before it goes to any pylon: its body holds exactly
/// `model`, `state`, and `questions`, and its request id is opaque.
///
/// # Errors
///
/// What the job carries that it must not.
pub fn carries_no_identity(job: &Job) -> Result<(), String> {
    let keys: BTreeSet<&str> = job
        .body
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if keys != BTreeSet::from(["model", "state", "questions"]) {
        return Err(format!("the job carries other fields: {keys:?}"));
    }
    let opaque = job.request.len() == 36
        && job.request.starts_with("dec-")
        && job.request[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !opaque {
        return Err("the job's request id is not opaque".into());
    }
    Ok(())
}

/// One strike against a contributor pylon: a line of
/// `<registry>/decisions/strikes.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    /// Unix seconds.
    pub at: u64,
    /// The pylon key (hex).
    pub pylon: String,
    /// Its `30200` address.
    pub address: String,
    /// Its NIP-OA owner, or the pylon key when unlinked.
    pub owner: String,
    /// `inconsistent`, `sampled_item`, `sampled_mean`, `canary_item`,
    /// `canary_mean`, or `canary_inconsistent`.
    pub reason: String,
    /// The distance (an item's, or the window's mean), when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The struck job's opaque request id, when it was a job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

/// The strikes the gateway holds, kept on disk.
#[derive(Debug)]
pub struct Strikes {
    path: PathBuf,
    held: Mutex<Vec<Strike>>,
}

impl Strikes {
    /// The strikes under `dir` (`<registry>/decisions`), read back.
    #[must_use]
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(STRIKES_FILE);
        let held = read_file(&path, 0);
        Self {
            path,
            held: Mutex::new(held),
        }
    }

    /// Keep a strike and append it to the file.
    pub fn record(&self, strike: &Strike) {
        if let Ok(mut held) = self.held.lock() {
            held.push(strike.clone());
        }
        let line = serde_json::to_string(strike).unwrap_or_default();
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| writeln!(file, "{line}"));
        if let Err(e) = written {
            eprintln!("decisions: {}: {e}", self.path.display());
        }
    }

    /// The times of `pylon`'s strikes.
    #[must_use]
    pub fn times(&self, pylon: &str) -> Vec<u64> {
        self.held
            .lock()
            .map(|held| {
                held.iter()
                    .filter(|s| s.pylon == pylon)
                    .map(|s| s.at)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The strikes feed: every strike the gateway under `registry` recorded
/// at or after `since` (Unix seconds), oldest first. The earnings ledger
/// reads it to forfeit a struck pylon's unpaid shares.
#[must_use]
pub fn read_strikes(registry: &Path, since: u64) -> Vec<Strike> {
    read_file(&registry.join("decisions").join(STRIKES_FILE), since)
}

fn read_file(path: &Path, since: u64) -> Vec<Strike> {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<Strike>(line).ok())
                .filter(|s| s.at >= since)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn questions() -> Map<String, Value> {
        json!({
            "topic": {"type": "choice", "instructions": "Which team?",
                      "criteria": {"billing": "money", "technical": "bugs"}},
            "refund": {"type": "noul", "instructions": "Money back?"},
            "sev": {"type": "score", "criteria": ["low", "mid", "high"]},
        })
        .as_object()
        .cloned()
        .unwrap()
    }

    fn answer(billing: f64, refund: f64) -> Value {
        json!({"model": "clef-flash", "answers": {
            "topic": {"type": "choice", "choice": if billing >= 0.5 { "billing" } else { "technical" },
                      "probabilities": {"billing": billing, "technical": 1.0 - billing}},
            "refund": {"type": "noul", "noul": refund},
            "sev": {"type": "score", "score": 1.8, "probabilities": {"0": 0.05, "1": 0.1, "2": 0.85}},
        }})
    }

    #[test]
    fn the_lane_comes_from_the_header() {
        let mut headers = HeaderMap::new();
        assert_eq!(Lane::from_header(&headers), None);
        headers.insert(LANE_HEADER, "Community".parse().unwrap());
        assert_eq!(Lane::from_header(&headers), Some(Lane::Community));
        headers.insert(LANE_HEADER, "default".parse().unwrap());
        assert_eq!(Lane::from_header(&headers), Some(Lane::Default));
        headers.insert(LANE_HEADER, "anything".parse().unwrap());
        assert_eq!(Lane::from_header(&headers), None);
    }

    #[test]
    fn only_the_pinned_clef_flash_is_a_contributor_identity() {
        let file = format!("clef-flash@sha256:{}", pylon::setup::CLEF_FLASH_SHA256);
        assert!(pinned_identity(&file));
        assert!(pinned_identity(&format!(
            "clef-flash@{}",
            pylon::setup::CLEF_FLASH_MODEL_ID
        )));
        assert!(!pinned_identity("clef-flash"));
        assert!(!pinned_identity(&format!(
            "clef-flash@sha256:{}",
            "ab".repeat(32)
        )));
        assert!(!pinned_identity(&file.replace("clef-flash", "clef")));
    }

    #[test]
    fn admission_needs_a_fresh_pass_newer_than_any_fail_or_strike() {
        let now = 10 * DAY;
        let pass = |at| Checks {
            pass: Some(at),
            fail: None,
        };
        assert_eq!(admission(Checks::default(), &[], now), Admission::Unchecked);
        assert_eq!(admission(pass(now - DAY - 1), &[], now), Admission::Stale);
        assert_eq!(admission(pass(now - 100), &[], now), Admission::Admitted);
        let failed = Checks {
            pass: Some(now - 100),
            fail: Some(now - 50),
        };
        assert_eq!(admission(failed, &[], now), Admission::Failed);
        let passed_again = Checks {
            pass: Some(now - 10),
            fail: Some(now - 50),
        };
        assert_eq!(admission(passed_again, &[], now), Admission::Admitted);
        // A strike benches until a newer check-pass.
        assert_eq!(
            admission(pass(now - 100), &[now - 60], now),
            Admission::Struck
        );
        assert_eq!(
            admission(pass(now - 30), &[now - 60], now),
            Admission::Admitted
        );
    }

    #[test]
    fn three_strikes_in_a_day_need_a_pass_newer_than_the_third() {
        let now = 10 * DAY;
        let strikes = [now - 3_000, now - 2_000, now - 1_000];
        let pass = |at| Checks {
            pass: Some(at),
            fail: None,
        };
        // A pass between the second and the third does not admit.
        assert_eq!(
            admission(pass(now - 1_500), &strikes, now),
            Admission::ThreeStrikes
        );
        assert_eq!(
            admission(pass(now - 1_000), &strikes, now),
            Admission::ThreeStrikes
        );
        // A pass newer than the third does.
        assert_eq!(
            admission(pass(now - 999), &strikes, now),
            Admission::Admitted
        );
        // Three strikes spread over more than a day are single strikes.
        let spread = [now - 3 * DAY, now - 2 * DAY, now - 100];
        assert_eq!(admission(pass(now - 200), &spread, now), Admission::Struck);
    }

    #[test]
    fn one_owner_holds_at_most_its_share_and_always_one() {
        assert_eq!(owner_cap(0.4, 1), 1);
        assert_eq!(owner_cap(0.4, 5), 2);
        assert_eq!(owner_cap(0.4, 10), 4);
        let fairness = Fairness::default();
        let lanes: Vec<_> = (0..5).map(|_| fairness.enter()).collect();
        let a = fairness.try_owner("alice", 0.4).unwrap();
        let b = fairness.try_owner("alice", 0.4).unwrap();
        assert!(fairness.try_owner("alice", 0.4).is_none());
        let other = fairness.try_owner("bob", 0.4);
        assert!(other.is_some());
        drop(a);
        assert!(fairness.try_owner("alice", 0.4).is_some());
        drop((b, other, lanes));
        assert_eq!(fairness.in_flight(), 0);
    }

    #[test]
    fn a_window_strikes_a_far_item_or_a_high_mean_and_logs_a_miss() {
        let limits = verify::CLEF_FLASH;
        let mut window = Window::default();
        assert_eq!(
            window.score(&[0.0; 3], &limits),
            Scored::Fine { mean: None }
        );
        assert!(matches!(
            window.score(&[0.6], &limits),
            Scored::Strike { reason: "item", .. }
        ));
        assert!(matches!(
            window.score(&[f64::INFINITY], &limits),
            Scored::Strike { reason: "item", .. }
        ));
        let mut window = Window::default();
        assert!(matches!(
            window.score(&[0.08; 8], &limits),
            Scored::Miss { .. }
        ));
        let mut window = Window::default();
        assert!(matches!(
            window.score(&[0.2; 8], &limits),
            Scored::Strike { reason: "mean", .. }
        ));
        // Emptied by the strike.
        assert_eq!(window.score(&[0.0], &limits), Scored::Fine { mean: None });
        let mut honest = Window::default();
        assert!(matches!(
            honest.score(&[0.001; 9], &limits),
            Scored::Fine { mean: Some(_) }
        ));
    }

    #[test]
    fn distances_compare_each_question_and_a_bad_reference_never_strikes() {
        let q = questions();
        let same = distances(&q, &answer(0.9, 0.2), &answer(0.9, 0.2)).unwrap();
        assert_eq!(same.len(), 3);
        assert!(same.iter().all(|d| *d < 1e-12));
        let far = distances(&q, &answer(0.6, 0.2), &answer(0.99, 0.2)).unwrap();
        assert!(far.iter().any(|d| *d > verify::CLEF_FLASH.item));
        // A choice that is not its own most likely option.
        let mut lying = answer(0.9, 0.2);
        lying["answers"]["topic"]["choice"] = json!("technical");
        assert!(consistent(&q, &lying).is_err());
        assert!(matches!(
            distances(&q, &lying, &answer(0.9, 0.2)),
            Err(Mismatch::Inconsistent(_))
        ));
        assert!(matches!(
            distances(&q, &answer(0.9, 0.2), &lying),
            Err(Mismatch::Reference(_))
        ));
        consistent(&q, &answer(0.9, 0.2)).unwrap();
    }

    #[test]
    fn a_job_carries_no_caller_identity() {
        let q = questions();
        let account = "acct_7f3a";
        let key = "oak_2c9d1e.s3cr3t";
        let idempotency = format!("{account}-{key}-turn-12");
        let built = job(
            "openagents/decide",
            &json!("I was billed twice"),
            &q,
            &idempotency,
            "ab12",
        );
        carries_no_identity(&built).unwrap();
        let text = format!("{} {}", built.body, built.request);
        for secret in [account, key, "2c9d1e", idempotency.as_str()] {
            assert!(!text.contains(secret), "{secret} reached the job");
        }
        // The same request to the same pylon is the same job; another
        // pylon sees another id.
        assert_eq!(
            built,
            job(
                "openagents/decide",
                &json!("I was billed twice"),
                &q,
                &idempotency,
                "ab12"
            )
        );
        assert_ne!(
            built.request,
            job("openagents/decide", &json!("x"), &q, &idempotency, "cd34").request
        );
        let mut extra = built.clone();
        extra.body["account"] = json!(account);
        assert!(carries_no_identity(&extra).is_err());
        let mut named = built;
        named.request = idempotency;
        assert!(carries_no_identity(&named).is_err());
    }

    #[test]
    fn strikes_survive_a_restart_and_feed_the_ledger() {
        let registry = tempfile::tempdir().unwrap();
        let dir = registry.path().join("decisions");
        std::fs::create_dir_all(&dir).unwrap();
        let strike = |at, pylon: &str| Strike {
            at,
            pylon: pylon.into(),
            address: format!("30200:{pylon}:box"),
            owner: pylon.into(),
            reason: "canary_item".into(),
            value: Some(0.9),
            request: None,
        };
        let strikes = Strikes::open(&dir);
        strikes.record(&strike(100, "aa"));
        strikes.record(&strike(200, "bb"));
        strikes.record(&strike(300, "aa"));
        let again = Strikes::open(&dir);
        assert_eq!(again.times("aa"), [100, 300]);
        assert_eq!(read_strikes(registry.path(), 150).len(), 2);
        assert_eq!(read_strikes(registry.path(), 0)[1], strike(200, "bb"));
    }
}
