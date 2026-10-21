//! The pylon router (#12366): which live pylon answers a person's message.
//!
//! [`route`] reads the beacons, admits the pylons that can take a free
//! conversation job right now ([`candidates`]), and ranks them in code.
//! When at least one live pylon serves decisions (a Clef model on the
//! `cj-decision` lane), it asks one typed Choice ([`question`]) through a
//! [`Decide`] seam: the caller sends it to the Decisions API, which asks a
//! connected Clef pylon first. The answer picks a pylon or `default` (the
//! caller's own chat). No word of the message is matched here; the message
//! is only the decision's input. Then the job goes to the chosen pylon with
//! [`client::ask_conversation`], failing over to the next candidate in
//! rank order on a refusal, a timeout, or an empty answer.
//!
//! A request that asks for one model ([`Settings::want`]) goes only to a
//! pylon or a ready model group serving exactly that model ([`crate::group`]),
//! ranked in code with no decision and no `default`: it is never answered
//! by another model. A request that names no model never goes to a group,
//! and nothing routes to a group's stage.
//!
//! A request with [`Settings::deadline`] goes out under `pylon.deadline.v1`
//! with that budget, so a model whose beacon declares a deadline past 90
//! seconds can take it; the budget then also bounds the tries together.
//! Without it, jobs keep the 90-second profile, and a model that needs
//! longer refuses them at once.
//!
//! Each [`Routed`] says how the choice was made and what was tried, for the
//! caller's log. It never carries the message.

use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use nostr::pylon::{self, Beacon, BeaconBook, Family, Freshness, Lane, Status, Tier};

use crate::client::{self, Answer, Ask};
use crate::engine::Turn;
use crate::group::{self, Want};
use crate::identity::Identity;
use crate::now;
use crate::relay::{self, LIFETIME};

/// The decision's question set, for the caller's metadata.
pub const QUESTION_SET: &str = "openagents.pylon-router.v1";
/// The question's name.
pub const QUESTION: &str = "pylon";
/// The option that hands the message to the caller's own chat.
pub const DEFAULT: &str = "default";
/// The settlement a free pylon advertises; the router sends free jobs only.
pub const FREE: &str = "free-v1";
/// The most pylons offered to the decision model, best ranked first.
pub const MAX_CANDIDATES: usize = 16;
/// How sure the model must be of `default` before a message skips the
/// pylons. A weak pylon answer costs less than skipping the pylons, so an
/// unsure `default` still goes to the best-ranked pylon.
pub const DEFAULT_CONFIDENCE: f64 = 0.6;
/// The most characters of the latest message the decision reads.
const DECIDE_LATEST: usize = 2_000;
/// The most earlier turns, and characters of each, the decision reads.
const DECIDE_EARLIER: usize = 3;
const DECIDE_TURN: usize = 400;
/// The most request text a job carries, inside NIP-CJ's
/// [`crate::job::MAX_INPUT_BYTES`] with room for the instructions.
const JOB_TEXT_BYTES: usize = crate::job::MAX_INPUT_BYTES - 4 * 1024;

/// One pylon that can answer now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The pylon's hex key.
    pub provider: String,
    /// The beacon's `d` slug.
    pub pylon: String,
    pub label: String,
    /// The conversation service's model.
    pub model: String,
    pub family: Family,
    pub tier: Tier,
    pub memory_gb: u32,
    pub free: u32,
    pub total: u32,
    pub observed_at: u64,
}

impl Candidate {
    fn from_beacon(beacon: &Beacon, service: &pylon::Service) -> Self {
        Self {
            provider: beacon.provider.clone(),
            pylon: beacon.pylon.clone(),
            label: beacon.label.clone(),
            model: service.model.clone(),
            family: beacon.class.family,
            tier: beacon.class.tier,
            memory_gb: beacon.class.memory_gb,
            free: beacon.slots.free,
            total: beacon.slots.total,
            observed_at: beacon.observed_at,
        }
    }

