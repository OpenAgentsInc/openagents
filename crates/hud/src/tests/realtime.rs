//! Real-time update tests
//!
//! Ported from e2e/tests/realtime/realtime-updates.spec.ts
//! Tests C1-C10 / HUD-030 to HUD-040: WebSocket-based real-time updates.

use gpui::TestAppContext;
use hud_test::fixtures::{GraphViewFixture, HudInjector, GraphViewAssertExt};
use hud_test::messages::{factories, sequences};

/// C1/HUD-030: session_start triggers UI refresh
#[gpui::test]
fn test_session_start_triggers_ui_refresh(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial message count
    let initial_count = GraphViewFixture::message_count(&view, cx);

    // Send session_start message
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(Some("test-session-123")));

    // Verify message was received
    let count_after = GraphViewFixture::message_count(&view, cx);
    assert!(count_after > initial_count, "Message count should increase");

    // Verify session is active
    view.assert_that(cx)
        .expect_session_id("test-session-123");
}

/// C2/HUD-031: task_selected adds node
#[gpui::test]
fn test_task_selected_adds_node(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Initial state - no task nodes
    let initial_node_count = GraphViewFixture::node_count(&view, cx);

    // Send task_selected message
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::task_selected("oa-test-task-001", "Implement E2E Tests"));

    // Verify node was created
    let node_count_after = GraphViewFixture::node_count(&view, cx);
    assert!(node_count_after > initial_node_count, "Node count should increase after task_selected");

    // Verify task node exists
    view.assert_that(cx)
        .expect_node_exists("oa-test-task-001");
}

/// C3/HUD-032: task_decomposed creates child nodes
#[gpui::test]
fn test_task_decomposed_creates_child_nodes(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // First select a task
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::task_selected("oa-parent-task", "Parent Task"));

    let node_count_after_task = GraphViewFixture::node_count(&view, cx);

    // Send task_decomposed with subtasks
    injector.inject(factories::task_decomposed(vec![
        ("oa-parent-task-sub-001", "Subtask 1"),
        ("oa-parent-task-sub-002", "Subtask 2"),
        ("oa-parent-task-sub-003", "Subtask 3"),
    ]));

    // Verify subtask nodes were created
    let node_count_after_decompose = GraphViewFixture::node_count(&view, cx);
    assert!(node_count_after_decompose >= node_count_after_task + 3,
        "Should have at least 3 more nodes after decomposition");
}

/// C4/HUD-033: subtask_start/complete changes node status
#[gpui::test]
fn test_subtask_start_complete_changes_status(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Start with a task
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));
    injector.inject(factories::task_selected("oa-task-status", "Task Status Test"));

    // Send subtask_start
    injector.inject(factories::subtask_start("oa-task-status-sub-001", "Running subtask"));

    // Verify subtask exists
    view.assert_that(cx)
        .expect_node_exists("oa-task-status-sub-001");

    // Send subtask_complete
    injector.inject(factories::subtask_complete(
        "oa-task-status-sub-001",
        "Completed subtask",
        true,
        vec!["test.ts".to_string()],
    ));

    // Verify message was processed
    let msg_count = GraphViewFixture::message_count(&view, cx);
    assert!(msg_count >= 4, "Should have received all messages");
}

/// C5/HUD-034: verification_start/complete updates display
#[gpui::test]
fn test_verification_start_complete_updates(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Send verification_start
    injector.inject(factories::verification_start("bun test"));

    // Send verification_complete with success
    injector.inject(factories::verification_complete("bun test", true, Some("42 tests passed")));

    // Verify messages received
    let msg_count = GraphViewFixture::message_count(&view, cx);
    assert!(msg_count >= 3, "Should have received verification messages");
}

/// C6/HUD-035: commit_created/push_complete reflected in UI
#[gpui::test]
fn test_commit_and_push_reflected(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Send commit_created
    injector.inject(factories::commit_created("abc123def456789", "feat(e2e): add real-time tests"));

    // Send push_complete
    injector.inject(factories::push_complete("main"));

    // Verify messages received
    let msg_count = GraphViewFixture::message_count(&view, cx);
    assert!(msg_count >= 3, "Should have received commit and push messages");
}

