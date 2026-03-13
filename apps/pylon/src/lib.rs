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
    ProviderPersistedSnapshot, ProviderPersistenceStore, ProviderReceiptSummary, ProviderRecentJob,
    ProviderRuntimeStatusSnapshot, ProviderSandboxDetectionConfig, ProviderSandboxProfile,
    ProviderSandboxProfileSpec, ProviderSandboxRuntimeHealth, ProviderSnapshotParts,
    ProviderStatusResponse, assemble_provider_persisted_snapshot, derive_provider_products,
    detect_sandbox_supply, provider_runtime_state_label, validate_provider_control_action,
};
use serde::de::DeserializeOwned;
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
    Backends { json: bool },
    Inventory { json: bool, limit: Option<usize> },
    Products { json: bool },
    Sandbox { json: bool, limit: Option<usize> },
    Jobs { json: bool, limit: Option<usize> },
    Earnings { json: bool },
    Receipts { json: bool, limit: Option<usize> },
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ReportContext {
    state: String,
    desired_mode: String,
    listen_addr: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct BackendReport {
    context: ReportContext,
    backends: Vec<BackendEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct BackendEntry {
    backend_id: String,
    display_label: String,
    health_state: String,
    reachable: bool,
    ready: bool,
    ready_model: Option<String>,
    available_models: Vec<String>,
    availability_message: Option<String>,
    launch_product_ids: Vec<String>,
    eligible_product_ids: Vec<String>,
    supported_execution_classes: Vec<String>,
    ready_execution_classes: Vec<String>,
    runtime_kinds: Vec<String>,
    ready_runtime_kinds: Vec<String>,
    profile_ids: Vec<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ProductReport {
    context: ReportContext,
    products: Vec<ProductEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ProductEntry {
    product_id: String,
    display_label: String,
    compute_family: String,
    backend: String,
    enabled: bool,
    backend_ready: bool,
    eligible: bool,
    capability_summary: String,
    price_floor_sats: u64,
    terms_label: String,
    forward_terms_label: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct InventoryReport {
    context: ReportContext,
    rows: Vec<ProviderInventoryRow>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct JobsReport {
    context: ReportContext,
    jobs: Vec<ProviderRecentJob>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct EarningsReport {
    context: ReportContext,
    earnings: Option<ProviderEarningsSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ReceiptsReport {
    context: ReportContext,
    receipts: Vec<ProviderReceiptSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct SandboxReport {
    context: ReportContext,
    supported_execution_classes: Vec<String>,
    ready_execution_classes: Vec<String>,
    last_scan_error: Option<String>,
    runtimes: Vec<ProviderSandboxRuntimeHealth>,
    profiles: Vec<ProviderSandboxProfile>,
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
        Command::Backends { json } => {
            let report = load_backend_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_backend_report(&report)))
        }
        Command::Inventory { json, limit } => {
            let report = load_inventory_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_inventory_report(&report)))
        }
        Command::Products { json } => {
            let report = load_product_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_product_report(&report)))
        }
        Command::Sandbox { json, limit } => {
            let report = load_sandbox_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_sandbox_report(&report)))
        }
        Command::Jobs { json, limit } => {
            let report = load_jobs_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_jobs_report(&report)))
        }
        Command::Earnings { json } => {
            let report = load_earnings_report(cli.config_path.as_path()).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_earnings_report(&report)))
        }
        Command::Receipts { json, limit } => {
            let report = load_receipts_report(cli.config_path.as_path(), limit).await?;
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_receipts_report(&report)))
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
  backends [--json]\n\
  inventory [--json] [--limit <n>]\n\
  products [--json]\n\
  sandbox [--json] [--limit <n>]\n\
  jobs [--json] [--limit <n>]\n\
  earnings [--json]\n\
  receipts [--json] [--limit <n>]\n\
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
        "backends" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "backends", false)?;
            if limit.is_some() {
                bail!("backends does not support --limit");
            }
            Ok(Command::Backends { json })
        }
        "inventory" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "inventory", true)?;
            Ok(Command::Inventory { json, limit })
        }
        "products" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "products", false)?;
            if limit.is_some() {
                bail!("products does not support --limit");
            }
            Ok(Command::Products { json })
        }
        "sandbox" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "sandbox", true)?;
            Ok(Command::Sandbox { json, limit })
        }
        "jobs" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "jobs", true)?;
            Ok(Command::Jobs { json, limit })
        }
        "earnings" => {
            let (json, limit) =
                parse_observability_flags(args, start_index + 1, "earnings", false)?;
            if limit.is_some() {
                bail!("earnings does not support --limit");
            }
            Ok(Command::Earnings { json })
        }
        "receipts" => {
            let (json, limit) = parse_observability_flags(args, start_index + 1, "receipts", true)?;
            Ok(Command::Receipts { json, limit })
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

fn parse_observability_flags(
    args: &[String],
    mut index: usize,
    command: &str,
    allow_limit: bool,
) -> Result<(bool, Option<usize>)> {
    let mut json = false;
    let mut limit = None;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--limit" if allow_limit => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| anyhow!("missing value for --limit"))?;
                limit =
                    Some(value.parse::<usize>().with_context(|| {
                        format!("invalid numeric limit for {command}: {value}")
                    })?);
                index += 1;
            }
            other => bail!("unexpected argument for {command}: {other}"),
        }
    }
    Ok((json, limit))
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

    assemble_provider_persisted_snapshot(ProviderSnapshotParts {
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
    })
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
    assemble_provider_persisted_snapshot(ProviderSnapshotParts {
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
    })
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
    if !availability.gpt_oss.ready {
        codes.push("GPT_OSS_UNAVAILABLE".to_string());
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
        .gpt_oss
        .last_error
        .clone()
        .or_else(|| availability.apple_foundation_models.last_error.clone())
        .or_else(|| availability.sandbox.last_scan_error.clone())
}

