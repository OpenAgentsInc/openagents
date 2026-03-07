use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use nostr::{NostrIdentity, derive_keypair, load_identity_from_path};
use openagents_provider_substrate::{
    ProviderAdminConfig, ProviderAdminRuntime, ProviderAdminUpdate, ProviderAdvertisedProduct,
    ProviderAvailability, ProviderBackendHealth, ProviderDesiredMode, ProviderEarningsSummary,
    ProviderHealthEvent, ProviderIdentityMetadata, ProviderInventoryControls,
    ProviderInventoryRow, ProviderJsonEntry, ProviderPayoutSummary, ProviderPersistedSnapshot,
    ProviderReceiptSummary, ProviderRecentJob, ProviderRuntimeStatusSnapshot,
    ProviderSandboxDetectionConfig, ProviderSandboxProfileSpec, ProviderStatusResponse,
    ProviderMode, detect_sandbox_supply, derive_provider_products,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const ENV_PYLON_HOME: &str = "OPENAGENTS_PYLON_HOME";
pub const ENV_PYLON_CONFIG_PATH: &str = "OPENAGENTS_PYLON_CONFIG_PATH";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonConfig {
    pub schema_version: u32,
    pub node_label: String,
    pub payout_destination: Option<String>,
    pub identity_path: PathBuf,
    pub admin_db_path: PathBuf,
    pub admin_listen_addr: String,
    pub ollama_base_url: String,
    pub apple_fm_base_url: Option<String>,
    pub inventory_controls: ProviderInventoryControls,
    pub declared_sandbox_profiles: Vec<ProviderSandboxProfileSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Init,
    Doctor,
    Serve,
    Status { json: bool },
    ConfigShow,
    ConfigSet { key: String, value: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cli {
    pub command: Command,
    pub config_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct InitReport {
    config_path: String,
    identity_path: String,
    npub: String,
    payout_destination: Option<String>,
    admin_listen_addr: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorReport {
    config_path: String,
    node_label: String,
    payout_destination: Option<String>,
    identity: ProviderIdentityMetadata,
    availability: ProviderAvailability,
    products: Vec<ProviderAdvertisedProduct>,
}

pub fn parse_args(args: Vec<String>) -> Result<Cli> {
    if args.is_empty() {
        return Err(anyhow!("missing command"));
    }

    let mut index = 0usize;
    let mut config_path = default_config_path();
    while index < args.len() {
        match args[index].as_str() {
            "--config-path" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --config-path"))?;
                config_path = PathBuf::from(value);
                index += 1;
            }
            "--help" | "-h" => {
                return Err(anyhow!(usage()));
            }
            _ => break,
        }
    }

    let command = parse_command(args.as_slice(), index)?;
    Ok(Cli {
        command,
        config_path,
    })
}

pub async fn run_cli(cli: Cli) -> Result<Option<String>> {
    match cli.command {
        Command::Init => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            let identity = ensure_identity(config.identity_path.as_path())?;
            Ok(Some(serde_json::to_string_pretty(&InitReport {
                config_path: cli.config_path.display().to_string(),
                identity_path: config.identity_path.display().to_string(),
                npub: identity.npub,
                payout_destination: config.payout_destination.clone(),
                admin_listen_addr: config.admin_listen_addr.clone(),
            })?))
        }
        Command::Doctor => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            let identity = ensure_identity(config.identity_path.as_path())?;
            let availability = detect_availability(&config).await?;
            let products =
                derive_provider_products(&availability, &config.inventory_controls);
            Ok(Some(serde_json::to_string_pretty(&DoctorReport {
                config_path: cli.config_path.display().to_string(),
                node_label: config.node_label.clone(),
                payout_destination: config.payout_destination.clone(),
                identity: identity_metadata(&identity, config.node_label.as_str()),
                availability,
                products,
            })?))
        }
        Command::Serve => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            serve(config).await?;
            Ok(None)
        }
        Command::Status { json } => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            let status = load_status_or_detect(&config).await?;
            if !json {
                return Ok(Some(serde_json::to_string_pretty(&status)?));
            }
            Ok(Some(serde_json::to_string_pretty(&status)?))
        }
        Command::ConfigShow => {
            let config = load_or_create_config(cli.config_path.as_path())?;
            Ok(Some(serde_json::to_string_pretty(&config)?))
        }
        Command::ConfigSet { key, value } => {
            let mut config = load_or_create_config(cli.config_path.as_path())?;
            apply_config_set(&mut config, key.as_str(), value.as_str())?;
            save_config(cli.config_path.as_path(), &config)?;
            Ok(Some(serde_json::to_string_pretty(&config)?))
        }
    }
}