    /// The beacon's address, `30200:<key>:<slug>`.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{}:{}:{}", pylon::BEACON_KIND, self.provider, self.pylon)
    }

    /// The option's description: the model and the hardware, in few words
    /// so a Clef door reads the whole set quickly (#11247).
    #[must_use]
    pub fn describe(&self) -> String {
        let family = match self.family {
            Family::UnifiedMemory => "unified-memory computer",
            Family::Gpu => "GPU",
            Family::Cpu => "CPU",
        };
        let tier = match self.tier {
            Tier::Small => "small",
            Tier::Medium => "medium",
            Tier::Large => "large",
            Tier::Xl => "extra-large",
        };
        format!(
            "{}: runs {} on a {tier} {family} ({} GB), {} of {} slots free",
            self.label.chars().take(40).collect::<String>(),
            self.model.chars().take(64).collect::<String>(),
            self.memory_gb,
            self.free,
            self.total,
        )
    }
}

fn tier_rank(tier: Tier) -> u8 {
    match tier {
        Tier::Small => 0,
        Tier::Medium => 1,
        Tier::Large => 2,
        Tier::Xl => 3,
    }
}

/// Whether `beacon` is a live complete pylon at `at`: no model group, fresh,
/// online, with a free slot, and not failing a trusted check.
fn live(beacon: &Beacon, at: u64, failing: &BTreeSet<String>) -> bool {
    beacon.group.is_none()
        && !failing.contains(&beacon.address())
        && pylon::freshness(beacon, at) == Freshness::Fresh
        && beacon.status == Status::Online
        && beacon.slots.free > 0
}

/// The pylons that can take a free conversation job at `at`, best first:
/// the largest tier, then the most free slots, then the newest beacon. At
/// most [`MAX_CANDIDATES`].
#[must_use]
pub fn candidates(book: &BeaconBook, at: u64, failing: &BTreeSet<String>) -> Vec<Candidate> {
    candidates_for(book, at, failing, None)
}

/// As [`candidates`], for a request that asks for `want`: only complete
/// pylons and ready group heads serving exactly that model, each group one
/// candidate with its one serial slot. `None` is [`candidates`].
#[must_use]
pub fn candidates_for(
    book: &BeaconBook,
    at: u64,
    failing: &BTreeSet<String>,
    want: Option<&Want>,
) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = book
        .iter()
        .map(|(_, beacon)| beacon)
        .filter(|b| b.settlement.iter().any(|s| s == FREE))
        .filter_map(|b| {
            match want {
                Some(want) => group::serves_want(b, at, failing, Lane::CjConversation, want),
                None => live(b, at, failing)
                    .then(|| b.serves(Lane::CjConversation))
                    .flatten(),
            }
            .map(|service| Candidate::from_beacon(b, service))
        })
        .collect();
    found.sort_by(|a, b| {
        (tier_rank(b.tier), b.free, b.observed_at, &a.provider).cmp(&(
            tier_rank(a.tier),
            a.free,
            a.observed_at,
            &b.provider,
        ))
    });
    found.truncate(MAX_CANDIDATES);
    found
}

/// How many live pylons serve decisions (a Clef model) at `at`.
#[must_use]
pub fn deciders(book: &BeaconBook, at: u64, failing: &BTreeSet<String>) -> usize {
    book.iter()
        .map(|(_, beacon)| beacon)
        .filter(|b| live(b, at, failing) && b.serves(Lane::CjDecision).is_some())
        .count()
}

/// The typed question the decision model answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub name: String,
    pub instructions: String,
    /// The conversation, bounded: the decision's input.
    pub input: String,
    /// `(value, description)`: `p1`…`pN` in candidate order, then
    /// [`DEFAULT`].
    pub choices: Vec<(String, String)>,
}

/// The option value for the candidate at `index`.
fn value(index: usize) -> String {
    format!("p{}", index + 1)
}

fn clip(text: &str, chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= chars {
        return text.to_owned();
    }
    let mut kept: String = text.chars().take(chars).collect();
    kept.push('…');
    kept
}

