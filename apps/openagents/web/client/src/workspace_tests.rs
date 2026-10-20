use super::*;
use std::collections::BTreeMap;

use openagents_doc::apply_op;
use openagents_proto::{HarnessId, SessionStatus};

const NOW: i64 = 1_800_000_000_000;

fn at(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms).single().unwrap()
}

/// The server's merged rows after `writes` on another device.
fn server_rows(writes: impl FnOnce(&mut RegistryDoc)) -> Vec<RegistryRow> {
    let mut desktop = RegistryDoc::new("mac-1");
    writes(&mut desktop);
    let mut rows: BTreeMap<(String, String), RegistryRow> = BTreeMap::new();
    for batch in desktop.take_pushable() {
        for op in batch.ops {
            let key = (op.kind.clone(), op.id.clone());
            if let (Some(row), _) = apply_op(rows.get(&key), &op) {
                rows.insert(key, row);
            }
        }
    }
    rows.into_values()
        .enumerate()
        .map(|(seq, mut row)| {
            row.seq = seq as u64 + 1;
            row
        })
        .collect()
}

fn device(id: &str, name: &str, platform: &str) -> Device {
    serde_json::from_value(json!({
        "id": id, "name": name, "platform": platform,
        "lastSeenAt": null, "capabilities": ["projectless-scratch-v1"],
    }))
    .unwrap()
}

fn chat(id: &str, device: &str, space: Option<&str>) -> Chat {
    Chat {
        links: Default::default(),
        id: id.into(),
        device_id: device.into(),
        title: Some(format!("Chat {id}")),
        archived: false,
        cwd: Some("/work/site".into()),
        branch: None,
        checkout_id: None,
        source_context: None,
        config: Some(config()),
        last_message_preview: None,
        last_message_at: Some(at(NOW - 5_000)),
        created_at: at(NOW - 60_000),
        harness_session_id: None,
        harness_session_cwd: None,
        space_id: space.map(Into::into),
        last_seen_at: Some(at(NOW)),
        room_gen: Some(2),
        parent_chat_id: None,
    }
}

fn config() -> ChatConfig {
    ChatConfig {
        harness: HarnessId::Codex,
        model: Some("gpt-6".into()),
        reasoning: None,
        model_options: Default::default(),
        sandbox: SandboxLevel::WorkspaceWrite,
    }
}

fn space() -> Space {
    serde_json::from_value(json!({"id": "s1", "deviceId": "mac-1", "path": "/work/site", "createdAt": "2026-10-20T00:00:00Z"})).unwrap()
}

fn joined(rows: Vec<RegistryRow>) -> Replica {
    let mut replica = Replica::new("web-tab");
    let hello: Value = serde_json::from_str(&replica.hello()).unwrap();
    assert_eq!(
        hello,
        json!({"t": "hello", "cursor": null, "device": "web-tab"})
    );
    let seq = rows.len() as u64;
    let state = json!({
        "t": "state", "seq": seq, "full": true, "gcFloor": 0, "rows": rows,
        "presence": {"mac-1": NOW - 10_000},
    });
    assert_eq!(replica.apply(&state.to_string()), Applied::Changed);
    assert!(replica.synced());
    // This tab's writes come after the fixture's, by the clock.
    std::thread::sleep(std::time::Duration::from_millis(3));
    replica
}

/// #12265: every computer's chats arrive from the registry with their
/// computer, project, and live status.
#[test]
fn the_registry_lists_every_computers_chats_with_their_status() {
    let rows = server_rows(|doc| {
        doc.upsert_device(&device("mac-1", "Studio Mac", "macos"))
            .unwrap();
        doc.upsert_device(&device("box-2", "Build box", "linux"))
            .unwrap();
        doc.upsert_device(&device("phone", "Phone", "ios")).unwrap();
        doc.upsert_space(&space()).unwrap();
        doc.upsert_chat(&chat("c1", "mac-1", Some("s1"))).unwrap();
        doc.upsert_chat(&chat("c2", "box-2", None)).unwrap();
        doc.upsert_session(&Session {
            last_completed_turn: None,
            chat_id: "c1".into(),
            device_id: "mac-1".into(),
            status: SessionStatus::Working,
            started_at: Some(at(NOW - 20_000)),
            updated_at: at(NOW - 2_000),
            running_subagents: 0,
            background_tasks: 0,
            last_output_at: None,
        })
        .unwrap();
    });
    let replica = joined(rows);
    let mut rows = replica.rows(NOW);
    rows.sort_by(|a, b| a.chat.id.cmp(&b.chat.id));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].computer, "Studio Mac");
    assert!(rows[0].online, "presence 10 s ago");
    assert_eq!(rows[0].project.as_deref(), Some("site"));
    assert!(rows[0].working);
    assert_eq!(rows[1].computer, "Build box");
    assert!(!rows[1].online);
    assert!(!rows[1].working);
    // Phones can't run sessions.
    let computers: Vec<_> = replica.computers().into_iter().map(|d| d.name).collect();
    assert_eq!(computers, ["Build box", "Studio Mac"]);
    // A stale "working" row is a dead engine, not a running chat.
    assert!(!replica.rows(NOW + 60_000).iter().any(|r| r.working));
}

