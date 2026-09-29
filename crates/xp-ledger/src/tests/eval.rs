//! `eval-check` and `eval-adopt` in the ledger: awards re-checked against
//! the signed results and checks they name, replayed, revoked, unique per
//! role and suite version, role collapse, untrusted referees, disputes,
//! self-checks and linked keys, the adoption queue, the adoption's
//! documents, and "what you made". Keys are throwaway, from labels.

use super::*;
use crate::adopt;
use crate::eval::fixture::{Run, published, release};
use crate::eval::{self, Standing};

const SEASON_OPENS: u64 = AT - 10_000;
const SEASON_CLOSES: u64 = AT + 100_000;

fn check_quest(referee: &RelaySigner, suite: &Event, subject: &Event, max_awards: u64) -> Event {
    check_quest_version(referee, suite, subject, max_awards, 1)
}

fn check_quest_version(
    referee: &RelaySigner,
    suite: &Event,
    subject: &Event,
    max_awards: u64,
    version: u64,
) -> Event {
    let spec = json!({
        "id": "ext-eval.project-map.check",
        "version": version,
        "season": {"id": "ext-eval-s1", "opens_at": SEASON_OPENS, "closes_at": SEASON_CLOSES},
        "title": "Check a Project map result",
        "objective": "Rerun the Project map test set on the same tool release and confirm a published result.",
        "acceptance": {
            "rule": "eval-check",
            "suite": {"id": suite.id, "pubkey": suite.pubkey, "kind": suite.kind},
            "subject": {"id": subject.id, "pubkey": subject.pubkey, "kind": subject.kind},
            "max_awards": max_awards,
        },
        "reference": null,
        "award": {"checker": 50, "evaluator": 25, "suite-author": 25},
    });
    let parts = xp::quest(&spec).unwrap();
    referee.sign(SEASON_OPENS, parts.kind, parts.tags, parts.content)
}

fn signed(referee: &RelaySigner, parts: Vec<nostr::kb::Unsigned>, at: u64) -> Vec<Event> {
    parts
        .into_iter()
        .map(|p| referee.sign(at, p.kind, p.tags, p.content))
        .collect()
}

struct World {
    referee: RelaySigner,
    suite: Event,
    subject: Event,
    quest: Event,
    result: Event,
    check: Event,
}

impl World {
    /// Alice's Better result on the suite-author's suite, and Bob's check
    /// confirming it.
    fn new() -> Self {
        Self::by("alice", "suite-author")
    }

    fn by(evaluator: &str, suite_author: &str) -> Self {
        let referee = signer("referee");
        let suite = release(&signer(suite_author), "project-map-tests", AT - 5_000);
        let subject = release(&signer("ext-author"), "project-map", AT - 5_000);
        let quest = check_quest(&referee, &suite, &subject, 500);
        let result = published(&signer(evaluator), &Run::better(&suite, &subject), None, AT);
        let check = published(
            &signer("bob"),
            &Run::better(&suite, &subject),
            Some(&result.id),
            AT + 10,
        );
        World {
            referee,
            suite,
            subject,
            quest,
            result,
            check,
        }
    }

    fn run(&self) -> Run<'_> {
        Run::better(&self.suite, &self.subject)
    }

    fn awards_for(&self, check: &Event) -> Vec<Event> {
        signed(
            &self.referee,
            xp::eval_check_awards(&self.quest, &self.result, check, &[], AT + 100).unwrap(),
            AT + 100,
        )
    }

    fn events(&self, more: &[Event]) -> Vec<Event> {
        let mut all = vec![
            self.suite.clone(),
            self.subject.clone(),
            self.quest.clone(),
            self.result.clone(),
            self.check.clone(),
        ];
        all.extend_from_slice(more);
        all
    }

    fn trust(&self) -> XpTrust {
        XpTrust {
            referees: BTreeSet::from([self.referee.pubkey().to_owned()]),
            runners: BTreeSet::new(),
        }
    }
}

