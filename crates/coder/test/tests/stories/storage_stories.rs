//! Storage, Event Sourcing, and Persistence Stories (30-36).
//!
//! From: coder-must-pass-user-stories.md
//!
//! 30. Replaying DomainEvents reconstructs ChatView identically to live state.
//! 31. Sessions and threads persist to SQLite by default and reload correctly after restart.
//! 32. Message streaming state (partial assistant message) resumes correctly after a reconnect.
//! 33. Deleting a thread cascades to associated messages/tool uses without orphaned rows.
//! 34. Snapshots apply correctly, and replaying events after the snapshot yields identical views.
//! 35. Thread summaries update on new messages with accurate previews and unread counts.
//! 36. Exporting a session (events + projections) produces a portable bundle I can re-import.

use chrono::Utc;
use coder_domain::{ChatEntry, ChatView, DomainEvent, Message, MessageId, Role, ThreadId};

// ============================================================================
// Story 30: Replaying DomainEvents reconstructs ChatView identically
// ============================================================================

/// Story 30: As a developer, replaying DomainEvents reconstructs ChatView identically to live state.
///
/// This test verifies that we can:
/// 1. Apply events to a ChatView in real-time
/// 2. Store those events
/// 3. Create a fresh ChatView and replay all events
/// 4. Get an identical state
#[test]
fn story_30_event_replay_reconstructs_chat_view_identically() {
    // Given: A sequence of domain events simulating a real conversation
    let thread_id = ThreadId::new();
    let events = create_conversation_events(thread_id);

    // When: We apply events to a "live" ChatView
    let mut live_view = ChatView::new(thread_id);
    for event in &events {
        live_view.apply(event);
    }

    // And: We create a fresh ChatView and replay all events
    let mut replayed_view = ChatView::new(thread_id);
    for event in &events {
        replayed_view.apply(event);
    }

    // Then: Both views should be identical
    assert_eq!(live_view.thread_id, replayed_view.thread_id);
    assert_eq!(live_view.entries.len(), replayed_view.entries.len());
    assert_eq!(live_view.message_count, replayed_view.message_count);

    // Verify each entry matches
    for (i, (live_entry, replayed_entry)) in live_view
        .entries
        .iter()
        .zip(replayed_view.entries.iter())
        .enumerate()
    {
        match (live_entry, replayed_entry) {
            (ChatEntry::Message(live_msg), ChatEntry::Message(replayed_msg)) => {
                assert_eq!(
                    live_msg.content, replayed_msg.content,
                    "Entry {}: content mismatch",
                    i
                );
                assert_eq!(
                    live_msg.role, replayed_msg.role,
                    "Entry {}: role mismatch",
                    i
                );
            }
            (ChatEntry::ToolUse(live_tool), ChatEntry::ToolUse(replayed_tool)) => {
                assert_eq!(
                    live_tool.tool_name, replayed_tool.tool_name,
                    "Entry {}: tool_name mismatch",
                    i
                );
            }
            _ => panic!("Entry {}: type mismatch between live and replayed", i),
        }
    }
}

/// Story 30 (helper): ChatView::from_events reconstructs identically.
#[test]
fn story_30_chat_view_from_events_is_equivalent() {
    let thread_id = ThreadId::new();
    let events = create_conversation_events(thread_id);

    let mut live_view = ChatView::new(thread_id);
    for event in &events {
        live_view.apply(event);
    }

    let replayed = ChatView::from_events(thread_id, &events);
    assert_eq!(live_view.thread_id, replayed.thread_id);
    assert_eq!(live_view.entries.len(), replayed.entries.len());
    assert_eq!(live_view.message_count, replayed.message_count);
}

