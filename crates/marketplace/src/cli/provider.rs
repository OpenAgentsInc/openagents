//! Provider CLI commands

use crate::compute::provider::{Provider, ProviderConfig, ProviderState};
use clap::Subcommand;
use std::path::PathBuf;

const CONFIG_DIR_ENV: &str = "OPENAGENTS_CONFIG_DIR";

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    /// Go online and start accepting compute jobs
    Online {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Go offline and stop accepting jobs
    Offline {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Configure provider capabilities and pricing
    Config {
        /// Provider name
        #[arg(long)]
        name: Option<String>,

        /// Provider description
        #[arg(long)]
        description: Option<String>,

        /// Website URL
        #[arg(long)]
        website: Option<String>,

        /// Icon URL
        #[arg(long)]
        icon: Option<String>,

        /// Provider region (e.g., us-west, eu-central)
        #[arg(long)]
        region: Option<String>,

        /// Availability schedule (e.g., "always", "weekdays 9-17")
        #[arg(long)]
        schedule: Option<String>,

        /// Add a capability/model (can be used multiple times)
        #[arg(long = "capability")]
        capabilities: Vec<String>,

        /// Price per 1k input tokens in millisats
        #[arg(long)]
        price_input: Option<u64>,

        /// Price per 1k output tokens in millisats
        #[arg(long)]
        price_output: Option<u64>,

        /// Add a relay URL (can be used multiple times)
        #[arg(long = "relay")]
        relays: Vec<String>,

        /// Re-advertisement interval in seconds
        #[arg(long)]
        readvertise_interval: Option<u64>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show current provider status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show earnings summary (placeholder)
    Earnings {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Time period (day, week, month, all)
        #[arg(long, default_value = "all")]
        period: String,
    },
}

impl ProviderCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        match self {
            ProviderCommands::Online { json } => self.online(*json),
            ProviderCommands::Offline { json } => self.offline(*json),
            ProviderCommands::Config {
                name,
                description,
                website,
                icon,
                region,
                schedule,
                capabilities,
                price_input,
                price_output,
                relays,
                readvertise_interval,
                json,
            } => self.config(
                name.as_deref(),
                description.as_deref(),
                website.as_deref(),
                icon.as_deref(),
                region.as_deref(),
                schedule.as_deref(),
                capabilities,
                *price_input,
                *price_output,
                relays,
                *readvertise_interval,
                *json,
            ),
            ProviderCommands::Status { json } => self.status(*json),
            ProviderCommands::Earnings { json, period } => self.earnings(*json, period),
        }
    }

    fn online(&self, json: bool) -> anyhow::Result<()> {
        // Load existing config or create default
        let config = load_provider_config()?;
        let mut provider = Provider::new(config);
        provider.go_online();

        // Save state
        save_provider_state(&provider)?;

        if json {
            println!("{{\"status\": \"online\"}}");
        } else {
            println!("Provider is now ONLINE");
            println!("Accepting compute jobs on configured relays");
            println!("\nNote: Relay connection and job monitoring not yet implemented.");
        }

        Ok(())
    }

    fn offline(&self, json: bool) -> anyhow::Result<()> {
        // Load existing config
        let config = load_provider_config()?;
        let mut provider = Provider::new(config);
        provider.go_offline();

        // Save state
        save_provider_state(&provider)?;

        if json {
            println!("{{\"status\": \"offline\"}}");
        } else {
            println!("Provider is now OFFLINE");
            println!("No longer accepting new compute jobs");
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn config(
        &self,
        name: Option<&str>,
        description: Option<&str>,
        website: Option<&str>,
        icon: Option<&str>,
        region: Option<&str>,
        schedule: Option<&str>,
        capabilities: &[String],
        price_input: Option<u64>,
        price_output: Option<u64>,
        relays: &[String],
        readvertise_interval: Option<u64>,
        json: bool,
    ) -> anyhow::Result<()> {
        // Load existing config or create default
        let mut config = load_provider_config().unwrap_or_default();

        // Update fields if provided
        if let Some(name) = name {
            config.name = name.to_string();
        }
        if let Some(description) = description {
            config.description = description.to_string();
        }
        if let Some(website) = website {
            config.website = Some(website.to_string());
        }
        if let Some(icon) = icon {
            config.icon_url = Some(icon.to_string());
        }
        if let Some(region) = region {
            config.region = Some(region.to_string());
        }
        if let Some(schedule) = schedule {
            config.schedule = schedule.to_string();
        }
        if !capabilities.is_empty() {
            config.capabilities = capabilities.to_vec();
        }
        if let Some(price_input) = price_input {
            config.price_per_1k_input = price_input;
        }
        if let Some(price_output) = price_output {
            config.price_per_1k_output = price_output;
        }
        if !relays.is_empty() {
            config.relays = relays.to_vec();
        }
        if let Some(interval) = readvertise_interval {
            config.readvertise_interval_secs = interval;
        }

        // Save config
        save_provider_config(&config)?;

        if json {
            let json_str = serde_json::to_string_pretty(&config)?;
            println!("{}", json_str);
        } else {
            println!("Provider Configuration Updated");
            println!("==============================\n");
            println!("Name: {}", config.name);
            println!("Description: {}", config.description);
            if let Some(ref website) = config.website {
                println!("Website: {}", website);
            }
            if let Some(ref region) = config.region {
                println!("Region: {}", region);
            }
            println!("Schedule: {}", config.schedule);
            println!("\nCapabilities:");
            for cap in &config.capabilities {
                println!("  - {}", cap);
            }
            println!("\nPricing:");
            println!("  Input:  {} msats/1k tokens", config.price_per_1k_input);
            println!("  Output: {} msats/1k tokens", config.price_per_1k_output);
            println!("\nRelays:");
            for relay in &config.relays {
                println!("  - {}", relay);
            }
            println!(
                "\nRe-advertisement interval: {}s",
                config.readvertise_interval_secs
            );
        }

        Ok(())
    }

    fn status(&self, json: bool) -> anyhow::Result<()> {
        // Load config and state
        let config = load_provider_config()?;
        let provider = load_provider_state().unwrap_or_else(|_| Provider::new(config.clone()));

        if json {
            let status_json = serde_json::json!({
                "state": format!("{:?}", provider.state()),
                "name": config.name,
                "schedule": config.schedule,
                "capabilities": config.capabilities,
                "relays": config.relays,
                "needs_readvertisement": provider.needs_readvertisement(),
            });
            println!("{}", serde_json::to_string_pretty(&status_json)?);
        } else {
            println!("Provider Status");
            println!("===============\n");
            println!("State: {:?}", provider.state());
            println!("Name: {}", config.name);
            println!("Schedule: {}", config.schedule);
            println!("\nCapabilities: {}", config.capabilities.join(", "));
            println!("Relays: {}", config.relays.len());

            if provider.needs_readvertisement() {
                println!("\nAdvertisement: Needs re-advertisement");
            } else if let Some(time_until) = provider.time_until_next_advertisement() {
                let secs = time_until.as_secs();
                println!("\nNext advertisement in: {}m {}s", secs / 60, secs % 60);
            }

            println!("\nNote: Provider state is local only.");
            println!("      Relay integration not yet implemented.");
        }

        Ok(())
    }

    fn earnings(&self, json: bool, period: &str) -> anyhow::Result<()> {
        if json {
            println!(
                "{{\"earnings\": 0, \"jobs_completed\": 0, \"period\": \"{}\"}}",
                period
            );
        } else {
            println!("Provider Earnings");
            println!("=================\n");
            println!("Period: {}", period);
            println!("Earnings: 0 sats");
            println!("Jobs completed: 0");
            println!("\nNote: Earnings tracking not yet implemented.");
        }
        Ok(())
    }
}

/// Get the config directory path
fn get_config_dir() -> anyhow::Result<PathBuf> {
    let config_dir = if let Ok(custom) = std::env::var(CONFIG_DIR_ENV) {
        if custom.trim().is_empty() {
            anyhow::bail!("OPENAGENTS_CONFIG_DIR is set but empty");
        }
        PathBuf::from(custom)
    } else {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;
        PathBuf::from(home).join(".openagents")
    };

    // Create directory if it doesn't exist
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

/// Load provider configuration from disk
fn load_provider_config() -> anyhow::Result<ProviderConfig> {
    let config_path = get_config_dir()?.join("marketplace.toml");

    if !config_path.exists() {
        return Ok(ProviderConfig::default());
    }

    let contents = std::fs::read_to_string(&config_path)?;
    let config: ProviderConfig = toml::from_str(&contents)?;
    Ok(config)
}

/// Save provider configuration to disk
fn save_provider_config(config: &ProviderConfig) -> anyhow::Result<()> {
    let config_path = get_config_dir()?.join("marketplace.toml");
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write(&config_path, toml_str)?;
    Ok(())
}

/// Load provider state from disk
fn load_provider_state() -> anyhow::Result<Provider> {
    let state_path = get_config_dir()?.join("provider_state.json");

    if !state_path.exists() {
        anyhow::bail!("No provider state found");
    }

    let contents = std::fs::read_to_string(&state_path)?;

    // Parse just the state from JSON
    #[derive(serde::Deserialize)]
    struct ProviderStateData {
        state: ProviderState,
    }

    let state_data: ProviderStateData = serde_json::from_str(&contents)?;
    let config = load_provider_config()?;
    let mut provider = Provider::new(config);

    // Apply the state
    match state_data.state {
        ProviderState::Online => provider.go_online(),
        ProviderState::Offline => provider.go_offline(),
        ProviderState::Paused => provider.pause(),
    }

    Ok(provider)
}

/// Save provider state to disk
fn save_provider_state(provider: &Provider) -> anyhow::Result<()> {
    let state_path = get_config_dir()?.join("provider_state.json");

    let state_json = serde_json::json!({
        "state": provider.state(),
    });

    std::fs::write(&state_path, serde_json::to_string_pretty(&state_json)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock test env")
    }

    fn with_temp_config_dir<F>(f: F)
    where
        F: FnOnce() -> anyhow::Result<()>,
    {
        let _guard = test_lock();
        let temp_dir = tempfile::tempdir().expect("create temp config dir");
        let original = std::env::var(CONFIG_DIR_ENV).ok();
        unsafe {
            std::env::set_var(CONFIG_DIR_ENV, temp_dir.path());
        }
        let result = f();
        if let Some(value) = original {
            unsafe {
                std::env::set_var(CONFIG_DIR_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(CONFIG_DIR_ENV);
            }
        }
        result.expect("provider command should succeed");
    }

    #[test]
    fn test_provider_commands_variants_exist() {
        let _online = ProviderCommands::Online { json: false };
        let _offline = ProviderCommands::Offline { json: false };
        let _config = ProviderCommands::Config {
            name: None,
            description: None,
            website: None,
            icon: None,
            region: None,
            schedule: None,
            capabilities: vec![],
            price_input: None,
            price_output: None,
            relays: vec![],
            readvertise_interval: None,
            json: false,
        };
        let _status = ProviderCommands::Status { json: false };
        let _earnings = ProviderCommands::Earnings {
            json: false,
            period: "all".to_string(),
        };
    }

    #[test]
    fn test_config_command_with_all_params() {
        with_temp_config_dir(|| {
            let cmd = ProviderCommands::Config {
                name: Some("Test Provider".to_string()),
                description: Some("A test provider".to_string()),
                website: Some("https://test.com".to_string()),
                icon: Some("https://test.com/icon.png".to_string()),
                region: Some("us-west".to_string()),
                schedule: Some("always".to_string()),
                capabilities: vec!["llama3".to_string(), "mistral".to_string()],
                price_input: Some(10),
                price_output: Some(20),
                relays: vec!["wss://relay1.com".to_string()],
                readvertise_interval: Some(7200),
                json: false,
            };

            cmd.execute()
        });
    }

    #[test]
    fn test_status_command() {
        with_temp_config_dir(|| {
            let cmd = ProviderCommands::Status { json: false };
            cmd.execute()
        });
    }

    #[test]
    fn test_earnings_command_periods() {
        with_temp_config_dir(|| {
            for period in &["day", "week", "month", "all"] {
                let cmd = ProviderCommands::Earnings {
                    json: false,
                    period: period.to_string(),
                };
                cmd.execute()?;
            }
            Ok(())
        });
    }

    #[test]
    fn test_online_offline_cycle() {
        with_temp_config_dir(|| {
            let online_cmd = ProviderCommands::Online { json: true };
            online_cmd.execute()?;

            let offline_cmd = ProviderCommands::Offline { json: true };
            offline_cmd.execute()
        });
    }
}