pub fn usage() -> &'static str {
    "Usage: pylon [--config-path <path>] <command>\n\
Commands:\n\
  init\n\
  doctor\n\
  serve\n\
  status [--json]\n\
  config show\n\
  config set <key> <value>\n"
}

fn parse_command(args: &[String], start_index: usize) -> Result<Command> {
    let command = args
        .get(start_index)
        .ok_or_else(|| anyhow!("missing command"))?;
    match command.as_str() {
        "init" => {
            if start_index + 1 != args.len() {
                bail!("init does not accept positional arguments");
            }
            Ok(Command::Init)
        }
        "doctor" => {
            if start_index + 1 != args.len() {
                bail!("doctor does not accept positional arguments");
            }
            Ok(Command::Doctor)
        }
        "serve" => {
            if start_index + 1 != args.len() {
                bail!("serve does not accept positional arguments");
            }
            Ok(Command::Serve)
        }
        "status" => {
            let json = match args.get(start_index + 1) {
                None => false,
                Some(value) if value == "--json" => true,
                Some(other) => bail!("unexpected argument for status: {other}"),
            };
            if json && start_index + 2 != args.len() {
                bail!("status --json does not accept additional arguments");
            }
            if !json && start_index + 1 != args.len() {
                bail!("status does not accept additional arguments");
            }
            Ok(Command::Status { json })
        }
        "config" => match args.get(start_index + 1).map(String::as_str) {
            Some("show") => {
                if start_index + 2 != args.len() {
                    bail!("config show does not accept additional arguments");
                }
                Ok(Command::ConfigShow)
            }
            Some("set") => {
                let key = args
                    .get(start_index + 2)
                    .ok_or_else(|| anyhow!("missing <key> for config set"))?;
                let value = args
                    .get(start_index + 3)
                    .ok_or_else(|| anyhow!("missing <value> for config set"))?;
                if start_index + 4 != args.len() {
                    bail!("config set accepts exactly <key> <value>");
                }
                Ok(Command::ConfigSet {
                    key: key.clone(),
                    value: value.clone(),
                })
            }
            Some(other) => bail!("unknown config command: {other}"),
            None => bail!("missing config subcommand"),
        },
        other => bail!("unknown command: {other}"),
    }
}

fn load_or_create_config(path: &Path) -> Result<PylonConfig> {
    if path.exists() {
        return load_config(path);
    }
    let base_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    let config = default_config(base_dir.as_path());
    save_config(path, &config)?;
    Ok(config)
}

fn load_config(path: &Path) -> Result<PylonConfig> {
    let payload = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read pylon config {}", path.display()))?;
    serde_json::from_str(payload.as_str())
        .with_context(|| format!("failed to parse pylon config {}", path.display()))
}

fn save_config(path: &Path, config: &PylonConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create pylon config dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(config)?))
        .with_context(|| format!("failed to write pylon config {}", path.display()))?;
    Ok(())
}

fn default_config(base_dir: &Path) -> PylonConfig {
    PylonConfig {
        schema_version: 1,
        node_label: "pylon".to_string(),
        payout_destination: None,
        identity_path: base_dir.join("identity.mnemonic"),
        admin_db_path: base_dir.join("provider-admin.sqlite"),
        admin_listen_addr: "127.0.0.1:9468".to_string(),
        ollama_base_url: "http://127.0.0.1:11434".to_string(),
        apple_fm_base_url: None,
        inventory_controls: ProviderInventoryControls::default(),
        declared_sandbox_profiles: Vec::new(),
    }
}

fn default_config_path() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_PYLON_CONFIG_PATH) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    default_home_dir().join("config.json")
}

fn default_home_dir() -> PathBuf {
    if let Ok(path) = std::env::var(ENV_PYLON_HOME) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("pylon")
}