fn render_sandbox_status_lines(availability: &ProviderAvailability) -> Vec<String> {
    let mut lines = Vec::new();
    let sandbox = &availability.sandbox;
    let supported = sandbox_supported_execution_classes(availability);
    let ready = sandbox_ready_execution_classes(availability);
    let runtimes = sandbox_runtime_kinds(availability, false);
    let profiles = sandbox_profile_ids(availability);
    if !supported.is_empty()
        || !runtimes.is_empty()
        || !profiles.is_empty()
        || sandbox.last_scan_error.is_some()
    {
        lines.push(format!(
            "sandbox_execution_classes: {}",
            comma_or_none(supported.as_slice())
        ));
        lines.push(format!(
            "sandbox_ready_classes: {}",
            comma_or_none(ready.as_slice())
        ));
        lines.push(format!(
            "sandbox_runtimes: {}",
            comma_or_none(runtimes.as_slice())
        ));
        lines.push(format!(
            "sandbox_profiles: {}",
            comma_or_none(profiles.as_slice())
        ));
        if let Some(last_scan_error) = sandbox.last_scan_error.as_deref() {
            lines.push(format!("sandbox_last_error: {last_scan_error}"));
        }
    }
    lines
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
        lines.extend(render_sandbox_status_lines(&snapshot.availability));
    }
    lines.join("\n")
}

fn load_config_or_default(path: &Path) -> Result<PylonConfig> {
    if path.exists() {
        return load_config(path);
    }
    let base_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_home_dir);
    Ok(default_config(base_dir.as_path()))
}

async fn load_config_and_status(
    config_path: &Path,
) -> Result<(PylonConfig, ProviderStatusResponse)> {
    let config = load_config_or_default(config_path)?;
    let status = load_status_or_detect(config_path).await?;
    Ok((config, status))
}

fn report_context(status: &ProviderStatusResponse) -> ReportContext {
    ReportContext {
        state: provider_runtime_state_label(status),
        desired_mode: status.desired_mode.label().to_string(),
        listen_addr: status.listen_addr.clone(),
    }
}

fn products_from_status(
    config: &PylonConfig,
    status: &ProviderStatusResponse,
) -> Vec<ProviderAdvertisedProduct> {
    status
        .snapshot
        .as_ref()
        .map(|snapshot| {
            derive_provider_products(&snapshot.availability, &config.inventory_controls)
        })
        .unwrap_or_default()
}

fn backend_entry(
    backend_id: &str,
    display_label: &str,
    health_state: String,
    health: &ProviderBackendHealth,
    products: &[ProviderAdvertisedProduct],
) -> BackendEntry {
    let launch_product_ids = products
        .iter()
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    let eligible_product_ids = products
        .iter()
        .filter(|product| product.eligible)
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    BackendEntry {
        backend_id: backend_id.to_string(),
        display_label: display_label.to_string(),
        health_state,
        reachable: health.reachable,
        ready: health.ready,
        ready_model: health.ready_model.clone(),
        available_models: health.available_models.clone(),
        availability_message: health.availability_message.clone(),
        launch_product_ids,
        eligible_product_ids,
        supported_execution_classes: Vec::new(),
        ready_execution_classes: Vec::new(),
        runtime_kinds: Vec::new(),
        ready_runtime_kinds: Vec::new(),
        profile_ids: Vec::new(),
        last_error: health.last_error.clone(),
    }
}

fn gpt_oss_health_state(config: &PylonConfig, health: &ProviderBackendHealth) -> String {
    if !config.inventory_controls.gpt_oss_inference_enabled
        && !config.inventory_controls.gpt_oss_embeddings_enabled
    {
        return "disabled".to_string();
    }
    if health.ready {
        return "healthy".to_string();
    }
    if health.reachable && health.available_models.is_empty() {
        return "misconfigured".to_string();
    }
    if !health.reachable || health.last_error.is_some() {
        return "unavailable".to_string();
    }
    "misconfigured".to_string()
}

fn apple_fm_health_state(config: &PylonConfig, health: &ProviderBackendHealth) -> String {
    if !config.inventory_controls.apple_fm_inference_enabled {
        return "disabled".to_string();
    }
    if health.ready {
        return "healthy".to_string();
    }
    if config.apple_fm_base_url.is_none() {
        return if std::env::consts::OS == "macos" {
            "misconfigured".to_string()
        } else {
            "unsupported".to_string()
        };
    }
    if !health.reachable || health.last_error.is_some() {
        return "unavailable".to_string();
    }
    if health.available_models.is_empty() {
        return "misconfigured".to_string();
    }
    "misconfigured".to_string()
}

fn sandbox_controls_enabled(config: &PylonConfig) -> bool {
    let controls = &config.inventory_controls;
    controls.sandbox_container_exec_enabled
        || controls.sandbox_python_exec_enabled
        || controls.sandbox_node_exec_enabled
        || controls.sandbox_posix_exec_enabled
}

fn sandbox_supported_execution_classes(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .declared_execution_classes()
        .into_iter()
        .map(|execution_class| execution_class.product_id().to_string())
        .collect()
}

fn sandbox_ready_execution_classes(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .ready_execution_classes()
        .into_iter()
        .map(|execution_class| execution_class.product_id().to_string())
        .collect()
}

fn sandbox_runtime_kinds(availability: &ProviderAvailability, ready_only: bool) -> Vec<String> {
    let kinds = if ready_only {
        availability.sandbox.ready_runtime_kinds()
    } else {
        availability.sandbox.detected_runtime_kinds()
    };
    kinds
        .into_iter()
        .map(|runtime_kind| runtime_kind.id().to_string())
        .collect()
}

fn sandbox_profile_ids(availability: &ProviderAvailability) -> Vec<String> {
    availability
        .sandbox
        .profiles
        .iter()
        .map(|profile| profile.profile_id.clone())
        .collect()
}

