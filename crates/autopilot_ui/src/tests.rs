use super::*;

fn approx_eq(a: f32, b: f32) {
    assert!(
        (a - b).abs() < 0.5,
        "expected {a} ~= {b} (diff {})",
        (a - b).abs()
    );
}

#[test]
fn layout_panels_are_consistent() {
    let bounds = Bounds::new(0.0, 0.0, 1200.0, 800.0);
    let layout = Layout::new(bounds);

    approx_eq(layout.left_panel_bounds.size.width, LEFT_PANEL_WIDTH);
    approx_eq(layout.right_panel_bounds.size.width, RIGHT_PANEL_WIDTH);

    let expected_center_width =
        bounds.size.width - LEFT_PANEL_WIDTH - RIGHT_PANEL_WIDTH - PANEL_GAP * 2.0;
    approx_eq(layout.center_panel_bounds.size.width, expected_center_width);

    approx_eq(layout.command_bar_bounds.size.height, COMMAND_BAR_HEIGHT);
    approx_eq(
        layout.command_bar_bounds.origin.y,
        bounds.size.height - COMMAND_BAR_HEIGHT,
    );

    assert!(
        layout.right_body_bounds.origin.y
            >= layout.right_header_bounds.origin.y + layout.right_header_bounds.size.height
    );
    assert!(
        layout.left_header_bounds.origin.x
            >= layout.left_panel_bounds.origin.x + PANEL_PADDING - 0.5
    );
}

#[test]
fn normalize_pane_rect_enforces_minimum_dimensions_for_invalid_values() {
    let rect = PaneRect {
        x: 42.0,
        y: 24.0,
        width: f32::NAN,
        height: -10.0,
    };

    let normalized = normalize_pane_rect(rect);
    assert_eq!(normalized.x, 42.0);
    assert_eq!(normalized.y, 24.0);
    assert_eq!(normalized.width, PANE_MIN_WIDTH);
    assert_eq!(normalized.height, PANE_MIN_HEIGHT);
}

#[test]
fn calculate_new_pane_position_wraps_to_margin_when_offset_overflows_screen() {
    let last = PaneRect {
        x: 1100.0,
        y: 650.0,
        width: 420.0,
        height: 300.0,
    };
    let screen = Size::new(1280.0, 720.0);

    let next = calculate_new_pane_position(Some(last), screen, 420.0, 300.0);
    assert_eq!(next.x, PANE_MARGIN);
    assert_eq!(next.y, PANE_MARGIN);
    assert_eq!(next.width, 420.0);
    assert_eq!(next.height, 300.0);
}

#[test]
fn extract_thread_hint_prefers_thread_id_fields() {
    let params = serde_json::json!({
        "threadId": "thread-primary",
        "conversationId": "thread-fallback",
        "msg": {
            "thread_id": "thread-nested"
        }
    });

    assert_eq!(
        extract_thread_hint(Some(&params)).as_deref(),
        Some("thread-primary")
    );
}

#[test]
fn extract_thread_hint_supports_codex_event_conversation_fields() {
    let params = serde_json::json!({
        "conversationId": "conv-root"
    });

    assert_eq!(
        extract_thread_hint(Some(&params)).as_deref(),
        Some("conv-root")
    );
}

#[test]
fn extract_thread_hint_supports_nested_codex_msg_thread_fields() {
    let params = serde_json::json!({
        "msg": {
            "thread_id": "thread-nested"
        }
    });

    assert_eq!(
        extract_thread_hint(Some(&params)).as_deref(),
        Some("thread-nested")
    );
}

#[test]
fn command_string_from_item_supports_array_commands() {
    let item = serde_json::json!({
        "command": ["cargo", "test", "--workspace", 42]
    });
    assert_eq!(
        command_string_from_item(&item).as_deref(),
        Some("cargo test --workspace")
    );
}

