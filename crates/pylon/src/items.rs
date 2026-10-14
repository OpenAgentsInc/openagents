//! Fresh decision items for checking a pylon's decision answers.
//!
//! Each item is one typed decision: a short state (a support message, a
//! code-review note, a calendar request, an invoice line, an agent's task
//! report) and exactly one `choice` (2 to 8 options), `noul`, or `score`
//! (3 to 5 levels) question that asks for judgement on it. There is no
//! answer key: a checker scores a pylon against its own run of the same
//! pinned model ([`crate::verify`]), so a logged item is worth nothing
//! later, and a seed drawn per check makes every round new.
//!
//! The generator is deterministic for a seed, so a calibration run and a
//! self-check reference set can be rebuilt from the seed alone. The idea
//! (seeded synthetic decisions with no answer key) comes from REAX's
//! subnet; the pools and shapes here are our own.

use serde_json::{Map, Value, json};

/// A small deterministic generator (SplitMix64). Not for secrets: a
/// checker draws the seed itself from the operating system.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// The generator for `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    /// One element of `pool`.
    pub fn pick<'a>(&mut self, pool: &[&'a str]) -> &'a str {
        pool[self.below(pool.len())]
    }

    /// `k` distinct elements of `pool`, in a random order.
    pub fn sample<'a>(&mut self, pool: &[&'a str], k: usize) -> Vec<&'a str> {
        let mut rest: Vec<&str> = pool.to_vec();
        let mut out = Vec::new();
        while out.len() < k && !rest.is_empty() {
            out.push(rest.remove(self.below(rest.len())));
        }
        out
    }
}

/// A seed from the operating system's random source, for a checker's
/// fresh round: a pylon cannot precompute it.
#[must_use]
pub fn fresh_seed() -> u64 {
    secp256k1::rand::random()
}

/// One decision item: the `POST /v1/systemone` body without the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// A stable ID for the item (seed and index).
    pub id: String,
    /// The state the question is about.
    pub state: Value,
    /// The question's ID inside `questions`.
    pub question_id: String,
    /// The one question.
    pub question: Value,
}

impl Item {
    /// The `questions` object with the one question.
    #[must_use]
    pub fn questions(&self) -> Map<String, Value> {
        let mut questions = Map::new();
        questions.insert(self.question_id.clone(), self.question.clone());
        questions
    }

    /// The request body for `model`.
    #[must_use]
    pub fn body(&self, model: &str) -> Value {
        json!({"model": model, "state": self.state, "questions": self.questions()})
    }
}

const NAMES: &[&str] = &[
    "Amara",
    "Bao",
    "Carmen",
    "Dov",
    "Esi",
    "Fynn",
    "Gita",
    "Hamid",
    "Ilse",
    "Joon",
    "Kalani",
    "Luca",
    "Marisol",
    "Nnamdi",
    "Odile",
    "Pavel",
    "Rhea",
    "Soren",
    "Tomasz",
    "Valentina",
    "Wiremu",
    "Yara",
    "Zeynep",
];

const PRODUCTS: &[&str] = &[
    "the desktop app",
    "the phone app",
    "the hosted API",
    "the CLI",
    "a team plan",
    "a Lightning wallet",
    "a GPU rental",
    "the plugin store",
    "the web chat",
    "a sandbox",
];

const ASKS: &[&str] = &[
    "was billed twice for {p}",
    "cannot sign in to {p} after updating",
    "wants a refund for {p}",
    "asks whether {p} works offline",
    "reports that {p} freezes when opening large files",
    "needs an invoice for {p} with a company address",
    "wants to move {p} to a colleague's account",
    "asks when {p} will support Linux on ARM",
    "says the payout from {p} has not arrived",
    "found a typo on the pricing page for {p}",
    "wants to delete all data held by {p}",
    "asks if {p} can be paid with Lightning",
    "says {p} uses too much battery",
    "is asking for an API key for {p}",
];

const TONES: &[&str] = &[
    "",
    "Second message this week. ",
    "Thanks so much! ",
    "This is unacceptable. ",
    "No hurry at all. ",
    "Need this fixed before our launch tomorrow. ",
    "Small thing, ",
    "Our legal team is copied. ",
    "Love the product, but ",
];

