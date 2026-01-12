use std::fs;
use wgpui::input::Modifiers as UiModifiers;

use crate::app::codex_app_server as app_server;
use crate::app::config::{
    config_dir, config_file, keybindings_file, CoderSettings, StoredKeybinding, StoredModifiers,
};
use crate::app::events::{key_from_string, key_to_string};
use crate::app::session::{RateLimitInfo, RateLimits};
use crate::app::{format_relative_time, now_timestamp, ModelOption};
use crate::keybindings::{default_keybindings, Action as KeyAction, Keybinding};

pub(super) fn clamp_font_size(size: f32) -> f32 {
    size.clamp(12.0, 18.0)
}

pub(super) fn normalize_settings(settings: &mut CoderSettings) {
    settings.font_size = clamp_font_size(settings.font_size);
}

/// Fetch rate limits for the active provider.
pub(super) async fn fetch_rate_limits() -> Option<RateLimits> {
    if !use_app_server_transport() {
        return None;
    }

    let cwd = std::env::current_dir().ok()?;
    let (client, _channels) = app_server::AppServerClient::spawn(app_server::AppServerConfig {
        cwd: Some(cwd),
        wire_log: None,
    })
    .await
    .ok()?;
    let client_info = app_server::ClientInfo {
        name: "autopilot".to_string(),
        title: Some("Autopilot".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    if client.initialize(client_info).await.is_err() {
        let _ = client.shutdown().await;
        return None;
    }

    let response = client.account_rate_limits_read().await.ok();
    let _ = client.shutdown().await;
    response.map(|response| rate_limits_from_snapshot(response.rate_limits))
}

pub(super) fn rate_limits_from_snapshot(snapshot: app_server::RateLimitSnapshot) -> RateLimits {
    RateLimits {
        primary: snapshot
            .primary
            .map(|window| rate_limit_info_from_window("Primary", window)),
        secondary: snapshot
            .secondary
            .map(|window| rate_limit_info_from_window("Secondary", window)),
    }
}

fn rate_limit_info_from_window(
    name: &str,
    window: app_server::RateLimitWindow,
) -> RateLimitInfo {
    let percent = window.used_percent.clamp(0, 100) as f64;
    RateLimitInfo {
        name: name.to_string(),
        percent_used: percent,
        resets_at: format_rate_limit_reset(window.resets_at),
    }
}

fn format_rate_limit_reset(resets_at: Option<i64>) -> String {
    let Some(timestamp) = resets_at else {
        return String::new();
    };
    if timestamp <= 0 {
        return String::new();
    }
    let now = now_timestamp() as i64;
    if timestamp <= now {
        return format_relative_time(timestamp as u64);
    }
    let delta = timestamp.saturating_sub(now);
    format!("in {}", format_future_delta(delta))
}

fn format_future_delta(delta_secs: i64) -> String {
    if delta_secs < 60 {
        format!("{}s", delta_secs.max(0))
    } else if delta_secs < 3600 {
        format!("{}m", delta_secs / 60)
    } else if delta_secs < 86_400 {
        format!("{}h", delta_secs / 3600)
    } else {
        format!("{}d", delta_secs / 86_400)
    }
}

fn use_app_server_transport() -> bool {
    match std::env::var("AUTOPILOT_CODEX_TRANSPORT") {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "app-server" | "appserver" | "app_server"
        ),
        Err(_) => false,
    }
}
fn parse_legacy_model_setting(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(model_id) = line.strip_prefix("model = \"").and_then(|s| s.strip_suffix("\"")) {
            return Some(model_id.to_string());
        }
    }
    None
}

pub(super) fn load_settings() -> CoderSettings {
    let path = config_file();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut settings) = toml::from_str::<CoderSettings>(&content) {
            normalize_settings(&mut settings);
            return settings;
        }
        let mut settings = CoderSettings::default();
        settings.model = parse_legacy_model_setting(&content);
        normalize_settings(&mut settings);
        return settings;
    }
    CoderSettings::default()
}

pub(super) fn save_settings(settings: &CoderSettings) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(content) = toml::to_string_pretty(settings) {
            let _ = fs::write(config_file(), content);
        }
    }
}


pub(super) fn settings_model_option(settings: &CoderSettings) -> ModelOption {
    settings
        .model
        .as_deref()
        .map(ModelOption::from_id)
        .unwrap_or(ModelOption::Default)
}

pub(super) fn load_keybindings() -> Vec<Keybinding> {
    let path = keybindings_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return default_keybindings();
    };
    let Ok(entries) = serde_json::from_str::<Vec<StoredKeybinding>>(&content) else {
        return default_keybindings();
    };
    let mut bindings = Vec::new();
    for entry in entries {
        let Some(action) = KeyAction::from_id(&entry.action) else {
            continue;
        };
        let Some(key) = key_from_string(&entry.key) else {
            continue;
        };
        let modifiers = UiModifiers {
            shift: entry.modifiers.shift,
            ctrl: entry.modifiers.ctrl,
            alt: entry.modifiers.alt,
            meta: entry.modifiers.meta,
        };
        bindings.push(Keybinding {
            key,
            modifiers,
            action,
        });
    }
    if bindings.is_empty() {
        default_keybindings()
    } else {
        bindings
    }
}

pub(super) fn save_keybindings(bindings: &[Keybinding]) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        let entries: Vec<StoredKeybinding> = bindings
            .iter()
            .map(|binding| StoredKeybinding {
                action: binding.action.id().to_string(),
                key: key_to_string(&binding.key),
                modifiers: StoredModifiers {
                    shift: binding.modifiers.shift,
                    ctrl: binding.modifiers.ctrl,
                    alt: binding.modifiers.alt,
                    meta: binding.modifiers.meta,
                },
            })
            .collect();
        if let Ok(content) = serde_json::to_string_pretty(&entries) {
            let _ = fs::write(keybindings_file(), content);
        }
    }
}