#[test]
fn a_confirming_check_credits_the_checker_the_evaluator_and_the_suite_author_once() {
    let w = World::new();
    let awards = w.awards_for(&w.check);
    assert_eq!(awards.len(), 3);
    let ledger = derive(&w.events(&awards), &w.trust());
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.conflicts, Vec::<String>::new());
    assert_eq!(ledger.totals.get(&pk("bob")), Some(&50));
    assert_eq!(ledger.totals.get(&pk("alice")), Some(&25));
    assert_eq!(ledger.totals.get(&pk("suite-author")), Some(&25));
    for credit in &ledger.credits {
        assert_eq!(credit.rule, "eval-check");
        assert_eq!(credit.evidence, [w.result.id.clone(), w.check.id.clone()]);
    }
}

fn pk(label: &str) -> String {
    signer(label).pubkey().to_owned()
}

#[test]
fn the_ledger_replays_to_the_same_totals_in_any_order_and_with_repeats() {
    let w = World::new();
    let awards = w.awards_for(&w.check);
    let events = w.events(&awards);
    let first = derive(&events, &w.trust());
    let mut shuffled: Vec<Event> = events.iter().rev().cloned().collect();
    shuffled.extend(events.iter().cloned());
    let again = derive(&shuffled, &w.trust());
    assert_eq!(first.totals, again.totals);
    assert_eq!(first.credits.len(), again.credits.len());
    assert_eq!(again.conflicts, Vec::<String>::new());
}

#[test]
fn a_revoked_eval_award_stops_counting_and_a_replacement_counts() {
    let w = World::new();
    let awards = w.awards_for(&w.check);
    let evaluator_award = awards
        .iter()
        .find(|a| xp::parse_award(a).unwrap().awardees[0].role == "evaluator")
        .unwrap()
        .clone();
    let revocation = sign(
        &w.referee,
        xp::revocation(&evaluator_award, "Awarded against the wrong result.").unwrap(),
    );
    let mut events = w.events(&awards);
    events.push(revocation);
    let ledger = derive(&events, &w.trust());
    assert_eq!(ledger.revoked, std::slice::from_ref(&evaluator_award.id));
    assert_eq!(ledger.totals.get(&pk("alice")), None);
    assert_eq!(ledger.totals.get(&pk("bob")), Some(&50));
    // The referee signs the key again; the replacement counts.
    let replacement = signed(
        &w.referee,
        xp::eval_check_awards(&w.quest, &w.result, &w.check, &[], AT + 200).unwrap(),
        AT + 200,
    )
    .into_iter()
    .find(|a| xp::parse_award(a).unwrap().awardees[0].role == "evaluator")
    .unwrap();
    events.push(replacement);
    let ledger = derive(&events, &w.trust());
    assert_eq!(ledger.totals.get(&pk("alice")), Some(&25));
}

#[test]
fn each_role_is_paid_once_per_suite_version_and_a_new_version_pays_again() {
    let w = World::new();
    let mut events = w.events(&w.awards_for(&w.check));
    // Carol checks too: her checker award counts, but a second evaluator
    // or suite-author award for the same suite version is a second live
    // award for one key, and counts for no one.
    let carol = published(&signer("carol"), &w.run(), Some(&w.result.id), AT + 20);
    let carols = w.awards_for(&carol);
    events.push(carol);
    events.extend(carols);
    let ledger = derive(&events, &w.trust());
    assert_eq!(ledger.totals.get(&pk("carol")), Some(&50));
    assert_eq!(ledger.totals.get(&pk("alice")), None);
    assert_eq!(ledger.totals.get(&pk("suite-author")), None);
    assert_eq!(ledger.conflicts.len(), 2, "{:?}", ledger.conflicts);

    // A new suite version is a new key: alice earns again under its quest.
    let v2 = World {
        suite: release(&signer("suite-author"), "project-map-tests-v2", AT - 4_000),
        ..World::new()
    };
    let quest = check_quest_version(&v2.referee, &v2.suite, &v2.subject, 500, 2);
    let result = published(&signer("alice"), &v2.run(), None, AT + 30);
    let check = published(&signer("bob"), &v2.run(), Some(&result.id), AT + 40);
    let awards = signed(
        &v2.referee,
        xp::eval_check_awards(&quest, &result, &check, &[], AT + 100).unwrap(),
        AT + 100,
    );
    let ledger = derive(
        &[
            w.events(&w.awards_for(&w.check)),
            vec![quest, result, check],
            awards,
        ]
        .concat(),
        &w.trust(),
    );
    assert_eq!(ledger.totals.get(&pk("alice")), Some(&50));
    assert_eq!(ledger.totals.get(&pk("bob")), Some(&100));
}

