use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bip39::{Language, Mnemonic};
use nostr::{NostrIdentity, derive_keypair, load_identity_from_path};
use openagents_provider_substrate::{
    ProviderAdminConfig, ProviderAdminRuntime, ProviderAdminUpdate, ProviderAdvertisedProduct,
    ProviderAvailability, ProviderBackendHealth, ProviderControlAction, ProviderDesiredMode,
    ProviderEarningsSummary, ProviderFailureClass, ProviderHealthEvent, ProviderIdentityMetadata,
    ProviderInventoryControls, ProviderInventoryRow, ProviderJsonEntry, ProviderMode,
    ProviderPersistedSnapshot, ProviderPersistenceStore, ProviderRuntimeStatusSnapshot,
    ProviderSandboxDetectionConfig, ProviderSandboxProfileSpec, ProviderStatusResponse,
    derive_provider_products, detect_sandbox_supply, provider_runtime_state_label,
    validate_provider_control_action,
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
    Online,
    Offline,
    Pause,
    Resume,
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
            let products = derive_provider_products(&availability, &config.inventory_controls);
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
            let status = load_status_or_detect(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&status)?));
            }
            Ok(Some(render_human_status(&status)))
        }
        Command::Online => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Online)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Offline => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Offline)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Pause => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Pause)
                    .await?;
            Ok(Some(render_human_status(&status)))
        }
        Command::Resume => {
            let status =
                apply_control_command(cli.config_path.as_path(), ProviderControlAction::Resume)
                    .await?;
            Ok(Some(render_human_status(&status)))
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
  online\n\
  offline\n\
  pause\n\
  resume\n\
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
        "online" => {
            if start_index + 1 != args.len() {
                bail!("online does not accept positional arguments");
            }
            Ok(Command::Online)
        }
        "offline" => {
            if start_index + 1 != args.len() {
                bail!("offline does not accept positional arguments");
            }
            Ok(Command::Offline)
        }
        "pause" => {
            if start_index + 1 != args.len() {
                bail!("pause does not accept positional arguments");
            }
            Ok(Command::Pause)
        }
        "resume" => {
            if start_index + 1 != args.len() {
                bail!("resume does not accept positional arguments");
            }
            Ok(Command::Resume)
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

fn load_config_required(path: &Path) -> Result<PylonConfig> {
    if !path.exists() {
        bail!("pylon is unconfigured; run `pylon init` first");
    }
    load_config(path)
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
    let admin_config = provider_admin_config(&config)?;
    let mut desired_mode = ProviderPersistenceStore::open(&admin_config)
        .map_err(anyhow::Error::msg)?
        .desired_mode()
        .map_err(anyhow::Error::msg)?;
    let mut runtime = ProviderAdminRuntime::spawn(admin_config).map_err(anyhow::Error::msg)?;
    let identity = ensure_identity(config.identity_path.as_path())?;
    let mut previous_snapshot = None::<ProviderPersistedSnapshot>;
    let mut needs_sync = true;
    loop {
        for update in runtime.drain_updates() {
            match update {
                ProviderAdminUpdate::ControlEvent(event) => {
                    desired_mode = event.desired_mode;
                    needs_sync = true;
                }
                ProviderAdminUpdate::WorkerError(error) => {
                    let snapshot = build_error_snapshot(
                        &config,
                        Some(&identity),
                        desired_mode,
                        previous_snapshot.as_ref(),
                        error.clone(),
                    );
                    let _ = runtime.sync_snapshot(snapshot);
                    return Err(anyhow!("provider admin runtime error: {error}"));
                }
            }
        }

        if needs_sync {
            let snapshot =
                build_snapshot(&config, &identity, desired_mode, previous_snapshot.as_ref())
                    .await?;
            runtime
                .sync_snapshot(snapshot.clone())
                .map_err(anyhow::Error::msg)?;
            previous_snapshot = Some(snapshot);
            needs_sync = false;
        }

        let sleep_duration = if needs_sync {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(2)
        };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("failed waiting for ctrl-c")?;
                break;
            }
            () = tokio::time::sleep(sleep_duration) => {
                needs_sync = true;
            }
        }
    }

    Ok(())
}

