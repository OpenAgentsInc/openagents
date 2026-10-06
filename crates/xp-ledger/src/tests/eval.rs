//! `eval-check` and `eval-adopt` in the ledger: awards re-checked against
//! the signed results and checks they name, replayed, revoked, unique per
//! role and suite version, role collapse, untrusted referees, disputes,
//! self-checks and linked keys, the adoption queue, the adoption's
//! documents, and "what you made". Keys are throwaway, from labels.

use super::*;
use crate::adopt;
use crate::eval::fixture::{Run, published, published_citing, release};
use crate::eval::{self, Standing};
use nostr::eval_ext::Cites;

/// The validator's suite, released after the tool, and Carol's Better
/// result on it that externally validates `w.result`.
fn validation_of(w: &World) -> (Event, Event) {
    let suite = release(&signer("validator"), "project-map-more-tests", AT - 4_000);
    let validation = published_citing(
        &signer("carol"),
        &Run::better(&suite, &w.subject),
        Some(Cites::Validates(&w.result.id)),
        AT + 30,
    );
    (suite, validation)
}

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
fn a_dispute_earns_credit_a_self_check_earns_nothing_and_the_dispute_stays_visible() {
    let w = World::new();
    let mut worse = w.run();
    worse.verdict = "fail";
    let dispute = published(&signer("bob"), &worse, Some(&w.result.id), AT + 10);
    // Credit is for the rerun, not for agreement: Bob's dispute pays the
    // same three roles his confirmation would.
    let paid = xp::eval_check_awards(&w.quest, &w.result, &dispute, &[], AT + 100).unwrap();
    assert_eq!(paid.len(), 3);
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
    // Bob's dispute stands beside his confirmation, both pending credit.
    let made = eval::made(&events, &ledger, &[pk("bob")]);
    assert_eq!(made.checks.len(), 2);
    assert!(made.checks.iter().all(|c| c.standing == Standing::Pending));
    // A result whose only check disputes it is Disputed, and visible.
    let only = [
        vec![w.suite.clone(), w.subject.clone(), w.result.clone()],
        vec![dispute],
    ]
    .concat();
    let ledger = derive(&only, &w.trust());
    let made = eval::made(&only, &ledger, &[pk("alice")]);
    assert_eq!(made.results[0].standing, Standing::Disputed);
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

#[test]
fn the_ledger_refuses_linked_self_checks_and_keeps_independent_credit() {
    let w = World::new();
    for owner in ["alice", "suite-author"] {
        let trainer = signer(owner);
        let bob = signer("bob");
        let profile = sign(
            &trainer,
            xp::profile(trainer.pubkey(), true, &[bob.pubkey().to_owned()]).unwrap(),
        );
        let link = sign(
            &bob,
            xp::link(bob.pubkey(), Some(trainer.pubkey())).unwrap(),
        );
        let awards = w.awards_for(&w.check);
        let mut events = w.events(&awards);
        events.push(profile.clone());
        events.push(link.clone());
        let ledger = derive(&events, &w.trust());
        assert!(ledger.totals.is_empty());
        assert!(ledger.credits.is_empty());
        assert_eq!(ledger.refused.len(), awards.len());
        assert!(ledger.refused.iter().all(|r| r.contains("linked")));
        events.reverse();
        assert_eq!(derive(&events, &w.trust()), ledger);

        // A profile alone does not establish a two-sided link.
        let mut unlinked = w.events(&awards);
        unlinked.push(profile);
        assert_eq!(derive(&unlinked, &w.trust()).credits.len(), 3);

        // A distinct trainer's check remains creditable beside refused awards.
        let carol = published(&signer("carol"), &w.run(), Some(&w.result.id), AT + 20);
        events.push(carol.clone());
        events.extend(w.awards_for(&carol));
        let independent = derive(&events, &w.trust());
        assert_eq!(independent.totals.get(&pk("carol")), Some(&50));
        assert_eq!(independent.credits.len(), 3);
    }
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
fn the_admission_lifetime_follows_the_subjects_identity_and_the_record_agrees() {
    use nostr::eval_ext::IdentityStrength;
    // The weaker the identity, the sooner the evidence expires; an
    // unresolved subject never becomes a shared default.
    assert_eq!(eval::expiry_days(None), Some(365));
    assert_eq!(
        eval::expiry_days(Some(IdentityStrength::Content)),
        Some(365)
    );
    assert_eq!(eval::expiry_days(Some(IdentityStrength::Version)), Some(90));
    assert_eq!(
        eval::expiry_days(Some(IdentityStrength::Endpoint)),
        Some(14)
    );
    assert_eq!(eval::expiry_days(Some(IdentityStrength::Unresolved)), None);
    let record: serde_json::Value = serde_json::from_str(adopt::PACKAGE_RECORD).unwrap();
    let days = &record["candidate"]["expiry_days"];
    for (identity, expected) in eval::EXPIRY_DAYS {
        assert_eq!(
            days[identity.word()].as_u64(),
            expected,
            "{}",
            identity.word()
        );
    }
    assert_eq!(
        record["candidate"]["confirming_checks"].as_u64(),
        Some(eval::CONFIRMING_CHECKS as u64)
    );
    assert_eq!(
        record["candidate"]["validations"].as_u64(),
        Some(eval::VALIDATIONS as u64)
    );
    // Results on one subject take the shortest lifetime among them; the
    // fixture's extensions are content-addressed, so the ordinary cadence.
    let suite = release(&signer("suite-author"), "project-map-tests", AT - 5_000);
    let subject = release(&signer("ext-author"), "project-map", AT - 5_000);
    let run = Run::better(&suite, &subject);
    let events = [
        published(&signer("alice"), &run, None, AT),
        published(&signer("bob"), &run, None, AT + 1),
    ];
    let results: Vec<nostr::eval_ext::Publication> = events
        .iter()
        .filter_map(|e| nostr::eval_ext::parse_publication(e).ok())
        .collect();
    assert_eq!(results.len(), 2);
    let refs: Vec<&nostr::eval_ext::Publication> = results.iter().collect();
    assert_eq!(eval::expiry_days_for(&refs), Some(365));
    assert_eq!(eval::expiry_days_for(&[]), None);
}

#[test]
fn a_better_result_is_a_candidate_after_three_distinct_trainers_confirm_and_one_validates() {
    let w = World::new();
    let (suite2, validation) = validation_of(&w);
    let releases = vec![w.suite.clone(), w.subject.clone(), suite2.clone()];
    let two = [
        releases.clone(),
        vec![w.result.clone(), validation.clone()],
        checks_by(&w, &["bob", "carol"]),
    ]
    .concat();
    assert!(eval::candidates(&two, &Trainers::default(), &BTreeSet::new()).is_empty());

    // Three confirmations on the author's own suite prove reproducibility
    // and nothing more: without an external validation, no candidate.
    let three_unvalidated = [
        releases.clone(),
        vec![w.result.clone()],
        checks_by(&w, &["bob", "carol", "dave"]),
    ]
    .concat();
    assert!(
        eval::candidates(&three_unvalidated, &Trainers::default(), &BTreeSet::new()).is_empty()
    );
    // A second suite by the tool's own author is provenance, not
    // independence; one released before the tool could have been tuned
    // against; neither counts.
    for (author, at) in [("ext-author", AT - 4_000), ("validator", AT - 6_000)] {
        let suite = release(&signer(author), "project-map-more-tests", at);
        let dependent = published_citing(
            &signer("carol"),
            &Run::better(&suite, &w.subject),
            Some(Cites::Validates(&w.result.id)),
            AT + 30,
        );
        let events = [
            vec![
                w.suite.clone(),
                w.subject.clone(),
                suite,
                w.result.clone(),
                dependent,
            ],
            checks_by(&w, &["bob", "carol", "dave"]),
        ]
        .concat();
        assert!(eval::candidates(&events, &Trainers::default(), &BTreeSet::new()).is_empty());
    }
    // Without the releases a reader can't see independence, so it doesn't
    // assume it.
    let blind = [
        vec![w.result.clone(), validation.clone()],
        checks_by(&w, &["bob", "carol", "dave"]),
    ]
    .concat();
    assert!(eval::candidates(&blind, &Trainers::default(), &BTreeSet::new()).is_empty());

    let three = [
        releases,
        vec![w.result.clone(), validation],
        checks_by(&w, &["bob", "carol", "dave"]),
    ]
    .concat();
    let queue = eval::candidates(&three, &Trainers::default(), &BTreeSet::new());
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].subject, w.subject.id);
    assert_eq!(queue[0].results[0].confirmed_by.len(), 3);
    assert_eq!(queue[0].results[0].validations.len(), 1);
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
    let events = [vec![w.suite.clone(), w.subject.clone(), result], confirms].concat();
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
    let (suite2, validation) = validation_of(&w);
    let mut checks = checks_by(&w, &["bob", "carol", "dave"]);
    checks.push(validation.clone());
    checks.push(suite2);
    let operator = signer("operator");
    let admission =
        adopt::admission(operator.pubkey(), &[&w.result], &[&validation], AT + 50_000).unwrap();
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

/// The referee pays a checker once per test set version, so Bob's second
/// confirming check on the same test set, after his first was awarded,
/// waits for nothing: it earns no credit and isn't pending (#9948).
#[test]
fn a_second_check_on_a_paid_test_set_is_not_pending() {
    let w = World::new();
    let carol = published(&signer("carol"), &w.run(), None, AT + 10);
    let again = published(&signer("bob"), &w.run(), Some(&carol.id), AT + 20);
    let events = w.events(&[w.awards_for(&w.check), vec![carol, again.clone()]].concat());
    let ledger = derive(&events, &w.trust());
    let made = eval::made(&events, &ledger, &[pk("bob")]);
    assert_eq!(made.checks.len(), 2);
    let second = made.checks.iter().find(|c| c.id == again.id).unwrap();
    assert_eq!(second.standing, Standing::NoCredit);
    assert_eq!(second.xp, 0);
    assert!(made.checks.iter().any(|c| c.standing == Standing::Awarded));
    assert_eq!(made.pending, 0);

    // Before the first award both checks wait for one.
    let events = w.events(&[published(&signer("carol"), &w.run(), None, AT + 10)]);
    let ledger = derive(&events, &w.trust());
    let made = eval::made(&events, &ledger, &[pk("bob")]);
    assert_eq!(made.checks[0].standing, Standing::Pending);
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

#[test]
fn the_defaults_admit_a_dependency_only_under_a_live_admission_and_the_lock_round_trips() {
    use crate::defaults;
    let a = adopted();
    let package = adopt::package_of(signer("operator").pubkey());
    let events = a.events();

    // Before the admission lapses, the tool is admitted under it.
    let now = defaults::current(&events, &package, &a.documents, AT + 1_000).unwrap();
    assert_eq!(now.release.id, a.release.id);
    assert_eq!(now.version, "1");
    assert_eq!(now.subjects(), vec![a.w.subject.id.clone()]);
    assert_eq!(
        now.admitted[0].definition,
        format!("{}:tool/main", a.w.subject.pubkey)
    );
    assert_eq!(now.admitted[0].expires_at, AT + 50_000);
    assert!(now.lapsed.is_empty());
    assert!(
        a.documents.contains_key(&now.admitted[0].admission),
        "the lock names a held admission"
    );

    // The lock round-trips through its bytes.
    let lock = defaults::lock_document(&now);
    assert_eq!(defaults::parse_lock(&lock).unwrap(), now);
    assert!(nostr::contracts::digest_bytes(&lock).starts_with("sha256:"));

    // After it lapses, the dependency is named and admits nothing.
    let later = defaults::current(&events, &package, &a.documents, AT + 50_000).unwrap();
    assert!(later.admitted.is_empty());
    assert_eq!(later.lapsed, vec![a.w.subject.id.clone()]);

    // Without the admission's bytes, the same: not held is not admitted.
    let manifest_only: eval::Documents = a
        .documents
        .iter()
        .filter(|(d, _)| **d == now.manifest)
        .map(|(d, b)| (d.clone(), b.clone()))
        .collect();
    let held = defaults::current(&events, &package, &manifest_only, AT + 1_000).unwrap();
    assert!(held.admitted.is_empty());
    assert_eq!(held.lapsed, vec![a.w.subject.id.clone()]);
    assert_eq!(
        defaults::wanted_digests(&a.release, &eval::Documents::new()),
        vec![now.manifest.clone()]
    );
    assert_eq!(
        defaults::wanted_digests(&a.release, &manifest_only),
        vec![now.manifest.clone(), now.admitted[0].admission.clone()]
    );

    // A reader that gathered only the awards' evidence still needs the
    // validation the admission cites; the admission says which reports.
    let (suite2, validation) = validation_of(&a.w);
    let _ = suite2;
    let cited = eval::cited_report_hexes(&a.documents);
    let hex = |e: &Event| {
        nostr::eval_ext::parse_publication(e)
            .unwrap()
            .report_ref
            .digest
            .trim_start_matches("sha256:")
            .to_string()
    };
    assert!(cited.contains(&hex(&a.w.result)));
    assert!(cited.contains(&hex(&validation)));
    assert_eq!(cited.len(), 2);

    // Without the manifest there are no defaults to read, and another
    // package's releases aren't these defaults.
    assert!(defaults::current(&events, &package, &eval::Documents::new(), AT + 1_000).is_none());
    assert!(
        defaults::current(&events, &adopt::package_of(&pk("nobody")), &a.documents, AT).is_none()
    );
}