#[test]
fn a_key_holding_two_roles_is_paid_once_in_the_larger() {
    // Alice wrote the suite and published the result.
    let w = World::by("alice", "alice");
    let awards = w.awards_for(&w.check);
    assert_eq!(awards.len(), 2);
    let ledger = derive(&w.events(&awards), &w.trust());
    assert_eq!(ledger.totals.get(&pk("alice")), Some(&25));
    assert_eq!(ledger.totals.get(&pk("bob")), Some(&50));
    let alice: Vec<&str> = ledger
        .credits
        .iter()
        .filter(|c| c.pubkey == pk("alice"))
        .map(|c| c.role.as_str())
        .collect();
    assert_eq!(alice, ["evaluator"]);
}

#[test]
fn an_untrusted_referee_is_ignored() {
    let w = World::new();
    let mallory = signer("mallory");
    let quest = check_quest(&mallory, &w.suite, &w.subject, 500);
    let forged = signed(
        &mallory,
        xp::eval_check_awards(&quest, &w.result, &w.check, &[], AT + 100).unwrap(),
        AT + 100,
    );
    let ledger = derive(&w.events(&[vec![quest], forged].concat()), &w.trust());
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.untrusted, 3);
}

#[test]
fn a_disputed_or_self_check_earns_nothing_and_the_dispute_stays_visible() {
    let w = World::new();
    let mut worse = w.run();
    worse.verdict = "fail";
    let dispute = published(&signer("bob"), &worse, Some(&w.result.id), AT + 10);
    let refused = xp::eval_check_awards(&w.quest, &w.result, &dispute, &[], AT + 100);
    assert_eq!(
        refused.unwrap_err().code,
        nostr::contracts::RefusalCode::NotAdmitted
    );
    let own = published(&signer("alice"), &w.run(), Some(&w.result.id), AT + 10);
    assert!(xp::eval_check_awards(&w.quest, &w.result, &own, &[], AT + 100).is_err());
    let by_author = published(
        &signer("suite-author"),
        &w.run(),
        Some(&w.result.id),
        AT + 10,
    );
    assert!(xp::eval_check_awards(&w.quest, &w.result, &by_author, &[], AT + 100).is_err());

    // An award that names a check the rule refuses isn't counted: the
    // referee's award for Bob's confirming check, read beside a dispute
    // re-signed under the confirming check's ID, can't be forged, so a
    // reader that holds only the dispute refuses the award as missing
    // its check.
    let awards = w.awards_for(&w.check);
    let events = [
        vec![
            w.suite.clone(),
            w.subject.clone(),
            w.quest.clone(),
            w.result.clone(),
            dispute.clone(),
        ],
        awards,
    ]
    .concat();
    let ledger = derive(&events, &w.trust());
    assert!(ledger.totals.is_empty());
    assert_eq!(ledger.refused.len(), 3);

    let events = [w.events(&[]), vec![dispute.clone()]].concat();
    let ledger = derive(&events, &w.trust());
    let made = eval::made(&events, &ledger, &[pk("alice")]);
    assert_eq!(made.results[0].disputed_by, 1);
    assert_eq!(made.results[0].standing, Standing::Pending);
    let made = eval::made(&events, &ledger, &[pk("bob")]);
    let standings: BTreeSet<Standing> = made.checks.iter().map(|c| c.standing).collect();
    assert!(standings.contains(&Standing::Disputed));
}

