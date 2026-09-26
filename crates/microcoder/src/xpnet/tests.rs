//! Quests, awards, revocations, and the ledger against the in-process
//! relay the `kb` tests use. The referee key is created in a scratch
//! directory by the code under test; the author and runner keys are
//! throwaway keys from small numbers.

use std::path::PathBuf;

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
            "id": kb::qualified_id(&entry.pubkey, "git.reflog-recovery"),
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

/// `kb` options.
fn kb_options(words: &[&str]) -> knowledge::cli::Options {
    let args: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
    knowledge::cli::parse(&args).unwrap()
}

/// A run record on the task its name gives, showing `shown`: entry IDs
/// with the digest each run's prompt showed.
fn run_record(dir: &Path, name: &str, reward: f64, shown: &[(&str, &str)]) {
    let at = dir.join(name);
    std::fs::create_dir_all(&at).unwrap();
    let knowledge: Vec<Value> = shown
        .iter()
        .map(|(id, digest)| json!({"id": id, "digest": digest, "kept_steps": 1, "expanded_steps": 1}))
        .collect();
    let summary = json!({"task": knowledge::evidence::task_of(name), "model": "m", "reward": reward,
        "outcome": {"model_usd": 0.1, "jev_usd": 0.0, "embedding_usd": 0.0, "knowledge": knowledge}});
    std::fs::write(at.join("summary.json"), summary.to_string()).unwrap();
}

/// The entry the author publishes: written from runs of `in-sample`.
fn authored() -> String {
    DOCUMENT.replace(
        "written_from: [reference]",
        "written_from: [in-sample-1790000000]",
    )
}

/// The three parties' keys, the author's published entry, the runner's
/// synced cache and run records, and the paths the commands use.
struct Transfer {
    url: String,
    store: Store,
    home: PathBuf,
    author_key: PathBuf,
    runner_key: PathBuf,
    referee_key: PathBuf,
    author: String,
    runner: String,
    cache: String,
    runs: String,
    reports: String,
}

impl Transfer {
    /// `publish-evidence` as the runner, about the author's entry, on `url`.
    fn runner_evidence(&self, url: &str, author: &str) -> knowledge::cli::Options {
        kb_options(&[
            "--relay",
            url,
            "--author",
            author,
            "--remote",
            &self.cache,
            "--runs",
            &self.runs,
            "--evidence-dir",
            &self.reports,
            "git.reflog-recovery",
        ])
    }

    fn evidence_by(&self, pubkey: &str) -> Vec<Event> {
        self.store
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.kind == kb::EVIDENCE_KIND && e.pubkey == pubkey)
            .cloned()
            .collect()
    }

    async fn publish_quest(&self, name: &str, id: &str, task: &str) {
        let path = self.home.join(name);
        let spec = json!({
            "id": id, "version": 1,
            "season": {"id": "2026-q4", "opens_at": 0, "closes_at": 4_000_000_000_u64},
            "title": format!("Beat the reference on {task}"),
            "objective": format!("Make a paired run pass {task} for less than the reference."),
            "acceptance": {"rule": "kb-transfer", "task": task, "min_pass_rate": 1.0, "max_usd_per_run": 0.2},
            "reference": null,
            "award": {"author": 6, "runner": 4},
        });
        std::fs::write(&path, spec.to_string()).unwrap();
        let publish = options(&[path.to_str().unwrap(), "--relay", &self.url]);
        assert_eq!(quest(&publish, &self.referee_key).await.unwrap(), 0);
    }

    async fn award(&self, address: &str, evidence: &str) -> u8 {
        let accept = options(&[
            "--relay",
            &self.url,
            "--quest",
            address,
            "--evidence",
            evidence,
        ]);
        award(&accept, &self.referee_key).await.unwrap()
    }
}

