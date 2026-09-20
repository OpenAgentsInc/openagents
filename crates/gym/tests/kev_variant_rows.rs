//! Reconciles the four Kev variant row sets openagents#9384 committed against
//! the pinned `support-v2-three-way` suite.
//!
//! The run predates the refusal-code contract this worktree adds — under the
//! envelope kev-serve then sent, a refusal would have reached the store as a
//! harness loss or nowhere, so coverage here is evidence of the record, not
//! of the classifier. What this file verifies is what the rows themselves
//! prove: every expected open item/split pair is present exactly once per
//! door, every row is internally consistent and pinned to the suite's
//! digest, the receipt chain verifies end to end, and each door's provenance
//! fields match the run that produced it. It reads the committed store and
//! writes nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gym::row::{LabelSource, Row};
use gym::store::Store;
use gym::suite::{Partition, support_v2_three_way};

/// The exact run timestamps and full published base signatures retained
/// for #9384. Later runs may append to the same store without changing
/// which historical evidence this check reconciles.
const DOORS: [(&str, &str, &str); 4] = [
    ("kev-0.5b", "2026-09-19T22:41:14Z", ""),
    (
        "kev-0.6b",
        "2026-09-19T22:42:33Z",
        "da87bfb608c14b7cf20ba1ce41287e8de496c0cd",
    ),
    (
        "kev-4b",
        "2026-09-19T22:44:49Z",
        "906bfd4b4dc7f14ee4320094d8b41684abff8539",
    ),
    (
        "kev-8b",
        "2026-09-19T22:51:11Z",
        "49e3418fbbbca6ecbdf9608b4d22e5a407081db4",
    ),
];

/// The committed result store the run appended to.
fn store_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("results/support-v2-three-way.jsonl")
}

#[test]
fn kev_variant_rows_reconcile_against_the_pinned_suite() {
    let suite = support_v2_three_way().expect("the committed suite loads and verifies");
    let store = Store::at(store_path());
    // Receipt integrity: the chain over the whole file verifies, or this
    // fails before any coverage claim is read from it.
    let values = store
        .verified_rows()
        .expect("the committed store's receipt chain verifies");
    let rows: Vec<Row> = values
        .iter()
        .map(|value| serde_json::from_value::<Row>(value.clone()).expect("a row parses"))
        .collect();

    // The open items: everything but the locked partition.
    let expected: BTreeSet<(String, String)> = suite
        .items
        .iter()
        .filter(|item| item.partition != Partition::Locked)
        .map(|item| (item.id.clone(), item.partition.as_str().to_string()))
        .collect();
    assert_eq!(expected.len(), 157, "the suite holds 157 open items");
    let by_id: std::collections::HashMap<&str, &gym::suite::Item> = suite
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    let gate = gym::gate::load("probability-v1").expect("the pinned gate loads");

    for (door, recorded_at, signature) in DOORS {
        let door_rows: Vec<&Row> = rows
            .iter()
            .filter(|row| row.door == door && row.recorded_at == recorded_at)
            .collect();
        assert_eq!(door_rows.len(), 157, "{door}: one row per open item");

        // #9384's rows are that pass's canonical trials: unpermuted, and
        // written before rows pinned the question set they were served.
        for row in &door_rows {
            assert!(
                row.permutation.is_none(),
                "{door} {}: a permuted trial is not part of this run",
                row.item_id
            );
            assert!(
                row.question_set.is_none() && row.question_digest.is_none(),
                "{door} {}: this run predates question-set pinning",
                row.item_id
            );
        }

        // Coverage: exactly the suite's open item/split pairs — no
        // duplicates, none missing, none unexpected.
        let pairs: BTreeSet<(String, String)> = door_rows
            .iter()
            .map(|row| (row.item_id.clone(), row.split.clone()))
            .collect();
        assert_eq!(pairs.len(), door_rows.len(), "{door}: duplicate rows");
        assert_eq!(
            pairs, expected,
            "{door}: the recorded pairs are the suite's open items"
        );

        // One pass: every row of a door shares one recorded_at.
        let recorded: BTreeSet<&str> = door_rows
            .iter()
            .map(|row| row.recorded_at.as_str())
            .collect();
        assert_eq!(recorded.len(), 1, "{door}: the rows are one run");

        for row in &door_rows {
            row.check()
                .unwrap_or_else(|e| panic!("{door} {}: {e}", row.item_id));
            assert_eq!(row.suite, suite.name, "{door} {}", row.item_id);
            assert_eq!(row.suite_digest, suite.digest, "{door} {}", row.item_id);

            // Every row scored: nothing refused and nothing missing, which
            // is the coverage evidence this run's numbers rest on.
            assert!(
                row.is_scored(),
                "{door} {}: the row is answered, not refused",
                row.item_id
            );
            assert!(row.refusal.is_none(), "{door} {}", row.item_id);
            assert_eq!(
                row.label_source,
                LabelSource::Author,
                "{door} {}",
                row.item_id
            );

            // Run and model provenance.
            let identity = &row.door_identity;
            assert_eq!(identity.model, door, "{door} {}", row.item_id);
            assert_eq!(
                identity.verified,
                !signature.is_empty(),
                "{door} {}",
                row.item_id
            );
            assert_eq!(
                identity.base_model_signature, signature,
                "{door} {}: signature {:?} does not match the run's checkpoint",
                row.item_id, identity.base_model_signature
            );
            assert_eq!(row.estimator, "unreported", "{door} {}", row.item_id);
            assert!(row.samples.is_none(), "{door} {}", row.item_id);
            assert!(row.seed_base.is_none(), "{door} {}", row.item_id);
            assert_eq!(
                row.gate_id.as_deref(),
                Some("probability-v1"),
                "{door} {}",
                row.item_id
            );
            // The rows carry the digest the earlier encoding produced.
            // `previously` carries it forward, so it still attributes to
            // this rule rather than passing only by equality with today's
            // encoding.
            assert!(
                row.gate_digest
                    .as_deref()
                    .is_some_and(|recorded| gate.has_digest(recorded)),
                "{door} {}: the recorded digest is not one this rule attributes",
                row.item_id
            );

            // The row names the same item the suite does.
            let item = by_id[row.item_id.as_str()];
            assert_eq!(row.family, item.family, "{door} {}", row.item_id);
            assert_eq!(row.split, item.partition.as_str(), "{door} {}", row.item_id);
        }
    }
}