/// A write shows at once, goes out as one push after the hello, and stays
/// after the server's ack.
#[test]
fn a_rename_shows_at_once_and_is_pushed_after_the_hello() {
    let rows = server_rows(|doc| {
        doc.upsert_device(&device("mac-1", "Studio Mac", "macos"))
            .unwrap();
        doc.upsert_chat(&chat("c1", "mac-1", None)).unwrap();
    });
    let mut replica = Replica::new("web-tab");
    replica.hello();
    // Before the state frame nothing is pushed.
    assert!(replica.pushes().is_empty());
    let seq = rows.len() as u64;
    replica.apply(&json!({"t": "state", "seq": seq, "full": true, "rows": rows}).to_string());
    std::thread::sleep(std::time::Duration::from_millis(3));
    replica.rename("c1", "  Fix the site  ").unwrap();
    assert_eq!(
        replica.chat("c1").unwrap().title.as_deref(),
        Some("Fix the site")
    );
    let pushes = replica.pushes();
    assert_eq!(pushes.len(), 1);
    let push: Value = serde_json::from_str(&pushes[0]).unwrap();
    assert_eq!(push["t"], "push");
    assert_eq!(push["ops"][0]["set"]["title"], "Fix the site");
    // Sent once per connection.
    assert!(replica.pushes().is_empty());
    assert!(!replica.saved());
    let batch = push["batch"].as_str().unwrap();
    replica.apply(&json!({"t": "ack", "batch": batch, "seq": seq + 1, "applied": 1}).to_string());
    assert!(replica.saved());
    // A reconnect asks from the cursor.
    let hello: Value = serde_json::from_str(&replica.hello()).unwrap();
    assert_eq!(hello["cursor"], seq);
    // Archive and delete reach unknown chats as a refusal.
    assert!(replica.archive("nope", true).is_err());
    replica.archive("c1", true).unwrap();
    assert!(replica.chat("c1").unwrap().archived);
    replica.delete("c1").unwrap();
    assert!(replica.chat("c1").is_none());
    // A missed broadcast asks for a reconnect.
    assert_eq!(
        replica.apply(&json!({"t": "rows", "seq": seq + 9, "rows": []}).to_string()),
        Applied::Resync
    );
}

/// Pins wait for the desktop's set-up, then pin, reorder, and file chats
/// in sections.
#[test]
fn pins_and_sections_follow_the_desktops_rules() {
    let mut replica = joined(server_rows(|doc| {
        doc.upsert_device(&device("mac-1", "Studio Mac", "macos"))
            .unwrap();
        for id in ["a", "b", "c"] {
            doc.upsert_chat(&chat(id, "mac-1", None)).unwrap();
        }
    }));
    assert!(replica.pin("a").unwrap_err().contains("aren't ready"));

    let mut replica = joined(server_rows(|doc| {
        doc.upsert_device(&device("mac-1", "Studio Mac", "macos"))
            .unwrap();
        for id in ["a", "b", "c"] {
            doc.upsert_chat(&chat(id, "mac-1", None)).unwrap();
        }
        doc.reconcile_sidebar_pins(true).unwrap();
    }));
    for id in ["a", "b", "c"] {
        replica.pin(id).unwrap();
    }
    let pins = || replica.preferences().unwrap().pinned_session_ids;
    assert_eq!(pins(), ["a", "b", "c"]);
    replica.move_pin("c", true).unwrap();
    assert_eq!(
        replica.preferences().unwrap().pinned_session_ids,
        ["a", "c", "b"]
    );
    replica.move_pin("a", false).unwrap();
    assert_eq!(
        replica.preferences().unwrap().pinned_session_ids,
        ["c", "a", "b"]
    );
    replica.move_pin("c", true).unwrap();
    replica.unpin("a").unwrap();
    assert_eq!(
        replica.preferences().unwrap().pinned_session_ids,
        ["c", "b"]
    );
    let row = |replica: &Replica, id: &str| {
        replica
            .rows(NOW)
            .into_iter()
            .find(|r| r.chat.id == id)
            .unwrap()
    };
    assert_eq!(row(&replica, "b").pin, Some(1));

    replica.create_section("sec1", " Launch ").unwrap();
    replica.assign("a", Some("sec1")).unwrap();
    assert_eq!(row(&replica, "a").section.as_deref(), Some("sec1"));
    replica.rename_section("sec1", "Ship").unwrap();
    replica.collapse_section("sec1", true).unwrap();
    let section = replica.preferences().unwrap().sections[0].clone();
    assert_eq!((section.name.as_str(), section.collapsed), ("Ship", true));
    replica.assign("a", None).unwrap();
    assert_eq!(row(&replica, "a").section, None);
    replica.delete_section("sec1").unwrap();
    assert!(replica.preferences().unwrap().sections.is_empty());
}

