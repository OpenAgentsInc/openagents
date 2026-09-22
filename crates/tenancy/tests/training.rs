//! The tenant-training contract end to end: corpus checks, leakage and
//! group rules, headroom verdicts, recipe freezes, the trial ledger,
//! the candidate seal, and the retention delete.

use serde_json::{Value, json};
use tenancy::training::{
    Book, Corpus, CorpusFault, HeadroomFault, RECIPE_SCHEMA, Role, SealFault, TRIAL_SCHEMA,
    TrialFault, Trouble, Verdict, assess_headroom,
};

fn item(id: &str, group: &str, partition: &str, text: &str) -> Value {
    json!({
        "id": id,
        "group": group,
        "partition": partition,
        "state": {"text": text},
        "label": "yes",
        "label_source": "author",
        "label_rule": "the label is the label",
        "provenance": {
            "source": "tenant export",
            "license": "tenant-owned",
            "permission": "grant-training"
        }
    })
}

fn corpus(items: Vec<Value>) -> Value {
    json!({
        "v": "openagents.tenant_training.corpus.v1",
        "workspace": "ws_t",
        "name": "corpus-one",
        "created": "2026-09-24",
        "retention": {"days": 30, "access": "owner", "artifacts": "digests-only"},
        "digest": "",
        "items": items
    })
}

fn eligible() -> Value {
    corpus(vec![
        item("t1", "g1", "training", "alpha bravo charlie"),
        item("t2", "g2", "training", "delta echo foxtrot"),
        item("c1", "g3", "calibration", "golf hotel india"),
        item("d1", "g4", "development", "juliett kilo lima"),
        item("d2", "g5", "development", "mike november oscar"),
        item("d3", "g6", "development", "papa quebec romeo"),
        item("d4", "g7", "development", "sierra tango uniform"),
        item("l1", "g8", "locked", "victor whiskey xray"),
    ])
}

fn recipe() -> Value {
    json!({
        "v": RECIPE_SCHEMA,
        "name": "recipe-one",
        "created": "2026-09-24",
        "base_model": {"id": "base-1", "signature": format!("sha256:{}", "a".repeat(64))},
        "adapter": {"kind": "lora", "rank": 8, "alpha": 16, "targets": ["q_proj"], "epochs": 3, "learning_rate": "2e-4"},
        "head": {"kind": "pointer", "dp": 256},
        "seeds": [11],
        "trials_max": 2,
        "budget": {"max_train_items": 10, "max_steps": 100, "max_seconds": 600},
        "metric": {"statistic": "accuracy", "baseline": 0.5, "min_margin": 0.1},
        "reject_rules": ["no locked reads"],
        "transfer_controls": {"serving": "candidate-only", "publish": "digests-only"}
    })
}

fn sig(letter: char) -> String {
    format!("sha256:{}", letter.to_string().repeat(64))
}

#[test]
fn an_eligible_corpus_checks_and_its_digest_binds_the_items() {
    let checked = Corpus::check(&eligible().to_string()).expect("an eligible corpus checks");
    assert!(checked.digest.starts_with("sha256:"));
    assert_eq!(checked.counts()[&Role::Training], 2);
    assert_eq!(checked.items_in(Role::Development).len(), 4);
    // A label edited under the same digest is a different corpus.
    let mut tampered = eligible();
    tampered["items"][0]["label"] = json!("no");
    tampered["digest"] = json!(checked.digest);
    let fault = Corpus::check(&tampered.to_string()).unwrap_err();
    assert!(matches!(fault, CorpusFault::Tampered { .. }), "{fault}");
}

