//! Quests, awards, revocations, and the ledger against the in-process
//! relay the `kb` tests use. The referee key is created in a scratch
//! directory by the code under test; the author and runner keys are
//! throwaway keys from small numbers.

use nostr::domain::RelaySigner;

use super::*;
use crate::kbnet::tests::{Store, relay, scratch};

/// A throwaway key: the secret is a small number.
fn signer(n: u64) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:064x}")).expect("throwaway key")
}

fn put(store: &Store, signer: &RelaySigner, parts: kb::Unsigned) -> Event {
    let event = signer.sign(now(), parts.kind, parts.tags, parts.content);
    store.lock().unwrap().push(event.clone());
    event
}

fn count(store: &Store, kind: u16) -> usize {
    store
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.kind == kind)
        .count()
}

fn options(words: &[&str]) -> XpOptions {
    let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
    parse(&args).unwrap()
}

const DOCUMENT: &str = "---\nid: git.reflog-recovery\nversion: 1\nkind: method\ntitle: Recover commits from the reflog\nsummary: >-\n  Lost commits stay reachable from the reflog.\ntags: [git]\napplies_when: >-\n  A branch lost commits.\nstatus: candidate\nauthor: someone\nprovenance:\n  written_from: [reference]\n  cites: [\"Pro Git, 2014\"]\nevidence: []\n---\n\n## Details\n\nBody.\n";

/// An entry by `author` and a passing report on it by `runner`, paired on
/// `fix-git`, both already on the relay. Returns the evidence event.
fn completion(store: &Store, author: &RelaySigner, runner: &RelaySigner) -> Event {
    let entry = put(
        store,
        author,
        kb::entry("git.reflog-recovery", 1, "method", &[], DOCUMENT).unwrap(),
    );
    let report = json!({
        "v": "openagents.eval-report.v1", "requires": [],
        "evaluator": runner.pubkey(),
        "subject": {"definition": {
            "id": kb::qualified_id(runner.pubkey(), "git.reflog-recovery"),
            "artifact": kb::document_artifact(DOCUMENT),
            "event": {"id": entry.id, "pubkey": entry.pubkey, "kind": kb::ENTRY_KIND},
        }},
        "verdict": "pass",
        "meta": {"kb": {"pairs": [{"task": "fix-git", "model": "m",
            "with": {"runs": 2, "passes": 2, "usd": 0.3, "unknown": 0},
            "without": {"runs": 2, "passes": 0, "usd": 0.6, "unknown": 0}, "side": "favors"}]}},
    })
    .to_string();
    put(
        store,
        runner,
        kb::evidence(&report, std::slice::from_ref(&entry.id)).unwrap(),
    )
}

fn quest_file(dir: &Path, title: &str) -> String {
    let path = dir.join("quest.json");
    let spec = json!({
        "id": "tb4.fix-git.beat-reference",
        "version": 1,
        "season": {"id": "2026-q4", "opens_at": 0, "closes_at": 4_000_000_000_u64},
        "title": title,
        "objective": "Make a paired run pass fix-git for less than the reference.",
        "acceptance": {"rule": "kb-transfer", "task": "fix-git", "min_pass_rate": 1.0, "max_usd_per_run": 0.2},
        "reference": {"label": "Fable 5.1 low, cheapest winning run", "usd": 0.2, "seconds": null, "source": null},
        "award": {"author": 6, "runner": 4},
    });
    std::fs::write(&path, spec.to_string()).unwrap();
    path.to_str().unwrap().to_string()
}

