use super::*;
use coder_history::{CatalogPage, Chat, Harness, SourceStatus};

fn app(root: &std::path::Path) -> App {
    App::new(Config {
        cache_dir: root.into(),
        secret_hex: "01".repeat(32),
        synthetic: true,
        loopback_test: false,
        push: None,
    })
    .unwrap()
}

fn find_button(value: &serde_json::Value, label: &str) -> String {
    if value["element"]["kind"] == "button" && value["element"]["props"]["label"] == label {
        return value["key"].as_str().unwrap().into();
    }
    if let Some(children) = value["element"]["props"]["children"].as_array() {
        for child in children {
            let found = find_button(child, label);
            if !found.is_empty() {
                return found;
            }
        }
    }
    String::new()
}

#[test]
fn native_projection_opens_a_cached_chat_and_refuses_stale_actions() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let packet = app.call(Request::Snapshot);
    assert!(!packet.reading);
    let view = packet.view.unwrap();
    let key = find_button(&view["root"], "Read-only chat preview\nCodex");
    assert!(!key.is_empty());
    let request = || Request::Activate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.clone(),
    };
    let opened = app.call(request());
    assert!(opened.error.is_none());
    assert!(opened.reading);
    let json = opened.view.unwrap().to_string();
    assert!(json.contains("Native timeline"));
    assert!(json.contains("日本語"));
    assert!(
        app.call(request())
            .error
            .unwrap()
            .contains("Screen changed")
    );
    let disconnected = app.call(Request::Disconnect);
    assert!(disconnected.error.is_none());
    assert!(app.cache.keys("").unwrap().is_empty());
}

#[test]
fn catalog_is_persisted_in_pages_and_source_gaps_are_visible() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.apply_catalog(
        CatalogPage {
            snapshot: "fixture".into(),
            entries: vec![Chat {
                id: "id".into(),
                harness: Harness::Codex,
                native_id: None,
                title: "A chat".into(),
                title_truncated: false,
                updated_at: None,
                archived: false,
                subagent: false,
                source_id: None,
                status: SourceStatus::Missing,
            }],
            next: None,
            notices: vec![],
        },
        true,
    )
    .unwrap();
    let state: serde_json::Value = app.cache.read("catalog_state").unwrap().unwrap();
    assert_eq!(state["pages"], 1);
    let packet = app.call(Request::Snapshot);
    assert!(packet.view.unwrap().to_string().contains("A chat"));
}

#[test]
fn ffi_rejects_unknown_operations_without_executing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let config = serde_json::to_vec(
        &serde_json::json!({"cache_dir":dir.path(),"secret_hex":"01".repeat(32),"synthetic":true}),
    )
    .unwrap();
    unsafe {
        let handle = crate::ffi::coder_mobile_create(config.as_ptr(), config.len());
        assert!(!handle.is_null());
        let request = br#"{"op":"submit","prompt":"run a command"}"#;
        let reply = crate::ffi::coder_mobile_call(handle, request.as_ptr(), request.len());
        let json: serde_json::Value =
            serde_json::from_slice(std::slice::from_raw_parts(reply.data, reply.len)).unwrap();
        assert!(json["error"].is_string());
        crate::ffi::coder_mobile_buffer_free(reply);
        crate::ffi::coder_mobile_destroy(handle);
    }
}

#[test]
fn invalid_pairing_input_preserves_cached_chats_and_does_not_echo_the_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let before = app.cache.keys("").unwrap();
    for code in [
        "https://unrelated.invalid/private-example",
        "coder-pair:not-a-valid-invitation",
    ] {
        let packet = app.call(Request::Connect { code: code.into() });
        assert!(packet.error.as_deref().unwrap().contains("pairing code"));
        assert!(!packet.error.as_deref().unwrap().contains(code));
        assert!(packet.paired);
        let error = packet.error;
        assert_eq!(app.call(Request::Foreground { active: true }).error, error);
        assert_eq!(app.cache.keys("").unwrap(), before);
        assert!(!app.catalog.is_empty());
    }
    let packet = app.call(Request::Disconnect);
    assert!(!packet.paired);
}