#[test]
fn linked_keys_of_one_trainer_are_one_trainer_to_the_referee() {
    // Alice links a second key; its check of her own result passes the
    // rule's key comparison, and the referee's trainer check refuses it.
    let alice = signer("alice");
    let second = signer("alice's laptop");
    let profile = sign(
        &alice,
        xp::profile(alice.pubkey(), true, &[second.pubkey().to_owned()]).unwrap(),
    );
    let link = sign(
        &second,
        xp::link(second.pubkey(), Some(alice.pubkey())).unwrap(),
    );
    let trainers = Trainers::read(&[profile, link]);
    assert!(
        eval::distinct_trainers(
            &trainers,
            second.pubkey(),
            alice.pubkey(),
            &pk("suite-author")
        )
        .is_err()
    );
    assert!(
        eval::distinct_trainers(&trainers, &pk("bob"), alice.pubkey(), &pk("suite-author")).is_ok()
    );
    assert!(
        eval::distinct_trainers(
            &Trainers::default(),
            second.pubkey(),
            alice.pubkey(),
            &pk("suite-author")
        )
        .is_ok()
    );
}

fn checks_by(w: &World, labels: &[&str]) -> Vec<Event> {
    labels
        .iter()
        .enumerate()
        .map(|(n, label)| {
            published(
                &signer(label),
                &w.run(),
                Some(&w.result.id),
                AT + 10 + n as u64,
            )
        })
        .collect()
}

#[test]
fn a_better_result_is_a_candidate_after_three_distinct_trainers_confirm() {
    let w = World::new();
    let two = [vec![w.result.clone()], checks_by(&w, &["bob", "carol"])].concat();
    assert!(eval::candidates(&two, &Trainers::default(), &BTreeSet::new()).is_empty());

    let three = [
        vec![w.result.clone()],
        checks_by(&w, &["bob", "carol", "dave"]),
    ]
    .concat();
    let queue = eval::candidates(&three, &Trainers::default(), &BTreeSet::new());
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].subject, w.subject.id);
    assert_eq!(queue[0].results[0].confirmed_by.len(), 3);
    // Once adopted, it leaves the queue.
    let adopted = BTreeSet::from([w.subject.id.clone()]);
    assert!(eval::candidates(&three, &Trainers::default(), &adopted).is_empty());

    // Dave's key linked to Carol makes them one trainer.
    let carol = signer("carol");
    let dave = signer("dave");
    let linked = Trainers::read(&[
        sign(
            &carol,
            xp::profile(carol.pubkey(), true, &[dave.pubkey().to_owned()]).unwrap(),
        ),
        sign(
            &dave,
            xp::link(dave.pubkey(), Some(carol.pubkey())).unwrap(),
        ),
    ]);
    assert!(eval::candidates(&three, &linked, &BTreeSet::new()).is_empty());

    // A Worse result is never a candidate, however many confirm it.
    let mut worse = w.run();
    worse.verdict = "fail";
    let result = published(&signer("alice"), &worse, None, AT);
    let confirms: Vec<Event> = ["bob", "carol", "dave"]
        .iter()
        .map(|l| published(&signer(l), &worse, Some(&result.id), AT + 10))
        .collect();
    let events = [vec![result], confirms].concat();
    assert!(eval::candidates(&events, &Trainers::default(), &BTreeSet::new()).is_empty());
}

struct Adopted {
    w: World,
    checks: Vec<Event>,
    quest: Event,
    release: Event,
    documents: eval::Documents,
    awards: Vec<Event>,
}

