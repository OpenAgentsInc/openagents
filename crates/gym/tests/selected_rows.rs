//! New selected-answer rows append without rewriting historical receipts.

use gym::row::{LEGACY_SCHEMA, Row, SCHEMA};
use gym::store::Store;
use indexmap::IndexMap;

#[test]
fn a_v2_row_extends_a_v1_chain_without_rewriting_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.jsonl");
    let store = Store::at(&path);
    let distribution: IndexMap<String, f64> = [("no".to_string(), 0.75), ("yes".to_string(), 0.25)]
        .into_iter()
        .collect();
    let mut old =
        Row::new("fixture", "suite:fixture", "old", "test").scored(distribution.clone(), true);
    old.schema = LEGACY_SCHEMA.to_string();
    old.selected = None;
    let sealed_old = store.append(&old).unwrap();
    let bytes_before = std::fs::read(&path).unwrap();

    let new = Row::new("fixture", "suite:fixture", "new", "test").scored_as(
        distribution,
        Some("yes".to_string()),
        false,
    );
    new.check().unwrap();
    assert_eq!(new.schema, SCHEMA);
    assert_eq!(new.raw_top, Some(0.25));
    store.append(&new).unwrap();

    let bytes_after = std::fs::read(&path).unwrap();
    assert!(bytes_after.starts_with(&bytes_before));
    let rows = store.verified_rows().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], sealed_old);
    assert!(rows[0].get("selected").is_none());
    assert_eq!(rows[0]["schema"], LEGACY_SCHEMA);
    assert_eq!(rows[1]["schema"], SCHEMA);
    assert_eq!(rows[1]["selected"], "yes");
    assert_eq!(rows[1]["previous_receipt"], rows[0]["receipt"]);
}