fn ensure_identity(path: &Path) -> Result<NostrIdentity> {
    if path.exists() {
        return load_identity_from_path(path);
    }
    let entropy: [u8; 16] = rand::random();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .context("failed to generate pylon mnemonic")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create identity dir {}", parent.display()))?;
    }
    std::fs::write(path, format!("{mnemonic}\n"))
        .with_context(|| format!("failed to write identity file {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set identity permissions {}", path.display()))?;
    }
    let keypair = derive_keypair(mnemonic.to_string().as_str())
        .context("failed to derive pylon nostr identity")?;
    Ok(NostrIdentity {
        identity_path: path.to_path_buf(),
        mnemonic: mnemonic.to_string(),
        npub: keypair.npub()?,
        nsec: keypair.nsec()?,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
    })
}

async fn serve(config: PylonConfig) -> Result<()> {
    let listen_addr = config
        .admin_listen_addr
        .parse()
        .with_context(|| format!("invalid admin listen addr {}", config.admin_listen_addr))?;
    let mut runtime = ProviderAdminRuntime::spawn(ProviderAdminConfig::new(
        config.admin_db_path.clone(),
        listen_addr,
    ))
    .map_err(anyhow::Error::msg)?;
    let mut desired_mode = ProviderDesiredMode::Online;
    runtime
        .set_desired_mode(desired_mode)
        .map_err(anyhow::Error::msg)?;

    loop {
        for update in runtime.drain_updates() {
            match update {
                ProviderAdminUpdate::ControlEvent(event) => {
                    desired_mode = event.desired_mode;
                }
                ProviderAdminUpdate::WorkerError(error) => {
                    return Err(anyhow!("provider admin runtime error: {error}"));
                }
            }
        }

        let identity = ensure_identity(config.identity_path.as_path())?;
        let snapshot = build_snapshot(&config, &identity, desired_mode).await?;
        runtime
            .sync_snapshot(snapshot)
            .map_err(anyhow::Error::msg)?;

        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("failed waiting for ctrl-c")?;
                break;
            }
            () = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }

    Ok(())
}

async fn load_status_or_detect(config: &PylonConfig) -> Result<ProviderStatusResponse> {
    if config.admin_db_path.exists() {
        let listen_addr = config
            .admin_listen_addr
            .parse()
            .with_context(|| format!("invalid admin listen addr {}", config.admin_listen_addr))?;
        let store = openagents_provider_substrate::ProviderPersistenceStore::open(
            &ProviderAdminConfig::new(config.admin_db_path.clone(), listen_addr),
        )
        .map_err(anyhow::Error::msg)?;
        return store.load_status().map_err(anyhow::Error::msg);
    }

    let identity = ensure_identity(config.identity_path.as_path())?;
    Ok(ProviderStatusResponse {
        listen_addr: Some(config.admin_listen_addr.clone()),
        desired_mode: ProviderDesiredMode::Offline,
        snapshot: Some(build_snapshot(config, &identity, ProviderDesiredMode::Offline).await?),
    })
}