fn sandbox_health_state(
    config: &PylonConfig,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> String {
    if !sandbox_controls_enabled(config) {
        return "disabled".to_string();
    }
    if availability.sandbox.last_scan_error.is_some() {
        return "error".to_string();
    }
    if products.iter().any(|product| product.eligible) {
        return "healthy".to_string();
    }
    if availability.sandbox.profiles.is_empty() {
        return if availability.sandbox.detected_runtime_kinds().is_empty() {
            "unsupported".to_string()
        } else {
            "misconfigured".to_string()
        };
    }
    if availability.sandbox.ready_runtime_kinds().is_empty() {
        return "unavailable".to_string();
    }
    "misconfigured".to_string()
}

fn sandbox_backend_entry(
    config: &PylonConfig,
    availability: &ProviderAvailability,
    products: &[ProviderAdvertisedProduct],
) -> BackendEntry {
    let visible_product_ids = products
        .iter()
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    let eligible_product_ids = products
        .iter()
        .filter(|product| product.eligible)
        .map(|product| product.product.product_id().to_string())
        .collect::<Vec<_>>();
    BackendEntry {
        backend_id: "sandbox".to_string(),
        display_label: "Declared sandbox runtime".to_string(),
        health_state: sandbox_health_state(config, availability, products),
        reachable: !availability.sandbox.detected_runtime_kinds().is_empty(),
        ready: !availability.sandbox.ready_runtime_kinds().is_empty(),
        ready_model: None,
        available_models: Vec::new(),
        availability_message: availability.sandbox.last_scan_error.clone().or_else(|| {
            if availability.sandbox.profiles.is_empty() {
                Some("no declared sandbox profiles".to_string())
            } else {
                None
            }
        }),
        launch_product_ids: visible_product_ids,
        eligible_product_ids,
        supported_execution_classes: sandbox_supported_execution_classes(availability),
        ready_execution_classes: sandbox_ready_execution_classes(availability),
        runtime_kinds: sandbox_runtime_kinds(availability, false),
        ready_runtime_kinds: sandbox_runtime_kinds(availability, true),
        profile_ids: sandbox_profile_ids(availability),
        last_error: availability.sandbox.last_scan_error.clone(),
    }
}

async fn load_backend_report(config_path: &Path) -> Result<BackendReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let availability = if config_path.exists() {
        try_live_json::<ProviderAvailability>(&config, "/v1/backend-health")
            .await?
            .or_else(|| {
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.clone())
            })
            .unwrap_or_default()
    } else {
        ProviderAvailability::default()
    };
    let products = derive_provider_products(&availability, &config.inventory_controls);
    let gpt_oss_products = products
        .iter()
        .filter(|product| product.product.backend_label() == "gpt_oss")
        .cloned()
        .collect::<Vec<_>>();
    let apple_fm_products = products
        .iter()
        .filter(|product| product.product.backend_label() == "apple_foundation_models")
        .cloned()
        .collect::<Vec<_>>();
    let sandbox_products = products
        .iter()
        .filter(|product| product.product.backend_label() == "sandbox")
        .cloned()
        .collect::<Vec<_>>();
    Ok(BackendReport {
        context: report_context(&status),
        backends: vec![
            backend_entry(
                "gpt_oss",
                "GPT-OSS",
                gpt_oss_health_state(&config, &availability.gpt_oss),
                &availability.gpt_oss,
                gpt_oss_products.as_slice(),
            ),
            backend_entry(
                "apple_foundation_models",
                "Apple Foundation Models",
                apple_fm_health_state(&config, &availability.apple_foundation_models),
                &availability.apple_foundation_models,
                apple_fm_products.as_slice(),
            ),
            sandbox_backend_entry(&config, &availability, sandbox_products.as_slice()),
        ],
    })
}

async fn load_inventory_report(
    config_path: &Path,
    limit: Option<usize>,
) -> Result<InventoryReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let rows = if config_path.exists() {
        if let Some(rows) =
            try_live_json::<Vec<ProviderInventoryRow>>(&config, inventory_endpoint(limit).as_str())
                .await?
        {
            rows
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_inventory_rows(limit)
                .map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.inventory_rows.clone())
                    .unwrap_or_default(),
                limit,
            )
        }
    } else {
        Vec::new()
    };
    Ok(InventoryReport {
        context: report_context(&status),
        rows,
    })
}

async fn load_product_report(config_path: &Path) -> Result<ProductReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let products = products_from_status(&config, &status)
        .into_iter()
        .map(|product| ProductEntry {
            product_id: product.product.product_id().to_string(),
            display_label: product.product.display_label().to_string(),
            compute_family: product.product.compute_family_label().to_string(),
            backend: product.product.backend_label().to_string(),
            enabled: product.enabled,
            backend_ready: product.backend_ready,
            eligible: product.eligible,
            capability_summary: product.capability_summary,
            price_floor_sats: product.price_floor_sats,
            terms_label: product.terms_label,
            forward_terms_label: product.forward_terms_label,
        })
        .collect();
    Ok(ProductReport {
        context: report_context(&status),
        products,
    })
}

async fn load_sandbox_report(config_path: &Path, limit: Option<usize>) -> Result<SandboxReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let (runtimes, profiles, last_scan_error) = if config_path.exists() {
        let runtimes = if let Some(runtimes) = try_live_json::<Vec<ProviderSandboxRuntimeHealth>>(
            &config,
            sandbox_runtimes_endpoint(limit).as_str(),
        )
        .await?
        {
            runtimes
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_sandbox_runtimes(limit)
                .map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.sandbox.runtimes.clone())
                    .unwrap_or_default(),
                limit,
            )
        };
        let profiles = if let Some(profiles) = try_live_json::<Vec<ProviderSandboxProfile>>(
            &config,
            sandbox_profiles_endpoint(limit).as_str(),
        )
        .await?
        {
            profiles
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_sandbox_profiles(limit)
                .map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.availability.sandbox.profiles.clone())
                    .unwrap_or_default(),
                limit,
            )
        };
        let last_scan_error = status
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.availability.sandbox.last_scan_error.clone());
        (runtimes, profiles, last_scan_error)
    } else {
        (Vec::new(), Vec::new(), None)
    };

    let supported_execution_classes = profiles
        .iter()
        .map(|profile| profile.execution_class.product_id().to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let ready_execution_classes = profiles
        .iter()
        .filter(|profile| profile.runtime_ready)
        .map(|profile| profile.execution_class.product_id().to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(SandboxReport {
        context: report_context(&status),
        supported_execution_classes,
        ready_execution_classes,
        last_scan_error,
        runtimes,
        profiles,
    })
}

async fn load_jobs_report(config_path: &Path, limit: Option<usize>) -> Result<JobsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let jobs = if config_path.exists() {
        if let Some(jobs) =
            try_live_json::<Vec<ProviderRecentJob>>(&config, jobs_endpoint(limit).as_str()).await?
        {
            jobs
        } else if let Some(store) = open_existing_store(&config)? {
            store.load_recent_jobs(limit).map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.recent_jobs.clone())
                    .unwrap_or_default(),
                limit,
            )
        }
    } else {
        Vec::new()
    };
    Ok(JobsReport {
        context: report_context(&status),
        jobs,
    })
}

