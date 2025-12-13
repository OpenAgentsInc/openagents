//! E2E tests for WsConnector
//!
//! Note: These tests use `#[test]` instead of `#[tokio::test]` because
//! ExecutorManager creates its own tokio runtime. All async operations
//! are run via `fixture.block_on()`.

use crate::fixtures::{
    ExecutorTestFixture, WsDisconnectingServer, WsEchoServer, fast_test_config,
    timeout_test_config, wait_for_ws_message, wait_for_ws_state,
};
use oanix::services::WsState;
use std::time::Duration;

/// Test basic WebSocket connection and echo
#[test]
fn test_ws_connect_and_echo() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let echo_server = fixture.block_on(WsEchoServer::start());

    fixture.start().unwrap();

    // Open connection via WsFs
    let conn_id = fixture.ws_fs.open_connection(echo_server.url()).unwrap();

    // Wait for connected state
    let ws_fs = &fixture.ws_fs;
    let connected = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Open,
        Duration::from_secs(5),
    ));
    assert!(connected, "Connection should become Open");

    // Send message
    fixture
        .ws_fs
        .send_message(&conn_id, b"Hello, WebSocket!".to_vec())
        .unwrap();

    // Wait for echo response
    let ws_fs = &fixture.ws_fs;
    let got_msg = fixture.block_on(wait_for_ws_message(ws_fs, &conn_id, Duration::from_secs(5)));
    assert!(got_msg, "Should receive echo message");

    // Read the message
    let msg = fixture.ws_fs.read_message(&conn_id).unwrap();
    assert!(msg.is_some());
    let data = msg.unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(
        text.contains("Hello"),
        "Echo should contain original message: {}",
        text
    );

    // Close connection
    fixture.ws_fs.close_connection(&conn_id).unwrap();
    let ws_fs = &fixture.ws_fs;
    let closed = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Closed,
        Duration::from_secs(5),
    ));
    assert!(closed, "Connection should become Closed");

    fixture.block_on(echo_server.shutdown());
    fixture.shutdown().unwrap();
}

/// Test connection timeout to non-existent server
#[test]
fn test_ws_connection_timeout() {
    let mut fixture = ExecutorTestFixture::new(timeout_test_config());
    fixture.start().unwrap();

    // Connect to a non-existent server (should timeout/fail)
    let conn_id = fixture
        .ws_fs
        .open_connection("ws://127.0.0.1:59999")
        .unwrap();

    // Wait for error state
    let ws_fs = &fixture.ws_fs;
    let got_error = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Error,
        Duration::from_secs(5),
    ));
    assert!(got_error, "Connection should fail with Error state");

    let info = fixture.ws_fs.get_connection(&conn_id).unwrap();
    assert!(info.error.is_some(), "Error message should be present");

    fixture.shutdown().unwrap();
}

/// Test multiple concurrent connections
#[test]
fn test_ws_multiple_connections() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let echo_server = fixture.block_on(WsEchoServer::start());

    fixture.start().unwrap();

    let mut conn_ids = Vec::new();
    for _ in 0..5 {
        let conn_id = fixture.ws_fs.open_connection(echo_server.url()).unwrap();
        conn_ids.push(conn_id);
    }

    // Wait for all to connect
    for conn_id in &conn_ids {
        let ws_fs = &fixture.ws_fs;
        let connected = fixture.block_on(wait_for_ws_state(
            ws_fs,
            conn_id,
            WsState::Open,
            Duration::from_secs(5),
        ));
        assert!(connected, "Connection {} should become Open", conn_id);
    }

    // Send message on each
    for (i, conn_id) in conn_ids.iter().enumerate() {
        fixture
            .ws_fs
            .send_message(conn_id, format!("msg-{}", i).into_bytes())
            .unwrap();
    }

    // Wait for echoes
    fixture.block_on(async { tokio::time::sleep(Duration::from_millis(500)).await });

    // Verify each received its echo
    for (i, conn_id) in conn_ids.iter().enumerate() {
        let msg = fixture.ws_fs.read_message(conn_id).unwrap();
        assert!(msg.is_some(), "Connection {} should have a message", i);
        let data = msg.unwrap();
        let text = String::from_utf8_lossy(&data);
        assert!(
            text.contains(&format!("msg-{}", i)),
            "Echo {} should contain original: {}",
            i,
            text
        );
    }

    // Close all connections
    for conn_id in &conn_ids {
        fixture.ws_fs.close_connection(conn_id).unwrap();
    }

    fixture.block_on(echo_server.shutdown());
    fixture.shutdown().unwrap();
}

