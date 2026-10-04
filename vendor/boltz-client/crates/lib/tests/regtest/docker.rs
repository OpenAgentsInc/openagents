#![allow(dead_code)]
//! Docker command helpers for the Boltz regtest stack.

use std::process::Command;

use anyhow::{Context, Result, bail};

/// Execute a command inside the `boltz-scripts` container.
pub fn exec_boltz_scripts(cmd: &str) -> Result<String> {
    let full_cmd = format!("source /etc/profile.d/utils.sh && {cmd}");
    let output = Command::new("docker")
        .args(["exec", "boltz-scripts", "bash", "-c", &full_cmd])
        .output()
        .context("Failed to execute docker command. Is the regtest stack running?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("boltz-scripts command failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Execute a command via `boltzr-cli` inside the `boltz-backend` container.
pub fn exec_boltzr_cli(cmd: &str) -> Result<String> {
    let output = Command::new("docker")
        .args([
            "exec",
            "boltz-backend",
            "boltzr-cli",
            "--grpc-certificates",
            "/boltz-data/certificates",
        ])
        .args(cmd.split_whitespace())
        .output()
        .context("Failed to execute boltzr-cli. Is the regtest stack running?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("boltzr-cli command failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Mine a Bitcoin regtest block.
pub fn generate_bitcoin_block() -> Result<()> {
    exec_boltz_scripts("bitcoin-cli-sim-client -generate")?;
    Ok(())
}

/// Mine Anvil (EVM) blocks.
pub fn generate_anvil_blocks(count: u32) -> Result<()> {
    exec_boltz_scripts(&format!(
        "cast rpc anvil_mine {count} --rpc-url http://anvil:8545"
    ))?;
    Ok(())
}

/// Pay a Lightning invoice via LND (synchronous — waits for completion).
pub fn pay_invoice_lnd(invoice: &str) -> Result<String> {
    exec_boltz_scripts(&format!("lncli-sim 1 payinvoice -f {invoice}"))
}

/// Pay a Lightning invoice in the background (fire-and-forget).
/// For reverse swaps, `payinvoice` blocks until the hold invoice settles,
/// so we must run it detached.
pub fn pay_invoice_lnd_background(invoice: &str) -> Result<()> {
    let full_cmd = format!("source /etc/profile.d/utils.sh && lncli-sim 1 payinvoice -f {invoice}");
    let output = Command::new("docker")
        .args(["exec", "-d", "boltz-scripts", "bash", "-c", &full_cmd])
        .output()
        .context("Failed to execute background pay command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Background pay failed: {stderr}");
    }
    Ok(())
}

/// Generate a Lightning invoice via LND.
pub fn generate_invoice_lnd(amount_sats: u64) -> Result<String> {
    let output = exec_boltz_scripts(&format!("lncli-sim 1 addinvoice --amt {amount_sats}"))?;
    // Output is JSON, extract payment_request
    let parsed: serde_json::Value =
        serde_json::from_str(&output).context("Failed to parse LND addinvoice output")?;
    parsed["payment_request"]
        .as_str()
        .map(String::from)
        .context("Missing payment_request in LND response")
}

/// Get a new Bitcoin regtest address.
pub fn get_bitcoin_address() -> Result<String> {
    exec_boltz_scripts("bitcoin-cli-sim-client getnewaddress")
}
