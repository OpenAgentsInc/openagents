//! The referee job and adoption end to end, against the in-process relay
//! the `kb` and `xp` tests use: a result, a check by another key, one
//! award per role, refusals for a self-check, an early check, and linked
//! keys, a rerun that signs nothing twice, the adoption queue, and an
//! operator's adoption that the referee then credits. Keys are throwaway;
//! the referee and `coder-defaults` keys are created in scratch
//! directories.

use std::collections::BTreeSet;

use nostr::domain::RelaySigner;
use nostr::eval_ext::Cites;
use xp_ledger::eval::fixture::{Run, published, published_citing, release, signer};

use super::*;
use crate::kbnet::tests::{Store, relay, scratch};

fn put(store: &Store, events: &[Event]) {
    store.lock().unwrap().extend(events.iter().cloned());
}

fn count(store: &Store, kind: u16) -> usize {
    store
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.kind == kind)
        .count()
}

fn events(store: &Store) -> Vec<Event> {
    store.lock().unwrap().clone()
}

fn season(opens: u64, closes: u64) -> Value {
    json!({"id": "ext-eval-test", "opens_at": opens, "closes_at": closes})
}

/// A templates directory whose Project map suite is published by
/// `publisher`, open from `opens` to `closes`.
fn templates_dir(name: &str, publisher: &str, opens: u64, closes: u64) -> PathBuf {
    let dir = scratch(name);
    for (file, text) in TEMPLATES {
        let mut value: Value = serde_json::from_str(text).unwrap();
        value["season"] = season(opens, closes);
        if value["rule"] == xp::EVAL_CHECK {
            value["suite"]["publishers"] = json!([publisher]);
        }
        std::fs::write(dir.join(file), value.to_string()).unwrap();
    }
    dir
}

struct Setup {
    store: Store,
    home: PathBuf,
    referee_key: PathBuf,
    suite: Event,
    subject: Event,
    result: Event,
    opts: Vec<String>,
}

impl Setup {
    async fn new(name: &str) -> Self {
        Self::with_season(name, 0, 4_000_000_000).await
    }

    async fn with_season(name: &str, opens: u64, closes: u64) -> Self {
        let (url, store) = relay().await;
        let home = scratch(name);
        let referee_key = home.join("nostr/referee-key");
        std::fs::create_dir_all(referee_key.parent().unwrap()).unwrap();
        Identity::load_from(&referee_key).unwrap();
        let at = now();
        let suite = release(&signer("suite-author"), "project-map-tests", at - 1_000);
        let subject = release(&signer("ext-author"), "project-map", at - 1_000);
        let result = published(
            &signer("alice"),
            &Run::better(&suite, &subject),
            None,
            at - 100,
        );
        put(&store, &[suite.clone(), subject.clone(), result.clone()]);
        let templates = templates_dir(
            &format!("{name}-templates"),
            signer("suite-author").pubkey(),
            opens,
            closes,
        );
        let opts = vec![
            "--relay".into(),
            url.clone(),
            "--quests".into(),
            templates.display().to_string(),
            "--documents".into(),
            home.join("documents").display().to_string(),
            "--queue".into(),
            home.join("candidates.json").display().to_string(),
            "--state".into(),
            home.join("refusals.json").display().to_string(),
            "--defaults-root".into(),
            signer("operator-root").pubkey().to_owned(),
        ];
        Setup {
            store,
            home,
            referee_key,
            suite,
            subject,
            result,
            opts,
        }
    }

    fn run(&self) -> Run<'_> {
        Run::better(&self.suite, &self.subject)
    }

    fn check(&self, by: &RelaySigner, at: u64) -> Event {
        let check = published(by, &self.run(), Some(&self.result.id), at);
        put(&self.store, std::slice::from_ref(&check));
        check
    }

    /// `by`'s Better result on a second suite the validator released after
    /// the tool, externally validating the result.
    fn validate(&self, by: &RelaySigner, at: u64) -> Event {
        let suite = release(&signer("validator"), "project-map-more-tests", at - 500);
        let validation = published_citing(
            by,
            &Run::better(&suite, &self.subject),
            Some(Cites::Validates(&self.result.id)),
            at,
        );
        put(&self.store, &[suite, validation.clone()]);
        validation
    }

    fn options(&self, more: &[&str]) -> XpOptions {
        let mut args = self.opts.clone();
        args.extend(more.iter().map(|m| (*m).to_string()));
        super::super::parse(&args).unwrap()
    }

    async fn pass(&self) -> Tally {
        let identity = Identity::load_from(&self.referee_key).unwrap();
        referee_pass(&self.options(&[]), &identity).await.unwrap()
    }

    fn trust(&self) -> XpTrust {
        XpTrust {
            referees: BTreeSet::from([knowledge::remote::own_pubkey(&self.referee_key).unwrap()]),
            runners: BTreeSet::new(),
        }
    }
}