/// Test message FIFO order preservation
#[test]
fn test_ws_message_fifo_order() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let echo_server = fixture.block_on(WsEchoServer::start());

    fixture.start().unwrap();

    let conn_id = fixture.ws_fs.open_connection(echo_server.url()).unwrap();
    let ws_fs = &fixture.ws_fs;
    let connected = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Open,
        Duration::from_secs(5),
    ));
    assert!(connected);

    // Send numbered messages
    for i in 0..10 {
        fixture
            .ws_fs
            .send_message(&conn_id, format!("msg-{}", i).into_bytes())
            .unwrap();
    }

    // Wait for all echoes
    fixture.block_on(async { tokio::time::sleep(Duration::from_millis(500)).await });

    // Verify order
    for i in 0..10 {
        let msg = fixture.ws_fs.read_message(&conn_id).unwrap();
        assert!(msg.is_some(), "Message {} should be present", i);
        let data = msg.unwrap();
        let text = String::from_utf8_lossy(&data);
        assert!(
            text.contains(&format!("msg-{}", i)),
            "Message {} should be in order: {}",
            i,
            text
        );
    }

    fixture.block_on(echo_server.shutdown());
    fixture.shutdown().unwrap();
}

/// Test server-initiated close
#[test]
fn test_ws_server_close() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    // Server that disconnects after 2 messages
    let server = fixture.block_on(WsDisconnectingServer::start(2));

    fixture.start().unwrap();

    let conn_id = fixture.ws_fs.open_connection(server.url()).unwrap();
    let ws_fs = &fixture.ws_fs;
    let connected = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Open,
        Duration::from_secs(5),
    ));
    assert!(connected);

    // Send text messages (server will close after 2)
    // Note: WsDisconnectingServer only counts text messages
    fixture
        .ws_fs
        .send_message(&conn_id, "msg-1".as_bytes().to_vec())
        .unwrap();
    fixture
        .ws_fs
        .send_message(&conn_id, "msg-2".as_bytes().to_vec())
        .unwrap();

    // Wait for server to close the connection
    let ws_fs = &fixture.ws_fs;
    let closed = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Closed,
        Duration::from_secs(5),
    ));
    assert!(closed, "Connection should be closed by server");

    fixture.block_on(server.shutdown());
    fixture.shutdown().unwrap();
}

/// Test binary message handling
#[test]
fn test_ws_binary_messages() {
    let mut fixture = ExecutorTestFixture::new(fast_test_config());

    let echo_server = fixture.block_on(WsEchoServer::start());

    fixture.start().unwrap();

    let conn_id = fixture.ws_fs.open_connection(echo_server.url()).unwrap();
    let ws_fs = &fixture.ws_fs;
    let connected = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn_id,
        WsState::Open,
        Duration::from_secs(5),
    ));
    assert!(connected);

    // Send binary data
    let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    fixture
        .ws_fs
        .send_message(&conn_id, binary_data.clone())
        .unwrap();

    // Wait for echo
    let ws_fs = &fixture.ws_fs;
    let got_msg = fixture.block_on(wait_for_ws_message(ws_fs, &conn_id, Duration::from_secs(5)));
    assert!(got_msg);

    // Verify binary data is echoed correctly
    let msg = fixture.ws_fs.read_message(&conn_id).unwrap().unwrap();
    assert_eq!(msg, binary_data, "Binary data should be echoed exactly");

    fixture.block_on(echo_server.shutdown());
    fixture.shutdown().unwrap();
}

/// Test connection count limit
#[test]
fn test_ws_connection_limit() {
    // Config with max 3 connections
    let config = oanix::executor::ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(10))
        .ws_connect_timeout(Duration::from_secs(2))
        .ws_max_concurrent(3)
        .build();

    let mut fixture = ExecutorTestFixture::new(config);

    let echo_server = fixture.block_on(WsEchoServer::start());

    fixture.start().unwrap();

    // Open 3 connections (should all succeed)
    let mut conn_ids = Vec::new();
    for _ in 0..3 {
        let conn_id = fixture.ws_fs.open_connection(echo_server.url()).unwrap();
        conn_ids.push(conn_id);
    }

    // Wait for all to connect
    for conn_id in &conn_ids {
        let ws_fs = &fixture.ws_fs;
        let connected = fixture.block_on(wait_for_ws_state(
            ws_fs,
            conn_id,
            WsState::Open,
            Duration::from_secs(5),
        ));
        assert!(connected);
    }

    // 4th connection should stay in Connecting (over limit)
    let conn4 = fixture.ws_fs.open_connection(echo_server.url()).unwrap();

    // Give it time, it should NOT become Open
    fixture.block_on(async { tokio::time::sleep(Duration::from_millis(500)).await });

    let info = fixture.ws_fs.get_connection(&conn4).unwrap();
    assert_eq!(
        info.state,
        WsState::Connecting,
        "4th connection should stay Connecting due to limit"
    );

    // Close one, 4th should now connect
    fixture.ws_fs.close_connection(&conn_ids[0]).unwrap();

    let ws_fs = &fixture.ws_fs;
    let connected = fixture.block_on(wait_for_ws_state(
        ws_fs,
        &conn4,
        WsState::Open,
        Duration::from_secs(5),
    ));
    assert!(connected, "4th connection should connect after slot freed");

    fixture.block_on(echo_server.shutdown());
    fixture.shutdown().unwrap();
}
