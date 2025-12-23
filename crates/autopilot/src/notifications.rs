//! Notification delivery for autopilot alerts
//!
//! Supports email and webhook notifications for critical autopilot events:
//! - Benchmark regressions
//! - Metric anomalies
//! - Daemon crashes
//! - Test failures

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Email addresses to notify
    #[serde(default)]
    pub email: Vec<String>,

    /// Webhook URLs to POST to (Slack, Discord, etc.)
    #[serde(default)]
    pub webhook: Vec<String>,

    /// SMTP configuration for email
    #[serde(default)]
    pub smtp: Option<SmtpConfig>,

    /// Minimum severity to trigger notifications
    #[serde(default = "default_min_severity")]
    pub min_severity: String,

    /// Rate limit: max notifications per hour
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_hour: usize,
}

fn default_min_severity() -> String {
    "error".to_string()
}

fn default_rate_limit() -> usize {
    10
}

/// SMTP configuration for email delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

/// Notification payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub severity: String,
    pub timestamp: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Notification {
    pub fn new(title: impl Into<String>, message: impl Into<String>, severity: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            severity: severity.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Send notification via webhook
    pub async fn send_webhook(&self, url: &str) -> Result<()> {
        let client = reqwest::Client::new();

        // Format payload for common webhook services
        let payload = if url.contains("slack.com") {
            self.format_slack()
        } else if url.contains("discord.com") {
            self.format_discord()
        } else {
            // Generic JSON payload
            serde_json::to_value(self)?
        };

        let response = client
            .post(url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send webhook")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Webhook POST failed with status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    /// Format for Slack incoming webhook
    fn format_slack(&self) -> serde_json::Value {
        let emoji = match self.severity.as_str() {
            "critical" => ":rotating_light:",
            "error" => ":x:",
            "warning" => ":warning:",
            _ => ":information_source:",
        };

        serde_json::json!({
            "text": format!("{} *{}*", emoji, self.title),
            "blocks": [
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": format!("{} *{}*", emoji, self.title)
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": &self.message
                    }
                },
                {
                    "type": "context",
                    "elements": [
                        {
                            "type": "mrkdwn",
                            "text": format!("Severity: `{}` | {}", self.severity, self.timestamp)
                        }
                    ]
                }
            ]
        })
    }

    /// Format for Discord webhook
    fn format_discord(&self) -> serde_json::Value {
        let color = match self.severity.as_str() {
            "critical" => 0xFF0000, // Red
            "error" => 0xFF6600,    // Orange
            "warning" => 0xFFCC00,  // Yellow
            _ => 0x0099FF,          // Blue
        };

        serde_json::json!({
            "embeds": [{
                "title": self.title,
                "description": self.message,
                "color": color,
                "footer": {
                    "text": format!("Severity: {} | {}", self.severity, self.timestamp)
                },
                "fields": self.metadata.iter().map(|(k, v)| {
                    serde_json::json!({
                        "name": k,
                        "value": v,
                        "inline": true
                    })
                }).collect::<Vec<_>>()
            }]
        })
    }

    /// Send notification via email (stub - requires email library)
    pub async fn send_email(&self, _to: &[String], _smtp: &SmtpConfig) -> Result<()> {
        // TODO: Implement email sending with lettre or similar
        // For now, log that we would send email
        eprintln!("EMAIL NOTIFICATION (not implemented):");
        eprintln!("  To: {:?}", _to);
        eprintln!("  Subject: {}", self.title);
        eprintln!("  Body: {}", self.message);
        Ok(())
    }
}

/// Notification manager with rate limiting
pub struct NotificationManager {
    config: NotificationConfig,
    sent_count: std::sync::Arc<std::sync::Mutex<usize>>,
    last_reset: std::sync::Arc<std::sync::Mutex<std::time::Instant>>,
}

impl NotificationManager {
    pub fn new(config: NotificationConfig) -> Self {
        Self {
            config,
            sent_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            last_reset: std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now())),
        }
    }

    /// Load configuration from TOML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read notification config file")?;
        let config: NotificationConfig = toml::from_str(&content)
            .context("Failed to parse notification config")?;
        Ok(Self::new(config))
    }

    /// Check if we can send another notification (rate limiting)
    fn can_send(&self) -> bool {
        let mut count = self.sent_count.lock().unwrap();
        let mut last_reset = self.last_reset.lock().unwrap();

        // Reset counter if an hour has passed
        if last_reset.elapsed().as_secs() >= 3600 {
            *count = 0;
            *last_reset = std::time::Instant::now();
        }

        if *count >= self.config.rate_limit_per_hour {
            return false;
        }

        *count += 1;
        true
    }

    /// Send notification via all configured channels
    pub async fn send(&self, notification: &Notification) -> Result<()> {
        // Check severity threshold
        let severity_order = ["info", "warning", "error", "critical"];
        let notif_severity_idx = severity_order
            .iter()
            .position(|s| *s == notification.severity.as_str())
            .unwrap_or(0);
        let min_severity_idx = severity_order
            .iter()
            .position(|s| *s == self.config.min_severity.as_str())
            .unwrap_or(2);

        if notif_severity_idx < min_severity_idx {
            // Below threshold, skip
            return Ok(());
        }

        // Rate limiting
        if !self.can_send() {
            eprintln!("Rate limit exceeded - skipping notification");
            return Ok(());
        }

        // Send to webhooks
        for url in &self.config.webhook {
            if let Err(e) = notification.send_webhook(url).await {
                eprintln!("Failed to send webhook to {}: {}", url, e);
            }
        }

        // Send to email
        if !self.config.email.is_empty() {
            if let Some(smtp) = &self.config.smtp {
                if let Err(e) = notification.send_email(&self.config.email, smtp).await {
                    eprintln!("Failed to send email: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_creation() {
        let notif = Notification::new("Test Alert", "This is a test", "error")
            .with_metadata("session_id", "test-123")
            .with_metadata("metric", "duration_ms");

        assert_eq!(notif.title, "Test Alert");
        assert_eq!(notif.severity, "error");
        assert_eq!(notif.metadata.get("session_id"), Some(&"test-123".to_string()));
    }

    #[test]
    fn test_slack_formatting() {
        let notif = Notification::new("Critical Regression", "Benchmark B-001 regressed by 25%", "critical");
        let slack_payload = notif.format_slack();

        assert!(slack_payload["text"].as_str().unwrap().contains("Critical Regression"));
        assert!(slack_payload["blocks"].is_array());
    }

    #[test]
    fn test_discord_formatting() {
        let notif = Notification::new("Warning", "High memory usage", "warning");
        let discord_payload = notif.format_discord();

        assert_eq!(discord_payload["embeds"][0]["title"], "Warning");
        assert_eq!(discord_payload["embeds"][0]["color"], 0xFFCC00);
    }
}
