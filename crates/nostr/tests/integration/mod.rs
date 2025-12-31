//! Integration tests for nostr client-relay communication
//!
//! These tests verify that the nostr-client and nostr-relay crates work together correctly.

pub mod client_relay;
pub mod error_handling;
pub mod filters;
pub mod nip01_protocol_messages;
pub mod nip_sa;
pub mod pool;
pub mod public_relays;
pub mod stress;
pub mod subscriptions;

use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Test relay configuration
pub fn test_relay_config(port: u16) -> RelayConfig {
    RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    }
}

/// Start a test relay server on the given port
pub async fn start_test_relay(port: u16) -> (Arc<RelayServer>, SocketAddr, tempfile::TempDir) {
    let config = test_relay_config(port);
    let bind_addr = config.bind_addr;

    // Create temp dir for this test
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_config = DatabaseConfig {
        path: db_path,
        ..Default::default()
    };

    let db = Database::new(db_config).unwrap();
    let server = Arc::new(RelayServer::new(config, db));

    // Start server in background
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        server_clone.start().await.ok();
    });

    // Give server time to start
    sleep(Duration::from_millis(200)).await;

    (server, bind_addr, temp_dir)
}

/// Create a test relay URL for the given port
pub fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}