async fn load_earnings_report(config_path: &Path) -> Result<EarningsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let earnings = if config_path.exists() {
        if let Some(earnings) =
            try_live_json::<Option<ProviderEarningsSummary>>(&config, "/v1/earnings").await?
        {
            earnings
        } else if let Some(store) = open_existing_store(&config)? {
            store
                .load_status()
                .map_err(anyhow::Error::msg)?
                .snapshot
                .and_then(|snapshot| snapshot.earnings)
        } else {
            status
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.earnings.clone())
        }
    } else {
        None
    };
    Ok(EarningsReport {
        context: report_context(&status),
        earnings,
    })
}

async fn load_receipts_report(config_path: &Path, limit: Option<usize>) -> Result<ReceiptsReport> {
    let (config, status) = load_config_and_status(config_path).await?;
    let receipts = if config_path.exists() {
        if let Some(receipts) =
            try_live_json::<Vec<ProviderReceiptSummary>>(&config, receipts_endpoint(limit).as_str())
                .await?
        {
            receipts
        } else if let Some(store) = open_existing_store(&config)? {
            store.load_receipts(limit).map_err(anyhow::Error::msg)?
        } else {
            take_limited_rows(
                status
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.receipts.clone())
                    .unwrap_or_default(),
                limit,
            )
        }
    } else {
        Vec::new()
    };
    Ok(ReceiptsReport {
        context: report_context(&status),
        receipts,
    })
}

fn open_existing_store(config: &PylonConfig) -> Result<Option<ProviderPersistenceStore>> {
    if !config.admin_db_path.exists() {
        return Ok(None);
    }
    let admin_config = provider_admin_config(config)?;
    Ok(Some(
        ProviderPersistenceStore::open(&admin_config).map_err(anyhow::Error::msg)?,
    ))
}

fn inventory_endpoint(limit: Option<usize>) -> String {
    format!("/v1/inventory?limit={}", limit.unwrap_or(32))
}

fn sandbox_runtimes_endpoint(limit: Option<usize>) -> String {
    format!("/v1/sandbox/runtimes?limit={}", limit.unwrap_or(32))
}

fn sandbox_profiles_endpoint(limit: Option<usize>) -> String {
    format!("/v1/sandbox/profiles?limit={}", limit.unwrap_or(32))
}

fn jobs_endpoint(limit: Option<usize>) -> String {
    format!("/v1/jobs?limit={}", limit.unwrap_or(32))
}

fn receipts_endpoint(limit: Option<usize>) -> String {
    format!("/v1/receipts?limit={}", limit.unwrap_or(32))
}

fn take_limited_rows<T>(mut values: Vec<T>, limit: Option<usize>) -> Vec<T> {
    if let Some(limit) = limit {
        values.truncate(limit);
    }
    values
}

fn render_report_context(context: &ReportContext) -> Vec<String> {
    let mut lines = vec![
        format!("state: {}", context.state),
        format!("desired_mode: {}", context.desired_mode),
    ];
    if let Some(listen_addr) = context.listen_addr.as_deref() {
        lines.push(format!("listen_addr: {listen_addr}"));
    }
    lines
}

fn render_backend_report(report: &BackendReport) -> String {
    let mut lines = render_report_context(&report.context);
    for backend in &report.backends {
        lines.push(String::new());
        lines.push(format!("backend: {}", backend.backend_id));
        lines.push(format!("display_label: {}", backend.display_label));
        lines.push(format!("health_state: {}", backend.health_state));
        lines.push(format!(
            "launch_products: {}",
            comma_or_none(backend.launch_product_ids.as_slice())
        ));
        lines.push(format!(
            "eligible_products: {}",
            comma_or_none(backend.eligible_product_ids.as_slice())
        ));
        if !backend.supported_execution_classes.is_empty() {
            lines.push(format!(
                "supported_execution_classes: {}",
                comma_or_none(backend.supported_execution_classes.as_slice())
            ));
        }
        if !backend.ready_execution_classes.is_empty() {
            lines.push(format!(
                "ready_execution_classes: {}",
                comma_or_none(backend.ready_execution_classes.as_slice())
            ));
        }
        if !backend.runtime_kinds.is_empty() {
            lines.push(format!(
                "runtime_kinds: {}",
                comma_or_none(backend.runtime_kinds.as_slice())
            ));
        }
        if !backend.ready_runtime_kinds.is_empty() {
            lines.push(format!(
                "ready_runtime_kinds: {}",
                comma_or_none(backend.ready_runtime_kinds.as_slice())
            ));
        }
        if !backend.profile_ids.is_empty() {
            lines.push(format!(
                "profile_ids: {}",
                comma_or_none(backend.profile_ids.as_slice())
            ));
        }
        lines.push(format!(
            "ready_model: {}",
            backend.ready_model.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "available_models: {}",
            comma_or_none(backend.available_models.as_slice())
        ));
        if let Some(message) = backend.availability_message.as_deref() {
            lines.push(format!("availability_message: {message}"));
        }
        if let Some(last_error) = backend.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
    }
    lines.join("\n")
}

fn render_inventory_report(report: &InventoryReport) -> String {
    let mut lines = render_report_context(&report.context);
    for row in &report.rows {
        lines.push(String::new());
        lines.push(format!("product: {}", row.target.product_id()));
        lines.push(format!("enabled: {}", row.enabled));
        lines.push(format!("backend_ready: {}", row.backend_ready));
        lines.push(format!("eligible: {}", row.eligible));
        lines.push(format!("delivery_state: {}", row.delivery_state));
        lines.push(format!(
            "quantity: total={} reserved={} available={}",
            row.total_quantity, row.reserved_quantity, row.available_quantity
        ));
        lines.push(format!("capability: {}", row.capability_summary));
    }
    lines.join("\n")
}

fn render_product_report(report: &ProductReport) -> String {
    let mut lines = render_report_context(&report.context);
    for product in &report.products {
        lines.push(String::new());
        lines.push(format!("product: {}", product.product_id));
        lines.push(format!("display_label: {}", product.display_label));
        lines.push(format!("family: {}", product.compute_family));
        lines.push(format!("backend: {}", product.backend));
        lines.push(format!("enabled: {}", product.enabled));
        lines.push(format!("backend_ready: {}", product.backend_ready));
        lines.push(format!("eligible: {}", product.eligible));
        lines.push(format!("price_floor_sats: {}", product.price_floor_sats));
        lines.push(format!("terms: {}", product.terms_label));
        lines.push(format!("forward_terms: {}", product.forward_terms_label));
        lines.push(format!("capability: {}", product.capability_summary));
    }
    lines.join("\n")
}