async fn build_snapshot(
    config: &PylonConfig,
    identity: &NostrIdentity,
    desired_mode: ProviderDesiredMode,
) -> Result<ProviderPersistedSnapshot> {
    let availability = detect_availability(config).await?;
    let products = derive_provider_products(&availability, &config.inventory_controls);
    let mode = provider_mode(desired_mode, products.as_slice());
    Ok(ProviderPersistedSnapshot {
        captured_at_ms: now_epoch_ms(),
        config_metadata: vec![
            ProviderJsonEntry {
                key: "node_label".to_string(),
                value: Value::String(config.node_label.clone()),
            },
            ProviderJsonEntry {
                key: "payout_destination".to_string(),
                value: json!(config.payout_destination),
            },
            ProviderJsonEntry {
                key: "ollama_base_url".to_string(),
                value: Value::String(config.ollama_base_url.clone()),
            },
            ProviderJsonEntry {
                key: "apple_fm_base_url".to_string(),
                value: json!(config.apple_fm_base_url),
            },
        ],
        identity: Some(identity_metadata(identity, config.node_label.as_str())),
        runtime: ProviderRuntimeStatusSnapshot {
            mode,
            last_action: Some(match desired_mode {
                ProviderDesiredMode::Online => "pylon serve loop active".to_string(),
                ProviderDesiredMode::Offline => "pylon is initialized but offline".to_string(),
                ProviderDesiredMode::Paused => "pylon is paused".to_string(),
            }),
            last_error: first_backend_error(&availability),
            degraded_reason_code: if mode == ProviderMode::Degraded {
                Some("NO_ELIGIBLE_SUPPLY".to_string())
            } else {
                None
            },
            authoritative_status: Some(mode.label().to_string()),
            authoritative_error_class: None,
            queue_depth: 0,
            online_uptime_seconds: 0,
            inventory_session_started_at_ms: None,
            last_completed_job_at_epoch_ms: None,
            last_authoritative_event_id: None,
            execution_backend_label: execution_backend_label(&availability, products.as_slice()),
            provider_blocker_codes: provider_blocker_codes(&availability, products.as_slice()),
        },
        availability,
        inventory_rows: inventory_rows(products.as_slice(), desired_mode),
        recent_jobs: Vec::<ProviderRecentJob>::new(),
        receipts: Vec::<ProviderReceiptSummary>::new(),
        payouts: Vec::<ProviderPayoutSummary>::new(),
        health_events: health_events(products.as_slice()),
        earnings: Some(ProviderEarningsSummary {
            sats_today: 0,
            lifetime_sats: 0,
            jobs_today: 0,
            online_uptime_seconds: 0,
            last_job_result: "none".to_string(),
            first_job_latency_seconds: None,
            completion_ratio_bps: None,
            payout_success_ratio_bps: None,
            avg_wallet_confirmation_latency_seconds: None,
        }),
    })
}

fn inventory_rows(
    products: &[ProviderAdvertisedProduct],
    desired_mode: ProviderDesiredMode,
) -> Vec<ProviderInventoryRow> {
    products
        .iter()
        .map(|product| {
            let active = desired_mode == ProviderDesiredMode::Online && product.eligible;
            ProviderInventoryRow {
                target: product.product,
                enabled: product.enabled,
                backend_ready: product.backend_ready,
                eligible: product.eligible,
                capability_summary: product.capability_summary.clone(),
                source_badge: if active {
                    "pylon.serve".to_string()
                } else {
                    "pylon.local_preview".to_string()
                },
                capacity_lot_id: None,
                total_quantity: u64::from(active),
                reserved_quantity: 0,
                available_quantity: u64::from(active),
                delivery_state: if !product.enabled {
                    "disabled".to_string()
                } else if !product.backend_ready {
                    "backend_unavailable".to_string()
                } else if desired_mode == ProviderDesiredMode::Online {
                    "idle".to_string()
                } else {
                    "offline".to_string()
                },
                price_floor_sats: product.price_floor_sats,
                terms_label: product.terms_label.clone(),
                forward_capacity_lot_id: None,
                forward_delivery_window_label: None,
                forward_total_quantity: 0,
                forward_reserved_quantity: 0,
                forward_available_quantity: 0,
                forward_terms_label: Some(product.forward_terms_label.clone()),
            }
        })
        .collect()
}

fn health_events(products: &[ProviderAdvertisedProduct]) -> Vec<ProviderHealthEvent> {
    products
        .iter()
        .filter(|product| product.enabled && !product.backend_ready)
        .map(|product| ProviderHealthEvent {
            event_id: format!("backend_unavailable:{}", product.product.product_id()),
            occurred_at_ms: now_epoch_ms(),
            severity: "warn".to_string(),
            code: "BACKEND_UNAVAILABLE".to_string(),
            detail: format!("{} is enabled but not ready", product.product.display_label()),
            source: "pylon".to_string(),
        })
        .collect()
}

fn provider_mode(
    desired_mode: ProviderDesiredMode,
    products: &[ProviderAdvertisedProduct],
) -> ProviderMode {
    match desired_mode {
        ProviderDesiredMode::Offline => ProviderMode::Offline,
        ProviderDesiredMode::Paused => ProviderMode::Degraded,
        ProviderDesiredMode::Online => {
            if products.iter().any(|product| product.eligible) {
                ProviderMode::Online
            } else if products.iter().any(|product| product.enabled) {
                ProviderMode::Degraded
            } else {
                ProviderMode::Offline
            }
        }
    }
}