async fn apply_control_command(
    config_path: &Path,
    action: ProviderControlAction,
) -> Result<ProviderStatusResponse> {
    let config = load_config_required(config_path)?;
    if try_live_control(&config, action).await? {
        let admin_config = provider_admin_config(&config)?;
        let store = ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?;
        return load_status_with_store(&config, Some(&store), None).await;
    }
    apply_control_locally(&config, action).await
}

async fn build_snapshot(
    config: &PylonConfig,
    identity: &NostrIdentity,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
) -> Result<ProviderPersistedSnapshot> {
    let availability = detect_availability(config).await?;
    Ok(build_snapshot_from_availability(
        config,
        Some(identity),
        desired_mode,
        previous_snapshot,
        availability,
        None,
    ))
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

async fn load_status_or_detect(config_path: &Path) -> Result<ProviderStatusResponse> {
    if !config_path.exists() {
        return Ok(build_unconfigured_status_for_path(config_path));
    }
    let config = load_config(config_path)?;
    if let Some(status) = try_live_status(&config).await? {
        return Ok(status);
    }
    let admin_config = provider_admin_config(&config)?;
    let store = if config.admin_db_path.exists() {
        Some(ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?)
    } else {
        None
    };
    load_status_with_store(&config, store.as_ref(), None).await
}

async fn load_status_with_store(
    config: &PylonConfig,
    store: Option<&ProviderPersistenceStore>,
    desired_mode_override: Option<ProviderDesiredMode>,
) -> Result<ProviderStatusResponse> {
    let stored_status = store
        .map(ProviderPersistenceStore::load_status)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let desired_mode = desired_mode_override
        .or_else(|| stored_status.as_ref().map(|status| status.desired_mode))
        .unwrap_or(ProviderDesiredMode::Offline);
    let previous_snapshot = stored_status
        .as_ref()
        .and_then(|status| status.snapshot.as_ref());

    if !config.identity_path.exists() {
        return Ok(ProviderStatusResponse {
            listen_addr: Some(config.admin_listen_addr.clone()),
            desired_mode,
            snapshot: Some(build_unconfigured_snapshot(
                Some(config),
                desired_mode,
                previous_snapshot,
                "identity file missing",
            )),
        });
    }

    let identity = match load_identity_from_path(config.identity_path.as_path()) {
        Ok(identity) => identity,
        Err(error) => {
            return Ok(ProviderStatusResponse {
                listen_addr: Some(config.admin_listen_addr.clone()),
                desired_mode,
                snapshot: Some(build_error_snapshot(
                    config,
                    None,
                    desired_mode,
                    previous_snapshot,
                    error.to_string(),
                )),
            });
        }
    };

    let snapshot = match detect_availability(config).await {
        Ok(availability) => build_snapshot_from_availability(
            config,
            Some(&identity),
            desired_mode,
            previous_snapshot,
            availability,
            None,
        ),
        Err(error) => build_error_snapshot(
            config,
            Some(&identity),
            desired_mode,
            previous_snapshot,
            error.to_string(),
        ),
    };
    Ok(ProviderStatusResponse {
        listen_addr: Some(config.admin_listen_addr.clone()),
        desired_mode,
        snapshot: Some(snapshot),
    })
}

async fn apply_control_locally(
    config: &PylonConfig,
    action: ProviderControlAction,
) -> Result<ProviderStatusResponse> {
    let admin_config = provider_admin_config(config)?;
    let mut store = ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?;
    store
        .set_listen_addr(config.admin_listen_addr.as_str())
        .map_err(anyhow::Error::msg)?;
    let current_status = load_status_with_store(config, Some(&store), None).await?;
    let desired_mode = validate_provider_control_action(&current_status, action)?;
    store
        .set_desired_mode(desired_mode)
        .map_err(anyhow::Error::msg)?;
    let updated_status = load_status_with_store(config, Some(&store), Some(desired_mode)).await?;
    if let Some(snapshot) = updated_status.snapshot.as_ref() {
        store
            .persist_snapshot(snapshot)
            .map_err(anyhow::Error::msg)?;
    }
    Ok(updated_status)
}

fn provider_admin_config(config: &PylonConfig) -> Result<ProviderAdminConfig> {
    let listen_addr = config
        .admin_listen_addr
        .parse()
        .with_context(|| format!("invalid admin listen addr {}", config.admin_listen_addr))?;
    Ok(ProviderAdminConfig::new(
        config.admin_db_path.clone(),
        listen_addr,
    ))
}

fn build_snapshot_from_availability(
    config: &PylonConfig,
    identity: Option<&NostrIdentity>,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    availability: ProviderAvailability,
    runtime_error: Option<String>,
) -> ProviderPersistedSnapshot {
    let captured_at_ms = now_epoch_ms();
    let products = derive_provider_products(&availability, &config.inventory_controls);
    let runtime = derive_runtime_snapshot(
        desired_mode,
        previous_snapshot.map(|snapshot| &snapshot.runtime),
        &availability,
        products.as_slice(),
        runtime_error.clone(),
    );
    let mut earnings = previous_snapshot
        .and_then(|snapshot| snapshot.earnings.clone())
        .unwrap_or_else(default_earnings_summary);
    earnings.online_uptime_seconds = runtime.online_uptime_seconds;

    ProviderPersistedSnapshot {
        captured_at_ms,
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
        identity: identity.map(|identity| identity_metadata(identity, config.node_label.as_str())),
        runtime,
        availability,
        inventory_rows: inventory_rows(products.as_slice(), desired_mode),
        recent_jobs: previous_snapshot
            .map(|snapshot| snapshot.recent_jobs.clone())
            .unwrap_or_default(),
        receipts: previous_snapshot
            .map(|snapshot| snapshot.receipts.clone())
            .unwrap_or_default(),
        payouts: previous_snapshot
            .map(|snapshot| snapshot.payouts.clone())
            .unwrap_or_default(),
        health_events: build_health_events(products.as_slice(), runtime_error.as_deref()),
        earnings: Some(earnings),
    }
}

fn derive_runtime_snapshot(
    desired_mode: ProviderDesiredMode,
    previous_runtime: Option<&ProviderRuntimeStatusSnapshot>,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
    runtime_error: Option<String>,
) -> ProviderRuntimeStatusSnapshot {
    let eligible_products = products.iter().filter(|product| product.eligible).count();
    let enabled_products = products.iter().filter(|product| product.enabled).count();
    let queue_depth = previous_runtime.map_or(0, |runtime| runtime.queue_depth);
    let state = if runtime_error.is_some() {
        "error".to_string()
    } else {
        match desired_mode {
            ProviderDesiredMode::Paused => "paused".to_string(),
            ProviderDesiredMode::Offline if queue_depth > 0 => "draining".to_string(),
            ProviderDesiredMode::Offline if eligible_products > 0 => "ready".to_string(),
            ProviderDesiredMode::Offline => "offline".to_string(),
            ProviderDesiredMode::Online if eligible_products > 0 => "online".to_string(),
            ProviderDesiredMode::Online => "degraded".to_string(),
        }
    };
    let mode = match state.as_str() {
        "online" => ProviderMode::Online,
        "degraded" | "draining" | "error" => ProviderMode::Degraded,
        _ => ProviderMode::Offline,
    };
    let degraded_reason_code = match state.as_str() {
        "degraded" => Some("NO_ELIGIBLE_SUPPLY".to_string()),
        "error" => Some("STATUS_BUILD_ERROR".to_string()),
        "draining" => Some("DRAINING_PENDING_WORK".to_string()),
        _ => None,
    };
    let last_error = runtime_error.or_else(|| {
        if state == "degraded" {
            first_backend_error(availability)
        } else {
            None
        }
    });
    let last_action = match state.as_str() {
        "online" => format!("pylon is online with {eligible_products} sellable launch products"),
        "ready" => format!("pylon is ready with {eligible_products} sellable launch products"),
        "paused" => "pylon is paused".to_string(),
        "draining" => "pylon is draining in-flight work".to_string(),
        "degraded" => format!(
            "pylon cannot go online because {enabled_products} enabled products are not sellable"
        ),
        "error" => "pylon hit a local control or status error".to_string(),
        _ => "pylon is initialized but offline".to_string(),
    };
    ProviderRuntimeStatusSnapshot {
        mode,
        last_action: Some(last_action),
        last_error,
        degraded_reason_code,
        authoritative_status: Some(state.clone()),
        authoritative_error_class: if state == "error" {
            Some(ProviderFailureClass::Reconciliation)
        } else if state == "degraded" {
            Some(ProviderFailureClass::Execution)
        } else {
            None
        },
        queue_depth,
        online_uptime_seconds: previous_runtime
            .map(|runtime| runtime.online_uptime_seconds)
            .unwrap_or(0),
        inventory_session_started_at_ms: if state == "online" {
            previous_runtime
                .and_then(|runtime| runtime.inventory_session_started_at_ms)
                .or(Some(now_epoch_ms()))
        } else {
            None
        },
        last_completed_job_at_epoch_ms: previous_runtime
            .and_then(|runtime| runtime.last_completed_job_at_epoch_ms),
        last_authoritative_event_id: previous_runtime
            .and_then(|runtime| runtime.last_authoritative_event_id.clone()),
        execution_backend_label: execution_backend_label(availability, products),
        provider_blocker_codes: provider_blocker_codes(availability, products, state.as_str()),
    }
}

fn build_unconfigured_status_for_path(config_path: &Path) -> ProviderStatusResponse {
    let base_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    let config = default_config(base_dir.as_path());
    ProviderStatusResponse {
        listen_addr: Some(config.admin_listen_addr.clone()),
        desired_mode: ProviderDesiredMode::Offline,
        snapshot: Some(build_unconfigured_snapshot(
            None,
            ProviderDesiredMode::Offline,
            None,
            "config missing",
        )),
    }
}

fn build_unconfigured_snapshot(
    config: Option<&PylonConfig>,
    _desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    detail: &str,
) -> ProviderPersistedSnapshot {
    let availability = ProviderAvailability::default();
    let runtime = ProviderRuntimeStatusSnapshot {
        mode: ProviderMode::Offline,
        last_action: Some("pylon is not initialized".to_string()),
        last_error: Some(detail.to_string()),
        degraded_reason_code: Some("UNCONFIGURED".to_string()),
        authoritative_status: Some("unconfigured".to_string()),
        authoritative_error_class: Some(ProviderFailureClass::Reconciliation),
        queue_depth: previous_snapshot.map_or(0, |snapshot| snapshot.runtime.queue_depth),
        online_uptime_seconds: 0,
        inventory_session_started_at_ms: None,
        last_completed_job_at_epoch_ms: previous_snapshot
            .and_then(|snapshot| snapshot.runtime.last_completed_job_at_epoch_ms),
        last_authoritative_event_id: previous_snapshot
            .and_then(|snapshot| snapshot.runtime.last_authoritative_event_id.clone()),
        execution_backend_label: "not configured".to_string(),
        provider_blocker_codes: vec!["CONFIG_MISSING".to_string(), "IDENTITY_MISSING".to_string()],
    };
    ProviderPersistedSnapshot {
        captured_at_ms: now_epoch_ms(),
        config_metadata: config
            .map(|config| {
                vec![ProviderJsonEntry {
                    key: "node_label".to_string(),
                    value: Value::String(config.node_label.clone()),
                }]
            })
            .unwrap_or_default(),
        identity: None,
        runtime,
        availability,
        inventory_rows: Vec::new(),
        recent_jobs: previous_snapshot
            .map(|snapshot| snapshot.recent_jobs.clone())
            .unwrap_or_default(),
        receipts: previous_snapshot
            .map(|snapshot| snapshot.receipts.clone())
            .unwrap_or_default(),
        payouts: previous_snapshot
            .map(|snapshot| snapshot.payouts.clone())
            .unwrap_or_default(),
        health_events: build_health_events(&[], Some(detail)),
        earnings: Some(default_earnings_summary()),
    }
}

fn build_error_snapshot(
    config: &PylonConfig,
    identity: Option<&NostrIdentity>,
    desired_mode: ProviderDesiredMode,
    previous_snapshot: Option<&ProviderPersistedSnapshot>,
    error_detail: String,
) -> ProviderPersistedSnapshot {
    build_snapshot_from_availability(
        config,
        identity,
        desired_mode,
        previous_snapshot,
        ProviderAvailability::default(),
        Some(error_detail),
    )
}

fn default_earnings_summary() -> ProviderEarningsSummary {
    ProviderEarningsSummary {
        sats_today: 0,
        lifetime_sats: 0,
        jobs_today: 0,
        online_uptime_seconds: 0,
        last_job_result: "none".to_string(),
        first_job_latency_seconds: None,
        completion_ratio_bps: None,
        payout_success_ratio_bps: None,
        avg_wallet_confirmation_latency_seconds: None,
    }
}

fn build_health_events(
    products: &[ProviderAdvertisedProduct],
    runtime_error: Option<&str>,
) -> Vec<ProviderHealthEvent> {
    let mut events = Vec::new();
    if let Some(runtime_error) = runtime_error {
        events.push(ProviderHealthEvent {
            event_id: "runtime_error".to_string(),
            occurred_at_ms: now_epoch_ms(),
            severity: "error".to_string(),
            code: "STATUS_BUILD_ERROR".to_string(),
            detail: runtime_error.to_string(),
            source: "pylon".to_string(),
        });
    }
    events.extend(
        products
            .iter()
            .filter(|product| product.enabled && !product.backend_ready)
            .map(|product| ProviderHealthEvent {
                event_id: format!("backend_unavailable:{}", product.product.product_id()),
                occurred_at_ms: now_epoch_ms(),
                severity: "warn".to_string(),
                code: "BACKEND_UNAVAILABLE".to_string(),
                detail: format!(
                    "{} is enabled but not ready",
                    product.product.display_label()
                ),
                source: "pylon".to_string(),
            }),
    );
    events
}

fn provider_blocker_codes(
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
    state: &str,
) -> Vec<String> {
    let mut codes = Vec::new();
    if !availability.ollama.ready {
        codes.push("OLLAMA_UNAVAILABLE".to_string());
    }
    if !availability.apple_foundation_models.ready {
        codes.push("APPLE_FM_UNAVAILABLE".to_string());
    }
    if !products.iter().any(|product| product.eligible)
        && matches!(state, "degraded" | "draining" | "offline")
    {
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

fn render_human_status(status: &ProviderStatusResponse) -> String {
    let mut lines = vec![
        format!("state: {}", provider_runtime_state_label(status)),
        format!("desired_mode: {}", status.desired_mode.label()),
    ];
    if let Some(listen_addr) = status.listen_addr.as_deref() {
        lines.push(format!("listen_addr: {listen_addr}"));
    }
    if let Some(snapshot) = status.snapshot.as_ref() {
        let eligible_products = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .count();
        lines.push(format!(
            "products: {} visible / {} eligible",
            snapshot.inventory_rows.len(),
            eligible_products
        ));
        lines.push(format!(
            "execution_backend: {}",
            snapshot.runtime.execution_backend_label
        ));
        if let Some(reason_code) = snapshot.runtime.degraded_reason_code.as_deref() {
            lines.push(format!("reason_code: {reason_code}"));
        }
        if let Some(last_error) = snapshot.runtime.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
        if !snapshot.runtime.provider_blocker_codes.is_empty() {
            lines.push(format!(
                "blockers: {}",
                snapshot.runtime.provider_blocker_codes.join(", ")
            ));
        }
    }
    lines.join("\n")
}

async fn try_live_status(config: &PylonConfig) -> Result<Option<ProviderStatusResponse>> {
    let client = admin_client()?;
    let url = format!("http://{}/v1/status", config.admin_listen_addr);
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(None),
        Err(error) => return Err(anyhow!("failed to query pylon admin status: {error}")),
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider status error"}));
        bail!(
            "provider admin status request failed: {}",
            api_error_detail(&payload)
        );
    }
    let status = response
        .json::<ProviderStatusResponse>()
        .await
        .context("failed to decode provider admin status response")?;
    Ok(Some(status))
}

async fn try_live_control(config: &PylonConfig, action: ProviderControlAction) -> Result<bool> {
    let client = admin_client()?;
    let endpoint = match action {
        ProviderControlAction::Online => "online",
        ProviderControlAction::Offline => "offline",
        ProviderControlAction::Pause => "pause",
        ProviderControlAction::Resume => "resume",
    };
    let url = format!("http://{}/v1/{endpoint}", config.admin_listen_addr);
    let response = match client.post(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(false),
        Err(error) => {
            return Err(anyhow!(
                "failed to call pylon admin {} endpoint: {error}",
                action.label()
            ));
        }
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider control error"}));
        bail!(
            "provider admin {} failed: {}",
            action.label(),
            api_error_detail(&payload)
        );
    }
    response
        .json::<ProviderStatusResponse>()
        .await
        .with_context(|| format!("failed to decode {} control response", action.label()))?;
    Ok(true)
}

fn admin_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon admin client")
}

fn is_local_control_unavailable(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.to_string().contains("Connection refused")
}

fn api_error_detail(payload: &Value) -> String {
    let code = payload.get("code").and_then(Value::as_str);
    let error = payload
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    let current_state = payload.get("current_state").and_then(Value::as_str);
    match (code, current_state) {
        (Some(code), Some(current_state)) => {
            format!("{code}: {error} (current_state={current_state})")
        }
        (Some(code), None) => format!("{code}: {error}"),
        (None, _) => error.to_string(),
    }
}

async fn detect_availability(config: &PylonConfig) -> Result<ProviderAvailability> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build pylon health-check client")?;
    let ollama = detect_ollama(&client, config).await;
    let apple_foundation_models = detect_apple_fm(&client, config).await;
    let sandbox = detect_sandbox_supply(
        &ProviderSandboxDetectionConfig::default()
            .with_declared_profiles(config.declared_sandbox_profiles.clone()),
    );
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
            };
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
            };
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
            };
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
                            model
                                .get("id")
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
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
        Command, PylonConfig, apply_config_set, apply_control_command, default_config,
        ensure_identity, load_or_create_config, load_status_or_detect, parse_args,
        render_human_status,
    };
    use openagents_provider_substrate::{
        ProviderControlAction, ProviderDesiredMode, ProviderInventoryControls,
        provider_runtime_state_label,
    };

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
    fn parse_args_supports_lifecycle_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["online".to_string()])?.command == Command::Online,
            "online should parse into the online command",
        )?;
        ensure(
            parse_args(vec!["offline".to_string()])?.command == Command::Offline,
            "offline should parse into the offline command",
        )?;
        ensure(
            parse_args(vec!["pause".to_string()])?.command == Command::Pause,
            "pause should parse into the pause command",
        )?;
        ensure(
            parse_args(vec!["resume".to_string()])?.command == Command::Resume,
            "resume should parse into the resume command",
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

    #[tokio::test(flavor = "current_thread")]
    async fn status_reports_unconfigured_before_init() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");

        let status = load_status_or_detect(config_path.as_path()).await?;

        ensure(
            status.desired_mode == ProviderDesiredMode::Offline,
            "unconfigured status should default desired mode to offline",
        )?;
        ensure(
            provider_runtime_state_label(&status) == "unconfigured",
            "status should report an unconfigured runtime before init",
        )?;
        let human = render_human_status(&status);
        ensure(
            human.contains("state: unconfigured"),
            "human-readable status should include the unconfigured state",
        )?;
        ensure(
            status.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .runtime
                    .provider_blocker_codes
                    .contains(&"CONFIG_MISSING".to_string())
            }),
            "unconfigured status should include a machine-readable CONFIG_MISSING blocker",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_control_transitions_cover_success_retry_and_failure_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        let config = load_or_create_config(config_path.as_path())?;
        ensure_identity(config.identity_path.as_path())?;

        let pause_error = match apply_control_command(
            config_path.as_path(),
            ProviderControlAction::Pause,
        )
        .await
        {
            Ok(_) => {
                return Err(std::io::Error::other(
                    "pause should fail while the provider is offline",
                )
                .into());
            }
            Err(error) => error,
        };
        ensure(
            pause_error.to_string().contains("provider_not_online"),
            "pause failure should expose a machine-readable transition code",
        )?;

        let online_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;
        ensure(
            online_status.desired_mode == ProviderDesiredMode::Online,
            "online should persist the online desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&online_status) == "degraded",
            "without a ready backend, online should still report degraded rather than healthy",
        )?;

        let retried_online_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Online).await?;
        ensure(
            retried_online_status.desired_mode == ProviderDesiredMode::Online,
            "repeated online should be an idempotent retry",
        )?;

        let paused_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Pause).await?;
        ensure(
            paused_status.desired_mode == ProviderDesiredMode::Paused,
            "pause should persist the paused desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&paused_status) == "paused",
            "pause should surface the paused runtime state",
        )?;

        let resumed_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Resume).await?;
        ensure(
            resumed_status.desired_mode == ProviderDesiredMode::Online,
            "resume should restore the online desired mode",
        )?;

        let offline_status =
            apply_control_command(config_path.as_path(), ProviderControlAction::Offline).await?;
        ensure(
            offline_status.desired_mode == ProviderDesiredMode::Offline,
            "offline should persist the offline desired mode",
        )?;
        ensure(
            provider_runtime_state_label(&offline_status) == "offline",
            "offline should surface the offline runtime state when no supply is ready",
        )
    }
}