use knowledge::xp::XpTrust;

#[tokio::test]
async fn a_confirmed_check_earns_one_award_per_role_and_a_rerun_signs_nothing_twice() {
    let s = Setup::new("eval-referee").await;
    // Nothing to do before a check.
    let tally = s.pass().await;
    assert_eq!((tally.signed, tally.quests), (0, 0));

    s.check(&signer("bob"), now() - 50);
    let tally = s.pass().await;
    assert_eq!(tally.signed, 3, "{tally:?}");
    assert_eq!(tally.quests, 1);
    assert_eq!(count(&s.store, xp::AWARD_KIND), 3);
    assert_eq!(count(&s.store, xp::QUEST_KIND), 1);

    let again = s.pass().await;
    assert_eq!((again.signed, again.quests, again.already), (0, 0, 3));
    assert_eq!(count(&s.store, xp::AWARD_KIND), 3);

    let ledger = knowledge::xp::derive(&events(&s.store), &s.trust());
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.conflicts, Vec::<String>::new());
    assert_eq!(ledger.totals.get(signer("bob").pubkey()), Some(&50));
    assert_eq!(ledger.totals.get(signer("alice").pubkey()), Some(&25));
    assert_eq!(
        ledger.totals.get(signer("suite-author").pubkey()),
        Some(&25)
    );

    // A second checker earns her own award; the evaluator and suite
    // author, already paid for this suite version, earn nothing more.
    s.check(&signer("carol"), now() - 40);
    let tally = s.pass().await;
    assert_eq!((tally.signed, tally.already), (1, 5));
    let ledger = knowledge::xp::derive(&events(&s.store), &s.trust());
    assert_eq!(ledger.totals.get(signer("carol").pubkey()), Some(&50));
    assert_eq!(ledger.totals.get(signer("alice").pubkey()), Some(&25));
}

#[tokio::test]
async fn a_self_check_an_early_check_and_a_linked_key_are_refused() {
    let s = Setup::new("eval-referee-refusals").await;
    // The evaluator checking her own result.
    s.check(&signer("alice"), now() - 50);
    // A check published before the result it checks.
    s.check(&signer("bob"), now() - 200);
    // A key alice linked to herself, both sides signed.
    let alice = signer("alice");
    let laptop = signer("alice's laptop");
    let profile = xp::profile(alice.pubkey(), true, &[laptop.pubkey().to_owned()]).unwrap();
    let link = xp::link(laptop.pubkey(), Some(alice.pubkey())).unwrap();
    put(
        &s.store,
        &[
            alice.sign(now() - 500, profile.kind, profile.tags, profile.content),
            laptop.sign(now() - 500, link.kind, link.tags, link.content),
        ],
    );
    s.check(&laptop, now() - 30);
    let tally = s.pass().await;
    assert_eq!(tally.signed, 0, "{tally:?}");
    assert_eq!(tally.refused, 3, "{tally:?}");
    assert_eq!(count(&s.store, xp::AWARD_KIND), 0);
    // Each refusal is remembered, so the next pass logs none again.
    let logged: BTreeSet<String> =
        serde_json::from_str(&std::fs::read_to_string(s.home.join("refusals.json")).unwrap())
            .unwrap();
    assert_eq!(logged.len(), 3);
    assert!(
        logged
            .iter()
            .any(|l| l.contains("checking your own result"))
    );
    assert!(
        logged
            .iter()
            .any(|l| l.contains("isn't newer than the result"))
    );
    assert!(
        logged
            .iter()
            .any(|l| l.contains("linked to the evaluator's trainer"))
    );
    let again = s.pass().await;
    assert_eq!(again.signed, 0);
}

