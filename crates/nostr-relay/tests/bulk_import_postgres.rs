use std::{
    io::{Cursor, Write},
    process::{Command, Stdio},
};

use nostr_relay::{
    bulk_import::{BulkImportReport, import_jsonl},
    domain::{Event, Tag},
    store::Store,
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::Deserialize;

const IMPORT_NOW: u64 = 30_000;

#[derive(Deserialize)]
struct Fixture {
    known_jsonl_events: Vec<Event>,
    known_report: BulkImportReport,
    ordered_replay: Vec<String>,
    first_report: BulkImportReport,
    replay_report: BulkImportReport,
    invalid_lines: Vec<InvalidLine>,
}

#[derive(Deserialize)]
struct InvalidLine {
    raw: String,
}

#[tokio::test]
async fn signed_event_jsonl_import_is_ordered_and_idempotent() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: import proof requires a disposable database guard");
        return;
    }
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/migration/signed-event-import-v1.json"
    ))
    .expect("bulk import fixture parses");
    assert_eq!(
        fixture.ordered_replay,
        [
            "regular",
            "regular-duplicate",
            "replaceable-old",
            "replaceable-new",
            "deletion",
            "ephemeral",
            "expired"
        ]
    );

    let known_jsonl = encode_jsonl(&fixture.known_jsonl_events);
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostr-relay"))
        .arg("import-jsonl")
        .env("DATABASE_URL", &database_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("import-jsonl command starts");
    command
        .stdin
        .take()
        .expect("import-jsonl stdin is piped")
        .write_all(&known_jsonl)
        .expect("known JSONL is written");
    let output = command
        .wait_with_output()
        .expect("import-jsonl command exits");
    assert!(
        output.status.success(),
        "import-jsonl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let known_report: BulkImportReport =
        serde_json::from_slice(&output.stdout).expect("import-jsonl report is JSON");
    assert_eq!(known_report, fixture.known_report);

    let mut store = Store::connect(&database_url).await.expect("store connects");
    let baseline = store.latest_ingest_seq().await.expect("baseline reads");
    let regular = signed_event(60, 20_000, 1, Vec::new(), "regular");
    let replaceable_old = signed_event(61, 20_000, 0, Vec::new(), "old");
    let replaceable_new = signed_event(61, 20_001, 0, Vec::new(), "new");
    let deletion = signed_event(
        60,
        20_002,
        5,
        vec![Tag::new(vec!["e".to_owned(), regular.id.clone()])],
        "delete",
    );
    let ephemeral = signed_event(62, 20_003, 20_000, Vec::new(), "ephemeral");
    let expired = signed_event(
        63,
        20_004,
        1,
        vec![Tag::new(vec!["expiration".to_owned(), "29999".to_owned()])],
        "expired",
    );
    let events = vec![
        regular.clone(),
        regular.clone(),
        replaceable_old.clone(),
        replaceable_new.clone(),
        deletion.clone(),
        ephemeral.clone(),
        expired.clone(),
    ];
    let jsonl = encode_jsonl(&events);

    let first = import_jsonl(Cursor::new(&jsonl), &mut store, IMPORT_NOW)
        .await
        .expect("first import succeeds");
    assert_eq!(first, fixture.first_report);
    assert_eq!(
        store
            .latest_ingest_seq()
            .await
            .expect("latest sequence reads"),
        baseline + 4
    );
    assert!(
        store
            .event_by_id(&regular.id, IMPORT_NOW)
            .await
            .expect("regular lookup succeeds")
            .is_none(),
        "deletion must remove the earlier regular event"
    );
    assert!(
        store
            .event_by_id(&replaceable_old.id, IMPORT_NOW)
            .await
            .expect("old replacement lookup succeeds")
            .is_none(),
        "new replacement must remove the old head"
    );
    let stored_replacement = store
        .event_by_id(&replaceable_new.id, IMPORT_NOW)
        .await
        .expect("new replacement lookup succeeds")
        .expect("new replacement remains stored");
    assert_eq!(stored_replacement.event, replaceable_new);
    assert_eq!(stored_replacement.ingest_seq, baseline + 3);
    let stored_deletion = store
        .event_by_id(&deletion.id, IMPORT_NOW)
        .await
        .expect("deletion lookup succeeds")
        .expect("deletion request remains stored");
    assert_eq!(stored_deletion.event, deletion);
    assert_eq!(stored_deletion.ingest_seq, baseline + 4);
    assert!(
        store
            .event_by_id(&ephemeral.id, IMPORT_NOW)
            .await
            .expect("ephemeral lookup succeeds")
            .is_none()
    );
    assert!(
        store
            .event_by_id(&expired.id, IMPORT_NOW)
            .await
            .expect("expired lookup succeeds")
            .is_none()
    );

    let replay = import_jsonl(Cursor::new(&jsonl), &mut store, IMPORT_NOW)
        .await
        .expect("full replay is idempotent");
    assert_eq!(replay, fixture.replay_report);
    assert_eq!(
        store
            .latest_ingest_seq()
            .await
            .expect("latest sequence reads"),
        baseline + 4,
        "replay must not add durable rows"
    );

    let prefix_event = signed_event(64, 20_005, 1, Vec::new(), "committed prefix");
    let mut partial = encode_jsonl(std::slice::from_ref(&prefix_event));
    partial.extend_from_slice(
        fixture
            .invalid_lines
            .first()
            .expect("fixture has an invalid line")
            .raw
            .as_bytes(),
    );
    partial.push(b'\n');
    let error = import_jsonl(Cursor::new(&partial), &mut store, IMPORT_NOW)
        .await
        .expect_err("invalid second line stops the import");
    assert!(error.to_string().contains("line 2"));
    assert!(
        store
            .event_by_id(&prefix_event.id, IMPORT_NOW)
            .await
            .expect("prefix lookup succeeds")
            .is_some(),
        "the valid prefix must remain committed"
    );
    assert_eq!(
        store
            .latest_ingest_seq()
            .await
            .expect("latest sequence reads"),
        baseline + 5
    );
}

fn encode_jsonl(events: &[Event]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for event in events {
        serde_json::to_writer(&mut bytes, event).expect("event serializes");
        bytes.push(b'\n');
    }
    bytes
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).expect("fixture key is valid");
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes().expect("fixture id computes");
    event.id = event.computed_id().expect("fixture id computes");
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    event
}