#[tokio::test]
async fn a_referee_publishes_awards_and_revokes_and_a_reader_derives_xp() {
    let (url, store) = relay().await;
    let home = scratch("xp-home");
    let key = home.join("nostr/referee-key");
    let (author, runner) = (signer(1), signer(2));
    let evidence = completion(&store, &author, &runner);

    // A quest version, published once; the same version with other content
    // is refused.
    let file = quest_file(&home, "Beat the reference on fix-git");
    let publish = options(&[&file, "--relay", &url]);
    assert_eq!(quest(&publish, &key).await.unwrap(), 0);
    assert_eq!(quest(&publish, &key).await.unwrap(), 0);
    assert_eq!(count(&store, xp::QUEST_KIND), 1);
    let file = quest_file(&home, "Beat the reference on fix-git, reworded");
    assert_eq!(
        quest(&options(&[&file, "--relay", &url]), &key)
            .await
            .unwrap(),
        1
    );
    let me = remote::own_pubkey(&key).unwrap();

    // The award, with a label; a second award for the version is refused.
    let accept = options(&[
        "--relay",
        &url,
        "--quest",
        "tb4.fix-git.beat-reference@1",
        "--evidence",
        &evidence.id,
        "--label",
        "beat-reference",
    ]);
    assert_eq!(award(&accept, &key).await.unwrap(), 0);
    assert_eq!(count(&store, xp::AWARD_KIND), 1);
    assert_eq!(count(&store, xp::LABEL_KIND), 1);
    assert_eq!(award(&accept, &key).await.unwrap(), 1);
    assert_eq!(count(&store, xp::AWARD_KIND), 1);

    // A reader that trusts the referee.
    let trust = XpTrust {
        referees: BTreeSet::from([me.clone()]),
        runners: BTreeSet::new(),
    };
    let read = options(&["--relay", &url]);
    assert_eq!(ledger(&read, &key, &trust).await.unwrap(), 0);
    let derived = derive_from(&store, &trust);
    assert_eq!(derived.totals.get(author.pubkey()), Some(&6));
    assert_eq!(derived.totals.get(runner.pubkey()), Some(&4));
    // One that trusts no one sees nothing.
    assert!(derive_from(&store, &XpTrust::default()).totals.is_empty());

    // Revoked: gone from the ledger, and the version can be awarded again.
    let award_id = store
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.kind == xp::AWARD_KIND)
        .unwrap()
        .id
        .clone();
    let revoke_it = options(&[&award_id, "--relay", &url, "--reason", "mislabeled runs"]);
    assert_eq!(revoke(&revoke_it, &key).await.unwrap(), 0);
    assert!(derive_from(&store, &trust).totals.is_empty());
    assert_eq!(award(&accept, &key).await.unwrap(), 0);
    assert_eq!(derive_from(&store, &trust).totals.values().sum::<u64>(), 10);
}

#[tokio::test]
async fn a_referee_refuses_self_evidence() {
    let (url, store) = relay().await;
    let home = scratch("xp-self");
    let key = home.join("referee-key");
    let author = signer(1);
    let evidence = completion(&store, &author, &author);
    let file = quest_file(&home, "Beat the reference on fix-git");
    assert_eq!(
        quest(&options(&[&file, "--relay", &url]), &key)
            .await
            .unwrap(),
        0
    );
    let accept = options(&[
        "--relay",
        &url,
        "--quest",
        "tb4.fix-git.beat-reference@1",
        "--evidence",
        &evidence.id,
    ]);
    assert_eq!(award(&accept, &key).await.unwrap(), 1);
    assert_eq!(count(&store, xp::AWARD_KIND), 0);
}

#[tokio::test]
async fn the_ledger_needs_a_trusted_referee_and_a_relay() {
    let key = scratch("xp-none").join("key");
    let error = ledger(
        &options(&["--relay", "ws://127.0.0.1:1"]),
        &key,
        &XpTrust::default(),
    )
    .await
    .unwrap_err();
    assert!(error.contains("--referee"));
    let error = quest(&options(&["quest.json"]), &key).await.unwrap_err();
    assert!(error.contains("--relay"));
    assert!(!key.exists());
}

fn derive_from(store: &Store, trust: &XpTrust) -> Ledger {
    ledger_xp::derive(&store.lock().unwrap().clone(), trust)
}
