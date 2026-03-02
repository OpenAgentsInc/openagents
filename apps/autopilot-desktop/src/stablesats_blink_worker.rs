use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct StableSatsBlinkRefreshRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub balance_script_path: PathBuf,
    pub price_script_path: PathBuf,
    pub env_overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkLiveSnapshot {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub btc_balance_sats: u64,
    pub usd_balance_cents: u64,
    pub price_usd_cents_per_btc: u64,
    pub source_ref: String,
}

#[derive(Clone, Debug)]
pub enum StableSatsBlinkUpdate {
    Completed(StableSatsBlinkLiveSnapshot),
    Failed { request_id: u64, error: String },
}

enum StableSatsBlinkCommand {
    Refresh(StableSatsBlinkRefreshRequest),
}

pub struct StableSatsBlinkWorker {
    command_tx: Sender<StableSatsBlinkCommand>,
    update_rx: Receiver<StableSatsBlinkUpdate>,
}

impl StableSatsBlinkWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<StableSatsBlinkCommand>();
        let (update_tx, update_rx) = mpsc::channel::<StableSatsBlinkUpdate>();
        std::thread::Builder::new()
            .name("stablesats-blink-worker".to_string())
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    let update = match command {
                        StableSatsBlinkCommand::Refresh(request) => match run_refresh(&request) {
                            Ok(snapshot) => StableSatsBlinkUpdate::Completed(snapshot),
                            Err(error) => StableSatsBlinkUpdate::Failed {
                                request_id: request.request_id,
                                error,
                            },
                        },
                    };
                    if update_tx.send(update).is_err() {
                        break;
                    }
                }
            })
            .expect("stablesats blink worker thread should spawn");

        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue_refresh(&self, request: StableSatsBlinkRefreshRequest) -> Result<(), String> {
        self.command_tx
            .send(StableSatsBlinkCommand::Refresh(request))
            .map_err(|error| format!("stablesats blink worker offline: {error}"))
    }

    pub fn drain_updates(&self, max_items: usize) -> Vec<StableSatsBlinkUpdate> {
        let mut updates = Vec::new();
        for _ in 0..max_items {
            match self.update_rx.try_recv() {
                Ok(update) => updates.push(update),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        updates
    }
}

fn run_refresh(
    request: &StableSatsBlinkRefreshRequest,
) -> Result<StableSatsBlinkLiveSnapshot, String> {
    let balance_json = run_blink_script_json(
        request.balance_script_path.as_path(),
        &[],
        request.env_overrides.as_slice(),
    )?;
    let btc_balance_sats = parse_json_u64(&balance_json, "btcBalanceSats")
        .or_else(|| parse_json_u64(&balance_json, "btcBalance"))
        .ok_or_else(|| "Blink balance payload missing btcBalanceSats".to_string())?;
    let usd_balance_cents = parse_json_u64(&balance_json, "usdBalanceCents")
        .or_else(|| parse_json_u64(&balance_json, "usdBalance"))
        .ok_or_else(|| "Blink balance payload missing usdBalanceCents".to_string())?;

    let price_json = run_blink_script_json(
        request.price_script_path.as_path(),
        &[],
        request.env_overrides.as_slice(),
    )
    .ok();
    let price_usd_cents_per_btc = price_json
        .as_ref()
        .and_then(|json| parse_json_f64(json, "btcPriceUsd"))
        .map(|price| (price * 100.0).round())
        .and_then(|price| {
            if price.is_finite() && price >= 1.0 {
                Some(price as u64)
            } else {
                None
            }
        })
        .unwrap_or(1);

    let btc_wallet_id = balance_json
        .get("btcWalletId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown-btc");
    let usd_wallet_id = balance_json
        .get("usdWalletId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown-usd");

    Ok(StableSatsBlinkLiveSnapshot {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        btc_balance_sats,
        usd_balance_cents,
        price_usd_cents_per_btc,
        source_ref: format!("btc:{} usd:{}", btc_wallet_id, usd_wallet_id),
    })
}

fn run_blink_script_json(
    script_path: &Path,
    args: &[String],
    env_overrides: &[(String, String)],
) -> Result<serde_json::Value, String> {
    let timeout = Duration::from_secs(8);
    let mut command = std::process::Command::new("node");
    command.arg(script_path);
    command.args(args);
    for (name, value) in env_overrides {
        command.env(name, value);
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        format!(
            "Failed launching Blink script {}: {error}",
            script_path.display()
        )
    })?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Blink script {} timed out after {} ms",
                        script_path.display(),
                        timeout.as_millis()
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                return Err(format!(
                    "Blink script {} wait failed: {error}",
                    script_path.display()
                ));
            }
        }
    }

    let output = child.wait_with_output().map_err(|error| {
        format!(
            "Failed collecting output from Blink script {}: {error}",
            script_path.display()
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let status = output.status.code().map_or_else(
            || "signal".to_string(),
            |value| format!("exit_code={value}"),
        );
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output".to_string()
        };
        return Err(format!(
            "Blink script {} failed ({status}): {details}",
            script_path.display()
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "Blink script {} returned empty stdout",
            script_path.display()
        ));
    }

    serde_json::from_str::<serde_json::Value>(&stdout).map_err(|error| {
        format!(
            "Blink script {} returned non-JSON stdout: {error}",
            script_path.display()
        )
    })
}

fn parse_json_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    let raw = value.get(key)?;
    raw.as_u64()
        .or_else(|| raw.as_i64().and_then(|number| u64::try_from(number).ok()))
        .or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<u64>().ok())
        })
}

fn parse_json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    let raw = value.get(key)?;
    raw.as_f64()
        .or_else(|| raw.as_i64().map(|number| number as f64))
        .or_else(|| raw.as_u64().map(|number| number as f64))
        .or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<f64>().ok())
        })
}