const CHANGES: &[&str] = &[
    "renames a public function used by three crates",
    "adds a retry loop with no upper bound",
    "removes a test that was flaky",
    "changes a SQL migration that already ran in production",
    "adds logging of request bodies",
    "bumps a dependency by a major version",
    "fixes a typo in a comment",
    "adds a new endpoint without authentication",
    "replaces a mutex with an atomic counter",
    "deletes an unused feature flag",
    "caches user sessions in memory",
    "adds a 30 second timeout to an HTTP client",
];

const DAYS: &[&str] = &[
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

const ITEMS: &[&str] = &[
    "GPU hours",
    "relay bandwidth",
    "storage",
    "support retainer",
    "domain renewal",
    "design work",
    "legal review",
    "travel",
    "office chairs",
    "conference tickets",
];

const REPORTS: &[&str] = &[
    "ran the tests; 2 of 40 failed on a timeout",
    "opened a pull request but did not run the linter",
    "could not reach the package registry and stopped",
    "finished the task and attached screenshots",
    "rewrote the whole module instead of the one function asked for",
    "asked for a secret it was not given",
    "pushed directly to main without review",
    "left a TODO where the error handling should be",
    "measured a 3% speedup and recorded the numbers",
    "skipped the migration because it looked risky",
];

const TEAMS: &[(&str, &str)] = &[
    ("billing", "Charges, refunds, invoices and payouts"),
    ("access", "Sign-in, accounts, keys and permissions"),
    ("bugs", "Crashes, freezes and wrong behaviour"),
    ("product", "Feature requests and roadmap questions"),
    ("privacy", "Data export, deletion and legal requests"),
    ("docs", "Typos and unclear documentation"),
    ("sales", "Plans, pricing and team purchases"),
    ("other", "Anything else"),
];

const VERDICTS: &[(&str, &str)] = &[
    ("approve", "Safe to merge as is"),
    ("comments", "Merge after small changes"),
    ("block", "Must not merge until fixed"),
    ("security", "Needs a security review first"),
    ("split", "Should be split into smaller changes"),
];

const NEXT: &[(&str, &str)] = &[
    ("accept", "Accept the work"),
    ("retry", "Ask the agent to try again"),
    ("human", "Hand it to a person"),
    ("revert", "Revert what it did"),
    ("investigate", "Look into it before deciding"),
    ("ignore", "Nothing to do"),
];

const URGENCY: &[&str] = &[
    "Not urgent",
    "Can wait a few days",
    "Should be handled today",
    "Needs attention within the hour",
    "Drop everything",
];

const RISK: &[&str] = &["No risk", "Low risk", "Some risk", "High risk"];

const MOOD: &[&str] = &["Very unhappy", "Unhappy", "Neutral", "Happy", "Delighted"];

fn fill(template: &str, product: &str) -> String {
    template.replace("{p}", product)
}

fn choice(rng: &mut Rng, instructions: &str, pool: &[(&str, &str)]) -> Value {
    let k = 2 + rng.below(pool.len().min(8) - 1);
    let keys: Vec<&str> = pool.iter().map(|(k, _)| *k).collect();
    let picked = rng.sample(&keys, k);
    let mut criteria = Map::new();
    for key in picked {
        let description = pool.iter().find(|(k, _)| *k == key).map_or("", |(_, d)| *d);
        criteria.insert(key.to_string(), json!(description));
    }
    json!({"type": "choice", "instructions": instructions, "criteria": criteria})
}

fn score(rng: &mut Rng, instructions: &str, levels: &[&str]) -> Value {
    let k = (3 + rng.below(3)).min(levels.len());
    let start = rng.below(levels.len() - k + 1);
    json!({"type": "score", "instructions": instructions, "criteria": levels[start..start + k]})
}

fn noul(instructions: &str) -> Value {
    json!({"type": "noul", "instructions": instructions})
}

/// Item `index` of the set for `seed`.
#[must_use]
pub fn item(seed: u64, index: u32) -> Item {
    let mut rng = Rng::new(seed ^ (u64::from(index).wrapping_mul(0xD6E8_FEB8_6659_FD93)));
    let id = format!("{seed:016x}-{index}");
    let shape = rng.below(5);
    let (state, question) = match shape {
        0 => {
            let who = rng.pick(NAMES);
            let product = rng.pick(PRODUCTS);
            let text = format!(
                "{}{who} {}.",
                rng.pick(TONES),
                fill(rng.pick(ASKS), product)
            );
            let question = match rng.below(3) {
                0 => choice(&mut rng, "Which team should take this message?", TEAMS),
                1 => score(&mut rng, "How urgent is this message?", URGENCY),
                _ => noul("Is the customer asking for money back?"),
            };
            (json!(text), question)
        }
        1 => {
            let change = rng.pick(CHANGES);
            let state = json!({
                "pull_request": format!("#{}", 100 + rng.below(9000)),
                "author": rng.pick(NAMES),
                "summary": format!("This change {change}."),
                "tests": if rng.below(2) == 0 { "passing" } else { "failing" },
                "lines_changed": 1 + rng.below(800),
            });
            let question = match rng.below(3) {
                0 => choice(&mut rng, "What should the reviewer do?", VERDICTS),
                1 => score(&mut rng, "How risky is this change?", RISK),
                _ => noul("Could this change break something for users?"),
            };
            (state, question)
        }
        2 => {
            let day = rng.pick(DAYS);
            let hour = 7 + rng.below(13);
            let state = json!({
                "request": format!(
                    "{} wants a {}-minute call on {day} at {hour}:00.",
                    rng.pick(NAMES),
                    [15, 30, 45, 60][rng.below(4)]
                ),
                "calendar": format!(
                    "{day}: busy {}:00-{}:00, focus block {}:00-{}:00",
                    hour.saturating_sub(1),
                    hour + 1,
                    hour + 2,
                    hour + 4
                ),
                "timezone_gap_hours": rng.below(10),
            });
            let question = match rng.below(2) {
                0 => noul("Should the call be accepted at the requested time?"),
                _ => score(&mut rng, "How hard is it to fit this call in?", RISK),
            };
            (state, question)
        }
        3 => {
            let amount = 5 + rng.below(4000);
            let state = json!({
                "line": rng.pick(ITEMS),
                "amount_usd": amount,
                "approved_budget_usd": 100 * (1 + rng.below(30)),
                "requested_by": rng.pick(NAMES),
                "note": rng.pick(TONES).trim(),
            });
            let question = match rng.below(2) {
                0 => noul("Should this expense be approved without asking anyone?"),
                _ => score(&mut rng, "How unusual is this expense?", RISK),
            };
            (state, question)
        }
        _ => {
            let state = json!({
                "agent": format!("coder-{}", rng.below(99)),
                "task": format!("Fix the bug where {}", fill(rng.pick(ASKS), rng.pick(PRODUCTS))),
                "report": format!("The agent {}.", rng.pick(REPORTS)),
            });
            let question = match rng.below(3) {
                0 => choice(&mut rng, "What should happen next?", NEXT),
                1 => score(
                    &mut rng,
                    "How would the customer feel about this work?",
                    MOOD,
                ),
                _ => noul("Did the agent do what it was asked?"),
            };
            (state, question)
        }
    };
    Item {
        id,
        state,
        question_id: "q".into(),
        question,
    }
}

/// The first `count` items for `seed`.
#[must_use]
pub fn items(seed: u64, count: u32) -> Vec<Item> {
    (0..count).map(|i| item(seed, i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_gives_the_same_items_and_another_seed_others() {
        assert_eq!(items(7, 20), items(7, 20));
        let a = items(7, 20);
        let b = items(8, 20);
        assert!(a.iter().zip(&b).filter(|(x, y)| x.state == y.state).count() < 5);
    }

    #[test]
    fn every_item_is_a_valid_typed_decision() {
        for item in items(42, 500) {
            let q = &item.question;
            match q["type"].as_str() {
                Some("choice") => {
                    let n = q["criteria"].as_object().map_or(0, Map::len);
                    assert!((2..=8).contains(&n), "{n} options");
                }
                Some("score") => {
                    let n = q["criteria"].as_array().map_or(0, Vec::len);
                    assert!((3..=5).contains(&n), "{n} levels");
                }
                Some("noul") => {}
                other => panic!("type {other:?}"),
            }
            nostr::decision::check_body("clef-flash", &item.state, &json!(item.questions()))
                .unwrap_or_else(|e| panic!("{}: {e:?}", item.id));
        }
    }

    #[test]
    fn all_three_question_types_appear() {
        let kinds: std::collections::BTreeSet<String> = items(1, 200)
            .iter()
            .map(|i| i.question["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn fresh_seeds_differ() {
        assert_ne!(fresh_seed(), fresh_seed());
    }
}
