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
        let excluded = xp_ledger::excluded_tasks(&self.entry).expect("a valid entry");
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

/// A Microcoder `summary.json` for `task` with `reward`; `nonce` makes two
/// runs' files differ.
#[must_use]
pub fn run_summary(task: &str, reward: f64, nonce: u64) -> Vec<u8> {
    json!({
        "task": task, "model": "gpt-6-luna", "effort": "medium",
        "outcome": {"ending": {"reason": "finished"}, "steps": 8, "seconds": 80 + nonce, "usd": 0.006},
        "reward": reward, "image": format!("example/{task}:1"), "kb": "off",
        "container": format!("microcoder-{task}-{nonce}"),
    })
    .to_string()
    .into_bytes()
}

/// A `reproduce` quest on `task`, the claim it pins, and a passing
/// reproduction by another key: the tutorial quest shape.
pub struct Reproduction {
    /// Signs the quest, the award, and labels.
    pub referee: RelaySigner,
    /// The `30193`.
    pub quest: Event,
    /// The claimant's run evidence.
    pub claim: Event,
    /// The reproducer's run evidence.
    pub reproduction: Event,
}

impl Reproduction {
    /// Signs a claim by `claimant`, a quest by `referee` that pins it and
    /// awards the reproducer `xp`, and `reproducer`'s passing
    /// reproduction, all at `at`.
    ///
    /// # Panics
    ///
    /// Never for a task without whitespace and `xp` from 1 to 1,000.
    #[must_use]
    pub fn new(
        referee: &RelaySigner,
        claimant: &RelaySigner,
        reproducer: &RelaySigner,
        task: &str,
        xp_award: u64,
        at: u64,
    ) -> Self {
        Self::build(referee, claimant, reproducer, task, xp_award, at, None)
    }

    /// As [`Reproduction::new`], with version 2 of the quest paying each
    /// distinct reproducer once, up to `max_awards` (`per-awardee`).
    ///
    /// # Panics
    ///
    /// Never for a task without whitespace, `xp` from 1 to 1,000, and
    /// `max_awards` from 1 to 10,000.
    #[must_use]
    pub fn per_awardee(
        referee: &RelaySigner,
        claimant: &RelaySigner,
        reproducer: &RelaySigner,
        task: &str,
        xp_award: u64,
        at: u64,
        max_awards: u64,
    ) -> Self {
        Self::build(
            referee,
            claimant,
            reproducer,
            task,
            xp_award,
            at,
            Some(max_awards),
        )
    }

    fn build(
        referee: &RelaySigner,
        claimant: &RelaySigner,
        reproducer: &RelaySigner,
        task: &str,
        xp_award: u64,
        at: u64,
        max_awards: Option<u64>,
    ) -> Self {
        let recipe = xp::recipe_from_summary(&run_summary(task, 1.0, 0), "terminal-bench", "2.1")
            .expect("a valid recipe");
        let claim = sign(
            claimant,
            at,
            xp::run_evidence(
                claimant.pubkey(),
                claimant.pubkey(),
                &recipe,
                &xp::record_from_summary(&run_summary(task, 1.0, 0)).expect("a record"),
                &[],
            )
            .expect("a valid claim"),
        );
        let mut spec = json!({
                "id": format!("tb21.{task}.reproduce"), "version": 1,
                "season": {"id": "tb21-tutorial-s1", "opens_at": at - 1_000, "closes_at": at + 90 * 86_400},
                "title": format!("Reproduce Microcoder's pass on {task}"),
                "objective": format!("Rerun the published pass on {task} from its recipe, and pass the task's tests."),
                "acceptance": {"rule": "reproduce", "task": task,
                    "recipe": xp::recipe_digest(&recipe).expect("a digest"),
                    "claim": {"id": claim.id, "pubkey": claim.pubkey, "kind": kb::EVIDENCE_KIND}},
                "reference": null,
                "award": {"claimant": 0, "reproducer": xp_award},
        });
        if let Some(max_awards) = max_awards {
            spec["version"] = json!(2);
            spec["completions"] = json!(xp::PER_AWARDEE);
            spec["max_awards"] = json!(max_awards);
        }
        let quest = sign(referee, at, xp::quest(&spec).expect("a valid quest"));
        let reproduction = sign(
            reproducer,
            at,
            xp::run_evidence(
                reproducer.pubkey(),
                claimant.pubkey(),
                &recipe,
                &xp::record_from_summary(&run_summary(task, 1.0, 1)).expect("a record"),
                std::slice::from_ref(&claim.id),
            )
            .expect("a valid reproduction"),
        );
        Self {
            referee: referee.clone(),
            quest,
            claim,
            reproduction,
        }
    }

    /// The referee's award for the reproduction, accepted at `at`.
    ///
    /// # Panics
    ///
    /// When `at` is outside the quest's season.
    #[must_use]
    pub fn award(&self, at: u64) -> Event {
        sign(
            &self.referee,
            at,
            xp::reproduce_award(&self.quest, &self.claim, &self.reproduction, at)
                .expect("the rule accepts the reproduction"),
        )
    }

    /// Every event a reader needs: the quest, the claim, the reproduction,
    /// and the award accepted at `at`.
    #[must_use]
    pub fn events(&self, at: u64) -> Vec<Event> {
        vec![
            self.quest.clone(),
            self.claim.clone(),
            self.reproduction.clone(),
            self.award(at),
        ]
    }
}

/// The tasks [`tutorial_events`] reproduces, in order: the six published
/// tutorial quests' tasks (`docs/verse/tutorial-quests.md`).
pub const TUTORIAL_TASKS: &[&str] = &[
    "prove-plus-comm",
    "fix-git",
    "openssl-selfsigned-cert",
    "regex-log",
    "sqlite-db-truncate",
    "build-pmars",
];