fn render_jobs_report(report: &JobsReport) -> String {
    let mut lines = render_report_context(&report.context);
    for job in &report.jobs {
        lines.push(String::new());
        lines.push(format!("job_id: {}", job.job_id));
        lines.push(format!("status: {}", job.status));
        lines.push(format!("demand_source: {}", job.demand_source));
        lines.push(format!(
            "product_id: {}",
            job.product_id.as_deref().unwrap_or("none")
        ));
        if let Some(compute_family) = job.compute_family.as_deref() {
            lines.push(format!("compute_family: {compute_family}"));
        }
        if let Some(backend_family) = job.backend_family.as_deref() {
            lines.push(format!("backend_family: {backend_family}"));
        }
        if let Some(execution_class) = job.sandbox_execution_class.as_deref() {
            lines.push(format!("sandbox_execution_class: {execution_class}"));
        }
        if let Some(profile_id) = job.sandbox_profile_id.as_deref() {
            lines.push(format!("sandbox_profile_id: {profile_id}"));
        }
        if let Some(profile_digest) = job.sandbox_profile_digest.as_deref() {
            lines.push(format!("sandbox_profile_digest: {profile_digest}"));
        }
        if let Some(termination_reason) = job.sandbox_termination_reason.as_deref() {
            lines.push(format!("sandbox_termination_reason: {termination_reason}"));
        }
        lines.push(format!("payout_sats: {}", job.payout_sats));
        if let Some(failure_reason) = job.failure_reason.as_deref() {
            lines.push(format!("failure_reason: {failure_reason}"));
        }
    }
    lines.join("\n")
}

fn render_earnings_report(report: &EarningsReport) -> String {
    let mut lines = render_report_context(&report.context);
    match report.earnings.as_ref() {
        Some(earnings) => {
            lines.push(String::new());
            lines.push(format!("sats_today: {}", earnings.sats_today));
            lines.push(format!("lifetime_sats: {}", earnings.lifetime_sats));
            lines.push(format!("jobs_today: {}", earnings.jobs_today));
            lines.push(format!(
                "online_uptime_seconds: {}",
                earnings.online_uptime_seconds
            ));
            lines.push(format!("last_job_result: {}", earnings.last_job_result));
        }
        None => {
            lines.push(String::new());
            lines.push("earnings: none".to_string());
        }
    }
    lines.join("\n")
}

fn render_receipts_report(report: &ReceiptsReport) -> String {
    let mut lines = render_report_context(&report.context);
    for receipt in &report.receipts {
        lines.push(String::new());
        lines.push(format!("receipt_id: {}", receipt.receipt_id));
        lines.push(format!("receipt_type: {}", receipt.receipt_type));
        lines.push(format!("canonical_hash: {}", receipt.canonical_hash));
        lines.push(format!("created_at_ms: {}", receipt.created_at_ms));
        if let Some(compute_family) = receipt.compute_family.as_deref() {
            lines.push(format!("compute_family: {compute_family}"));
        }
        if let Some(backend_family) = receipt.backend_family.as_deref() {
            lines.push(format!("backend_family: {backend_family}"));
        }
        if let Some(execution_class) = receipt.sandbox_execution_class.as_deref() {
            lines.push(format!("sandbox_execution_class: {execution_class}"));
        }
        if let Some(profile_id) = receipt.sandbox_profile_id.as_deref() {
            lines.push(format!("sandbox_profile_id: {profile_id}"));
        }
        if let Some(profile_digest) = receipt.sandbox_profile_digest.as_deref() {
            lines.push(format!("sandbox_profile_digest: {profile_digest}"));
        }
        if let Some(termination_reason) = receipt.sandbox_termination_reason.as_deref() {
            lines.push(format!("sandbox_termination_reason: {termination_reason}"));
        }
        if let Some(reason_code) = receipt.reason_code.as_deref() {
            lines.push(format!("reason_code: {reason_code}"));
        }
        if let Some(failure_reason) = receipt.failure_reason.as_deref() {
            lines.push(format!("failure_reason: {failure_reason}"));
        }
        if let Some(notional_sats) = receipt.notional_sats {
            lines.push(format!("notional_sats: {notional_sats}"));
        }
    }
    lines.join("\n")
}