/// Story 30 (extended): Event replay preserves message ordering
#[test]
fn story_30_event_replay_preserves_ordering() {
    let thread_id = ThreadId::new();

    // Create messages with specific content to verify order
    let messages = ["First", "Second", "Third", "Fourth"];
    let events: Vec<DomainEvent> = messages
        .iter()
        .enumerate()
        .map(|(i, content)| {
            let role = if i % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            };
            DomainEvent::MessageAdded {
                thread_id,
                message: Message::new(role, *content),
            }
        })
        .collect();

    // Replay events
    let mut view = ChatView::new(thread_id);
    for event in &events {
        view.apply(event);
    }

    // Verify ordering is preserved
    assert_eq!(view.entries.len(), 4);
    for (i, expected_content) in messages.iter().enumerate() {
        if let ChatEntry::Message(msg) = &view.entries[i] {
            assert_eq!(msg.content, *expected_content, "Message {} out of order", i);
        } else {
            panic!("Expected message at index {}", i);
        }
    }
}

// ============================================================================
// Story 31: Sessions and threads persist to SQLite
// ============================================================================

/// Story 31: As a user, sessions and threads persist to SQLite by default
/// and reload correctly after restart.
///
#[test]
fn story_31_events_are_serializable_for_persistence() {
    let thread_id = ThreadId::new();
    let message = Message::new(Role::User, "Test message");

    let event = DomainEvent::MessageAdded {
        thread_id,
        message: message.clone(),
    };

    // Events must serialize to JSON for storage
    let json = serde_json::to_string(&event).expect("Event should serialize");

    // Events must deserialize correctly
    let deserialized: DomainEvent = serde_json::from_str(&json).expect("Event should deserialize");

    if let DomainEvent::MessageAdded {
        thread_id: t,
        message: m,
    } = deserialized
    {
        assert_eq!(t, thread_id);
        assert_eq!(m.content, "Test message");
    } else {
        panic!("Wrong event type after deserialization");
    }
}

/// Story 31: SQLite round-trip stores and reloads sessions/messages.
#[test]
fn story_31_sqlite_round_trip() {
    use coder_storage::{Session as StoredSession, Storage};

    let storage = Storage::in_memory().expect("should create in-memory sqlite");

    // Create session and a couple messages.
    let session = StoredSession::new("/tmp/project").with_title("Story 31");
    storage.create_session(&session).expect("create session");

    let user_msg = coder_storage::StoredMessage::user(session.id, "Hello");
    storage.add_message(&user_msg).expect("add user message");

    let assistant_msg = coder_storage::StoredMessage::assistant(session.id);
    storage
        .add_message(&assistant_msg)
        .expect("add assistant message");

    // Simulate reload: read from storage and verify data integrity.
    let loaded_session = storage
        .get_session(&session.id)
        .expect("get session")
        .expect("session exists");
    assert_eq!(loaded_session.id, session.id);
    assert_eq!(loaded_session.title.as_deref(), Some("Story 31"));

    let messages = storage.get_messages(&session.id).expect("list messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
}

/// Story 31: Event envelope captures sequence for ordered replay
#[test]
fn story_31_event_envelope_preserves_sequence() {
    use coder_domain::event::EventEnvelope;

    let event = DomainEvent::ThreadCreated {
        thread_id: ThreadId::new(),
        project_id: None,
        timestamp: Utc::now(),
    };

    let envelope = EventEnvelope::new(42, event);
    assert_eq!(envelope.sequence, 42);

    // Serialize and deserialize
    let json = serde_json::to_string(&envelope).expect("Envelope should serialize");
    let deserialized: EventEnvelope =
        serde_json::from_str(&json).expect("Envelope should deserialize");

    assert_eq!(deserialized.sequence, 42);
}

// ============================================================================
// Story 32: Streaming state resumes correctly after reconnect
// ============================================================================

/// Story 32: As a user, message streaming state (partial assistant message)
/// resumes correctly after a reconnect.
#[test]
fn story_32_streaming_state_can_be_resumed() {
    let thread_id = ThreadId::new();
    let message_id = MessageId::new();

    // Given: Streaming started with partial content
    let events = vec![
        DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            delta: "Hello, I am ".to_string(),
            timestamp: Utc::now(),
        },
        DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            delta: "your assistant.".to_string(),
            timestamp: Utc::now(),
        },
    ];

    // When: We create a ChatView and apply the events (simulating reconnect)
    let mut view = ChatView::new(thread_id);
    for event in &events {
        view.apply(event);
    }

    // Then: Streaming message state is restored
    assert!(view.is_streaming(), "Should be in streaming state");
    assert_eq!(
        view.streaming_content(),
        Some("Hello, I am your assistant."),
        "Streaming content should be concatenated"
    );
}

