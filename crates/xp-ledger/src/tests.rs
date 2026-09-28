//! Ledger derivation over signed fixtures: throwaway keys derived from a
//! label, no relay.

use nostr::domain::RelaySigner;
use nostr::kb::Unsigned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::*;

const AT: u64 = 1_790_000_000;

fn signer(label: &str) -> RelaySigner {
    let hex: String = Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn sign(signer: &RelaySigner, parts: Unsigned) -> Event {
    signer.sign(AT, parts.kind, parts.tags, parts.content)
}

fn document(written_from: &str) -> String {
    format!(
        "---\nid: git.reflog-recovery\nversion: 1\nkind: method\ntitle: Recover commits from the reflog\nsummary: >-\n  Lost commits stay reachable from the reflog.\ntags: [git]\napplies_when: >-\n  A branch lost commits.\nstatus: candidate\nauthor: someone\nprovenance:\n  written_from: [{written_from}]\n  cites: [\"Pro Git, 2014\"]\nevidence: []\n---\n\n## Details\n\nBody.\n"
    )
}

fn quest_spec(version: u64, title: &str) -> Value {
    json!({
        "id": "tb4.fix-git.beat-reference",
        "version": version,
        "season": {"id": "2026-q4", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
        "title": title,
        "objective": "Make a paired run pass fix-git for less than the reference.",
        "acceptance": {"rule": "kb-transfer", "task": "fix-git", "min_pass_rate": 1.0, "max_usd_per_run": 0.2},
        "reference": null,
        "award": {"author": 6, "runner": 4},
    })
}

fn evidence(runner: &RelaySigner, entry: &Event, text: &str) -> Event {
    let report = json!({
        "v": "openagents.eval-report.v1", "requires": [],
        "evaluator": runner.pubkey(),
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
    .to_string();
    sign(
        runner,
        kb::evidence(&report, std::slice::from_ref(&entry.id)).unwrap(),
    )
}

struct Fixture {
    referee: RelaySigner,
    author: RelaySigner,
    runner: RelaySigner,
    quest: Event,
    entry: Event,
    evidence: Event,
}

impl Fixture {
    fn new(written_from: &str) -> Self {
        let referee = signer("referee");
        let author = signer("author");
        let runner = signer("runner");
        let quest = sign(
            &referee,
            xp::quest(&quest_spec(1, "Beat the reference")).unwrap(),
        );
        let text = document(written_from);
        let entry = sign(
            &author,
            kb::entry("git.reflog-recovery", 1, "method", &[], &text).unwrap(),
        );
        let evidence = evidence(&runner, &entry, &text);
        Fixture {
            referee,
            author,
            runner,
            quest,
            entry,
            evidence,
        }
    }

    fn award(&self, at: u64) -> Event {
        let excluded = excluded_tasks(&self.entry).unwrap();
        let parts = xp::award(&self.quest, &self.entry, &self.evidence, &excluded, at).unwrap();
        self.referee.sign(at, parts.kind, parts.tags, parts.content)
    }

    fn events(&self, more: &[Event]) -> Vec<Event> {
        let mut all = vec![
            self.quest.clone(),
            self.entry.clone(),
            self.evidence.clone(),
        ];
        all.extend_from_slice(more);
        all
    }

    fn trust(&self) -> XpTrust {
        XpTrust {
            referees: BTreeSet::from([self.referee.pubkey().to_string()]),
            runners: BTreeSet::new(),
        }
    }
}

#[test]
fn an_accepted_award_credits_its_author_and_runner_once() {
    let f = Fixture::new("reference");
    let award = f.award(AT);
    // The same award from two relays counts once.
    let ledger = derive(&f.events(&[award.clone(), award.clone()]), &f.trust());
    assert_eq!(ledger.totals.get(f.author.pubkey()), Some(&6));
    assert_eq!(ledger.totals.get(f.runner.pubkey()), Some(&4));
    assert_eq!(ledger.credits.len(), 2);
    assert!(ledger.refused.is_empty() && ledger.conflicts.is_empty());
    assert_eq!(
        ledger
            .quests
            .get("tb4.fix-git.beat-reference@1")
            .map(String::as_str),
        Some("Beat the reference")
    );
}

#[test]
fn a_reader_counts_only_referees_it_trusts() {
    let f = Fixture::new("reference");
    let ledger = derive(&f.events(&[f.award(AT)]), &XpTrust::default());
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.untrusted, 1);
}

#[test]
fn a_revoked_award_stops_counting_and_a_replacement_counts() {
    let f = Fixture::new("reference");
    let first = f.award(AT);
    let revocation = sign(
        &f.referee,
        xp::revocation(&first, "mislabeled runs").unwrap(),
    );
    let ledger = derive(&f.events(&[first.clone(), revocation.clone()]), &f.trust());
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.revoked, vec![first.id.clone()]);
    let second = f.award(AT + 1);
    let ledger = derive(&f.events(&[first, revocation, second]), &f.trust());
    assert_eq!(ledger.totals.values().sum::<u64>(), 10);
}

#[test]
fn a_second_live_award_for_one_key_counts_for_no_one() {
    let f = Fixture::new("reference");
    let ledger = derive(&f.events(&[f.award(AT), f.award(AT + 1)]), &f.trust());
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.conflicts.len(), 1);
    assert!(ledger.conflicts[0].contains("2 live awards"));
}

#[test]
fn a_rewritten_quest_version_counts_for_no_one() {
    let f = Fixture::new("reference");
    let award = f.award(AT);
    let rewrite = sign(
        &f.referee,
        xp::quest(&quest_spec(1, "Beat the reference, reworded")).unwrap(),
    );
    let ledger = derive(&f.events(&[award, rewrite]), &f.trust());
    assert!(ledger.totals.is_empty());
    assert!(
        ledger
            .conflicts
            .iter()
            .any(|c| c.contains("published twice"))
    );
    assert_eq!(ledger.refused.len(), 1);
}

#[test]
fn awards_the_reader_cannot_verify_are_refused() {
    let f = Fixture::new("reference");
    let award = f.award(AT);

    // The evidence isn't available.
    let events = vec![f.quest.clone(), f.entry.clone(), award.clone()];
    let ledger = derive(&events, &f.trust());
    assert!(ledger.totals.is_empty());
    assert!(ledger.refused[0].contains("evidence"));

    // The runner isn't on the reader's runner list.
    let mut trust = f.trust();
    trust
        .runners
        .insert(signer("someone else").pubkey().to_string());
    let ledger = derive(&f.events(&[award]), &trust);
    assert!(ledger.totals.is_empty());
    assert!(ledger.refused[0].contains("runner"));

    // A referee that signs an award for an entry written from the quest's
    // task: the reader re-checks the document and refuses it.
    let g = Fixture::new("fix-git-1790000000");
    assert_eq!(
        excluded_tasks(&g.entry).unwrap(),
        vec!["fix-git".to_string()]
    );
    let parts = xp::award(&g.quest, &g.entry, &g.evidence, &[], AT).unwrap();
    let forced = sign(&g.referee, parts);
    let ledger = derive(&g.events(&[forced]), &g.trust());
    assert!(ledger.totals.is_empty());
    assert!(ledger.refused[0].contains("out of sample"));
}

#[test]
fn the_trust_file_reads_npubs_and_hex() {
    let dir = std::env::temp_dir().join(format!("knowledge-xp-trust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("xp-trust.json");
    let referee = signer("referee");
    let npub = crate::npub(referee.pubkey());
    std::fs::write(
        &path,
        json!({"referees": [npub], "runners": [signer("runner").pubkey()]}).to_string(),
    )
    .unwrap();
    let trust = XpTrust::read(&path).unwrap();
    assert!(trust.referees.contains(referee.pubkey()));
    assert_eq!(trust.runners.len(), 1);
    assert_eq!(
        XpTrust::read(&dir.join("missing")).unwrap(),
        XpTrust::default()
    );
    std::fs::write(&path, r#"{"referees": ["nope"]}"#).unwrap();
    assert!(XpTrust::read(&path).is_err());
}

/// A `reproduce` quest, its claim, and a reproduction by another key.
struct Reproduction {
    referee: RelaySigner,
    claimant: RelaySigner,
    reproducer: RelaySigner,
    quest: Event,
    claim: Event,
    reproduction: Event,
}

fn run_summary(nonce: u64) -> Vec<u8> {
    json!({
        "task": "build-pmars", "model": "gpt-6-luna", "effort": "medium",
        "outcome": {"ending": {"reason": "finished"}, "steps": 8, "seconds": 80 + nonce, "usd": 0.006},
        "reward": 1.0, "image": "alexgshaw/build-pmars:20251031", "kb": "off",
        "container": format!("c-{nonce}"),
    })
    .to_string()
    .into_bytes()
}

impl Reproduction {
    fn new() -> Self {
        let referee = signer("referee");
        let claimant = signer("claimant");
        let reproducer = signer("reproducer");
        let recipe = xp::recipe_from_summary(&run_summary(0), "terminal-bench", "2.1").unwrap();
        let claim_record = xp::record_from_summary(&run_summary(0)).unwrap();
        let claim = sign(
            &claimant,
            xp::run_evidence(
                claimant.pubkey(),
                claimant.pubkey(),
                &recipe,
                &claim_record,
                &[],
            )
            .unwrap(),
        );
        let quest = sign(
            &referee,
            xp::quest(&json!({
                "id": "tb21.build-pmars.reproduce", "version": 1,
                "season": {"id": "tutorial", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
                "title": "Reproduce the pass on build-pmars",
                "objective": "Rerun the published pass from its recipe.",
                "acceptance": {"rule": "reproduce", "task": "build-pmars",
                    "recipe": xp::recipe_digest(&recipe).unwrap(),
                    "claim": {"id": claim.id, "pubkey": claim.pubkey, "kind": kb::EVIDENCE_KIND}},
                "reference": null,
                "award": {"claimant": 0, "reproducer": 50},
            }))
            .unwrap(),
        );
        let record = xp::record_from_summary(&run_summary(1)).unwrap();
        let reproduction = sign(
            &reproducer,
            xp::run_evidence(
                reproducer.pubkey(),
                claimant.pubkey(),
                &recipe,
                &record,
                std::slice::from_ref(&claim.id),
            )
            .unwrap(),
        );
        Reproduction {
            referee,
            claimant,
            reproducer,
            quest,
            claim,
            reproduction,
        }
    }

    fn award(&self) -> Event {
        sign(
            &self.referee,
            xp::reproduce_award(&self.quest, &self.claim, &self.reproduction, AT).unwrap(),
        )
    }

    fn trust(&self) -> XpTrust {
        XpTrust {
            referees: BTreeSet::from([self.referee.pubkey().to_string()]),
            runners: BTreeSet::new(),
        }
    }
}

#[test]
fn a_reproduction_credits_the_reproducer_and_not_a_zero_share() {
    let r = Reproduction::new();
    let award = r.award();
    let events = vec![
        r.quest.clone(),
        r.claim.clone(),
        r.reproduction.clone(),
        award.clone(),
        award,
    ];
    let ledger = derive(&events, &r.trust());
    assert!(ledger.refused.is_empty(), "{:?}", ledger.refused);
    assert_eq!(ledger.totals.get(r.reproducer.pubkey()), Some(&50));
    assert_eq!(ledger.totals.get(r.claimant.pubkey()), None);
    assert_eq!(ledger.credits.len(), 1);
    assert_eq!(ledger.credits[0].rule, "reproduce");
    assert_eq!(ledger.credits[0].role, "reproducer");

    // Listing runners lists reproducers too.
    let mut trust = r.trust();
    trust
        .runners
        .insert(signer("someone else").pubkey().to_string());
    let ledger = derive(&events, &trust);
    assert!(ledger.totals.is_empty());
    assert!(ledger.refused[0].contains("runner"));
    trust.runners.insert(r.reproducer.pubkey().to_string());
    assert_eq!(
        derive(&events, &trust).totals.get(r.reproducer.pubkey()),
        Some(&50)
    );
}

#[test]
fn a_reproduce_award_without_its_reproduction_is_refused() {
    let r = Reproduction::new();
    let events = vec![r.quest.clone(), r.claim.clone(), r.award()];
    let ledger = derive(&events, &r.trust());
    assert!(ledger.totals.is_empty());
    assert!(ledger.refused[0].contains("reproduction"));
}

fn playtest_quest(referee: &RelaySigner, contribution: &str, max_awards: u64) -> Event {
    let mut acceptance = json!({
        "rule": "playtest", "contribution": contribution,
        "builds": ["1.0.0 (15)"], "max_awards": max_awards,
    });
    if contribution == "bug" {
        acceptance["severities"] = json!(["p2", "p3"]);
    }
    sign(
        referee,
        xp::quest(&json!({
            "id": format!("playtest-s1.{contribution}"),
            "version": 1,
            "season": {"id": "playtest-s1", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
            "title": format!("Playtest {contribution}"),
            "objective": "An accepted playtest contribution.",
            "acceptance": acceptance,
            "reference": null,
            "award": {"tester": 20, "triager": 0},
        }))
        .unwrap(),
    )
}

fn playtest_report(tester: &RelaySigner, kind: &str) -> Event {
    sign(
        tester,
        xp::playtest_report("1.0.0 (15)", "android", kind, &"ab".repeat(32), None).unwrap(),
    )
}

fn playtest_award(
    referee: &RelaySigner,
    quest: &Event,
    report: &Event,
    issue: &str,
    severity: Option<&str>,
) -> Event {
    let fields = xp::PlaytestAward {
        issue: Some(issue.into()),
        severity: severity.map(str::to_owned),
        commit: None,
    };
    sign(
        referee,
        xp::playtest_award(quest, report, None, signer("triager").pubkey(), &fields, AT).unwrap(),
    )
}

#[test]
fn an_accepted_playtest_contribution_credits_the_tester_only() {
    let referee = signer("playtest-referee");
    let tester = signer("tester");
    let quest = playtest_quest(&referee, "bug", 10);
    let report = playtest_report(&tester, "bug");
    let award = playtest_award(
        &referee,
        &quest,
        &report,
        "OpenAgentsInc/openagents#1",
        Some("p2"),
    );
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        // A runner list doesn't apply to playtest awards.
        runners: BTreeSet::from([signer("runner").pubkey().to_owned()]),
    };
    let ledger = derive(&[quest.clone(), report.clone(), award.clone()], &trust);
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.totals.get(tester.pubkey()), Some(&20));
    assert_eq!(ledger.credits.len(), 1);
    assert_eq!(ledger.credits[0].rule, "playtest");
    assert_eq!(ledger.credits[0].role, "tester");
    // Without the tester's report, the award isn't counted.
    let ledger = derive(&[quest, award], &trust);
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.refused.len(), 1);
}

#[test]
fn one_issue_earns_one_report_class_award_across_quests() {
    let referee = signer("playtest-referee");
    let tester = signer("tester");
    let bug = playtest_quest(&referee, "bug", 10);
    let feedback = playtest_quest(&referee, "feedback", 10);
    let report = playtest_report(&tester, "bug");
    let issue = "OpenAgentsInc/openagents#2";
    let first = playtest_award(&referee, &bug, &report, issue, Some("p3"));
    let second = playtest_award(&referee, &feedback, &report, issue, None);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let ledger = derive(&[bug, feedback, report, first, second], &trust);
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.conflicts.len(), 1);
    assert!(ledger.conflicts[0].starts_with("playtest:playtest-s1:report:"));
}

#[test]
fn a_quest_version_over_its_max_awards_counts_none() {
    let referee = signer("playtest-referee");
    let quest = playtest_quest(&referee, "feedback", 1);
    let (a, b) = (signer("tester-a"), signer("tester-b"));
    let (ra, rb) = (
        playtest_report(&a, "idea"),
        playtest_report(&b, "confusing"),
    );
    let award_a = playtest_award(&referee, &quest, &ra, "OpenAgentsInc/openagents#3", None);
    let award_b = playtest_award(&referee, &quest, &rb, "OpenAgentsInc/openagents#4", None);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let ledger = derive(
        &[
            quest.clone(),
            ra.clone(),
            rb,
            award_a.clone(),
            award_b.clone(),
        ],
        &trust,
    );
    assert!(ledger.totals.is_empty());
    assert!(ledger.conflicts[0].contains("max_awards of 1"));
    // Revoking the extra one brings the quest within its limit.
    let revoked = sign(
        &referee,
        xp::revocation(&award_b, "Duplicate of #3.").unwrap(),
    );
    let ledger = derive(&[quest, ra, award_a, award_b, revoked], &trust);
    assert_eq!(ledger.totals.get(a.pubkey()), Some(&20));
    assert_eq!(ledger.revoked.len(), 1);
}
