use super::*;
use coder_history::{CatalogPage, Chat, Harness, SourceStatus};

fn app(root: &std::path::Path) -> App {
    App::new(Config {
        cache_dir: root.into(),
        secret_hex: "01".repeat(32),
        synthetic: true,
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
    let view = packet.view.unwrap();
    let key = find_button(
        &view["root"],
        "Read-only transcript preview\nCodex · time unavailable",
    );
    assert!(!key.is_empty());
    let request = || Request::Activate {
        instance: view["instance"].as_str().unwrap().into(),
        revision: view["revision"].as_u64().unwrap(),
        node: key.clone(),
    };
    let opened = app.call(request());
    assert!(opened.error.is_none());
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