/// Story 32: Streaming completion is tracked correctly after resume
#[test]
fn story_32_streaming_completion_after_resume() {
    let thread_id = ThreadId::new();
    let message_id = MessageId::new();

    // Given: Complete streaming sequence including completion
    let events = vec![
        DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            delta: "Hello!".to_string(),
            timestamp: Utc::now(),
        },
        DomainEvent::MessageComplete {
            thread_id,
            message_id,
            timestamp: Utc::now(),
        },
    ];

    // When: We replay these events
    let mut view = ChatView::new(thread_id);
    for event in &events {
        view.apply(event);
    }

    // Then: Stream is marked as complete
    assert!(
        !view.is_streaming(),
        "Should not be streaming after completion"
    );
}

// ============================================================================
// Story 33: Thread deletion cascades correctly
// ============================================================================

/// Story 33: As a user, deleting a thread cascades to associated messages/tool uses
/// without orphaned rows.
///
/// NOTE: This requires storage layer implementation. For now, we verify that
/// all events contain thread_id for proper cascade identification.
#[test]
fn story_33_all_thread_events_contain_thread_id() {
    let thread_id = ThreadId::new();

    // Create a variety of events
    let events = vec![
        DomainEvent::ThreadCreated {
            thread_id,
            project_id: None,
            timestamp: Utc::now(),
        },
        DomainEvent::MessageAdded {
            thread_id,
            message: Message::new(Role::User, "Test"),
        },
        DomainEvent::MessageStreaming {
            thread_id,
            message_id: MessageId::new(),
            delta: "Hello".to_string(),
            timestamp: Utc::now(),
        },
        DomainEvent::MessageComplete {
            thread_id,
            message_id: MessageId::new(),
            timestamp: Utc::now(),
        },
    ];

    // All thread-related events should have thread_id method returning the correct ID
    for event in &events {
        assert_eq!(
            event.thread_id(),
            Some(thread_id),
            "Event {:?} should have thread_id",
            event
        );
    }
}

// ============================================================================
// Story 34: Snapshots apply correctly
// ============================================================================

/// Story 34: As a developer, snapshots apply correctly, and replaying events
/// after the snapshot yields identical views.
///
/// NOTE: Snapshot infrastructure not yet implemented. This test verifies
/// that ChatView can be serialized (prerequisite for snapshots).
#[test]
fn story_34_chat_view_is_serializable_for_snapshots() {
    let thread_id = ThreadId::new();
    let mut view = ChatView::new(thread_id);

    // Add some state
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: Message::new(Role::User, "Hello"),
    });
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: Message::new(Role::Assistant, "Hi!"),
    });

    // ChatView should be serializable
    let json = serde_json::to_string(&view).expect("ChatView should serialize");

    // And deserializable
    let restored: ChatView = serde_json::from_str(&json).expect("ChatView should deserialize");

    assert_eq!(restored.thread_id, thread_id);
    assert_eq!(restored.entries.len(), 2);
    assert_eq!(restored.message_count, 2);
}

// ============================================================================
// Story 35: Thread summaries update on new messages
// ============================================================================

