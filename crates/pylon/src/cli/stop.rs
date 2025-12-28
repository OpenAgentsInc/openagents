//! pylon stop - Stop the Pylon daemon

use clap::Args;
use std::time::Duration;

use crate::daemon::{
    is_daemon_running, pid_path, socket_path, ControlClient, DaemonCommand, DaemonResponse,
    PidFile,
};

/// Arguments for the stop command
#[derive(Args)]
pub struct StopArgs {
    /// Force stop without graceful shutdown (SIGKILL)
    #[arg(long, short)]
    pub force: bool,

    /// Timeout in seconds for graceful shutdown (default: 10)
    #[arg(long, short, default_value = "10")]
    pub timeout: u64,
}

/// Run the stop command
pub async fn run(args: StopArgs) -> anyhow::Result<()> {
    if !is_daemon_running() {
        println!("Pylon daemon is not running.");
        return Ok(());
    }

    let pid_file = PidFile::new(pid_path()?);
    let pid = pid_file.read()?;

    println!("Stopping Pylon daemon (PID: {})...", pid);

    if args.force {
        // Force kill immediately
        println!("Sending SIGKILL...");
        pid_file.kill()?;
        pid_file.remove()?;

        // Also remove socket file if it exists
        let socket = socket_path()?;
        if socket.exists() {
            let _ = std::fs::remove_file(socket);
        }

        println!("Pylon daemon killed.");
        return Ok(());
    }

    // Try graceful shutdown via control socket first
    let socket = socket_path()?;
    if socket.exists() {
        println!("Requesting graceful shutdown via control socket...");
        let client = ControlClient::new(socket.clone());

        match client.send(DaemonCommand::Shutdown) {
            Ok(DaemonResponse::Ok) => {
                println!("Shutdown request acknowledged.");
            }
            Ok(other) => {
                println!("Unexpected response: {:?}", other);
            }
            Err(e) => {
                println!("Control socket unavailable ({}), falling back to SIGTERM...", e);
            }
        }
    }

    // Send SIGTERM for graceful shutdown
    println!("Sending SIGTERM...");
    if let Err(e) = pid_file.terminate() {
        println!("Failed to send SIGTERM: {}", e);
        // Process might have already exited
        if !is_daemon_running() {
            pid_file.remove()?;
            println!("Pylon daemon stopped.");
            return Ok(());
        }
    }

    // Wait for process to exit
    let timeout = Duration::from_secs(args.timeout);
    let start = std::time::Instant::now();

    println!("Waiting for graceful shutdown (timeout: {}s)...", args.timeout);

    while start.elapsed() < timeout {
        if !pid_file.is_running() {
            pid_file.remove()?;
            println!("Pylon daemon stopped gracefully.");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Timeout - force kill
    println!("Graceful shutdown timed out, sending SIGKILL...");
    pid_file.kill()?;

    // Wait a bit for the kill to take effect
    std::thread::sleep(Duration::from_millis(500));

    if !pid_file.is_running() {
        pid_file.remove()?;
        println!("Pylon daemon killed.");
    } else {
        println!("Warning: Failed to kill daemon. Manual intervention may be required.");
        println!("Try: kill -9 {}", pid);
    }

    Ok(())
}