/// Signed events in which `reproducer` completed `count` tutorial
/// reproductions of 50 XP each, refereed by `referee`, and published a
/// trainer profile that asks to be shown: a labeled fixture for captures
/// and tests. The claims are signed by a throwaway key.
#[must_use]
pub fn tutorial_events(
    referee: &RelaySigner,
    reproducer: &RelaySigner,
    count: usize,
    at: u64,
) -> Vec<Event> {
    let claimant = signer(0x7c_1a_1b);
    let mut events: Vec<Event> = TUTORIAL_TASKS
        .iter()
        .take(count)
        .flat_map(|task| Reproduction::new(referee, &claimant, reproducer, task, 50, at).events(at))
        .collect();
    events.push(sign(
        reproducer,
        at,
        xp::profile(reproducer.pubkey(), true, &[]).expect("a valid profile"),
    ));
    events
}

/// Signed events in which `reproducer` completed version 2 of the
/// `prove-plus-comm` tutorial, which pays each distinct reproducer 50 XP
/// once, up to `max_awards`: a labeled fixture for tests.
#[must_use]
pub fn per_awardee_events(
    referee: &RelaySigner,
    reproducer: &RelaySigner,
    max_awards: u64,
    at: u64,
) -> Vec<Event> {
    let claimant = signer(0x7c_1a_1b);
    Reproduction::per_awardee(
        referee,
        &claimant,
        reproducer,
        TUTORIAL_TASKS[0],
        50,
        at,
        max_awards,
    )
    .events(at)
}

/// Signed events in which `tester` has accepted playtest contributions
/// from `referee`, a labeled fixture for captures, tests, and the app's
/// preview card: one reproducible bug (20 XP), one verified fix (10 XP),
/// and a moderated session (25 XP), with the titles `playtester`,
/// `founding-playtester`, `bug-hunter`, `fix-verifier`, and `raider` on
/// them. The triager and moderator are throwaway keys. Trust `referee`
/// alone to read it; it never counts toward a trainer level.
///
/// # Panics
///
/// Never: the fixtures are valid.
#[must_use]
pub fn playtest_events(referee: &RelaySigner, tester: &RelaySigner, at: u64) -> Vec<Event> {
    let triager = signer(0x7e_1a_9e);
    let moderator = signer(0x30_de_7a);
    let season =
        json!({"id": "playtest-s1", "opens_at": at - 1_000, "closes_at": at + 28 * 86_400});
    let builds = json!(["1.0.0 (15)", "1.0.0 (16)"]);
    let quest = |id: &str, title: &str, acceptance: serde_json::Value, xp: u64| {
        let mut acceptance = acceptance;
        acceptance["rule"] = json!("playtest");
        acceptance["builds"] = builds.clone();
        acceptance["max_awards"] = json!(500);
        sign(
            referee,
            at,
            xp::quest(&json!({
                "id": id, "version": 1, "season": season, "title": title,
                "objective": "An accepted playtest contribution in season playtest-s1.",
                "acceptance": acceptance, "reference": null,
                "award": {"tester": xp, "triager": 0},
            }))
            .expect("a valid playtest quest"),
        )
    };
    let bug = quest(
        "playtest-s1.bug-minor",
        "Reproducible bug report, P2 or P3",
        json!({"contribution": "bug", "severities": ["p2", "p3"]}),
        20,
    );
    let fix = quest(
        "playtest-s1.verified-fix",
        "Verified fix",
        json!({"contribution": "verified-fix"}),
        10,
    );
    let raid = quest(
        "playtest-s1.raid",
        "Group session: the Grid raid",
        json!({"contribution": "session", "script": "raid", "format": "group"}),
        25,
    );
    let digest = "ab".repeat(32);
    let report = |kind: &str, script: Option<&str>| {
        sign(
            tester,
            at,
            xp::playtest_report("1.0.0 (15)", "ios", kind, &digest, script)
                .expect("a valid report"),
        )
    };
    let (found, verified, played) = (
        report("bug", None),
        report("verified", None),
        report("session", Some("raid")),
    );
    let record = sign(
        &moderator,
        at,
        xp::playtest_session("raid", "group", "1.0.0 (15)", tester.pubkey(), at)
            .expect("a valid session record"),
    );
    let issue = Some("OpenAgentsInc/openagents#9901".to_owned());
    let award = |quest: &Event,
                 report: &Event,
                 session: Option<&Event>,
                 by: &RelaySigner,
                 fields: xp::PlaytestAward| {
        let parts = xp::playtest_award(quest, report, session, by.pubkey(), &fields, at)
            .expect("the rule accepts the contribution");
        sign(referee, at, parts)
    };
    let bug_award = award(
        &bug,
        &found,
        None,
        &triager,
        xp::PlaytestAward {
            issue: issue.clone(),
            severity: Some("p2".into()),
            commit: None,
        },
    );
    let fix_award = award(
        &fix,
        &verified,
        None,
        &triager,
        xp::PlaytestAward {
            issue,
            ..xp::PlaytestAward::default()
        },
    );
    let raid_award = award(
        &raid,
        &played,
        Some(&record),
        &moderator,
        xp::PlaytestAward::default(),
    );
    let labels = [
        (&bug_award, "playtester"),
        (&bug_award, "founding-playtester"),
        (&bug_award, "bug-hunter"),
        (&fix_award, "fix-verifier"),
        (&raid_award, "raider"),
    ]
    .map(|(award, value)| Completion::label(referee, award, value, at));
    let mut events = vec![
        bug, fix, raid, found, verified, played, record, bug_award, fix_award, raid_award,
    ];
    events.extend(labels);
    events
}
