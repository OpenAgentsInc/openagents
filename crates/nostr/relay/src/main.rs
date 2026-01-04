//! Simple Nostr relay binary

use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure database (use temp file for testing)
    let db_config = DatabaseConfig {
        path: PathBuf::from("/tmp/nostr-relay-test.db"),
        max_reader_connections: 10,
        max_metadata_connections: 5,
    };
    let db = Database::new(db_config)?;

    // Configure relay (use 7001 to avoid conflict with macOS AirPlay on 7000)
    let config = RelayConfig {
        bind_addr: "127.0.0.1:7001".parse()?,
        max_message_size: 512 * 1024, // 512 KB
        ..Default::default()
    };

    println!("Starting Nostr relay on ws://127.0.0.1:7001");

    // Start relay
    let server = RelayServer::new(config, db);
    server.start().await?;

    Ok(())
}
