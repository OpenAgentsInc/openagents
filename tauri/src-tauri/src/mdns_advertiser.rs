//! mDNS/Bonjour service advertisement for desktop platforms
//!
//! Advertises the Tinyvex WebSocket server via mDNS so mobile devices
//! can automatically discover it on the local network.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::sync::Once;
use tracing::{error, info};

static START_ADVERTISING: Once = Once::new();

const SERVICE_TYPE: &str = "_openagents._tcp.local.";
const SERVICE_NAME: &str = "OpenAgents Desktop";

/// Start advertising the Tinyvex WebSocket server via mDNS
///
/// This allows mobile devices to discover the desktop server automatically
/// using Bonjour/mDNS service discovery.
pub fn start_advertising(port: u16) -> anyhow::Result<()> {
    let mut result = Ok(());

    START_ADVERTISING.call_once(|| {
        match try_start_advertising(port) {
            Ok(()) => {
                info!("mDNS advertising started successfully");
            }
            Err(e) => {
                error!(?e, "Failed to start mDNS advertising");
                result = Err(e);
            }
        }
    });

    result
}

fn try_start_advertising(port: u16) -> anyhow::Result<()> {
    // Create mDNS daemon
    let mdns = ServiceDaemon::new()?;

    // Get hostname for the service - must end with .local. for mDNS
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();
    let hostname_local = if hostname.ends_with(".local.") {
        hostname
    } else {
        format!("{}.local.", hostname)
    };

    // Create service info
    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        SERVICE_NAME,
        &hostname_local,
        (), // No specific IP addresses - use all interfaces
        port,
        None, // No TXT records needed for now
    )?;

    // Register the service
    mdns.register(service_info)?;

    info!(
        service_type = SERVICE_TYPE,
        service_name = SERVICE_NAME,
        port = port,
        hostname = %hostname_local,
        "mDNS service registered"
    );

    // Keep the daemon alive by leaking it (it will run for the lifetime of the app)
    std::mem::forget(mdns);

    Ok(())
}