/// The question for `candidates` about `turns`, whose last is the person's
/// message.
#[must_use]
pub fn question(candidates: &[Candidate], turns: &[Turn]) -> Question {
    let latest = turns
        .iter()
        .rev()
        .find(|t| t.role == "user")
        .map(|t| clip(&t.content, DECIDE_LATEST))
        .unwrap_or_default();
    let mut input = format!("The person's latest message:\n{latest}");
    let earlier: Vec<&Turn> = turns
        .iter()
        .rev()
        .skip(1)
        .take(DECIDE_EARLIER)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if !earlier.is_empty() {
        input.push_str("\n\nEarlier in the conversation:");
        for turn in earlier {
            let who = if turn.role == "user" {
                "Person"
            } else {
                "Answer"
            };
            input.push_str(&format!("\n{who}: {}", clip(&turn.content, DECIDE_TURN)));
        }
    }
    let mut choices: Vec<(String, String)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (value(i), c.describe()))
        .collect();
    choices.push((
        DEFAULT.into(),
        "OpenAgents' hosted chat, for messages that need current information, \
the person's files or accounts, tools, or long careful reasoning"
            .into(),
    ));
    Question {
        name: QUESTION.into(),
        instructions:
            "Pick who should answer the person's latest message. Each p option is a live \
pylon: a computer on the OpenAgents network running the named model on the named hardware. \
Larger models on larger hardware answer harder questions better. Pick the pylon that can answer \
this message well. Pick default only when no listed pylon could."
                .into(),
        input,
        choices,
    }
}

/// The decision model's answer to a [`Question`].
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The option at argmax.
    pub choice: String,
    /// Its probability.
    pub confidence: f64,
    /// Which door answered (`pylon:<slug>`, `clef`, ...), when the API said.
    pub door: Option<String>,
    /// The model that answered, by its served name.
    pub model: Option<String>,
}

/// What a verdict picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// The candidate at this index answers.
    Pylon(usize),
    /// The caller's own chat answers.
    Default,
}

/// What `verdict` picks among `candidates` candidates: a named pylon; a
/// confident [`DEFAULT`]; or, for an unsure `default`, the best-ranked
/// pylon. `None` for an option that was never offered.
#[must_use]
pub fn read(verdict: &Verdict, candidates: usize) -> Option<Pick> {
    if verdict.choice == DEFAULT {
        return Some(
            if verdict.confidence >= DEFAULT_CONFIDENCE || candidates == 0 {
                Pick::Default
            } else {
                Pick::Pylon(0)
            },
        );
    }
    let index: usize = verdict.choice.strip_prefix('p')?.parse().ok()?;
    (1..=candidates)
        .contains(&index)
        .then(|| Pick::Pylon(index - 1))
}

/// A decision in progress.
pub type Deciding<'a> = Pin<Box<dyn Future<Output = Result<Verdict, String>> + Send + 'a>>;

/// Where the router's question is answered: the Decisions API in
/// production, a fixed answer in tests.
pub trait Decide: Send + Sync {
    fn decide<'a>(&'a self, question: &'a Question) -> Deciding<'a>;
}

/// How the router runs.
#[derive(Debug, Clone)]
pub struct Settings {
    pub relay: String,
    /// Where job receipts are recorded ([`Ask::home`]).
    pub home: PathBuf,
    /// Checkers whose `check-fail` keeps a pylon out.
    pub checkers: BTreeSet<String>,
    /// How long reading the beacons and verdicts may take.
    pub discover: Duration,
    /// How long the decision may take before the ranking decides.
    pub decide: Duration,
    /// How long one pylon has to answer.
    pub try_wait: Duration,
    /// How long all the tries together may take.
    pub budget: Duration,
    /// The most pylons tried.
    pub tries: usize,
    /// The model the request asks for, when it names one: only a pylon or
    /// a ready group serving exactly it answers, and nothing else does.
    pub want: Option<Want>,
    /// The caller's budget for one answer under `pylon.deadline.v1`; `None`
    /// keeps the ordinary 90-second profile.
    pub deadline: Option<Duration>,
}