fn adopted() -> Adopted {
    let w = World::new();
    let checks = checks_by(&w, &["bob", "carol", "dave"]);
    let operator = signer("operator");
    let admission = adopt::admission(operator.pubkey(), &[&w.result], AT + 50_000).unwrap();
    let manifest = adopt::manifest(
        &adopt::package_of(operator.pubkey()),
        "1",
        std::slice::from_ref(&w.subject.id),
        &[adopt::receipt(&admission)],
    )
    .unwrap();
    let parts = adopt::release(&manifest).unwrap();
    let release = operator.sign(AT + 500, parts.kind, parts.tags, parts.content);
    let spec = json!({
        "id": "ext-eval.adopt",
        "version": 1,
        "season": {"id": "ext-eval-s1", "opens_at": SEASON_OPENS, "closes_at": SEASON_CLOSES},
        "title": "Coder adopts a tool",
        "objective": "Get a tool into Coder's defaults with a confirmed result.",
        "acceptance": {
            "rule": "eval-adopt",
            "defaults": adopt::package_of(operator.pubkey()),
            "subject": {"id": w.subject.id, "pubkey": w.subject.pubkey, "kind": w.subject.kind},
        },
        "reference": null,
        "award": {"extension-author": 200, "suite-author": 100, "evaluator": 50},
    });
    let parts = xp::quest(&spec).unwrap();
    let quest = w
        .referee
        .sign(SEASON_OPENS, parts.kind, parts.tags, parts.content);
    let publications = [vec![w.result.clone()], checks.clone()].concat();
    let adoption = xp::Adoption {
        release: &release,
        manifest: &manifest,
        admission: &admission,
        results: &publications,
        checks: &publications,
        requests: &[],
    };
    let awards = signed(
        &w.referee,
        xp::eval_adopt_awards(&quest, &adoption, AT + 600).unwrap(),
        AT + 600,
    );
    Adopted {
        documents: eval::documents([manifest, admission]),
        w,
        checks,
        quest,
        release,
        awards,
    }
}

impl Adopted {
    fn events(&self) -> Vec<Event> {
        [
            vec![
                self.w.result.clone(),
                self.quest.clone(),
                self.release.clone(),
            ],
            self.checks.clone(),
            self.awards.clone(),
        ]
        .concat()
    }
}

#[test]
fn an_adoption_credits_the_tool_author_the_suite_author_and_the_evaluator() {
    let a = adopted();
    assert_eq!(a.awards.len(), 3);
    let ledger = derive_with(&a.events(), &a.documents, &a.w.trust());
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.totals.get(&pk("ext-author")), Some(&200));
    assert_eq!(ledger.totals.get(&pk("suite-author")), Some(&100));
    assert_eq!(ledger.totals.get(&pk("alice")), Some(&50));

    // Without the documents the release pins, a reader can't check them.
    let ledger = derive(&a.events(), &a.w.trust());
    assert!(ledger.totals.is_empty());
    assert!(
        ledger.refused[0].contains("manifest isn't available"),
        "{:?}",
        ledger.refused
    );

    // The adopted tool leaves the queue.
    let package = adopt::package_of(signer("operator").pubkey());
    let done = eval::adopted(&a.events(), &package, &a.documents);
    assert_eq!(done, BTreeSet::from([a.w.subject.id.clone()]));
    assert!(eval::candidates(&a.events(), &Trainers::default(), &done).is_empty());

    let made = eval::made(&a.events(), &ledger_with(&a), &[pk("ext-author")]);
    assert_eq!(made.adoptions.len(), 1);
    assert_eq!(made.xp, 200);
}

fn ledger_with(a: &Adopted) -> Ledger {
    derive_with(&a.events(), &a.documents, &a.w.trust())
}