#[test]
fn extract_file_changes_preserves_first_path_and_first_diff() {
    let item = serde_json::json!({
        "changes": [
            {"path": "src/main.rs", "diff": "first-diff"},
            {"path": "Cargo.toml", "diff": "second-diff"}
        ]
    });
    let (paths, first_path, first_diff) = extract_file_changes(&item);

    assert_eq!(
        paths,
        vec!["src/main.rs".to_string(), "Cargo.toml".to_string()]
    );
    assert_eq!(first_path.as_deref(), Some("src/main.rs"));
    assert_eq!(first_diff.as_deref(), Some("first-diff"));
}

#[test]
fn agent_delta_aliases_do_not_double_append_modern_then_legacy() {
    let mut chat = ChatPaneState::new(DEFAULT_THREAD_MODEL);
    let modern = serde_json::json!({
        "method": "item/agentMessage/delta",
        "params": {
            "itemId": "msg-1",
            "delta": "Hi "
        }
    });
    let legacy = serde_json::json!({
        "method": "codex/event/agent_message_content_delta",
        "params": {
            "msg": {
                "item_id": "msg-1",
                "delta": "Hi "
            }
        }
    });

    chat.apply_formatted_event(&modern);
    chat.apply_formatted_event(&legacy);

    let stream = chat
        .formatted_message_streams
        .get("msg-1")
        .expect("agent stream should exist");
    assert_eq!(stream.source(), "Hi ");
}

#[test]
fn agent_delta_aliases_do_not_double_append_legacy_then_modern() {
    let mut chat = ChatPaneState::new(DEFAULT_THREAD_MODEL);
    let modern = serde_json::json!({
        "method": "item/agentMessage/delta",
        "params": {
            "itemId": "msg-1",
            "delta": "Hi "
        }
    });
    let legacy = serde_json::json!({
        "method": "codex/event/agent_message_content_delta",
        "params": {
            "msg": {
                "item_id": "msg-1",
                "delta": "Hi "
            }
        }
    });

    chat.apply_formatted_event(&legacy);
    chat.apply_formatted_event(&modern);

    let stream = chat
        .formatted_message_streams
        .get("msg-1")
        .expect("agent stream should exist");
    assert_eq!(stream.source(), "Hi ");
}

#[test]
fn repeated_modern_agent_deltas_still_append() {
    let mut chat = ChatPaneState::new(DEFAULT_THREAD_MODEL);
    let modern = serde_json::json!({
        "method": "item/agentMessage/delta",
        "params": {
            "itemId": "msg-1",
            "delta": "go "
        }
    });

    chat.apply_formatted_event(&modern);
    chat.apply_formatted_event(&modern);

    let stream = chat
        .formatted_message_streams
        .get("msg-1")
        .expect("agent stream should exist");
    assert_eq!(stream.source(), "go go ");
}

#[test]
fn inbox_list_toggle_opens_and_closes_pane() {
    let mut root = MinimalRoot::new();
    let screen = Size::new(1280.0, 720.0);

    root.toggle_inbox_list_pane(screen);
    let pane = root
        .pane_store
        .pane("inbox-list")
        .expect("pane should open");
    assert_eq!(pane.kind, PaneKind::InboxList);

    root.toggle_inbox_list_pane(screen);
    assert!(root.pane_store.pane("inbox-list").is_none());
}

#[test]
fn inbox_snapshot_event_updates_state() {
    let mut root = MinimalRoot::new();
    let snapshot = InboxSnapshot {
        threads: vec![
            InboxThreadSummary {
                id: "thread-1".to_string(),
                subject: "Subject".to_string(),
                from_address: "sender@example.com".to_string(),
                snippet: "snippet".to_string(),
                category: "ops".to_string(),
                risk: "low".to_string(),
                policy: "send_with_approval".to_string(),
                draft_preview: "draft".to_string(),
                pending_approval: true,
                updated_at: "2026-02-21T00:00:00Z".to_string(),
            },
            InboxThreadSummary {
                id: "thread-2".to_string(),
                subject: "Subject 2".to_string(),
                from_address: "sender2@example.com".to_string(),
                snippet: "snippet 2".to_string(),
                category: "sales".to_string(),
                risk: "medium".to_string(),
                policy: "draft_only".to_string(),
                draft_preview: "draft 2".to_string(),
                pending_approval: false,
                updated_at: "2026-02-21T00:00:01Z".to_string(),
            },
        ],
        selected_thread_id: Some("thread-1".to_string()),
        audit_log: vec![InboxAuditEntry {
            thread_id: "thread-1".to_string(),
            action: "select_thread".to_string(),
            detail: "selected".to_string(),
            created_at: "2026-02-21T00:00:02Z".to_string(),
        }],
    };

    root.apply_event(AppEvent::InboxUpdated {
        snapshot,
        source: "test".to_string(),
    });

    assert_eq!(root.inbox.threads.len(), 2);
    assert_eq!(
        root.inbox
            .selected_thread()
            .map(|thread| thread.id.as_str()),
        Some("thread-1")
    );
    assert_eq!(root.inbox.pending_threads().len(), 1);
    assert_eq!(root.inbox.audit_log.len(), 1);
    assert_eq!(root.inbox.source.as_deref(), Some("test"));
}