impl Settings {
    #[must_use]
    pub fn new(relay: impl Into<String>, home: PathBuf) -> Self {
        Self {
            relay: relay.into(),
            home,
            checkers: BTreeSet::new(),
            discover: Duration::from_secs(5),
            decide: Duration::from_secs(3),
            try_wait: Duration::from_secs(30),
            budget: Duration::from_secs(50),
            tries: 3,
            want: None,
            deadline: None,
        }
    }
}

/// How the order of pylons was chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum How {
    /// The decision model chose.
    Decided {
        choice: String,
        confidence: f64,
        door: Option<String>,
        model: Option<String>,
        ms: u64,
    },
    /// The code ranking chose, and why the model didn't.
    Ranked { why: String },
}

/// How routing ended.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// A pylon answered.
    Answered(Box<Answer>),
    /// The decision handed the message to the caller's own chat.
    Declined,
    /// No pylon could answer: none admitted, the relay unreachable, or
    /// every try failed.
    Unavailable(String),
}

/// What [`route`] did.
#[derive(Debug, Clone)]
pub struct Routed {
    pub outcome: Outcome,
    pub how: How,
    /// How many pylons were admitted.
    pub candidates: usize,
    /// How many live pylons serve decisions.
    pub deciders: usize,
    /// Each pylon tried and why it gave no answer, in order.
    pub failed: Vec<(String, String)>,
}

impl Routed {
    fn unavailable(why: String, how: How) -> Self {
        Self {
            outcome: Outcome::Unavailable(why),
            how,
            candidates: 0,
            deciders: 0,
            failed: Vec::new(),
        }
    }
}

/// The job's text from `turns`: the last user turn as the prompt, and the
/// newest earlier turns that fit [`JOB_TEXT_BYTES`] and NIP-CJ's turn
/// bound as history.
#[must_use]
pub fn job_text(turns: &[Turn]) -> (Vec<Turn>, String) {
    let Some(last) = turns.iter().rposition(|t| t.role == "user") else {
        return (Vec::new(), String::new());
    };
    // Keep the current question intact. The job admission checks refuse
    // an oversized prompt instead of silently changing what was asked.
    let prompt = turns[last].content.clone();
    let mut room = JOB_TEXT_BYTES.saturating_sub(prompt.len());
    let mut history = Vec::new();
    for turn in turns[..last].iter().rev() {
        if history.len() + 2 > crate::job::MAX_TURNS || turn.content.len() > room {
            break;
        }
        room -= turn.content.len();
        history.push(turn.clone());
    }
    history.reverse();
    (history, prompt)
}

/// The order to try `candidates` in, and how it was chosen: the decision
/// model's pick first when `deciders` is not zero and it answers in time,
/// else the ranking. `None` when the model handed the message back.
async fn order(
    candidates: Vec<Candidate>,
    deciders: usize,
    turns: &[Turn],
    decide: &dyn Decide,
    wait: Duration,
) -> (Option<Vec<Candidate>>, How) {
    if deciders == 0 {
        return (
            Some(candidates),
            How::Ranked {
                why: "no decision model online".into(),
            },
        );
    }
    let asked = question(&candidates, turns);
    let started = Instant::now();
    let verdict = match tokio::time::timeout(wait, decide.decide(&asked)).await {
        Ok(Ok(verdict)) => verdict,
        Ok(Err(why)) => {
            return (
                Some(candidates),
                How::Ranked {
                    why: format!("the decision failed: {why}"),
                },
            );
        }
        Err(_) => {
            return (
                Some(candidates),
                How::Ranked {
                    why: format!("no decision within {} ms", wait.as_millis()),
                },
            );
        }
    };
    let how = How::Decided {
        choice: verdict.choice.clone(),
        confidence: verdict.confidence,
        door: verdict.door.clone(),
        model: verdict.model.clone(),
        ms: started.elapsed().as_millis() as u64,
    };
    match read(&verdict, candidates.len()) {
        Some(Pick::Default) => (None, how),
        Some(Pick::Pylon(index)) => {
            let mut ordered = candidates;
            let chosen = ordered.remove(index);
            ordered.insert(0, chosen);
            (Some(ordered), how)
        }
        None => (
            Some(candidates),
            How::Ranked {
                why: format!(
                    "the decision named {:?}, which wasn't offered",
                    verdict.choice
                ),
            },
        ),
    }
}