fn render_sandbox_report(report: &SandboxReport) -> String {
    let mut lines = render_report_context(&report.context);
    lines.push(String::new());
    lines.push(format!(
        "supported_execution_classes: {}",
        comma_or_none(report.supported_execution_classes.as_slice())
    ));
    lines.push(format!(
        "ready_execution_classes: {}",
        comma_or_none(report.ready_execution_classes.as_slice())
    ));
    if let Some(last_scan_error) = report.last_scan_error.as_deref() {
        lines.push(format!("last_scan_error: {last_scan_error}"));
    }
    for runtime in &report.runtimes {
        let supported_execution_classes = runtime
            .supported_execution_classes
            .iter()
            .map(|execution_class| execution_class.product_id().to_string())
            .collect::<Vec<_>>();
        lines.push(String::new());
        lines.push(format!("runtime_kind: {}", runtime.runtime_kind.id()));
        lines.push(format!("detected: {}", runtime.detected));
        lines.push(format!("ready: {}", runtime.ready));
        lines.push(format!(
            "supported_execution_classes: {}",
            comma_or_none(supported_execution_classes.as_slice())
        ));
        if let Some(binary_name) = runtime.binary_name.as_deref() {
            lines.push(format!("binary_name: {binary_name}"));
        }
        if let Some(binary_path) = runtime.binary_path.as_deref() {
            lines.push(format!("binary_path: {binary_path}"));
        }
        if let Some(runtime_version) = runtime.runtime_version.as_deref() {
            lines.push(format!("runtime_version: {runtime_version}"));
        }
        if let Some(last_error) = runtime.last_error.as_deref() {
            lines.push(format!("last_error: {last_error}"));
        }
    }
    for profile in &report.profiles {
        lines.push(String::new());
        lines.push(format!("profile_id: {}", profile.profile_id));
        lines.push(format!(
            "execution_class: {}",
            profile.execution_class.product_id()
        ));
        lines.push(format!("profile_digest: {}", profile.profile_digest));
        lines.push(format!("runtime_kind: {}", profile.runtime_kind.id()));
        lines.push(format!("runtime_ready: {}", profile.runtime_ready));
        lines.push(format!("network_mode: {}", profile.network_mode));
        lines.push(format!("filesystem_mode: {}", profile.filesystem_mode));
        lines.push(format!("timeout_limit_s: {}", profile.timeout_limit_s));
        if let Some(accelerator_policy) = profile.accelerator_policy.as_deref() {
            lines.push(format!("accelerator_policy: {accelerator_policy}"));
        }
    }
    lines.join("\n")
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
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

async fn try_live_json<T: DeserializeOwned>(
    config: &PylonConfig,
    endpoint: &str,
) -> Result<Option<T>> {
    let client = admin_client()?;
    let url = format!("http://{}{}", config.admin_listen_addr, endpoint);
    let response = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(error) if is_local_control_unavailable(&error) => return Ok(None),
        Err(error) => {
            return Err(anyhow!(
                "failed to query pylon admin endpoint {}: {error}",
                endpoint
            ));
        }
    };
    if !response.status().is_success() {
        let payload = response
            .json::<Value>()
            .await
            .unwrap_or_else(|_| json!({"error": "failed to decode provider admin error"}));
        bail!(
            "provider admin endpoint {} failed: {}",
            endpoint,
            api_error_detail(&payload)
        );
    }
    let value = response
        .json::<T>()
        .await
        .with_context(|| format!("failed to decode pylon admin endpoint {}", endpoint))?;
    Ok(Some(value))
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
    let gpt_oss = detect_ollama(&client, config).await;
    let apple_foundation_models = detect_apple_fm(&client, config).await;
    let sandbox = detect_sandbox_supply(
        &ProviderSandboxDetectionConfig::default()
            .with_declared_profiles(config.declared_sandbox_profiles.clone()),
    );
    Ok(ProviderAvailability {
        gpt_oss,
        apple_foundation_models,
        sandbox,
    })
}

