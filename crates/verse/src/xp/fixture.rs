//! Throwaway NIP-XP fixtures: a quest, a knowledge entry, its evidence,
//! an award, and an achievement label, signed with keys derived from small
//! numbers. The tests use them, and `examples/xp_seed.rs` publishes them to
//! a local relay. Never use these keys for anything real.

use nostr::domain::{Event, RelaySigner};
use nostr::{kb, xp};
use serde_json::json;

/// A throwaway key: the secret is `n` as 64 hex digits.
///
/// # Panics
///
/// Never for `n` below the curve order, which every `u64` is.
#[must_use]
pub fn signer(n: u64) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{:064x}", n.max(1))).expect("a throwaway key")
}

fn sign(signer: &RelaySigner, at: u64, parts: kb::Unsigned) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

/// A quest spec: beat a reference run on `fix-git`, open from `at - 1000`
/// for 90 days.
#[must_use]
pub fn quest_spec(version: u64, title: &str, at: u64) -> serde_json::Value {
    json!({
        "id": "tb4.fix-git.beat-fable-low",
        "version": version,
        "season": {"id": "2026-q4", "opens_at": at - 1_000, "closes_at": at + 90 * 86_400},
        "title": title,
        "objective": "Publish an entry that makes a paired Microcoder run pass fix-git for less than Fable 5.1 low's cheapest winning run.",
        "acceptance": {"rule": "kb-transfer", "task": "fix-git", "min_pass_rate": 1.0, "max_usd_per_run": 0.21},
        "reference": {"label": "Fable 5.1 low, cheapest winning run", "usd": 0.21, "seconds": 312, "source": null},
        "award": {"author": 6, "runner": 4},
    })
}

/// A knowledge entry document written from a reference, not from a task.
#[must_use]
pub fn document() -> String {
    "---\nid: git.reflog-recovery\nversion: 1\nkind: method\ntitle: Recover commits from the reflog\nsummary: >-\n  Lost commits stay reachable from the reflog.\ntags: [git]\napplies_when: >-\n  A branch lost commits.\nstatus: candidate\nauthor: someone\nprovenance:\n  written_from: [reference]\n  cites: [\"Pro Git, 2014\"]\nevidence: []\n---\n\n## Details\n\nBody.\n".to_owned()
}

/// The evidence report a runner signs: paired runs on `fix-git` where the
/// entry helped.
#[must_use]
pub fn report(runner: &str, entry: &Event, text: &str) -> String {
    json!({
        "v": "openagents.eval-report.v1", "requires": [],
        "evaluator": runner,
        "subject": {"definition": {
            "id": kb::qualified_id(&entry.pubkey, "git.reflog-recovery"),
            "artifact": kb::document_artifact(text),
            "event": {"id": entry.id, "pubkey": entry.pubkey, "kind": kb::ENTRY_KIND},
        }},
        "verdict": "pass",
        "meta": {"kb": {"pairs": [{"task": "fix-git", "model": "m",
            "with": {"runs": 2, "passes": 2, "usd": 0.3, "unknown": 0},
            "without": {"runs": 2, "passes": 0, "usd": 0.6, "unknown": 0}, "side": "favors"}]}},
    })
    .to_string()
}

/// A quest, entry, and evidence that a referee can accept.
pub struct Completion {
    /// Signs the quest, the award, and labels.
    pub referee: RelaySigner,
    /// Signs the entry.
    pub author: RelaySigner,
    /// Signs the evidence.
    pub runner: RelaySigner,
    /// The `30193`.
    pub quest: Event,
    /// The `3190`.
    pub entry: Event,
    /// The `3189`.
    pub evidence: Event,
}

impl Completion {
    /// Signs the quest, the entry, and a runner's passing evidence at `at`,
    /// with the keys `referee`, `author`, and `runner`.
    ///
    /// # Panics
    ///
    /// Never: the fixtures are valid.
    #[must_use]
    pub fn new(referee: u64, author: u64, runner: u64, at: u64) -> Self {
        let (referee, author, runner) = (signer(referee), signer(author), signer(runner));
        let quest = sign(
            &referee,
            at,
            xp::quest(&quest_spec(
                1,
                "Beat Fable 5.1 low's cheapest winning run on fix-git",
                at,
            ))
            .expect("a valid quest"),
        );
        let text = document();
        let entry = sign(
            &author,
            at,
            kb::entry("git.reflog-recovery", 1, "method", &[], &text).expect("a valid entry"),
        );
        let evidence = sign(
            &runner,
            at,
            kb::evidence(
                &report(runner.pubkey(), &entry, &text),
                std::slice::from_ref(&entry.id),
            )
            .expect("valid evidence"),
        );
        Self {
            referee,
            author,
            runner,
            quest,
            entry,
            evidence,
        }
    }

    /// The referee's award for this completion, accepted at `at`.
    ///
    /// # Panics
    ///
    /// When `at` is outside the quest's season.
    #[must_use]
    pub fn award(&self, at: u64) -> Event {
        let excluded = knowledge::xp::excluded_tasks(&self.entry).expect("a valid entry");
        let parts = xp::award(&self.quest, &self.entry, &self.evidence, &excluded, at)
            .expect("the rule accepts the completion");
        self.referee.sign(at, parts.kind, parts.tags, parts.content)
    }

    /// An achievement label on `award`, signed by `by`.
    ///
    /// # Panics
    ///
    /// When `value` isn't a slug.
    #[must_use]
    pub fn label(by: &RelaySigner, award: &Event, value: &str, at: u64) -> Event {
        sign(
            by,
            at,
            xp::achievement(award, value).expect("a valid label"),
        )
    }
}