#[test]
fn paused_follow_pins_the_visible_page_across_new_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.selected = app.catalog.first().cloned();
    let packet = app.call(Request::Snapshot);
    let pinned = packet.follow_page.clone().unwrap();
    let keys = app.page_keys().unwrap();
    assert_eq!(keys.len(), 2);
    let mut page: coder_history::TranscriptPage = app.cache.read(&keys[1]).unwrap().unwrap();
    let end = page.next.offset;
    let old_start = page.chunks[0].offset;
    let shift = end - old_start;
    for chunk in &mut page.chunks {
        chunk.offset += shift;
        chunk.record_offset += shift;
        chunk.end_offset += shift;
    }
    page.next.offset += shift;
    page.next.record_offset += shift;
    page.snapshot_bytes = page.next.offset;
    app.apply_page(page).unwrap();
    // This models a gesture arriving after its in-flight refresh completed.
    let paused = app.call(Request::Follow {
        enabled: false,
        page: Some(pinned.clone()),
    });
    assert!(paused.error.is_none());
    assert_eq!(paused.follow_page.as_deref(), Some(pinned.as_str()));
    assert!(paused.follow_target.is_none());
    let resumed = app.call(Request::Follow {
        enabled: true,
        page: None,
    });
    assert_ne!(resumed.follow_page, Some(pinned));
    assert!(resumed.follow_target.is_some());
}

#[test]
fn large_message_is_readable_across_source_pages_without_a_one_kib_cut() {
    use base64::Engine;
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.selected = app.catalog.first().cloned();
    app.cache.erase("page_synthetic_").unwrap();
    app.transcript = crate::app::TranscriptState::default();
    let text = format!("{}TAIL-日本語", "a".repeat(40_000));
    let mut raw=serde_json::to_vec(&serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}})).unwrap();
    raw.push(b'\n');
    let total = raw.len() as u64;
    let mut offset = 0;
    for page in raw.chunks(coder_history::MAX_PAGE_BYTES as usize) {
        let mut chunks = vec![];
        for bytes in page.chunks(coder_history::MAX_CHUNK_BYTES) {
            let end = offset + bytes.len() as u64;
            chunks.push(coder_history::RecordChunk {
                id: format!("record-{offset}"),
                index: 0,
                record_offset: 0,
                offset,
                end_offset: end,
                raw_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                complete: end == total,
                oversized: false,
                readable: None,
            });
            offset = end;
        }
        app.apply_page(coder_history::TranscriptPage {
            source_id: "synthetic".into(),
            incarnation: "long".into(),
            snapshot_bytes: total,
            chunks,
            next: coder_history::TranscriptCursor {
                source_id: "synthetic".into(),
                incarnation: "long".into(),
                offset,
                record_offset: if offset == total { total } else { 0 },
                record_index: if offset == total { 1 } else { 0 },
                prefix_sha256: "fixture".into(),
            },
            has_more: offset < total,
            pending_line: false,
            notices: vec![],
            previous: None,
        })
        .unwrap();
    }
    app.text_parts.insert(0, 4);
    let packet = app.call(Request::Snapshot);
    assert!(packet.error.is_none(), "{:?}", packet.error);
    let view = packet.view.unwrap().to_string();
    assert!(view.contains("TAIL-日本語"));
    assert!(view.contains("Message part 5 of 5"));
}

#[test]
fn unchanged_catalog_refresh_keeps_all_cached_pages_visible() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let mut extra = app.catalog[0].clone();
    extra.id = "second".into();
    extra.title = "Second chat".into();
    app.apply_catalog(
        CatalogPage {
            snapshot: "synthetic".into(),
            entries: vec![extra],
            next: None,
            notices: vec![],
        },
        false,
    )
    .unwrap();
    let mut first = app.catalog[0].clone();
    first.title = "Updated title".into();
    app.apply_catalog(
        CatalogPage {
            snapshot: "synthetic".into(),
            entries: vec![first],
            next: None,
            notices: vec![],
        },
        true,
    )
    .unwrap();
    assert_eq!(app.catalog.len(), 2);
    assert_eq!(app.catalog_state.pages, 2);
    assert_eq!(app.catalog[0].title, "Updated title");
}

#[test]
fn the_computers_surface_is_separate_from_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let packet = app.call(Request::Snapshot);
    let reader = packet.view.unwrap();
    let computers = packet.computers.unwrap();
    assert_ne!(reader["instance"], computers["instance"]);
    assert!(
        computers["instance"]
            .as_str()
            .unwrap()
            .starts_with("computers:")
    );
    assert!(!packet.computers_exit);
    let activate = |view: &serde_json::Value, label: &str| Request::ComputersActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: find_button(&view["root"], label),
    };
    // Scanning asks the native host for a value; the reader is unchanged.
    let asked = app.call(activate(&computers, "Scan invitation"));
    let input = asked.computers_input.clone().unwrap();
    assert!(input.scan);
    assert_eq!(asked.view.unwrap()["revision"], reader["revision"]);
    let added = app.call(Request::ComputersInput {
        token: input.token.clone(),
        value: "coder-host:synthetic".into(),
    });
    assert!(added.computers_input.is_none());
    assert!(added.error.is_none());
    let view = added.computers.unwrap();
    assert!(view.to_string().contains("6 computers added."));
    // A stale activation is refused on the Computers surface only.
    let stale = app.call(activate(&computers, "Scan invitation"));
    assert!(stale.error.is_none());
    let view = stale.computers.unwrap();
    assert!(view.to_string().contains("The screen changed"));
    // Continue reports the exit once.
    let finished = app.call(activate(&view, "Continue"));
    assert!(finished.computers_exit);
    assert!(
        finished
            .computers
            .unwrap()
            .to_string()
            .contains("Up to date.")
    );
    assert!(!app.call(Request::Snapshot).computers_exit);
}

