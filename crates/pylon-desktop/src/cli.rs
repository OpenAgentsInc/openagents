//! CLI mode - headless provider operation

use std::time::Duration;
use std::thread;
use crate::core::PylonCore;
use crate::state::{FmConnectionStatus, NostrConnectionStatus};

/// Run Pylon in headless CLI mode
pub fn run_cli_mode(relay_url: Option<String>) {
    println!("Pylon CLI Provider starting...");
    println!();

    let mut core = PylonCore::new();

    // Override relay URL if provided
    if let Some(url) = relay_url {
        core.state.relay_url = url;
    }

    println!("Bridge:  {}", core.state.bridge_url);
    println!("Relay:   {}", core.state.relay_url);
    println!("Pubkey:  {}", core.state.pubkey.as_deref().unwrap_or("unknown"));
    println!();

    // Check bridge status
    if core.state.connection_status == FmConnectionStatus::Error {
        eprintln!("ERROR: {}", core.state.error_message.as_deref().unwrap_or("Bridge failed"));
        return;
    }

    // Connect to FM Bridge
    println!("Connecting to FM Bridge...");
    core.connect_bridge();

    // Connect to Nostr relay
    println!("Connecting to Nostr relay...");
    core.connect_nostr();

    // Main event loop (synchronous)
    let mut last_status = String::new();
    loop {
        core.poll();

        // Print status changes
        let status = format_status(&core);
        if status != last_status {
            println!("{}", status);
            last_status = status;
        }

        // Print job activity
        if let Some(ref job_id) = core.state.current_job_id {
            let tokens = core.state.token_count;
            let tps = core.state.tokens_per_sec;
            print!("\rServing job {} | {} tokens | {:.1} t/s    ",
                &job_id[..8.min(job_id.len())], tokens, tps);
            std::io::Write::flush(&mut std::io::stdout()).ok();
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn format_status(core: &PylonCore) -> String {
    let fm = match core.state.connection_status {
        FmConnectionStatus::Disconnected => "FM:OFF",
        FmConnectionStatus::Connecting => "FM:...",
        FmConnectionStatus::Connected => "FM:ON",
        FmConnectionStatus::Error => "FM:ERR",
    };

    let nostr = match core.state.nostr_status {
        NostrConnectionStatus::Disconnected => "NOSTR:OFF",
        NostrConnectionStatus::Connecting => "NOSTR:...",
        NostrConnectionStatus::Connected => "NOSTR:ON",
        NostrConnectionStatus::Authenticated => "NOSTR:AUTH",
        NostrConnectionStatus::Error => "NOSTR:ERR",
    };

    let balance = if core.state.pending_earnings > 0 {
        format!("{}(+{}) sats", core.state.balance_sats, core.state.pending_earnings)
    } else {
        format!("{} sats", core.state.balance_sats)
    };
    let served = format!("Served:{}", core.state.jobs_served);

    format!("[{}] [{}] [{}] [{}]", fm, nostr, balance, served)
}
