//! Optional OS-backed scheduler adapter descriptors for persisted goal schedules.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum OsSchedulerAdapterKind {
    Launchd,
    Cron,
    Systemd,
}

impl OsSchedulerAdapterKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Cron => "cron",
            Self::Systemd => "systemd",
        }
    }

    pub fn from_label(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "launchd" | "launchctl" => Some(Self::Launchd),
            "cron" | "crontab" => Some(Self::Cron),
            "systemd" | "systemctl" => Some(Self::Systemd),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct OsSchedulerAdapterConfig {
    pub enabled: bool,
    pub adapter: Option<OsSchedulerAdapterKind>,
    pub adapter_job_id: Option<String>,
    pub descriptor_path: Option<String>,
    pub last_reconciled_epoch_seconds: Option<u64>,
    pub last_reconcile_result: Option<String>,
    pub reconciliation_marker: Option<String>,
}

impl Default for OsSchedulerAdapterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            adapter: None,
            adapter_job_id: None,
            descriptor_path: None,
            last_reconciled_epoch_seconds: None,
            last_reconcile_result: None,
            reconciliation_marker: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OsSchedulerCapability {
    pub adapter: OsSchedulerAdapterKind,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OsSchedulerScheduleSpec {
    Manual,
    IntervalSeconds {
        seconds: u64,
    },
    Cron {
        expression: String,
        timezone: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OsSchedulerReconcileResult {
    pub adapter_job_id: String,
    pub descriptor_path: PathBuf,
    pub reconciliation_marker: String,
    pub install_command_preview: String,
}

pub fn preferred_adapter_for_host() -> Option<OsSchedulerAdapterKind> {
    #[cfg(target_os = "macos")]
    {
        if detect_adapter_capability(OsSchedulerAdapterKind::Launchd).available {
            return Some(OsSchedulerAdapterKind::Launchd);
        }
        if detect_adapter_capability(OsSchedulerAdapterKind::Cron).available {
            return Some(OsSchedulerAdapterKind::Cron);
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        if detect_adapter_capability(OsSchedulerAdapterKind::Systemd).available {
            return Some(OsSchedulerAdapterKind::Systemd);
        }
        if detect_adapter_capability(OsSchedulerAdapterKind::Cron).available {
            return Some(OsSchedulerAdapterKind::Cron);
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        if detect_adapter_capability(OsSchedulerAdapterKind::Cron).available {
            return Some(OsSchedulerAdapterKind::Cron);
        }
        None
    }
}

pub fn detect_adapter_capability(adapter: OsSchedulerAdapterKind) -> OsSchedulerCapability {
    match adapter {
        OsSchedulerAdapterKind::Launchd => {
            if !cfg!(target_os = "macos") {
                return OsSchedulerCapability {
                    adapter,
                    available: false,
                    reason: Some("launchd adapter is only available on macOS".to_string()),
                };
            }
            if !command_exists("launchctl") {
                return OsSchedulerCapability {
                    adapter,
                    available: false,
                    reason: Some("launchctl command not found in PATH".to_string()),
                };
            }
            OsSchedulerCapability {
                adapter,
                available: true,
                reason: None,
            }
        }
        OsSchedulerAdapterKind::Cron => {
            if !command_exists("crontab") {
                return OsSchedulerCapability {
                    adapter,
                    available: false,
                    reason: Some("crontab command not found in PATH".to_string()),
                };
            }
            OsSchedulerCapability {
                adapter,
                available: true,
                reason: None,
            }
        }
        OsSchedulerAdapterKind::Systemd => {
            if !cfg!(target_os = "linux") {
                return OsSchedulerCapability {
                    adapter,
                    available: false,
                    reason: Some("systemd adapter is only available on Linux".to_string()),
                };
            }
            if !command_exists("systemctl") {
                return OsSchedulerCapability {
                    adapter,
                    available: false,
                    reason: Some("systemctl command not found in PATH".to_string()),
                };
            }
            OsSchedulerCapability {
                adapter,
                available: true,
                reason: None,
            }
        }
    }
}

pub fn reconcile_os_scheduler_descriptor(
    goal_id: &str,
    schedule: &OsSchedulerScheduleSpec,
    adapter: OsSchedulerAdapterKind,
) -> Result<OsSchedulerReconcileResult, String> {
    let capability = detect_adapter_capability(adapter);
    if !capability.available {
        return Err(capability.reason.unwrap_or_else(|| {
            format!("{} adapter unavailable on current host", adapter.as_str())
        }));
    }

    let normalized_goal = sanitize_goal_id(goal_id);
    if normalized_goal.is_empty() {
        return Err("goal id cannot be empty for OS scheduler reconciliation".to_string());
    }
    let adapter_job_id = format!("openagents.autopilot.goal.{}", normalized_goal);

    let descriptor_root = descriptor_root_for_adapter(adapter);
    std::fs::create_dir_all(&descriptor_root)
        .map_err(|error| format!("failed to create scheduler descriptor dir: {error}"))?;

    let (descriptor_path, descriptor_payload, install_command_preview) = match adapter {
        OsSchedulerAdapterKind::Launchd => {
            build_launchd_descriptor(&descriptor_root, &adapter_job_id, goal_id, schedule)?
        }
        OsSchedulerAdapterKind::Cron => {
            build_cron_descriptor(&descriptor_root, &adapter_job_id, goal_id, schedule)?
        }
        OsSchedulerAdapterKind::Systemd => {
            build_systemd_descriptor(&descriptor_root, &adapter_job_id, goal_id, schedule)?
        }
    };

    std::fs::write(&descriptor_path, &descriptor_payload)
        .map_err(|error| format!("failed writing scheduler descriptor: {error}"))?;

    let mut hasher = DefaultHasher::new();
    adapter_job_id.hash(&mut hasher);
    descriptor_path.to_string_lossy().hash(&mut hasher);
    descriptor_payload.hash(&mut hasher);
    install_command_preview.hash(&mut hasher);
    let marker = format!("{:x}", hasher.finish());

    Ok(OsSchedulerReconcileResult {
        adapter_job_id,
        descriptor_path,
        reconciliation_marker: marker,
        install_command_preview,
    })
}

fn build_launchd_descriptor(
    root: &Path,
    adapter_job_id: &str,
    goal_id: &str,
    schedule: &OsSchedulerScheduleSpec,
) -> Result<(PathBuf, String, String), String> {
    let interval_seconds = schedule_interval_seconds(schedule)?;
    let path = root.join(format!("{}.plist", adapter_job_id));
    let payload = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{adapter_job_id}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>/usr/bin/env</string>\n    <string>sh</string>\n    <string>-lc</string>\n    <string>openagents-autopilot run-goal {goal_id}</string>\n  </array>\n  <key>StartInterval</key>\n  <integer>{interval_seconds}</integer>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>StandardOutPath</key>\n  <string>/tmp/{adapter_job_id}.out.log</string>\n  <key>StandardErrorPath</key>\n  <string>/tmp/{adapter_job_id}.err.log</string>\n</dict>\n</plist>\n"
    );
    let install_preview = format!(
        "launchctl bootstrap gui/$(id -u) {} && launchctl enable gui/$(id -u)/{}",
        path.display(),
        adapter_job_id
    );
    Ok((path, payload, install_preview))
}

fn build_cron_descriptor(
    root: &Path,
    adapter_job_id: &str,
    goal_id: &str,
    schedule: &OsSchedulerScheduleSpec,
) -> Result<(PathBuf, String, String), String> {
    let (expression, timezone) = match schedule {
        OsSchedulerScheduleSpec::Cron {
            expression,
            timezone,
        } => {
            if expression.trim().is_empty() {
                return Err("cron expression cannot be empty".to_string());
            }
            (expression.trim().to_string(), timezone.clone())
        }
        OsSchedulerScheduleSpec::IntervalSeconds { seconds } => {
            let minutes = ((*seconds).max(60) / 60).max(1).min(59);
            (format!("*/{} * * * *", minutes), Some("UTC".to_string()))
        }
        OsSchedulerScheduleSpec::Manual => {
            return Err("manual schedules cannot be exported to cron adapter".to_string());
        }
    };

    let path = root.join(format!("{}.cron", adapter_job_id));
    let timezone_line = timezone
        .as_deref()
        .map(|value| format!("CRON_TZ={}\n", value))
        .unwrap_or_default();
    let payload = format!(
        "# OpenAgents generated cron descriptor for {}\n{}{} /usr/bin/env sh -lc 'openagents-autopilot run-goal {}'\n",
        adapter_job_id, timezone_line, expression, goal_id
    );
    let install_preview = format!("crontab {}", path.display());
    Ok((path, payload, install_preview))
}

fn build_systemd_descriptor(
    root: &Path,
    adapter_job_id: &str,
    goal_id: &str,
    schedule: &OsSchedulerScheduleSpec,
) -> Result<(PathBuf, String, String), String> {
    let interval_seconds = schedule_interval_seconds(schedule)?;
    let service_name = format!("{}.service", adapter_job_id);
    let timer_name = format!("{}.timer", adapter_job_id);

    let service_path = root.join(&service_name);
    let timer_path = root.join(&timer_name);

    let service_payload = format!(
        "[Unit]\nDescription=OpenAgents Autopilot goal {goal_id}\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/env sh -lc 'openagents-autopilot run-goal {goal_id}'\n"
    );
    std::fs::write(&service_path, &service_payload)
        .map_err(|error| format!("failed writing systemd service descriptor: {error}"))?;

    let timer_payload = format!(
        "[Unit]\nDescription=OpenAgents timer for goal {goal_id}\n\n[Timer]\nOnUnitActiveSec={}s\nPersistent=true\nUnit={}\n\n[Install]\nWantedBy=timers.target\n",
        interval_seconds, service_name
    );
    let install_preview = format!(
        "systemctl --user daemon-reload && systemctl --user enable --now {}",
        timer_path.display()
    );
    Ok((timer_path, timer_payload, install_preview))
}

fn schedule_interval_seconds(schedule: &OsSchedulerScheduleSpec) -> Result<u64, String> {
    match schedule {
        OsSchedulerScheduleSpec::IntervalSeconds { seconds } => Ok((*seconds).max(1)),
        OsSchedulerScheduleSpec::Cron { .. } => Ok(60),
        OsSchedulerScheduleSpec::Manual => {
            Err("manual schedules cannot be exported to this adapter".to_string())
        }
    }
}

fn descriptor_root_for_adapter(adapter: OsSchedulerAdapterKind) -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-scheduler")
        .join(adapter.as_str())
}

fn sanitize_goal_id(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn command_exists(command: &str) -> bool {
    if command.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(command).is_file();
    }

    let Some(path_value) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path_value).any(|dir| {
        let path = dir.join(command);
        if path.is_file() {
            return true;
        }

        #[cfg(target_os = "windows")]
        {
            let exe = dir.join(format!("{command}.exe"));
            if exe.is_file() {
                return true;
            }
        }

        false
    })
}

#[cfg(test)]
mod tests {
    use super::{
        OsSchedulerAdapterKind, OsSchedulerScheduleSpec, detect_adapter_capability,
        preferred_adapter_for_host, reconcile_os_scheduler_descriptor,
    };

    #[test]
    fn adapter_labels_roundtrip() {
        assert_eq!(
            OsSchedulerAdapterKind::from_label("launchd"),
            Some(OsSchedulerAdapterKind::Launchd)
        );
        assert_eq!(
            OsSchedulerAdapterKind::from_label("cron"),
            Some(OsSchedulerAdapterKind::Cron)
        );
        assert_eq!(
            OsSchedulerAdapterKind::from_label("systemd"),
            Some(OsSchedulerAdapterKind::Systemd)
        );
        assert!(OsSchedulerAdapterKind::from_label("unknown").is_none());
    }

    #[test]
    fn unsupported_host_adapter_reports_unavailable() {
        #[cfg(not(target_os = "macos"))]
        {
            let capability = detect_adapter_capability(OsSchedulerAdapterKind::Launchd);
            assert!(!capability.available);
        }

        #[cfg(not(target_os = "linux"))]
        {
            let capability = detect_adapter_capability(OsSchedulerAdapterKind::Systemd);
            assert!(!capability.available);
        }
    }

    #[test]
    fn reconcile_rejects_manual_schedule() {
        let adapter = preferred_adapter_for_host().unwrap_or(OsSchedulerAdapterKind::Cron);
        let error = reconcile_os_scheduler_descriptor(
            "goal-manual",
            &OsSchedulerScheduleSpec::Manual,
            adapter,
        )
        .expect_err("manual schedule should not reconcile");
        assert!(
            error.contains("manual schedules")
                || error.contains("not found")
                || error.contains("only available"),
            "unexpected error: {error}"
        );
    }
}