fn provider_blocker_codes(
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> Vec<String> {
    let mut codes = Vec::new();
    if !availability.ollama.ready {
        codes.push("OLLAMA_UNAVAILABLE".to_string());
    }
    if !availability.apple_foundation_models.ready {
        codes.push("APPLE_FM_UNAVAILABLE".to_string());
    }
    if !products.iter().any(|product| product.eligible) {
        codes.push("NO_ELIGIBLE_SUPPLY".to_string());
    }
    codes
}

fn execution_backend_label(
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> String {
    let label = availability.execution_backend_label();
    if label != "no active inference backend" {
        return label.to_string();
    }
    if products
        .iter()
        .any(|product| product.product.compute_family_label() == "sandbox_execution")
    {
        return "sandbox runtime".to_string();
    }
    "no active runtime".to_string()
}

fn first_backend_error(availability: &ProviderAvailability) -> Option<String> {
    availability
        .ollama
        .last_error
        .clone()
        .or_else(|| availability.apple_foundation_models.last_error.clone())
        .or_else(|| availability.sandbox.last_scan_error.clone())
}

fn identity_metadata(identity: &NostrIdentity, node_label: &str) -> ProviderIdentityMetadata {
    ProviderIdentityMetadata {
        npub: Some(identity.npub.clone()),
        public_key_hex: Some(identity.public_key_hex.clone()),
        display_name: Some("Pylon".to_string()),
        node_label: Some(node_label.to_string()),
    }
}

async fn detect_availability(config: &PylonConfig) -> Result<ProviderAvailability> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon health-check client")?;
    let ollama = detect_ollama(&client, config).await;
    let apple_foundation_models = detect_apple_fm(&client, config).await;
    let sandbox = detect_sandbox_supply(&ProviderSandboxDetectionConfig::default().with_declared_profiles(
        config.declared_sandbox_profiles.clone(),
    ));
    Ok(ProviderAvailability {
        ollama,
        apple_foundation_models,
        sandbox,
    })
}

async fn detect_ollama(client: &reqwest::Client, config: &PylonConfig) -> ProviderBackendHealth {
    if !config.inventory_controls.ollama_inference_enabled
        && !config.inventory_controls.ollama_embeddings_enabled
    {
        return ProviderBackendHealth {
            last_action: Some("disabled by config".to_string()),
            ..ProviderBackendHealth::default()
        };
    }
    let url = format!("{}/api/tags", config.ollama_base_url.trim_end_matches('/'));
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) => {
            return ProviderBackendHealth {
                reachable: false,
                ready: false,
                last_error: Some(error.to_string()),
                last_action: Some("health check failed".to_string()),
                ..ProviderBackendHealth::default()
            }
        }
    };
    let payload = match response.json::<Value>().await {
        Ok(payload) => payload,
        Err(error) => {
            return ProviderBackendHealth {
                reachable: true,
                ready: false,
                last_error: Some(error.to_string()),
                last_action: Some("invalid ollama health payload".to_string()),
                ..ProviderBackendHealth::default()
            }
        }
    };
    let models = payload
        .get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    model
                        .get("name")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ProviderBackendHealth {
        reachable: true,
        ready: !models.is_empty(),
        configured_model: models.first().cloned(),
        ready_model: models.first().cloned(),
        available_models: models,
        last_action: Some("health check ready".to_string()),
        ..ProviderBackendHealth::default()
    }
}

