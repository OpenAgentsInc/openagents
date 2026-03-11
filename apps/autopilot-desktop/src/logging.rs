use std::collections::VecDeque;
use std::fmt;
use std::sync::{Mutex, Once, OnceLock};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

static LOG_INIT: Once = Once::new();
static MIRRORED_LOGS: OnceLock<Mutex<MirroredLogBuffer>> = OnceLock::new();

const MIRRORED_LOG_CAPACITY: usize = 1_024;

#[derive(Clone, Debug)]
pub(crate) struct MirroredLogEntry {
    pub id: u64,
    pub at_epoch_seconds: u64,
    pub level: Level,
    pub line: String,
}

#[derive(Default)]
struct MirroredLogVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl MirroredLogVisitor {
    fn record_value(&mut self, field: &Field, value: String) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push(format!("{}={value}", field.name()));
        }
    }
}

impl Visit for MirroredLogVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_value(field, format!("{value:?}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_value(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_value(field, value.to_string());
    }
}

#[derive(Default)]
struct MirroredLogBuffer {
    next_id: u64,
    entries: VecDeque<MirroredLogEntry>,
}

impl MirroredLogBuffer {
    fn push(&mut self, level: Level, line: String) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        self.next_id = self.next_id.saturating_add(1);
        let at_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        self.entries.push_back(MirroredLogEntry {
            id: self.next_id,
            at_epoch_seconds,
            level,
            line: line.to_string(),
        });
        while self.entries.len() > MIRRORED_LOG_CAPACITY {
            self.entries.pop_front();
        }
    }
}

struct MirroredLogLayer;

impl<S> Layer<S> for MirroredLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MirroredLogVisitor::default();
        event.record(&mut visitor);
        let line = format_mirrored_log_line(
            metadata.level(),
            metadata.target(),
            visitor.message.as_deref(),
            visitor.fields.as_slice(),
        );
        if !should_mirror_to_mission_control(*metadata.level(), metadata.target(), line.as_str()) {
            return;
        }
        push_mirrored_log(*metadata.level(), line);
    }
}

pub fn init() {
    LOG_INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,wgpu=warn,winit=warn"));

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_level(true)
            .with_target(true)
            .without_time();

        let subscriber = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(MirroredLogLayer);

        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

pub(crate) fn latest_mirrored_log_id() -> u64 {
    mirrored_log_buffer()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .entries
        .back()
        .map_or(0, |entry| entry.id)
}

pub(crate) fn mirrored_logs_after(last_id: u64) -> Vec<MirroredLogEntry> {
    mirrored_log_buffer()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .entries
        .iter()
        .filter(|entry| entry.id > last_id)
        .cloned()
        .collect()
}

fn mirrored_log_buffer() -> &'static Mutex<MirroredLogBuffer> {
    MIRRORED_LOGS.get_or_init(|| Mutex::new(MirroredLogBuffer::default()))
}

fn push_mirrored_log(level: Level, line: String) {
    mirrored_log_buffer()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .push(level, line);
}

fn format_mirrored_log_line(
    level: &Level,
    target: &str,
    message: Option<&str>,
    fields: &[String],
) -> String {
    let mut content = String::new();
    if let Some(message) = message.map(str::trim).filter(|message| !message.is_empty()) {
        content.push_str(message);
    }
    for field in fields {
        if !content.is_empty() {
            content.push(' ');
        }
        content.push_str(field);
    }
    if content.is_empty() {
        content.push_str("event");
    }
    format!("{level} {target}: {content}")
}

fn should_mirror_to_mission_control(level: Level, target: &str, line: &str) -> bool {
    let target = target.trim();
    if target.is_empty() {
        return false;
    }

    if target.starts_with("autopilot_desktop::buyer")
        || target.starts_with("autopilot_desktop::buy_mode")
    {
        return true;
    }

    let normalized = line.to_ascii_lowercase();
    if target.starts_with("autopilot_desktop::input") {
        return matches!(level, Level::WARN | Level::ERROR)
            && (normalized.contains("ui error [network.requests]")
                || normalized.contains("ui error [spark.wallet]"));
    }

    if target.starts_with("breez_sdk_spark::sdk") {
        return normalized.contains("polling lightning send payment")
            || normalized.contains("polling payment status =")
            || normalized.contains("polling payment completed status =")
            || normalized.contains("timeout waiting for payment");
    }

    if target.starts_with("autopilot_desktop::spark_wallet") {
        return matches!(level, Level::WARN | Level::ERROR);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{format_mirrored_log_line, should_mirror_to_mission_control};

    #[test]
    fn mirrored_log_line_includes_message_and_fields() {
        let line = format_mirrored_log_line(
            &tracing::Level::INFO,
            "autopilot_desktop::buy_mode",
            Some("Queued buy mode request"),
            &[
                "request_id=req-123".to_string(),
                "budget_sats=2".to_string(),
            ],
        );

        assert_eq!(
            line,
            "INFO autopilot_desktop::buy_mode: Queued buy mode request request_id=req-123 budget_sats=2"
        );
    }

    #[test]
    fn mission_control_mirror_filters_to_buyer_targets() {
        assert!(should_mirror_to_mission_control(
            tracing::Level::INFO,
            "autopilot_desktop::buyer",
            "INFO autopilot_desktop::buyer: queued request"
        ));
        assert!(should_mirror_to_mission_control(
            tracing::Level::ERROR,
            "autopilot_desktop::buy_mode",
            "ERROR autopilot_desktop::buy_mode: failed"
        ));
        assert!(should_mirror_to_mission_control(
            tracing::Level::INFO,
            "breez_sdk_spark::sdk",
            "INFO breez_sdk_spark::sdk: Polling payment status = failed UserSwapReturned"
        ));
        assert!(should_mirror_to_mission_control(
            tracing::Level::ERROR,
            "autopilot_desktop::input",
            "ERROR autopilot_desktop::input: ui error [network.requests]: Request req-1 payment failed"
        ));
        assert!(!should_mirror_to_mission_control(
            tracing::Level::WARN,
            "spark::events::server_stream",
            "WARN spark::events::server_stream: disconnected"
        ));
        assert!(!should_mirror_to_mission_control(
            tracing::Level::INFO,
            "breez_sdk_spark::sdk",
            "INFO breez_sdk_spark::sdk: Balance updated successfully 507"
        ));
    }
}
