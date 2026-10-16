//! Edge registry-core.test.ts vectors against the server's pinned merge core.
use coder_sync::server::registry::{valid_hlc, validate_op};
use serde_json::{Value, json};
use zeron_doc::{RegistryRow, RowOp, apply_op, encode_hlc, row_to_seed_op};
fn clock(ms: i64, device: &str) -> String {
    encode_hlc(ms, 0, device)
}
fn op(kind: &str, id: &str, action: &str, ms: i64, device: &str, set: Option<Value>) -> RowOp {
    let mut value = json!({"kind":kind,"id":id,"op":action,"hlc":clock(ms,device)});
    if let Some(set) = set {
        value["set"] = set;
    }
    validate_op(&value).unwrap()
}
fn create(ms: i64) -> RowOp {
    op(
        "chats",
        "chat-1",
        "upsert",
        ms,
        "dev-a",
        Some(json!({"title":"hello","archived":false})),
    )
}
fn apply(row: Option<&RegistryRow>, op: &RowOp) -> RegistryRow {
    let (row, changed) = apply_op(row, op);
    assert!(changed);
    row.unwrap()
}
#[test]
fn hlc_orders_timestamp_counter_and_device() {
    assert!(clock(2, "a") > clock(1, "a"));
    assert!(encode_hlc(1, 2, "a") > encode_hlc(1, 1, "a"));
    assert!(encode_hlc(1, 1, "b") > encode_hlc(1, 1, "a"));
    assert!(clock(10000, "a") > clock(999, "a"));
    assert!(valid_hlc(&clock(1, "a")));
    for invalid in [
        "",
        "not-a-clock",
        "0000000000001-000001-",
        "0000000000001-000001-a/",
    ] {
        assert!(!valid_hlc(invalid));
    }
}
#[test]
fn creates_only_by_upsert_and_replays_are_noops() {
    let row = apply(None, &create(1000));
    assert_eq!(row.fields["title"], "hello");
    assert_eq!(row.clocks["title"], clock(1000, "dev-a"));
    assert!(!row.deleted);
    let update = op(
        "chats",
        "chat-1",
        "update",
        3000,
        "dev-a",
        Some(json!({"title":"early"})),
    );
    assert_eq!(apply_op(None, &update), (None, false));
    assert!(!apply_op(Some(&row), &create(1000)).1);
}
#[test]
fn per_field_lww_keeps_untouched_clocks_and_device_ties_converge() {
    let base = apply(None, &create(1000));
    assert!(!apply_op(Some(&base), &create(500)).1);
    let a = op(
        "chats",
        "chat-1",
        "update",
        5000,
        "dev-a",
        Some(json!({"title":"A"})),
    );
    let b = op(
        "chats",
        "chat-1",
        "update",
        5000,
        "dev-b",
        Some(json!({"title":"B"})),
    );
    let ab = apply_op(Some(&apply(Some(&base), &a)), &b).0.unwrap();
    let ba = apply_op(Some(&apply(Some(&base), &b)), &a).0.unwrap();
    assert_eq!(ab, ba);
    assert_eq!(ab.fields["title"], "B");
    assert_eq!(ab.clocks["archived"], clock(1000, "dev-a"));
}
#[test]
fn null_deletes_fields_but_retains_their_clock() {
    let row = apply(
        None,
        &op(
            "chats",
            "chat-1",
            "upsert",
            1000,
            "dev-a",
            Some(json!({"title":"x","name":"y"})),
        ),
    );
    let row = apply(
        Some(&row),
        &op(
            "chats",
            "chat-1",
            "update",
            2000,
            "dev-a",
            Some(json!({"name":null})),
        ),
    );
    assert_eq!(
        row.fields,
        json!({"title":"x"})
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    );
    assert_eq!(row.clocks["name"], clock(2000, "dev-a"));
    assert!(
        !apply_op(
            Some(&row),
            &op(
                "chats",
                "chat-1",
                "update",
                1500,
                "dev-a",
                Some(json!({"name":"zombie"}))
            )
        )
        .1
    );
}
#[test]
fn delete_and_revive_respect_whole_row_causality() {
    let row = apply(None, &create(1000));
    assert!(
        !apply_op(
            Some(&row),
            &op("chats", "chat-1", "delete", 500, "dev-a", None)
        )
        .1
    );
    let gone = apply(
        Some(&row),
        &op("chats", "chat-1", "delete", 2000, "dev-a", None),
    );
    assert!(gone.deleted);
    assert!(gone.fields.is_empty() && gone.clocks.is_empty());
    assert!(
        !apply_op(
            Some(&gone),
            &op(
                "chats",
                "chat-1",
                "update",
                3000,
                "dev-a",
                Some(json!({"title":"ghost"}))
            )
        )
        .1
    );
    assert!(!apply_op(Some(&gone), &create(1500)).1);
    let revived = apply(
        Some(&gone),
        &op(
            "chats",
            "chat-1",
            "upsert",
            4000,
            "dev-a",
            Some(json!({"title":"back"})),
        ),
    );
    assert!(!revived.deleted);
    assert_eq!(revived.fields.len(), 1);
    assert_eq!(revived.fields["title"], "back");
}
#[test]
fn delete_missing_plants_a_guard_and_seed_roundtrips_tombstones() {
    let gone = apply(None, &op("spaces", "sp-1", "delete", 7000, "dev-a", None));
    assert!(gone.deleted);
    assert!(
        !apply_op(
            Some(&gone),
            &op(
                "spaces",
                "sp-1",
                "upsert",
                500,
                "dev-a",
                Some(json!({"name":"late"}))
            )
        )
        .1
    );
    assert_eq!(apply(None, &row_to_seed_op(&gone)), gone);
}
#[test]
fn seed_preserves_field_causality_and_newest_clock() {
    let row = apply(
        None,
        &op(
            "chats",
            "chat-1",
            "upsert",
            1000,
            "dev-a",
            Some(json!({"title":"old","status":"idle"})),
        ),
    );
    let row = apply(
        Some(&row),
        &op(
            "chats",
            "chat-1",
            "update",
            9000,
            "dev-a",
            Some(json!({"status":"working"})),
        ),
    );
    let seed = row_to_seed_op(&row);
    assert_eq!(seed.hlc, clock(9000, "dev-a"));
    let seeded = apply(None, &seed);
    assert_eq!(seeded, row);
    assert!(
        !apply_op(
            Some(&seeded),
            &op(
                "chats",
                "chat-1",
                "update",
                5000,
                "dev-a",
                Some(json!({"status":"errored"}))
            )
        )
        .1
    );
    assert_eq!(
        row_to_seed_op(&apply(None, &op("x", "y", "delete", 9, "dev-a", None))).hlc,
        clock(9, "dev-a")
    );
}
#[test]
fn all_six_causally_valid_arrival_orders_converge() {
    let base = apply(None, &create(1000));
    let races = [
        op(
            "chats",
            "chat-1",
            "update",
            3000,
            "dev-b",
            Some(json!({"title":"renamed"})),
        ),
        op(
            "chats",
            "chat-1",
            "update",
            2000,
            "dev-c",
            Some(json!({"archived":true})),
        ),
        op(
            "chats",
            "chat-1",
            "upsert",
            2500,
            "dev-d",
            Some(json!({"title":"other","cwd":"/fixture"})),
        ),
    ];
    let mut outcomes = vec![];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut row = base.clone();
        for i in order {
            row = apply_op(Some(&row), &races[i]).0.unwrap();
        }
        outcomes.push(row);
    }
    for row in &outcomes {
        assert_eq!(row, &outcomes[0]);
    }
    assert_eq!(outcomes[0].fields["title"], "renamed");
    assert_eq!(outcomes[0].fields["archived"], true);
    assert_eq!(outcomes[0].fields["cwd"], "/fixture");
}
#[test]
fn validation_and_sidebar_pin_vectors() {
    let valid = serde_json::to_value(create(1000)).unwrap();
    assert!(validate_op(&valid).is_ok());
    for (key, value) in [
        ("kind", json!("Nope Kind")),
        ("id", json!("")),
        ("op", json!("merge")),
        ("hlc", json!("junk")),
        ("set", Value::Null),
        ("set", json!({"bad field!":1})),
        ("clocks", json!({"title":"junk"})),
        ("clocks", Value::Null),
        ("set", json!({"blob":"x".repeat(20000)})),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert!(validate_op(&bad).is_err(), "{bad}");
    }
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("set");
    assert!(validate_op(&missing).is_err());
    let mut delete = valid.clone();
    delete["op"] = json!("delete");
    assert!(validate_op(&delete).is_err());
    for fields in [json!({"pinned":false}), json!({"orderKey":"8ab8"})] {
        let mut pin = valid.clone();
        pin["kind"] = json!("sidebarPins");
        pin["set"] = fields;
        assert!(validate_op(&pin).is_ok());
    }
    for fields in [
        json!({"orderKey":"80"}),
        json!({"orderKey":""}),
        json!({"orderKey":"G"}),
        json!({"pinned":"true"}),
        json!({"other":true}),
    ] {
        let mut pin = valid.clone();
        pin["kind"] = json!("sidebarPins");
        pin["set"] = fields;
        assert!(validate_op(&pin).is_err());
    }
    assert!(
        validate_op(&json!({"kind":"sidebarPins","id":"a","op":"delete","hlc":clock(1,"dev-a")}))
            .is_err()
    );
}