#[test]
fn a_phone_enters_the_owner_key_through_a_masked_request() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let activate = |view: &serde_json::Value, label: &str| Request::ComputersActivate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: find_button(&view["root"], label),
    };
    let first = app.call(Request::Snapshot).computers.unwrap();
    let list = app.call(activate(&first, "Continue")).computers.unwrap();
    let asked = app.call(activate(&list, "Enter owner key"));
    let input = asked.computers_input.clone().unwrap();
    assert!(input.secret && !input.scan);
    // The adapter reads the flag from the packet it decodes.
    let encoded = serde_json::to_value(&asked).unwrap();
    assert_eq!(encoded["computers_input"]["secret"], true);
    assert_eq!(encoded["computers_input"]["purpose"], "owner_key");
    let key = coder_computers::synthetic::owner_secret_hex();
    let held = app.call(Request::ComputersInput {
        token: input.token,
        value: key.clone(),
    });
    assert!(held.computers_input.is_none());
    let text = serde_json::to_string(&held).unwrap();
    assert!(text.contains("This device now holds your owner key."));
    assert!(text.contains("Your directory is empty."));
    // The key never comes back to the adapter.
    assert!(!text.contains(&key));
}

#[test]
fn the_normal_app_offers_live_computers_and_validates_what_it_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Config {
        cache_dir: dir.path().into(),
        secret_hex: "02".repeat(32),
        synthetic: false,
        loopback_test: false,
        push: None,
    })
    .unwrap();
    let packet = app.call(Request::Snapshot);
    let computers = packet.computers.unwrap();
    let text = computers.to_string();
    assert!(!text.contains("This build can't reach computers yet"));
    assert!(packet.computers_qr.is_none());
    let paste = find_button(&computers["root"], "Paste invitation");
    let request = Request::ComputersActivate {
        instance: computers["instance"].as_str().unwrap().into(),
        revision: computers["revision"].as_u64().unwrap(),
        node: paste,
    };
    let asked = app.call(request);
    let input = asked.computers_input.unwrap();
    // A Chats pairing code is refused before any network use.
    let refused = app.call(Request::ComputersInput {
        token: input.token.clone(),
        value: "coder-pair:AAAA".into(),
    });
    assert!(
        refused
            .computers
            .unwrap()
            .to_string()
            .contains("This is a Chats pairing code")
    );
    // A malformed invitation is refused by the invitation parser; nothing is
    // saved, and the list stays empty.
    let refused = app.call(Request::ComputersInput {
        token: input.token,
        value: "coder-host:AAAA".into(),
    });
    let view = refused.computers.unwrap().to_string();
    assert!(view.contains("couldn't accept this"), "{view}");
    assert!(!dir.path().join("computers/computers.cache").exists());
    // Lifecycle signals reach the Computers surface without disturbing the
    // reader.
    let resumed = app.call(Request::Lifecycle { active: false });
    assert!(resumed.error.is_none());
    let resumed = app.call(Request::Lifecycle { active: true });
    assert!(resumed.error.is_none() && resumed.computers.is_some());
}

#[test]
fn push_is_off_until_configured_and_refuses_cleartext_outside_tests() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    let snapshot = serde_json::to_value(app.call(Request::Snapshot)).unwrap();
    assert!(snapshot.get("push").is_none());
    let request: Request = serde_json::from_str(r#"{"op":"push_token","token":"ab"}"#).unwrap();
    let packet = app.call(request);
    assert_eq!(
        packet.push.as_deref(),
        Some("Push notifications are off in this build.")
    );
    assert!(packet.error.is_none());

    let config: Config = serde_json::from_value(serde_json::json!({
        "cache_dir": dir.path().join("configured"),
        "secret_hex": "02".repeat(32),
        "synthetic": true,
        "push": {
            "relay_url": "ws://127.0.0.1:1",
            "gateway_url": "http://127.0.0.1:2",
            "app_profile": "com.openagents.coder/ios"
        }
    }))
    .unwrap();
    let mut cleartext = App::new(config).unwrap();
    let packet = cleartext.call(Request::PushDisable);
    assert!(
        packet
            .push
            .unwrap()
            .contains("needs a wss:// relay and an https:// gateway")
    );
}