async fn detect_apple_fm(client: &reqwest::Client, config: &PylonConfig) -> ProviderBackendHealth {
    let Some(base_url) = config.apple_fm_base_url.as_deref() else {
        return ProviderBackendHealth {
            last_action: Some("not configured".to_string()),
            availability_message: Some("not_configured".to_string()),
            ..ProviderBackendHealth::default()
        };
    };
    if !config.inventory_controls.apple_fm_inference_enabled {
        return ProviderBackendHealth {
            last_action: Some("disabled by config".to_string()),
            availability_message: Some("disabled".to_string()),
            ..ProviderBackendHealth::default()
        };
    }
    let health_url = format!("{}/health", base_url.trim_end_matches('/'));
    let health_response = match client.get(health_url.as_str()).send().await {
        Ok(response) => response,
        Err(error) => {
            return ProviderBackendHealth {
                reachable: false,
                ready: false,
                last_error: Some(error.to_string()),
                last_action: Some("health check failed".to_string()),
                availability_message: Some("bridge_unreachable".to_string()),
                ..ProviderBackendHealth::default()
            }
        }
    };
    if !health_response.status().is_success() {
        return ProviderBackendHealth {
            reachable: true,
            ready: false,
            last_error: Some(format!("bridge returned {}", health_response.status())),
            last_action: Some("health check failed".to_string()),
            availability_message: Some("bridge_unhealthy".to_string()),
            ..ProviderBackendHealth::default()
        };
    }
    let models_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let models = match client.get(models_url.as_str()).send().await {
        Ok(response) => match response.json::<Value>().await {
            Ok(payload) => payload
                .get("data")
                .and_then(Value::as_array)
                .map(|models| {
                    models
                        .iter()
                        .filter_map(|model| {
                            model.get("id").and_then(Value::as_str).map(ToString::to_string)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };
    ProviderBackendHealth {
        reachable: true,
        ready: !models.is_empty(),
        ready_model: models.first().cloned(),
        available_models: models,
        last_action: Some("health check ready".to_string()),
        availability_message: Some("bridge_ready".to_string()),
        ..ProviderBackendHealth::default()
    }
}

fn apply_config_set(config: &mut PylonConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "node_label" => config.node_label = value.to_string(),
        "payout_destination" => {
            config.payout_destination = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "admin_listen_addr" => config.admin_listen_addr = value.to_string(),
        "ollama_base_url" => config.ollama_base_url = value.to_string(),
        "apple_fm_base_url" => {
            config.apple_fm_base_url = if value.trim().is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "backend.ollama_inference_enabled" => {
            config.inventory_controls.ollama_inference_enabled = parse_bool(value)?;
        }
        "backend.ollama_embeddings_enabled" => {
            config.inventory_controls.ollama_embeddings_enabled = parse_bool(value)?;
        }
        "backend.apple_fm_inference_enabled" => {
            config.inventory_controls.apple_fm_inference_enabled = parse_bool(value)?;
        }
        "backend.sandbox_container_exec_enabled" => {
            config.inventory_controls.sandbox_container_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_python_exec_enabled" => {
            config.inventory_controls.sandbox_python_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_node_exec_enabled" => {
            config.inventory_controls.sandbox_node_exec_enabled = parse_bool(value)?;
        }
        "backend.sandbox_posix_exec_enabled" => {
            config.inventory_controls.sandbox_posix_exec_enabled = parse_bool(value)?;
        }
        other => bail!("unsupported config key: {other}"),
    }
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => bail!("invalid boolean value: {other}"),
    }
}

fn now_epoch_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Command, PylonConfig, apply_config_set, default_config, parse_args,
    };
    use openagents_provider_substrate::ProviderInventoryControls;

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    #[test]
    fn parse_args_supports_status_json() -> Result<(), Box<dyn std::error::Error>> {
        let cli = parse_args(vec!["status".to_string(), "--json".to_string()])?;
        ensure(
            cli.command == Command::Status { json: true },
            "status --json should parse into json status command",
        )
    }

    #[test]
    fn config_set_updates_backend_flags() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = PylonConfig {
            inventory_controls: ProviderInventoryControls::default(),
            ..default_config(std::path::Path::new("/tmp/pylon-test"))
        };
        apply_config_set(&mut config, "backend.sandbox_python_exec_enabled", "true")?;
        ensure(
            config.inventory_controls.sandbox_python_exec_enabled,
            "config set should update sandbox python toggle",
        )
    }

    #[test]
    fn config_set_updates_payout_destination() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_config(std::path::Path::new("/tmp/pylon-test"));
        apply_config_set(&mut config, "payout_destination", "lnurlp:alice")?;
        ensure(
            config.payout_destination.as_deref() == Some("lnurlp:alice"),
            "config set should update payout destination",
        )
    }
}
