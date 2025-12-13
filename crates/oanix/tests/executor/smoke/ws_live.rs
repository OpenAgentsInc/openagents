//! Live WebSocket smoke tests against public echo servers
//!
//! Run with: `cargo test --features "net-executor,nostr" -p oanix -- --ignored ws_live`

use crate::fixtures::{ExecutorTestFixture, wait_for_ws_message, wait_for_ws_state};
use oanix::executor::ExecutorConfig;
use oanix::services::WsState;
use std::time::Duration;

fn live_test_config() -> ExecutorConfig {
    ExecutorConfig::builder()
        .poll_interval(Duration::from_millis(50))
        .ws_connect_timeout(Duration::from_secs(10))
        .ws_ping_interval(Duration::from_secs(30))
        .build()
}

/// Public WebSocket echo servers to try
const ECHO_SERVERS: &[&str] = &[
    "wss://ws.ifelse.io",
    "wss://echo.websocket.events",
    // "wss://echo.websocket.org", // Often down
];

/// Live test: Connect to public echo server and exchange messages
#[tokio::test]
#[ignore]
async fn test_ws_live_echo() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    let mut connected = false;
    let mut working_url = String::new();

    // Try each echo server
    for url in ECHO_SERVERS {
        println!("Trying echo server: {}", url);

        let conn_id = fixture.ws_fs.open_connection(*url).unwrap();

        // Wait for connection (10 seconds)
        if wait_for_ws_state(
            &fixture.ws_fs,
            &conn_id,
            WsState::Open,
            Duration::from_secs(10),
        )
        .await
        {
            println!("Connected to {}", url);
            working_url = url.to_string();

            // Send a message
            let test_msg = "Hello from OANIX E2E test!";
            fixture
                .ws_fs
                .send_message(&conn_id, test_msg.as_bytes().to_vec())
                .unwrap();

            // Wait for echo
            if wait_for_ws_message(&fixture.ws_fs, &conn_id, Duration::from_secs(5)).await {
                let msg = fixture.ws_fs.read_message(&conn_id).unwrap();
                if let Some(data) = msg {
                    let text = String::from_utf8_lossy(&data);
                    println!("Received echo: {}", text);

                    // Close connection
                    fixture.ws_fs.close_connection(&conn_id).unwrap();
                    connected = true;
                    break;
                }
            }

            // If we didn't get a message, try next server
            fixture.ws_fs.close_connection(&conn_id).unwrap();
        } else {
            println!("Could not connect to {}", url);
        }
    }

    assert!(
        connected,
        "Should connect to at least one public WebSocket echo server. Tried: {:?}",
        ECHO_SERVERS
    );
    println!("Live WebSocket test passed using {}", working_url);

    fixture.shutdown().unwrap();
}

/// Live test: WebSocket binary message handling
#[tokio::test]
#[ignore]
async fn test_ws_live_binary() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    // Try to connect to an echo server
    for url in ECHO_SERVERS {
        let conn_id = fixture.ws_fs.open_connection(*url).unwrap();

        if wait_for_ws_state(
            &fixture.ws_fs,
            &conn_id,
            WsState::Open,
            Duration::from_secs(10),
        )
        .await
        {
            // Send binary data
            let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
            fixture
                .ws_fs
                .send_message(&conn_id, binary_data.clone())
                .unwrap();

            // Wait for echo
            if wait_for_ws_message(&fixture.ws_fs, &conn_id, Duration::from_secs(5)).await {
                let msg = fixture.ws_fs.read_message(&conn_id).unwrap();
                if let Some(data) = msg {
                    println!("Received {} bytes", data.len());
                    // Some servers may modify binary data, just verify we got something
                    assert!(!data.is_empty(), "Should receive binary echo");

                    fixture.ws_fs.close_connection(&conn_id).unwrap();
                    fixture.shutdown().unwrap();
                    println!("Live WebSocket binary test passed!");
                    return;
                }
            }

            fixture.ws_fs.close_connection(&conn_id).unwrap();
        }
    }

    panic!("Could not complete binary test on any echo server");
}

/// Live test: Multiple sequential messages
#[tokio::test]
#[ignore]
async fn test_ws_live_multiple_messages() {
    let mut fixture = ExecutorTestFixture::new(live_test_config());
    fixture.start().unwrap();

    // Try to connect
    for url in ECHO_SERVERS {
        let conn_id = fixture.ws_fs.open_connection(*url).unwrap();

        if wait_for_ws_state(
            &fixture.ws_fs,
            &conn_id,
            WsState::Open,
            Duration::from_secs(10),
        )
        .await
        {
            let mut success_count = 0;

            // Send multiple messages
            for i in 0..5 {
                fixture
                    .ws_fs
                    .send_message(&conn_id, format!("Message {}", i).into_bytes())
                    .unwrap();
            }

            // Wait a bit for all echoes
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Read all available messages
            while let Ok(Some(_msg)) = fixture.ws_fs.read_message(&conn_id) {
                success_count += 1;
            }

            if success_count > 0 {
                println!("Received {} echo messages from {}", success_count, url);
                fixture.ws_fs.close_connection(&conn_id).unwrap();
                fixture.shutdown().unwrap();
                println!("Live WebSocket multiple messages test passed!");
                return;
            }

            fixture.ws_fs.close_connection(&conn_id).unwrap();
        }
    }

    panic!("Could not complete multiple messages test on any echo server");
}