async fn detect_ollama(client: &reqwest::Client, config: &PylonConfig) -> ProviderBackendHealth {
    if !config.inventory_controls.gpt_oss_inference_enabled
        && !config.inventory_controls.gpt_oss_embeddings_enabled
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
                last_action: Some("invalid gpt-oss health payload".to_string()),
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
        "backend.gpt_oss_inference_enabled" | "backend.ollama_inference_enabled" => {
            config.inventory_controls.gpt_oss_inference_enabled = parse_bool(value)?;
        }
        "backend.gpt_oss_embeddings_enabled" | "backend.ollama_embeddings_enabled" => {
            config.inventory_controls.gpt_oss_embeddings_enabled = parse_bool(value)?;
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
        Command, PylonConfig, apply_config_set, apply_control_command,
        build_snapshot_from_availability, default_config, ensure_identity, inventory_rows,
        load_backend_report, load_earnings_report, load_inventory_report, load_jobs_report,
        load_or_create_config, load_product_report, load_receipts_report, load_sandbox_report,
        load_status_or_detect, parse_args, provider_admin_config, render_human_status,
        render_sandbox_report, save_config,
    };
    use openagents_provider_substrate::{
        ProviderAvailability, ProviderBackendHealth, ProviderControlAction, ProviderDesiredMode,
        ProviderEarningsSummary, ProviderInventoryControls, ProviderPersistenceStore,
        ProviderReceiptSummary, ProviderRecentJob, ProviderSandboxAvailability,
        ProviderSandboxExecutionClass, ProviderSandboxProfile, ProviderSandboxProfileSpec,
        ProviderSandboxRuntimeHealth, ProviderSandboxRuntimeKind, provider_runtime_state_label,
    };
    use serde_json::json;

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

    #[test]
    fn parse_args_supports_observability_commands() -> Result<(), Box<dyn std::error::Error>> {
        ensure(
            parse_args(vec!["backends".to_string(), "--json".to_string()])?.command
                == Command::Backends { json: true },
            "backends should parse with --json",
        )?;
        ensure(
            parse_args(vec![
                "inventory".to_string(),
                "--limit".to_string(),
                "5".to_string(),
            ])?
            .command
                == Command::Inventory {
                    json: false,
                    limit: Some(5),
                },
            "inventory should parse with --limit",
        )?;
        ensure(
            parse_args(vec![
                "jobs".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                "2".to_string(),
            ])?
            .command
                == Command::Jobs {
                    json: true,
                    limit: Some(2),
                },
            "jobs should parse with json and limit flags",
        )?;
        ensure(
            parse_args(vec![
                "receipts".to_string(),
                "--limit".to_string(),
                "3".to_string(),
            ])?
            .command
                == Command::Receipts {
                    json: false,
                    limit: Some(3),
                },
            "receipts should parse with a list limit",
        )?;
        ensure(
            parse_args(vec![
                "sandbox".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                "2".to_string(),
            ])?
            .command
                == Command::Sandbox {
                    json: true,
                    limit: Some(2),
                },
            "sandbox should parse with json and limit flags",
        )
    }

    fn ready_health(
        ready_model: &str,
        available_models: &[&str],
        availability_message: Option<&str>,
    ) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: true,
            ready: true,
            configured_model: Some(ready_model.to_string()),
            ready_model: Some(ready_model.to_string()),
            available_models: available_models
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
            last_error: None,
            last_action: Some("health check ready".to_string()),
            availability_message: availability_message.map(str::to_string),
            latency_ms_p50: Some(110),
        }
    }

    fn seed_observability_snapshot(
        config_path: &std::path::Path,
    ) -> Result<PylonConfig, Box<dyn std::error::Error>> {
        let mut config = load_or_create_config(config_path)?;
        let identity = ensure_identity(config.identity_path.as_path())?;
        config.inventory_controls.sandbox_python_exec_enabled = true;
        config.declared_sandbox_profiles = vec![ProviderSandboxProfileSpec {
            profile_id: "python-batch".to_string(),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            runtime_family: "python3".to_string(),
            runtime_version: Some("Python 3.11.8".to_string()),
            sandbox_engine: "local_subprocess".to_string(),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 120,
            network_mode: "none".to_string(),
            filesystem_mode: "workspace_only".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["python3".to_string()],
            toolchain_inventory: vec!["python3".to_string()],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
        }];
        save_config(config_path, &config)?;

        let availability = ProviderAvailability {
            gpt_oss: ready_health(
                "llama3.2:latest",
                &["llama3.2:latest", "nomic-embed-text:latest"],
                None,
            ),
            apple_foundation_models: ready_health(
                "apple-foundation-model",
                &["apple-foundation-model"],
                Some("bridge_ready"),
            ),
            sandbox: ProviderSandboxAvailability {
                runtimes: vec![ProviderSandboxRuntimeHealth {
                    runtime_kind: ProviderSandboxRuntimeKind::Python,
                    detected: true,
                    ready: true,
                    binary_name: Some("python3".to_string()),
                    binary_path: Some("/usr/bin/python3".to_string()),
                    runtime_version: Some("Python 3.11.8".to_string()),
                    supported_execution_classes: vec![
                        ProviderSandboxExecutionClass::PythonExec,
                    ],
                    last_error: None,
                }],
                profiles: vec![ProviderSandboxProfile {
                    profile_id: "python-batch".to_string(),
                    profile_digest: "sha256:python-profile".to_string(),
                    execution_class: ProviderSandboxExecutionClass::PythonExec,
                    runtime_family: "python3".to_string(),
                    runtime_version: "Python 3.11.8".to_string(),
                    sandbox_engine: "local_subprocess".to_string(),
                    os_family: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                    cpu_limit: 2,
                    memory_limit_mb: 2048,
                    disk_limit_mb: 4096,
                    timeout_limit_s: 120,
                    network_mode: "none".to_string(),
                    filesystem_mode: "workspace_only".to_string(),
                    workspace_mode: "ephemeral".to_string(),
                    artifact_output_mode: "declared_paths_only".to_string(),
                    secrets_mode: "none".to_string(),
                    allowed_binaries: vec!["python3".to_string()],
                    toolchain_inventory: vec!["python3".to_string()],
                    container_image: None,
                    runtime_image_digest: None,
                    accelerator_policy: None,
                    runtime_kind: ProviderSandboxRuntimeKind::Python,
                    runtime_ready: true,
                    runtime_binary_path: Some("/usr/bin/python3".to_string()),
                    capability_summary: "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch".to_string(),
                }],
                last_scan_error: None,
            },
        };
        let mut snapshot = build_snapshot_from_availability(
            &config,
            Some(&identity),
            ProviderDesiredMode::Online,
            None,
            availability.clone(),
            None,
        );
        snapshot.inventory_rows = inventory_rows(
            &super::derive_provider_products(&availability, &config.inventory_controls),
            ProviderDesiredMode::Online,
        );
        snapshot.recent_jobs = vec![
            ProviderRecentJob {
                job_id: "job-1".to_string(),
                request_id: Some("req-1".to_string()),
                status: "settled".to_string(),
                demand_source: "open_network".to_string(),
                product_id: Some("gpt_oss.embeddings".to_string()),
                compute_family: Some("embeddings".to_string()),
                backend_family: Some("gpt_oss".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                completed_at_epoch_seconds: 1_762_300_030,
                payout_sats: 42,
                payment_pointer: "payment-1".to_string(),
                failure_reason: None,
                delivery_proof_id: Some("proof-1".to_string()),
            },
            ProviderRecentJob {
                job_id: "job-2".to_string(),
                request_id: Some("req-2".to_string()),
                status: "failed".to_string(),
                demand_source: "open_network".to_string(),
                product_id: Some("sandbox.python.exec".to_string()),
                compute_family: Some("sandbox_execution".to_string()),
                backend_family: Some("sandbox".to_string()),
                sandbox_execution_class: Some("sandbox.python.exec".to_string()),
                sandbox_profile_id: Some("python-batch".to_string()),
                sandbox_profile_digest: Some("sha256:python-profile".to_string()),
                sandbox_termination_reason: Some("timeout".to_string()),
                completed_at_epoch_seconds: 1_762_300_032,
                payout_sats: 0,
                payment_pointer: "payment-2".to_string(),
                failure_reason: Some("sandbox execution exceeded timeout".to_string()),
                delivery_proof_id: Some("proof-2".to_string()),
            },
        ];
        snapshot.receipts = vec![
            ProviderReceiptSummary {
                receipt_id: "receipt-1".to_string(),
                receipt_type: "earn.job.settled.v1".to_string(),
                created_at_ms: 1_762_300_030_500,
                canonical_hash: "sha256:receipt-1".to_string(),
                compute_family: Some("embeddings".to_string()),
                backend_family: Some("gpt_oss".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                reason_code: Some("SETTLED".to_string()),
                failure_reason: None,
                severity: Some("low".to_string()),
                notional_sats: Some(42),
                liability_premium_sats: Some(0),
                work_unit_id: Some("work-unit-1".to_string()),
            },
            ProviderReceiptSummary {
                receipt_id: "receipt-2".to_string(),
                receipt_type: "sandbox.execution.delivery.v1".to_string(),
                created_at_ms: 1_762_300_032_500,
                canonical_hash: "sha256:receipt-2".to_string(),
                compute_family: Some("sandbox_execution".to_string()),
                backend_family: Some("sandbox".to_string()),
                sandbox_execution_class: Some("sandbox.python.exec".to_string()),
                sandbox_profile_id: Some("python-batch".to_string()),
                sandbox_profile_digest: Some("sha256:python-profile".to_string()),
                sandbox_termination_reason: Some("timeout".to_string()),
                reason_code: Some("SANDBOX_TIMEOUT".to_string()),
                failure_reason: Some("sandbox execution exceeded timeout".to_string()),
                severity: Some("warn".to_string()),
                notional_sats: Some(0),
                liability_premium_sats: Some(0),
                work_unit_id: Some("work-unit-2".to_string()),
            },
        ];
        snapshot.earnings = Some(ProviderEarningsSummary {
            sats_today: 42,
            lifetime_sats: 420,
            jobs_today: 1,
            online_uptime_seconds: 45,
            last_job_result: "settled".to_string(),
            first_job_latency_seconds: Some(8),
            completion_ratio_bps: Some(10_000),
            payout_success_ratio_bps: Some(10_000),
            avg_wallet_confirmation_latency_seconds: Some(3),
        });
        snapshot.config_metadata.push(super::ProviderJsonEntry {
            key: "test_marker".to_string(),
            value: json!("observability"),
        });

        let admin_config = provider_admin_config(&config)?;
        let mut store = ProviderPersistenceStore::open(&admin_config)?;
        store.set_listen_addr(config.admin_listen_addr.as_str())?;
        store.set_desired_mode(ProviderDesiredMode::Online)?;
        store.persist_snapshot(&snapshot)?;
        Ok(config)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn backend_and_product_reports_preserve_launch_family_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let backend_report = load_backend_report(config_path.as_path()).await?;
        let product_report = load_product_report(config_path.as_path()).await?;

        let gpt_oss = backend_report
            .backends
            .iter()
            .find(|backend| backend.backend_id == "gpt_oss")
            .ok_or_else(|| std::io::Error::other("missing gpt_oss backend entry"))?;
        ensure(
            gpt_oss.launch_product_ids
                == vec![
                    "gpt_oss.text_generation".to_string(),
                    "gpt_oss.embeddings".to_string(),
                ],
            "gpt_oss backend should expose inference and embeddings launch products",
        )?;

        let apple_fm = backend_report
            .backends
            .iter()
            .find(|backend| backend.backend_id == "apple_foundation_models")
            .ok_or_else(|| std::io::Error::other("missing apple fm backend entry"))?;
        ensure(
            apple_fm.launch_product_ids
                == vec!["apple_foundation_models.text_generation".to_string()],
            "apple fm backend should only expose inference at launch",
        )?;
        ensure(
            product_report
                .products
                .iter()
                .all(|product| product.product_id != "apple_foundation_models.embeddings"),
            "product report must not overclaim Apple FM embeddings support",
        )?;
        ensure(
            product_report.products.iter().any(|product| {
                product.product_id == "gpt_oss.embeddings"
                    && product.capability_summary.contains("family=embeddings")
            }),
            "product report should preserve capability-envelope qualifiers for embeddings",
        )?;
        let sandbox = backend_report
            .backends
            .iter()
            .find(|backend| backend.backend_id == "sandbox")
            .ok_or_else(|| std::io::Error::other("missing sandbox backend entry"))?;
        ensure(
            sandbox.supported_execution_classes == vec!["sandbox.python.exec".to_string()],
            "sandbox backend should expose declared execution classes",
        )?;
        ensure(
            sandbox.profile_ids == vec!["python-batch".to_string()],
            "sandbox backend should expose declared profile ids",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn inventory_jobs_earnings_and_receipts_reports_round_trip_store_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let inventory_report = load_inventory_report(config_path.as_path(), Some(8)).await?;
        let jobs_report = load_jobs_report(config_path.as_path(), Some(4)).await?;
        let earnings_report = load_earnings_report(config_path.as_path()).await?;
        let receipts_report = load_receipts_report(config_path.as_path(), Some(2)).await?;

        ensure(
            inventory_report
                .rows
                .iter()
                .any(|row| row.target.product_id() == "gpt_oss.embeddings" && row.eligible),
            "inventory report should show eligible embedding supply",
        )?;
        ensure(
            inventory_report
                .rows
                .iter()
                .any(|row| row.target.product_id() == "sandbox.python.exec" && row.eligible),
            "inventory report should show eligible sandbox supply when profiles are declared",
        )?;
        ensure(
            jobs_report.jobs.len() == 2
                && jobs_report
                    .jobs
                    .iter()
                    .any(|job| job.product_id.as_deref() == Some("gpt_oss.embeddings")),
            "jobs report should surface persisted recent jobs",
        )?;
        let sandbox_job = jobs_report
            .jobs
            .iter()
            .find(|job| job.product_id.as_deref() == Some("sandbox.python.exec"))
            .ok_or_else(|| std::io::Error::other("missing sandbox job row"))?;
        ensure(
            sandbox_job.sandbox_execution_class.as_deref() == Some("sandbox.python.exec")
                && sandbox_job.failure_reason.as_deref()
                    == Some("sandbox execution exceeded timeout"),
            "jobs report should surface sandbox failure classification and reason",
        )?;
        ensure(
            earnings_report
                .earnings
                .as_ref()
                .is_some_and(|earnings| earnings.lifetime_sats == 420),
            "earnings report should surface persisted earnings",
        )?;
        ensure(
            receipts_report.receipts.len() == 2
                && receipts_report
                    .receipts
                    .iter()
                    .any(|receipt| receipt.receipt_id == "receipt-1"),
            "receipts report should surface persisted receipts",
        )?;
        let sandbox_receipt = receipts_report
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_id == "receipt-2")
            .ok_or_else(|| std::io::Error::other("missing sandbox receipt row"))?;
        ensure(
            sandbox_receipt.sandbox_profile_id.as_deref() == Some("python-batch")
                && sandbox_receipt.sandbox_termination_reason.as_deref() == Some("timeout"),
            "receipts report should surface sandbox receipt integrity fields",
        )
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sandbox_reports_surface_profiles_status_and_failures()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let config_path = temp_dir.path().join("config.json");
        seed_observability_snapshot(config_path.as_path())?;

        let status = load_status_or_detect(config_path.as_path()).await?;
        let status_render = render_human_status(&status);
        ensure(
            status_render.contains("sandbox_execution_classes: sandbox.python.exec"),
            "status should surface supported sandbox execution classes",
        )?;
        ensure(
            status_render.contains("sandbox_profiles: python-batch"),
            "status should surface declared sandbox profile ids",
        )?;

        let sandbox_report = load_sandbox_report(config_path.as_path(), Some(4)).await?;
        ensure(
            sandbox_report.supported_execution_classes == vec!["sandbox.python.exec".to_string()],
            "sandbox report should expose declared execution classes",
        )?;
        ensure(
            sandbox_report.profiles.first().is_some_and(|profile| {
                profile.profile_id == "python-batch" && profile.runtime_ready
            }),
            "sandbox report should expose runtime-ready declared profiles",
        )?;

        let rendered = render_sandbox_report(&sandbox_report);
        ensure(
            rendered.contains("runtime_kind: python"),
            "sandbox report should render runtime kinds",
        )?;
        ensure(
            rendered.contains("profile_digest: sha256:python-profile"),
            "sandbox report should render profile digests for verification",
        )?;
        ensure(
            rendered.contains("execution_class: sandbox.python.exec"),
            "sandbox report should render execution classes for policy matching",
        )
    }
}