/// Author A publishes an entry; runner B syncs it and records runs that
/// showed its exact version on two tasks it wasn't written from, on the
/// task it was written from, and runs that showed another version.
async fn transfer(name: &str) -> Transfer {
    let (url, store) = relay().await;
    let home = scratch(name);
    let author_key = home.join("author/knowledge-key");
    let runner_key = home.join("runner/knowledge-key");
    let referee_key = home.join("referee/referee-key");
    let entries = home.join("entries");
    let corpus = home.join("corpus");
    std::fs::create_dir_all(&entries).unwrap();
    std::fs::create_dir_all(&corpus).unwrap();
    let document = authored();
    std::fs::write(entries.join("git.reflog-recovery.md"), &document).unwrap();

    let publish_entry = kb_options(&["--dir", entries.to_str().unwrap(), "--relay", &url]);
    assert_eq!(
        crate::kbnet::publish(&publish_entry, &author_key)
            .await
            .unwrap(),
        0
    );
    let author = remote::own_pubkey(&author_key).unwrap();

    let cache = home.join("runner/remote");
    let pull = kb_options(&[
        "--relay",
        &url,
        "--author",
        &npub(&author),
        "--corpus",
        corpus.to_str().unwrap(),
    ]);
    assert_eq!(
        crate::kbnet::sync(&pull, &runner_key, &cache)
            .await
            .unwrap(),
        0
    );
    let runner = remote::own_pubkey(&runner_key).unwrap();

    let runs = home.join("runner/runs");
    let digest = knowledge::digest(document.as_bytes());
    let id = "git.reflog-recovery";
    let shown: &[(&str, &str)] = &[(id, &digest)];
    let other: &[(&str, &str)] = &[(id, "sha256:0000")];
    for (run, reward, used) in [
        ("fix-git-1790000001", 1.0, shown),
        ("fix-git-1790000002", 1.0, shown),
        ("fix-git-1790000003", 0.0, &[][..]),
        ("other-task-1790000004", 1.0, shown),
        ("other-task-1790000005", 0.0, &[][..]),
        // In sample: written from this task.
        ("in-sample-1790000006", 1.0, shown),
        ("in-sample-1790000007", 0.0, &[][..]),
        // Another version: counted in neither arm.
        ("fix-git-1790000008", 0.0, other),
    ] {
        run_record(&runs, run, reward, used);
    }
    let path = |p: PathBuf| p.to_str().unwrap().to_string();
    Transfer {
        url,
        store,
        author_key,
        runner_key,
        referee_key,
        author,
        runner,
        cache: path(cache),
        runs: path(runs),
        reports: path(home.join("runner/evidence")),
        home,
    }
}

#[tokio::test]
async fn a_runner_completes_a_quest_with_another_authors_synced_entry() {
    let t = transfer("xp-transfer").await;
    let author = npub(&t.author);
    assert_eq!(
        crate::kbnet::publish_evidence(&t.runner_evidence(&t.url, &author), &t.runner_key)
            .await
            .unwrap(),
        0
    );
    let published = t.evidence_by(&t.runner);
    assert_eq!(published.len(), 1);
    let evidence = &published[0];
    let parsed = kb::parse_evidence(evidence).unwrap();
    let entry_event = parsed.subject.event.clone().unwrap();
    assert_eq!(entry_event.pubkey, t.author);
    assert_eq!(parsed.entries, std::slice::from_ref(&entry_event.id));
    assert_eq!(
        parsed.subject.id,
        kb::qualified_id(&t.author, "git.reflog-recovery")
    );
    let report: Value = serde_json::from_str(&parsed.report_bytes).unwrap();
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["meta"]["kb"]["other_version_runs"], 1);
    let tasks: Vec<&str> = report["meta"]["kb"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["task"].as_str().unwrap())
        .collect();
    assert_eq!(tasks, ["fix-git", "other-task"]);

    // The referee's quest on fix-git accepts it.
    t.publish_quest("quest.json", "tb4.fix-git.beat-reference", "fix-git")
        .await;
    assert_eq!(
        t.award("tb4.fix-git.beat-reference@1", &evidence.id).await,
        0
    );
    assert_eq!(count(&t.store, xp::AWARD_KIND), 1);

    // The ledger credits the author and the runner.
    let referee = remote::own_pubkey(&t.referee_key).unwrap();
    let trust = XpTrust {
        referees: BTreeSet::from([referee]),
        runners: BTreeSet::new(),
    };
    assert_eq!(
        ledger(&options(&["--relay", &t.url]), &t.runner_key, &trust)
            .await
            .unwrap(),
        0
    );
    let derived = derive_from(&t.store, &trust);
    assert_eq!(derived.totals.get(&t.author), Some(&6));
    assert_eq!(derived.totals.get(&t.runner), Some(&4));
    assert!(derived.refused.is_empty(), "{:?}", derived.refused);
}