#[test]
fn a_corpus_without_training_is_not_a_training_corpus() {
    let mut doc = eligible();
    doc["items"] = json!(
        doc["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["partition"] != "training")
            .cloned()
            .collect::<Vec<_>>()
    );
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(matches!(fault, CorpusFault::NoTraining), "{fault}");
}

#[test]
fn provenance_and_permission_are_not_optional() {
    let mut doc = eligible();
    doc["items"][0]["provenance"]["license"] = json!("");
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(
        matches!(fault, CorpusFault::Undocumented { ref id, .. } if id == "t1"),
        "{fault}"
    );
}

#[test]
fn a_group_may_not_span_partitions() {
    let mut doc = eligible();
    doc["items"][3]["group"] = json!("g1"); // d1 joins a training group
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(
        matches!(fault, CorpusFault::GroupSpills { ref group, .. } if group == "g1"),
        "{fault}"
    );
}

#[test]
fn exact_and_paraphrased_duplicates_leak_across_partitions() {
    let mut doc = eligible();
    doc["items"][3]["state"] = json!({"text": "alpha bravo charlie"}); // d1 == t1
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(
        matches!(fault, CorpusFault::Leak(ref leak) if leak.kind == "exact"),
        "{fault}"
    );

    let mut doc = eligible();
    // 4 of 5 tokens shared: a paraphrase, not an exact copy.
    doc["items"][0]["state"] = json!({"text": "alpha bravo charlie delta"});
    doc["items"][3]["state"] = json!({"text": "alpha bravo charlie delta echo"});
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(
        matches!(fault, CorpusFault::Leak(ref leak) if leak.kind == "near"),
        "{fault}"
    );
}

#[test]
fn a_model_label_is_draft_not_ground_truth() {
    let mut doc = eligible();
    doc["items"][0]["label_source"] = json!("model");
    let fault = Corpus::check(&doc.to_string()).unwrap_err();
    assert!(
        matches!(fault, CorpusFault::UnconfirmedLabel { .. }),
        "{fault}"
    );
    doc["items"][0]["annotations"]["confirmed"] = json!(true);
    Corpus::check(&doc.to_string()).expect("a confirmed model label checks");
}

#[test]
fn locked_items_are_not_readable() {
    let checked = Corpus::check(&eligible().to_string()).unwrap();
    assert_eq!(checked.items_in(Role::Locked).len(), 0);
    assert_eq!(checked.counts()[&Role::Locked], 1);
}

#[test]
fn headroom_separates_causes_and_refuses_incomplete_scores() {
    let mut doc = eligible();
    doc["items"][4]["annotations"]["ambiguous"] = json!(true); // d2
    let checked = Corpus::check(&doc.to_string()).unwrap();
    let scores = json!({
        "v": "openagents.tenant_training.scores.v1",
        "door": "baseline-1",
        "measured": "2026-09-24",
        "rows": [
            {"item_id": "d1", "predicted": "no", "correct": false},
            {"item_id": "d2", "predicted": "no", "correct": false},
            {"item_id": "d3", "predicted": "no", "correct": false},
            {"item_id": "d4", "predicted": "no", "correct": false}
        ]
    });
    let report = assess_headroom(&checked, &scores.to_string(), "now").unwrap();
    assert_eq!(report.verdict, Verdict::Train);
    assert_eq!(report.causes["model_addressable"], 3);
    assert_eq!(report.causes["ambiguous_question"], 1);

    // Under MIN_HEADROOM, a documented no-train is the recorded verdict.
    let sparse = json!({
        "v": "openagents.tenant_training.scores.v1",
        "door": "baseline-1",
        "measured": "2026-09-24",
        "rows": [
            {"item_id": "d1", "predicted": "yes", "correct": true},
            {"item_id": "d2", "predicted": "no", "correct": false},
            {"item_id": "d3", "predicted": "yes", "correct": true},
            {"item_id": "d4", "predicted": "yes", "correct": true}
        ]
    });
    let report = assess_headroom(&checked, &sparse.to_string(), "now").unwrap();
    assert_eq!(report.verdict, Verdict::NoTrain);

    // A scores file missing a development item is refused.
    let short = json!({
        "v": "openagents.tenant_training.scores.v1",
        "door": "baseline-1",
        "measured": "2026-09-24",
        "rows": [{"item_id": "d1", "predicted": "yes", "correct": true}]
    });
    let fault = assess_headroom(&checked, &short.to_string(), "now").unwrap_err();
    assert!(matches!(fault, HeadroomFault::Incomplete { .. }), "{fault}");
}

#[test]
fn the_full_flow_seals_a_candidate_and_delete_tombstones_the_corpus() {
    let dir = tempfile::tempdir().unwrap();
    let book = Book::open(dir.path()).unwrap();

    let corpus = book.register_corpus(&eligible().to_string()).unwrap();
    let recipe = book.freeze_recipe(&recipe().to_string()).unwrap();
    assert!(recipe.digest.starts_with("sha256:"));

    // A rejected trial is retained like any other.
    let rejected = json!({
        "v": TRIAL_SCHEMA,
        "recipe_digest": recipe.digest,
        "seed": 11,
        "params": {"rank": 8},
        "metrics": {"accuracy": 0.4},
        "artifacts": {},
        "outcome": "rejected",
        "reason": "below the margin",
        "recorded_at": "2026-09-24"
    });
    assert_eq!(book.record_trial(&rejected.to_string()).unwrap(), 1);

    let kept = json!({
        "v": TRIAL_SCHEMA,
        "recipe_digest": recipe.digest,
        "seed": 11,
        "params": {"rank": 8},
        "metrics": {"accuracy": 0.8},
        "artifacts": {"adapter": sig('b'), "head": sig('c'), "tokenizer": sig('d')},
        "outcome": "kept",
        "recorded_at": "2026-09-24"
    });
    assert_eq!(book.record_trial(&kept.to_string()).unwrap(), 2);

    // The recipe's trial cap holds — a third record is refused.
    let third = json!({
        "v": TRIAL_SCHEMA,
        "recipe_digest": recipe.digest,
        "seed": 11,
        "params": {},
        "metrics": {},
        "artifacts": {},
        "outcome": "failed",
        "recorded_at": "2026-09-24"
    });
    let fault = book.record_trial(&third.to_string()).unwrap_err();
    assert!(
        matches!(fault, Trouble::Trial(TrialFault::TrialCap { .. })),
        "{fault}"
    );

    // A foreign seed is refused before the cap is even read.
    let book2 = Book::open(dir.path()).unwrap();
    let foreign = json!({
        "v": TRIAL_SCHEMA,
        "recipe_digest": recipe.digest,
        "seed": 99,
        "params": {},
        "metrics": {},
        "artifacts": {},
        "outcome": "failed",
        "recorded_at": "2026-09-24"
    });
    let _ = book2.record_trial(&foreign.to_string()); // would be cap; check seed order
    // (seed check runs before cap check only when under cap — assert on
    // a fresh recipe with capacity)
    let mut recipe2 = self::recipe();
    recipe2["name"] = json!("recipe-two");
    recipe2["trials_max"] = json!(4);
    let recipe2 = book.freeze_recipe(&recipe2.to_string()).unwrap();
    let mut foreign = foreign.clone();
    foreign["recipe_digest"] = json!(recipe2.digest);
    let fault = book.record_trial(&foreign.to_string()).unwrap_err();
    assert!(
        matches!(fault, Trouble::Trial(TrialFault::ForeignSeed { .. })),
        "{fault}"
    );

    // The candidate seal binds the kept trial's artifacts.
    let candidate = json!({
        "v": "openagents.tenant_training.candidate.v1",
        "workspace": "ws_t",
        "name": "candidate-one",
        "created": "2026-09-24",
        "identities": {
            "code": {"tool": "demo", "version": "1"},
            "recipe_digest": recipe.digest,
            "corpus_digest": corpus.digest,
            "base_model": {"id": "base-1", "signature": sig('a')},
            "adapter": sig('b'),
            "head": sig('c'),
            "tokenizer": sig('d')
        },
        "evidence": {"trial": 2, "metrics": {"accuracy": 0.8}, "confirmation": "locked, unspent"},
        "retention": {"days": 30, "access": "owner", "artifacts": "digests-only"}
    });
    let sealed = book.seal_candidate(&candidate.to_string()).unwrap();
    assert!(sealed.signature.starts_with("sha256:"));
    let admission = sealed.admission_candidate();
    assert_eq!(admission.artifact_signature, sealed.signature);
    assert_eq!(admission.model, "base-1");
    assert_eq!(admission.adapter.as_deref(), Some("candidate-one"));

    // A seal naming artifacts the kept trial never produced is refused.
    let mut wrong = candidate.clone();
    wrong["name"] = json!("candidate-two");
    wrong["identities"]["adapter"] = json!(sig('e'));
    let fault = book.seal_candidate(&wrong.to_string()).unwrap_err();
    assert!(
        matches!(fault, Trouble::Seal(SealFault::Unknown { .. })),
        "{fault}"
    );

    // Retention: delete tombstones content, the digest still resolves.
    let tombstone = book
        .delete_corpus("corpus-one", "2026-09-25", "window")
        .unwrap();
    assert_eq!(tombstone.item_digests.len(), 8);
    let fault = book.corpus("corpus-one").unwrap_err();
    assert!(
        matches!(fault, Trouble::Corpus(CorpusFault::Deleted)),
        "{fault}"
    );
    // And the sealed candidate's corpus digest still resolves — the
    // tombstoned file keeps the recorded digest.
    let still = book.candidate("candidate-one").unwrap();
    assert_eq!(still.signature, sealed.signature);
}

#[test]
fn a_recipe_with_an_edit_after_freeze_is_tampered() {
    let dir = tempfile::tempdir().unwrap();
    let book = Book::open(dir.path()).unwrap();
    let recipe = book.freeze_recipe(&recipe().to_string()).unwrap();
    let mut text = serde_json::to_value(&recipe).unwrap();
    text["adapter"]["rank"] = json!(16);
    let fault = book.freeze_recipe(&text.to_string()).unwrap_err();
    assert!(matches!(fault, Trouble::Recipe(_)), "{fault}");
}