/// Read the beacons and the failing set from the relay.
async fn discover(
    buyer: &Identity,
    settings: &Settings,
) -> Result<(BeaconBook, BTreeSet<String>), String> {
    let mut conn = relay::connect(&settings.relay, buyer, LIFETIME).await?;
    let book = if settings.want.is_some() {
        client::all_beacons(&mut conn, None).await?
    } else {
        client::beacons(&mut conn, None).await?
    };
    let failing = if settings.checkers.is_empty() {
        BTreeSet::new()
    } else {
        let (labels, checked) = crate::check::fetch(
            &mut conn,
            &settings.checkers,
            now().saturating_sub(crate::check::CHECK_WINDOW_SECS),
        )
        .await?;
        crate::check::Verdicts::new(&labels, &checked, &settings.checkers)
            .standings()
            .into_iter()
            .filter(|(_, r)| r.standing == pylon::Standing::Failing)
            .map(|(address, _)| address)
            .collect()
    };
    let _ = conn.close().await;
    Ok((book, failing))
}

/// Route `turns` (the last is the person's message) to a live pylon as
/// `buyer`, with `instructions` as the job's system turn.
pub async fn route(
    buyer: &Identity,
    settings: &Settings,
    decide: &dyn Decide,
    turns: &[Turn],
    instructions: Option<&str>,
) -> Routed {
    let ranked = |why: &str| How::Ranked { why: why.into() };
    let (book, failing) = match tokio::time::timeout(settings.discover, discover(buyer, settings))
        .await
    {
        Ok(Ok(found)) => found,
        Ok(Err(why)) => {
            return Routed::unavailable(format!("the pylon relay: {why}"), ranked("no beacons"));
        }
        Err(_) => {
            return Routed::unavailable(
                "the pylon relay did not answer in time".into(),
                ranked("no beacons"),
            );
        }
    };
    let at = now();
    let want = settings.want.as_ref();
    let found = candidates_for(&book, at, &failing, want);
    let deciders = deciders(&book, at, &failing);
    let admitted = found.len();
    if found.is_empty() {
        let mut routed = Routed::unavailable(
            if want.is_some() {
                "no live pylon serves the requested model".into()
            } else {
                "no live pylon can take a free conversation job".into()
            },
            ranked("no candidates"),
        );
        routed.deciders = deciders;
        return routed;
    }
    // A named model is never handed to the decision, whose `default` is
    // another model.
    let (ordered, how) = if want.is_some() {
        (Some(found), ranked("the request asks for one model"))
    } else {
        order(found, deciders, turns, decide, settings.decide).await
    };
    let Some(ordered) = ordered else {
        return Routed {
            outcome: Outcome::Declined,
            how,
            candidates: admitted,
            deciders,
            failed: Vec::new(),
        };
    };
    let (history, prompt) = job_text(turns);
    let started = Instant::now();
    let mut failed = Vec::new();
    // An extended request may take its whole budget on one try.
    let budget = settings.deadline.map_or(settings.budget, |deadline| {
        settings.budget.max(deadline + client::GRACE)
    });
    for candidate in ordered.into_iter().take(settings.tries) {
        let left = budget.saturating_sub(started.elapsed());
        if left.is_zero() {
            break;
        }
        let ask = Ask {
            relay: settings.relay.clone(),
            pylon: Some(candidate.provider.clone()),
            prompt: prompt.clone(),
            wait: settings.try_wait.min(left),
            publish_receipt: true,
            home: settings.home.clone(),
            checkers: settings.checkers.clone(),
            pay: None,
        };
        let within = settings
            .deadline
            .map(|deadline| client::Within::new(deadline.min(left)));
        let asked = match (want, &within) {
            (Some(want), Some(within)) => {
                client::ask_model_within(buyer, &ask, want, within, &history, instructions).await
            }
            (Some(want), None) => {
                client::ask_model(buyer, &ask, want, &history, instructions).await
            }
            (None, Some(within)) => {
                client::ask_within(buyer, &ask, within, &history, instructions).await
            }
            (None, None) => client::ask_conversation(buyer, &ask, &history, instructions).await,
        };
        match asked {
            Ok(answer) if answer.text.as_deref().is_some_and(|t| !t.trim().is_empty()) => {
                return Routed {
                    outcome: Outcome::Answered(Box::new(answer)),
                    how,
                    candidates: admitted,
                    deciders,
                    failed,
                };
            }
            Ok(answer) => failed.push((
                candidate.address(),
                answer.error.unwrap_or_else(|| "an empty answer".into()),
            )),
            Err(why) => failed.push((candidate.address(), why)),
        }
    }
    Routed {
        outcome: Outcome::Unavailable("no pylon answered".into()),
        how,
        candidates: admitted,
        deciders,
        failed,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use nostr::pylon::{BEACON_V, Class, Service, Slots, beacon_event};

    const AT: u64 = 1_800_000_000;

    struct Spec {
        slug: String,
        tier: Tier,
        free: u32,
        status: Status,
        lanes: Vec<Lane>,
        settlement: &'static str,
        observed_at: u64,
    }

    fn spec(slug: &str, tier: Tier, free: u32) -> Spec {
        Spec {
            slug: slug.into(),
            tier,
            free,
            status: Status::Online,
            lanes: vec![Lane::CjConversation],
            settlement: FREE,
            observed_at: AT - 10,
        }
    }

    fn book(specs: Vec<Spec>) -> (BeaconBook, Vec<String>) {
        let mut book = BeaconBook::default();
        let mut keys = Vec::new();
        for spec in specs {
            let key = Identity::generate();
            let services = spec
                .lanes
                .iter()
                .map(|lane| Service {
                    capability: format!("{}:pylon/x", key.pubkey()),
                    model: if *lane == Lane::CjDecision {
                        "clef-flash".into()
                    } else {
                        format!("model-{}", spec.slug)
                    },
                    lanes: vec![*lane],
                    offering: None,
                    price_hint_msat: None,
                })
                .collect();
            let beacon = Beacon {
                v: BEACON_V.into(),
                requires: vec![],
                meta: None,
                provider: key.pubkey().into(),
                pylon: spec.slug.clone(),
                label: spec.slug.to_uppercase(),
                status: spec.status,
                generation: 1,
                since: spec.observed_at,
                observed_at: spec.observed_at,
                valid_until: spec.observed_at + 240,
                class: Class {
                    family: Family::Gpu,
                    tier: spec.tier,
                    memory_gb: 16,
                },
                slots: Slots {
                    total: 2,
                    free: spec.free,
                },
                services,
                settlement: vec![spec.settlement.into()],
                pools: vec![],
                group: None,
            };
            let event = beacon_event(key.signer(), &beacon).unwrap();
            book.offer(event, beacon);
            keys.push(key.pubkey().to_string());
        }
        (book, keys)
    }

    fn slugs(found: &[Candidate]) -> Vec<&str> {
        found.iter().map(|c| c.pylon.as_str()).collect()
    }

    #[test]
    fn only_live_free_conversation_pylons_are_candidates() {
        let mut offline = spec("offline", Tier::Xl, 2);
        offline.status = Status::Draining;
        let mut stale = spec("stale", Tier::Xl, 2);
        stale.observed_at = AT - 1_000;
        let mut paid = spec("paid", Tier::Xl, 2);
        paid.settlement = "x402";
        let mut decides = spec("decides", Tier::Xl, 2);
        decides.lanes = vec![Lane::CjDecision];
        let (book, keys) = book(vec![
            spec("ok", Tier::Medium, 1),
            spec("full", Tier::Xl, 0),
            offline,
            stale,
            paid,
            decides,
            spec("failing", Tier::Xl, 2),
        ]);
        let failing = BTreeSet::from([format!("30200:{}:failing", keys[6])]);
        assert_eq!(slugs(&candidates(&book, AT, &failing)), ["ok"]);
        assert_eq!(deciders(&book, AT, &failing), 1);
    }

    #[test]
    fn candidates_rank_by_tier_then_free_slots() {
        let (book, _) = book(vec![
            spec("small", Tier::Small, 2),
            spec("medium-1", Tier::Medium, 1),
            spec("large", Tier::Large, 1),
            spec("medium-2", Tier::Medium, 2),
        ]);
        assert_eq!(
            slugs(&candidates(&book, AT, &BTreeSet::new())),
            ["large", "medium-2", "medium-1", "small"]
        );
    }

    #[test]
    fn at_most_sixteen_candidates_are_offered() {
        let specs = (0..20)
            .map(|i| spec(&format!("p{i}"), Tier::Medium, 1))
            .collect();
        let (book, _) = book(specs);
        assert_eq!(
            candidates(&book, AT, &BTreeSet::new()).len(),
            MAX_CANDIDATES
        );
    }

    #[test]
    fn the_question_offers_each_pylon_and_default() {
        let (book, _) = book(vec![
            spec("rig", Tier::Large, 2),
            spec("cpu", Tier::Small, 1),
        ]);
        let found = candidates(&book, AT, &BTreeSet::new());
        let turns = vec![
            Turn {
                role: "user".into(),
                content: "Hi".into(),
            },
            Turn {
                role: "assistant".into(),
                content: "Hello".into(),
            },
            Turn {
                role: "user".into(),
                content: "What is a pylon?".into(),
            },
        ];
        let asked = question(&found, &turns);
        let values: Vec<&str> = asked.choices.iter().map(|(v, _)| v.as_str()).collect();
        assert_eq!(values, ["p1", "p2", DEFAULT]);
        assert_eq!(
            asked.choices[0].1,
            "RIG: runs model-rig on a large GPU (16 GB), 2 of 2 slots free"
        );
        assert!(
            asked
                .input
                .starts_with("The person's latest message:\nWhat is a pylon?")
        );
        assert!(asked.input.contains("Person: Hi\nAnswer: Hello"));
    }

    fn verdict(choice: &str, confidence: f64) -> Verdict {
        Verdict {
            choice: choice.into(),
            confidence,
            door: None,
            model: None,
        }
    }

    #[test]
    fn a_verdict_picks_a_pylon_or_a_confident_default() {
        assert_eq!(read(&verdict("p2", 0.4), 3), Some(Pick::Pylon(1)));
        assert_eq!(read(&verdict(DEFAULT, 0.9), 3), Some(Pick::Default));
        assert_eq!(read(&verdict(DEFAULT, 0.5), 3), Some(Pick::Pylon(0)));
        assert_eq!(read(&verdict("p4", 0.9), 3), None);
        assert_eq!(read(&verdict("p0", 0.9), 3), None);
        assert_eq!(read(&verdict("rig", 0.9), 3), None);
    }

    #[test]
    fn the_job_keeps_the_newest_turns_that_fit() {
        let mut turns: Vec<Turn> = (0..40)
            .map(|i| Turn {
                role: if i % 2 == 0 { "user" } else { "assistant" }.into(),
                content: format!("turn {i}"),
            })
            .collect();
        turns.push(Turn {
            role: "user".into(),
            content: "last".into(),
        });
        let (history, prompt) = job_text(&turns);
        assert_eq!(prompt, "last");
        assert_eq!(history.len(), crate::job::MAX_TURNS - 1);
        assert_eq!(history.last().unwrap().content, "turn 39");

        let long = vec![
            Turn {
                role: "user".into(),
                content: "x".repeat(JOB_TEXT_BYTES),
            },
            Turn {
                role: "assistant".into(),
                content: "y".repeat(JOB_TEXT_BYTES),
            },
            Turn {
                role: "user".into(),
                content: "é".repeat(JOB_TEXT_BYTES),
            },
        ];
        let (history, prompt) = job_text(&long);
        assert!(history.is_empty());
        assert_eq!(prompt, long[2].content);
    }
}