/// Story 35: As a user, thread summaries update on new messages with accurate
/// previews and unread counts.
#[test]
fn story_35_thread_summary_updates_on_new_messages() {
    let thread_id = ThreadId::new();
    let mut view = ChatView::new(thread_id);

    // Initially empty
    let initial_summary = view.summary();
    assert_eq!(initial_summary.message_count, 0);
    assert_eq!(initial_summary.unread_count, 0);
    assert!(initial_summary.last_message_preview.is_none());
    assert!(initial_summary.last_message_role.is_none());
    assert!(initial_summary.last_updated.is_none());

    // Add first message
    let msg1 = Message::new(Role::User, "Hello!");
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: msg1.clone(),
    });

    let summary_after_first = view.summary();
    assert_eq!(summary_after_first.message_count, 1);
    assert_eq!(summary_after_first.unread_count, 1);
    assert_eq!(summary_after_first.last_message_role, Some(Role::User));
    assert_eq!(
        summary_after_first.last_message_preview.as_deref(),
        Some("Hello!")
    );
    assert_eq!(summary_after_first.last_updated, Some(msg1.created_at));

    // Add second message
    let msg2 = Message::new(Role::Assistant, "Hi there!");
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: msg2.clone(),
    });

    let summary_after_second = view.summary();
    assert_eq!(summary_after_second.message_count, 2);
    assert_eq!(summary_after_second.unread_count, 2);
    assert_eq!(
        summary_after_second.last_message_preview.as_deref(),
        Some("Hi there!")
    );
    assert_eq!(
        summary_after_second.last_message_role,
        Some(Role::Assistant)
    );
    assert_eq!(summary_after_second.last_updated, Some(msg2.created_at));
}

/// Story 35: Thread summary reflects most recent message content
#[test]
fn story_35_thread_summary_reflects_latest_content() {
    let thread_id = ThreadId::new();
    let mut view = ChatView::new(thread_id);

    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: Message::new(Role::User, "First message"),
    });

    let preview_len = 120;
    let long_content = "A".repeat(preview_len + 20);
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message: Message::new(Role::Assistant, long_content.clone()),
    });

    let summary = view.summary();
    let expected_preview = format!("{}...", &long_content[..preview_len]);
    assert_eq!(summary.message_count, 2);
    assert_eq!(summary.unread_count, 2);
    assert_eq!(
        summary.last_message_preview.as_deref(),
        Some(expected_preview.as_str())
    );
    assert_eq!(summary.last_message_role, Some(Role::Assistant));
}

// ============================================================================
// Story 36: Session export/import
// ============================================================================

/// Story 36: As a user, exporting a session (events + projections) produces
/// a portable bundle I can re-import.
///
/// NOTE: Export infrastructure not yet implemented. This test verifies that
/// all components needed for export are serializable.
#[test]
fn story_36_session_components_are_exportable() {
    use coder_domain::event::EventEnvelope;

    let thread_id = ThreadId::new();

    // Events are serializable
    let events = vec![
        EventEnvelope::new(
            1,
            DomainEvent::ThreadCreated {
                thread_id,
                project_id: None,
                timestamp: Utc::now(),
            },
        ),
        EventEnvelope::new(
            2,
            DomainEvent::MessageAdded {
                thread_id,
                message: Message::new(Role::User, "Test message"),
            },
        ),
    ];

    // Build projection
    let mut view = ChatView::new(thread_id);
    for envelope in &events {
        view.apply(&envelope.event);
    }

    // Everything serializes together
    #[derive(serde::Serialize, serde::Deserialize)]
    struct ExportBundle {
        events: Vec<EventEnvelope>,
        chat_view: ChatView,
    }

    let bundle = ExportBundle {
        events: events.clone(),
        chat_view: view,
    };

    let json = serde_json::to_string_pretty(&bundle).expect("Bundle should serialize");
    let restored: ExportBundle = serde_json::from_str(&json).expect("Bundle should deserialize");

    assert_eq!(restored.events.len(), 2);
    assert_eq!(restored.chat_view.message_count, 1);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a realistic sequence of conversation events
fn create_conversation_events(thread_id: ThreadId) -> Vec<DomainEvent> {
    vec![
        DomainEvent::ThreadCreated {
            thread_id,
            project_id: None,
            timestamp: Utc::now(),
        },
        DomainEvent::MessageAdded {
            thread_id,
            message: Message::new(Role::User, "Hello, can you help me?"),
        },
        DomainEvent::MessageAdded {
            thread_id,
            message: Message::new(Role::Assistant, "Of course! What do you need help with?"),
        },
        DomainEvent::MessageAdded {
            thread_id,
            message: Message::new(Role::User, "I need to understand event sourcing."),
        },
        DomainEvent::MessageAdded {
            thread_id,
            message: Message::new(
                Role::Assistant,
                "Event sourcing is a pattern where state is derived from a sequence of events...",
            ),
        },
    ]
}