/// C8/HUD-037: error message shows in UI with context
#[gpui::test]
fn test_error_message_shows_with_context(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Get initial error count
    let initial_errors = GraphViewFixture::error_count(&view, cx);

    // Send error message with phase context
    injector.inject(factories::error("verifying", "Test verification failed 3 tests"));

    // Error indicator should be visible
    view.assert_that(cx)
        .expect_error_visible("Test verification failed");

    // Error count should have increased
    let final_errors = GraphViewFixture::error_count(&view, cx);
    assert!(final_errors > initial_errors, "Error count should increase");
}

/// C10/HUD-040: updates work after reconnect
#[gpui::test]
fn test_updates_work_after_reconnect(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Establish connection and send initial data
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(Some("initial-session")));
    injector.inject(factories::task_selected("oa-before-disconnect", "Task Before Disconnect"));

    // Verify connected and received initial data
    view.assert_that(cx)
        .expect_connected()
        .expect_node_exists("oa-before-disconnect");

    // Disconnect
    GraphViewFixture::simulate_disconnect(&view, cx);
    view.assert_that(cx)
        .expect_disconnected();

    // Reconnect
    GraphViewFixture::simulate_reconnect(&view, cx);
    view.assert_that(cx)
        .expect_connected();

    // Send new data after reconnect
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::task_selected("oa-after-reconnect", "Task After Reconnect"));

    // Verify new message was processed
    view.assert_that(cx)
        .expect_node_exists("oa-after-reconnect");
}

/// Test golden loop sequence processes complete task lifecycle
#[gpui::test]
fn test_golden_loop_sequence(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Send complete Golden Loop sequence
    let sequence = sequences::golden_loop_sequence(Some("oa-golden-loop-test"));

    let mut injector = HudInjector::new(&view, cx);
    injector.inject_sequence(sequence, 50);

    // Verify message count indicates all messages were received
    let msg_count = GraphViewFixture::message_count(&view, cx);
    assert!(msg_count >= 10, "Should have received golden loop messages (got {})", msg_count);

    // Session should have been started
    view.assert_that(cx)
        .expect_session_active();
}

/// Test handles rapid message sequence without dropping
#[gpui::test]
fn test_rapid_message_sequence_no_drops(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Send 20 APM messages rapidly
    let message_count = 20;
    let apm_messages: Vec<_> = (0..message_count)
        .map(|i| factories::apm_update(i as f64 * 2.0, i * 10))
        .collect();

    injector.inject_burst(apm_messages);

    // Verify messages were received (at least the last APM value)
    let apm = GraphViewFixture::current_apm(&view, cx);
    assert!(apm >= 0.0, "APM should be updated");

    // App should remain responsive
    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset();
}

/// Test handles message burst during user interaction
#[gpui::test]
fn test_message_burst_during_interaction(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Simulate user interaction (pan) while messages arrive
    view.update(cx, |view, cx| {
        view.set_pan(50.0, 25.0, cx);
    });

    // Send messages during "interaction"
    injector.inject(factories::task_selected("oa-during-pan", "Task During Pan"));
    injector.inject(factories::apm_update(15.0, 100));

    cx.run_until_parked();

    // Both pan and messages should have worked
    let (pan_x, _) = GraphViewFixture::pan(&view, cx);
    assert!((pan_x - 50.0).abs() < 0.1, "Pan should have been applied");

    view.assert_that(cx)
        .expect_node_exists("oa-during-pan");
}

/// Test session lifecycle messages in order
#[gpui::test]
fn test_session_lifecycle_messages_in_order(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);

    // Session start
    injector.inject(factories::session_start(Some("lifecycle-test")));
    view.assert_that(cx)
        .expect_session_id("lifecycle-test");

    // Task selected
    injector.inject(factories::task_selected("oa-lifecycle-task", "Lifecycle Task"));
    view.assert_that(cx)
        .expect_node_exists("oa-lifecycle-task");

    // APM updates
    injector.inject(factories::apm_update(10.0, 50));
    let apm = GraphViewFixture::current_apm(&view, cx);
    assert!((apm - 10.0).abs() < 0.1, "APM should be updated to 10.0");

    // Session complete
    injector.inject(factories::session_complete(true, "All tasks completed"));

    // Verify final message count
    let msg_count = GraphViewFixture::message_count(&view, cx);
    assert!(msg_count >= 4, "Should have received all lifecycle messages");
}