#[tokio::test]
async fn transfer_evidence_is_refused_for_self_in_sample_and_other_bytes() {
    let t = transfer("xp-transfer-refused").await;

    // The runner names its own key as the author.
    let error =
        crate::kbnet::publish_evidence(&t.runner_evidence(&t.url, &npub(&t.runner)), &t.runner_key)
            .await
            .unwrap_err();
    assert!(error.contains("your own key"), "{error}");

    // The author's own evidence, from the same runs: published, but the
    // referee doesn't accept it.
    let own = kb_options(&[
        "--dir",
        t.home.join("entries").to_str().unwrap(),
        "--relay",
        &t.url,
        "--runs",
        &t.runs,
        "--evidence-dir",
        &t.reports,
        "git.reflog-recovery",
    ]);
    assert_eq!(
        crate::kbnet::publish_evidence(&own, &t.author_key)
            .await
            .unwrap(),
        0
    );
    let own_evidence = t.evidence_by(&t.author);
    assert_eq!(own_evidence.len(), 1);
    t.publish_quest("quest.json", "tb4.fix-git.beat-reference", "fix-git")
        .await;
    assert_eq!(
        t.award("tb4.fix-git.beat-reference@1", &own_evidence[0].id)
            .await,
        1
    );

    // The runner's evidence is accepted on fix-git, but not on the task the
    // entry was written from.
    let author = npub(&t.author);
    assert_eq!(
        crate::kbnet::publish_evidence(&t.runner_evidence(&t.url, &author), &t.runner_key)
            .await
            .unwrap(),
        0
    );
    let evidence = t.evidence_by(&t.runner)[0].id.clone();
    t.publish_quest(
        "in-sample.json",
        "tb4.in-sample.beat-reference",
        "in-sample",
    )
    .await;
    assert_eq!(
        t.award("tb4.in-sample.beat-reference@1", &evidence).await,
        1
    );
    assert_eq!(count(&t.store, xp::AWARD_KIND), 0);

    // A relay without the author's version with this digest: nothing to
    // cite, so nothing is published there.
    let (other_url, other_store) = relay().await;
    assert_eq!(
        crate::kbnet::publish_evidence(&t.runner_evidence(&other_url, &author), &t.runner_key)
            .await
            .unwrap(),
        1
    );
    assert_eq!(count(&other_store, kb::EVIDENCE_KIND), 0);

    // Evidence that cites the author's entry event but measured other
    // bytes: the referee refuses it.
    let parsed = kb::parse_evidence(&t.evidence_by(&t.runner)[0]).unwrap();
    let mut report: Value = serde_json::from_str(&parsed.report_bytes).unwrap();
    report["subject"]["definition"]["artifact"] =
        kb::document_artifact(&authored().replace("Body.", "Another body."));
    let runner =
        RelaySigner::from_secret_hex(std::fs::read_to_string(&t.runner_key).unwrap().trim())
            .unwrap();
    let entry_id = parsed.subject.event.unwrap().id;
    let tampered = put(
        &t.store,
        &runner,
        kb::evidence(&report.to_string(), std::slice::from_ref(&entry_id)).unwrap(),
    );
    assert_eq!(
        t.award("tb4.fix-git.beat-reference@1", &tampered.id).await,
        1
    );
    assert_eq!(count(&t.store, xp::AWARD_KIND), 0);
    assert_eq!(t.award("tb4.fix-git.beat-reference@1", &evidence).await, 0);
}

fn derive_from(store: &Store, trust: &XpTrust) -> Ledger {
    ledger_xp::derive(&store.lock().unwrap().clone(), trust)
}