#[tokio::test]
async fn a_closed_season_and_a_suite_that_isnt_a_starter_are_left_alone() {
    let closed = Setup::with_season("eval-referee-closed", 0, now() - 60).await;
    closed.check(&signer("bob"), now() - 50);
    assert_eq!(closed.pass().await, Tally::default());
    assert_eq!(count(&closed.store, xp::QUEST_KIND), 0);

    // A suite of the same name from a key the template doesn't list.
    let s = Setup::new("eval-referee-foreign").await;
    let mallory_suite = release(&signer("mallory"), "project-map-tests", now() - 1_000);
    let run = Run::better(&mallory_suite, &s.subject);
    let result = published(&signer("mallory-2"), &run, None, now() - 100);
    let check = published(&signer("mallory-3"), &run, Some(&result.id), now() - 50);
    put(&s.store, &[mallory_suite, result, check]);
    assert_eq!(s.pass().await, Tally::default());
    assert_eq!(count(&s.store, xp::AWARD_KIND), 0);
}

#[tokio::test]
async fn an_operator_adopts_a_candidate_and_the_referee_credits_the_adoption() {
    let s = Setup::new("eval-adopt").await;
    s.check(&signer("bob"), now() - 50);
    s.check(&signer("carol"), now() - 40);
    let tally = s.pass().await;
    assert!(tally.candidates.is_empty(), "two checks aren't enough");
    s.check(&signer("dave"), now() - 30);
    let tally = s.pass().await;
    assert!(
        tally.candidates.is_empty(),
        "three checks on the author's suite prove reproducibility, not external validity"
    );
    s.validate(&signer("carol"), now() - 20);
    let tally = s.pass().await;
    assert_eq!(tally.candidates.len(), 1);
    assert_eq!(tally.candidates[0].subject, s.subject.id);
    let queue: Value =
        serde_json::from_str(&std::fs::read_to_string(s.home.join("candidates.json")).unwrap())
            .unwrap();
    assert_eq!(queue[0]["subject"], json!(s.subject.id));

    // The operator's key; `defaults-keygen` makes it once.
    let key = s.home.join("nostr/coder-defaults-key");
    let keygen = s.options(&["--key", &key.display().to_string()]);
    assert_eq!(defaults_keygen(&keygen).unwrap(), 0);
    assert_eq!(defaults_keygen(&keygen).unwrap(), 1);
    let root = knowledge::remote::own_pubkey(&key).unwrap();

    // Listing changes nothing; a tool that isn't a candidate is refused.
    let package_dir = s.home.join("package");
    assert_eq!(adopt_command(&s.options(&[]), &key).await.unwrap(), 0);
    let other = release(&signer("ext-author"), "other-tool", now() - 1_000);
    assert_eq!(
        adopt_command(&s.options(&["--subject", &other.id]), &key)
            .await
            .unwrap(),
        1
    );
    // The suite, the tool, and the validator's second suite.
    assert_eq!(count(&s.store, nostr::ext::RELEASE_KIND), 3);

    let adopt = s.options(&[
        "--subject",
        &s.subject.id,
        "--package-dir",
        &package_dir.display().to_string(),
    ]);
    assert_eq!(adopt_command(&adopt, &key).await.unwrap(), 0);
    assert_eq!(count(&s.store, nostr::ext::RELEASE_KIND), 4);
    assert_eq!(count(&s.store, nostr::ext::LOCATOR_KIND), 2);
    assert_eq!(
        std::fs::read_dir(package_dir.join("documents"))
            .unwrap()
            .count(),
        2
    );
    // Adopted once; a second adoption is refused.
    assert_eq!(adopt_command(&adopt, &key).await.unwrap(), 1);

    // The referee, reading the operator's root, credits the adoption.
    let mut opts = s.opts.clone();
    let at = opts.iter().position(|o| o == "--defaults-root").unwrap();
    opts[at + 1] = root.clone();
    let identity = Identity::load_from(&s.referee_key).unwrap();
    let o = super::super::parse(&opts).unwrap();
    let tally = referee_pass(&o, &identity).await.unwrap();
    assert_eq!(tally.signed, 3, "{tally:?}");
    assert!(tally.candidates.is_empty(), "adopted tools leave the queue");
    let again = referee_pass(&o, &identity).await.unwrap();
    assert_eq!(again.signed, 0);

    let documents = read_documents(Some(&s.home.join("documents")));
    let ledger = knowledge::xp::derive_with(&events(&s.store), &documents, &s.trust());
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.totals.get(signer("ext-author").pubkey()), Some(&200));
    // The suite author: 25 for the check, 100 for the adoption.
    assert_eq!(
        ledger.totals.get(signer("suite-author").pubkey()),
        Some(&125)
    );
    assert_eq!(ledger.totals.get(signer("alice").pubkey()), Some(&75));
}

