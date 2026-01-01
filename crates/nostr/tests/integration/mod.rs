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
use std::sync::Once;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::runtime::Builder;
use tokio::sync::oneshot;
use tokio::time::{Duration, Instant, sleep, timeout};
use tokio_tungstenite::connect_async;

static NEXT_PORT: AtomicU16 = AtomicU16::new(17000);

fn init_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Set once at test startup before any threads are spawned.
        unsafe {
            std::env::set_var("NOSTR_QUEUE_DB_PATH", ":memory:");
        }
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();
    });
}

pub fn next_test_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

/// Test relay configuration
pub fn test_relay_config(port: u16) -> RelayConfig {
    RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    }
}

/// Start a test relay server on the given port
pub async fn start_test_relay(port: u16) -> (Arc<RelayServer>, SocketAddr, tempfile::TempDir) {
    init_tracing();
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

    // Start server in background on its own runtime
    let server_clone = Arc::clone(&server);
    let (err_tx, mut err_rx) = oneshot::channel();
    std::thread::spawn(move || {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build relay runtime");
        runtime.block_on(async move {
            if let Err(err) = server_clone.start().await {
                let _ = err_tx.send(err);
            }
        });
    });

    // Wait for server to start or report error
    let start_deadline = Instant::now() + Duration::from_secs(2);
    let ws_url = test_relay_url(bind_addr.port());
    loop {
        if Instant::now() > start_deadline {
            panic!("Test relay failed to start on {}", bind_addr);
        }

        tokio::select! {
            result = &mut err_rx => {
                if let Ok(err) = result {
                    panic!("Test relay failed to start on {}: {}", bind_addr, err);
                }
            }
            _ = sleep(Duration::from_millis(50)) => {}
        }

        if let Ok(Ok((mut ws_stream, _))) = timeout(
            Duration::from_millis(200),
            connect_async(&ws_url),
        )
        .await
        {
            let _ = ws_stream.close(None).await;
            break;
        }
    }

    (server, bind_addr, temp_dir)
}

/// Create a test relay URL for the given port
pub fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}