/// #12266: a new session's chat row and first command, as the desktop
/// writes them.
#[test]
fn a_new_session_writes_its_chat_row_and_its_first_run() {
    let mut replica = joined(server_rows(|doc| {
        doc.upsert_device(&device("mac-1", "Studio Mac", "macos"))
            .unwrap();
        doc.upsert_device(&device("phone", "Phone", "ios")).unwrap();
        doc.upsert_space(&space()).unwrap();
    }));
    let new = |id: &str, computer: &str, project: Option<&str>| NewSession {
        id: id.into(),
        computer: computer.into(),
        project: project.map(Into::into),
        cwd: None,
        branch: Some("main".into()),
        config: config(),
        title: None,
    };
    let made = replica
        .create_session(
            &new("00000000-0000-4000-8000-00000000000a", "mac-1", Some("s1")),
            NOW,
        )
        .unwrap();
    assert_eq!(made.device_id, "mac-1");
    assert_eq!(made.cwd.as_deref(), Some("/work/site"));
    assert_eq!(made.space_id.as_deref(), Some("s1"));
    assert_eq!(made.room_gen, Some(2));
    assert_eq!(replica.chat(&made.id).unwrap().config, Some(config()));
    // Starting the same chat twice is refused.
    assert!(
        replica
            .create_session(
                &new("00000000-0000-4000-8000-00000000000a", "mac-1", Some("s1")),
                NOW
            )
            .is_err()
    );
    // No project: a scratch folder of its own.
    let scratch = replica
        .create_session(
            &new("00000000-0000-4000-8000-00000000000b", "mac-1", None),
            NOW,
        )
        .unwrap();
    assert_ne!(scratch.cwd.as_deref(), Some("~"));
    assert_eq!(scratch.space_id, None);
    // A phone can't run one.
    assert!(
        replica
            .create_session(
                &new("00000000-0000-4000-8000-00000000000c", "phone", None),
                NOW
            )
            .is_err()
    );

    let command = run_command(&made, "m1", "Fix the build", None);
    assert_eq!(command["chatId"], made.id.as_str());
    assert_eq!(command["command"]["kind"], "run");
    assert_eq!(command["command"]["messageId"], "m1");
    let request: RunRequest =
        serde_json::from_value(command["command"]["request"].clone()).unwrap();
    assert_eq!(request.prompt, "Fix the build");
    assert_eq!(request.harness, Some(HarnessId::Codex));
    assert_eq!(request.model.as_deref(), Some("gpt-6"));
    assert_eq!(request.cwd, "/work/site");
    assert!(request.mcp.is_none());
    // The host reads it as its own command payload.
    let _: openagents_doc::SessionCommandPayload =
        serde_json::from_value(command["command"].clone()).unwrap();
    let _: openagents_doc::SessionCommandPayload =
        serde_json::from_value(interrupt_command("c1")["command"].clone()).unwrap();
}

#[test]
fn the_view_round_trips_through_its_stored_words() {
    for view in [
        View::default(),
        View {
            organize: Organize::ByProject,
            sort: Sort::Created,
            compact: true,
            archived: true,
        },
        View {
            organize: Organize::OneList,
            ..View::default()
        },
    ] {
        assert_eq!(View::decode(&view.encode()), view);
    }
    assert_eq!(View::decode("nonsense"), View::default());
}

#[test]
fn computer_rename_is_shared_with_the_desktop() {
    let mut browser = joined(server_rows(|r| {
        r.upsert_device(&device("mac-1", "Scratch", "linux"))
            .unwrap();
    }));
    assert!(browser.rename_device("missing", "New").is_err());
    assert!(browser.rename_device("mac-1", "  ").is_err());
    browser
        .rename_device("mac-1", "  Scratch renamed  ")
        .unwrap();
    assert_eq!(browser.devices()[0].name, "Scratch renamed");
    assert!(!browser.pushes().is_empty());
}