#[test]
fn what_you_made_shows_pending_and_awarded_credit() {
    let w = World::new();
    let pending = w.events(&[]);
    let ledger = derive(&pending, &w.trust());
    let made = eval::made(&pending, &ledger, &[pk("alice")]);
    assert_eq!(made.results.len(), 1);
    assert_eq!(made.results[0].confirmed_by, 1);
    assert_eq!(made.results[0].standing, Standing::Pending);
    assert_eq!(made.pending, 1);
    let made = eval::made(&pending, &ledger, &[pk("suite-author")]);
    assert_eq!(made.suites.len(), 1);
    assert_eq!(made.suites[0].results, 2);

    let awarded = w.events(&w.awards_for(&w.check));
    let ledger = derive(&awarded, &w.trust());
    let made = eval::made(&awarded, &ledger, &[pk("alice")]);
    assert_eq!(made.results[0].standing, Standing::Awarded);
    assert_eq!(made.results[0].xp, 25);
    assert_eq!(made.xp, 25);
    let made = eval::made(&awarded, &ledger, &[pk("bob")]);
    assert_eq!(made.checks[0].standing, Standing::Awarded);
    assert_eq!(made.checks[0].xp, 50);
    let made = eval::made(&awarded, &ledger, &[pk("suite-author")]);
    assert_eq!(made.suites[0].xp, 25);

    // A result no one has checked is waiting.
    let lone = published(&signer("erin"), &w.run(), None, AT + 5);
    let made = eval::made(&[lone], &Ledger::default(), &[pk("erin")]);
    assert_eq!(made.results[0].standing, Standing::Waiting);
}

#[test]
fn playtest_xp_stays_off_a_reader_that_trusts_only_the_trainer_referee() {
    let w = World::new();
    let playtest_referee = signer("playtest-referee");
    let tester = signer("tester");
    let quest = playtest_quest(&playtest_referee, "bug", 10);
    let report = playtest_report(&tester, "bug");
    let award = playtest_award(
        &playtest_referee,
        &quest,
        &report,
        "OpenAgentsInc/openagents#9",
        Some("p2"),
    );
    let events = w.events(&[w.awards_for(&w.check), vec![quest, report, award]].concat());
    let ledger = derive(&events, &w.trust());
    assert_eq!(ledger.totals.get(tester.pubkey()), None);
    assert_eq!(ledger.totals.values().sum::<u64>(), 100);
    assert!(ledger.credits.iter().all(|c| c.rule == "eval-check"));
}

/// Money words that must not appear in the ledger's code: nothing here
/// converts, spends, or pays XP.
const MONEY: &[&str] = &[
    "sats",
    "msat",
    "invoice",
    "payout",
    "bolt11",
    "lightning",
    "wallet",
    "usd",
    "price",
    "convert",
    "transfer(",
    "spend(",
    "redeem",
];

#[test]
fn no_code_path_converts_xp_and_no_copy_promises_money() {
    let sources = [
        ("lib.rs", include_str!("../lib.rs")),
        ("eval.rs", include_str!("../eval.rs")),
        ("adopt.rs", include_str!("../adopt.rs")),
        ("trainers.rs", include_str!("../trainers.rs")),
    ];
    for (file, text) in sources {
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            let lower = code.to_lowercase();
            for word in MONEY {
                assert!(
                    !lower.contains(word),
                    "{file}:{}: `{word}` in `{code}`",
                    n + 1
                );
            }
        }
    }
    // Every Credit field is XP and who earned it; there's no amount in
    // another unit.
    let credit = serde_json::to_value(Credit {
        award: String::new(),
        referee: String::new(),
        quest: String::new(),
        title: String::new(),
        season: String::new(),
        rule: String::new(),
        role: String::new(),
        pubkey: String::new(),
        xp: 0,
        evidence: Vec::new(),
    })
    .unwrap();
    let mut fields: Vec<&str> = credit
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "award", "evidence", "pubkey", "quest", "referee", "role", "rule", "season", "title",
            "xp"
        ]
    );
    // The package's copy mentions money only to say there is none.
    for line in adopt::POLICY.lines().chain(adopt::PACKAGE_RECORD.lines()) {
        let lower = line.to_lowercase();
        if ["money", "sats", "paid", "pays", "payment"]
            .iter()
            .any(|w| lower.contains(w))
        {
            assert!(
                ["no ", "never", "not ", "n't"]
                    .iter()
                    .any(|n| lower.contains(n)),
                "copy that may promise money: {line}"
            );
        }
    }
}