/// Hosted results: the runner signs both the result and the check, each
/// naming the trainer who asked, and each carries that trainer's signed
/// `25920` inline, because relays keep no `25920`. The referee credits the
/// two trainers without any request on the relay; a hosted result without
/// its request inline, whose request no relay holds, is refused.
#[tokio::test]
async fn hosted_results_credit_their_trainers_from_the_inline_request() {
    let s = Setup::new("eval-referee-hosted").await;
    let runner = signer("hosted-runner");
    let at = now();
    let asked = xp_ledger::eval::fixture::request(&signer("dana"), runner.pubkey(), at - 300);
    let mut run = s.run();
    run.request = Some(&asked);
    let result = published(&runner, &run, None, at - 90);
    let rechecked = xp_ledger::eval::fixture::request(&signer("erin"), runner.pubkey(), at - 80);
    let mut check_run = s.run();
    check_run.request = Some(&rechecked);
    let check = published(&runner, &check_run, Some(&result.id), at - 20);
    put(&s.store, &[result.clone(), check]);
    assert_eq!(count(&s.store, nostr::kinds::CJ_EXECUTION_REQUEST), 0);

    let tally = s.pass().await;
    assert_eq!(tally.signed, 3, "{tally:?}");
    let ledger = knowledge::xp::derive(&events(&s.store), &s.trust());
    assert_eq!(ledger.refused, Vec::<String>::new());
    assert_eq!(ledger.totals.get(signer("erin").pubkey()), Some(&50));
    assert_eq!(ledger.totals.get(signer("dana").pubkey()), Some(&25));
    assert_eq!(
        ledger.totals.get(runner.pubkey()),
        None,
        "the runner earns nothing"
    );

    // Without the request inline and none on the relay, nobody is credited.
    let bare = Setup::new("eval-referee-hosted-bare").await;
    let mut run = bare.run();
    run.request = Some(&asked);
    let result = xp_ledger::eval::fixture::published_bare(&runner, &run, None, at - 90);
    let mut check_run = bare.run();
    check_run.request = Some(&rechecked);
    let check =
        xp_ledger::eval::fixture::published_bare(&runner, &check_run, Some(&result.id), at - 20);
    put(&bare.store, &[result, check]);
    let tally = bare.pass().await;
    assert_eq!(tally.signed, 0, "{tally:?}");
    assert_eq!(tally.refused, 1, "{tally:?}");
}

#[test]
fn the_built_in_templates_parse_and_promise_no_money() {
    let built = templates(None).unwrap();
    assert_eq!(built.checks.len(), 3);
    let adopt = built.adopt.as_ref().unwrap();
    for t in &built.checks {
        assert_eq!(
            t.award,
            json!({"checker": 50, "evaluator": 25, "suite-author": 25})
        );
        assert!(t.id.starts_with("ext-eval.") && t.id.ends_with(".check"));
    }
    assert_eq!(
        adopt.award,
        json!({"extension-author": 200, "suite-author": 100, "evaluator": 50})
    );
    let copy = TEMPLATES
        .iter()
        .map(|(_, t)| *t)
        .chain([super::super::USAGE])
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    for word in ["sats", "money", "paid", "payment", "reward", "credits", "$"] {
        for line in copy.lines().filter(|l| l.contains(word)) {
            assert!(
                ["no ", "never", "not ", "n't", "nothing"]
                    .iter()
                    .any(|n| line.contains(n)),
                "copy that may promise money: {line}"
            );
        }
    }
}
