//! mDNS/Bonjour service discovery for mobile platforms
//!
//! Discovers Tinyvex WebSocket servers on the local network via mDNS.
//! Mobile devices use this to automatically find and connect to desktop servers.

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

const SERVICE_TYPE: &str = "_openagents._tcp.local.";
const DISCOVERY_TIMEOUT_SECS: u64 = 5;

/// Information about a discovered OpenAgents server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Human-readable name of the server
    pub name: String,
    /// IP address or hostname
    pub host: String,
    /// Port number
    pub port: u16,
    /// When this server was discovered (Unix timestamp in milliseconds)
    pub discovered_at: i64,
}

/// Discover OpenAgents servers on the local network
///
/// Scans for mDNS services of type "_openagents._tcp.local." and returns
/// all discovered servers within the timeout period.
pub async fn discover_servers() -> anyhow::Result<Vec<ServerInfo>> {
    info!("Starting mDNS service discovery");

    let mdns = ServiceDaemon::new()
        .map_err(|e| anyhow::anyhow!("Failed to create mDNS daemon: {}", e))?;

    // Browse for OpenAgents services
    let receiver = mdns.browse(SERVICE_TYPE)
        .map_err(|e| anyhow::anyhow!("Failed to browse mDNS services: {}", e))?;

    let mut servers = Vec::new();
    let timeout = Duration::from_secs(DISCOVERY_TIMEOUT_SECS);
    let start = std::time::Instant::now();

    // Collect discovered services until timeout
    while start.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        info!(
                            name = %info.get_fullname(),
                            hostname = %info.get_hostname(),
                            port = info.get_port(),
                            "Discovered OpenAgents server"
                        );

                        // Extract server information
                        let host = info.get_addresses()
                            .iter()
                            .next()
                            .map(|addr| addr.to_string())
                            .unwrap_or_else(|| info.get_hostname().to_string());

                        let server = ServerInfo {
                            name: info.get_fullname().to_string(),
                            host,
                            port: info.get_port(),
                            discovered_at: chrono::Utc::now().timestamp_millis(),
                        };

                        servers.push(server);
                    }
                    ServiceEvent::ServiceFound(_, fullname) => {
                        info!(service = %fullname, "Found OpenAgents service, resolving...");
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        warn!(service = %fullname, "OpenAgents service removed");
                    }
                    _ => {}
                }
            }
            Err(_) => {
                // Timeout on recv - just continue waiting until overall timeout
                continue;
            }
        }
    }

    info!(count = servers.len(), "mDNS discovery completed");

    // Shutdown the mDNS daemon
    if let Err(e) = mdns.shutdown() {
        warn!(?e, "Failed to shutdown mDNS daemon");
    }

    Ok(servers)
}

/// Test connection to a server
///
/// Attempts to establish a TCP connection to verify the server is reachable.
pub async fn test_connection(host: &str, port: u16) -> anyhow::Result<bool> {
    let addr = format!("{}:{}", host, port);
    info!(addr = %addr, "Testing connection to server");

    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(&addr)
    ).await {
        Ok(Ok(_)) => {
            info!(addr = %addr, "Connection successful");
            Ok(true)
        }
        Ok(Err(e)) => {
            warn!(addr = %addr, error = ?e, "Connection failed");
            Ok(false)
        }
        Err(_) => {
            warn!(addr = %addr, "Connection timeout");
            Ok(false)
        }
    }
}