#[test]
fn inbox_row_hit_testing_returns_thread_id() {
    let mut inbox = InboxPaneState::default();
    inbox.list_row_bounds = vec![(
        "thread-hit".to_string(),
        Bounds::new(10.0, 20.0, 100.0, 30.0),
    )];

    assert_eq!(
        inbox.row_at(Point::new(50.0, 35.0)).as_deref(),
        Some("thread-hit")
    );
    assert!(inbox.row_at(Point::new(200.0, 200.0)).is_none());
}

#[test]
fn route_state_tracks_codex_and_inbox_context_together() {
    let mut root = MinimalRoot::new();
    let screen = Size::new(1280.0, 720.0);
    let chat_id = root.open_chat_pane(screen, true, false, DEFAULT_THREAD_MODEL);
    if let Some(chat) = root.chat_panes.get_mut(&chat_id) {
        chat.thread_id = Some("codex-thread-1".to_string());
    }

    root.apply_event(AppEvent::InboxUpdated {
        snapshot: InboxSnapshot {
            threads: vec![InboxThreadSummary {
                id: "inbox-thread-1".to_string(),
                subject: "Inbox Subject".to_string(),
                from_address: "sender@example.com".to_string(),
                snippet: "snippet".to_string(),
                category: "ops".to_string(),
                risk: "low".to_string(),
                policy: "draft_only".to_string(),
                draft_preview: "draft".to_string(),
                pending_approval: true,
                updated_at: "2026-02-21T00:00:00Z".to_string(),
            }],
            selected_thread_id: Some("inbox-thread-1".to_string()),
            audit_log: Vec::new(),
        },
        source: "test".to_string(),
    });

    root.toggle_inbox_list_pane(screen);
    root.pane_store.bring_to_front("inbox-list");
    root.sync_route_state();

    assert_eq!(root.route_state.active_surface, DesktopSurfaceRoute::Inbox);
    assert_eq!(
        root.route_state.codex_thread_id.as_deref(),
        Some("codex-thread-1")
    );
    assert_eq!(
        root.route_state.inbox_thread_id.as_deref(),
        Some("inbox-thread-1")
    );
    assert_eq!(root.route_state.inbox_pane, Some(InboxRoutePane::List));
}

#[test]
fn route_state_switches_surface_without_losing_other_domain_context() {
    let mut root = MinimalRoot::new();
    let screen = Size::new(1280.0, 720.0);
    let chat_id = root.open_chat_pane(screen, true, false, DEFAULT_THREAD_MODEL);
    if let Some(chat) = root.chat_panes.get_mut(&chat_id) {
        chat.thread_id = Some("codex-thread-2".to_string());
    }
    root.toggle_inbox_list_pane(screen);
    root.inbox.selected_thread_id = Some("inbox-thread-2".to_string());

    root.pane_store.bring_to_front(&chat_id);
    root.sync_route_state();

    assert_eq!(root.route_state.active_surface, DesktopSurfaceRoute::Codex);
    assert_eq!(
        root.route_state.codex_thread_id.as_deref(),
        Some("codex-thread-2")
    );
    assert_eq!(
        root.route_state.inbox_thread_id.as_deref(),
        Some("inbox-thread-2")
    );
}
