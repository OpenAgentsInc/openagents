//! Trajectory contribution to marketplace via Nostr

use super::validate::validate_trajectory;
use super::{Anonymizer, RedactionEngine, RedactionLevel, RewardCalculator, TrajectorySession};
use crate::db::init_db;
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::str::FromStr;

/// Status of a trajectory contribution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributionStatus {
    /// Pending review
    Pending,
    /// Accepted and paid
    Accepted,
    /// Rejected
    Rejected,
}

impl ContributionStatus {
    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

impl FromStr for ContributionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("Unknown contribution status: {}", s)),
        }
    }
}

/// Request to contribute a trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionRequest {
    /// Session to contribute
    pub session: TrajectorySession,

    /// Redacted and anonymized content
    pub content: String,

    /// Hash of the trajectory for verification
    pub trajectory_hash: String,

    /// Lightning address for payment
    pub lightning_address: String,
}

/// Response from contribution submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionResponse {
    /// Contribution ID
    pub contribution_id: String,

    /// Initial status
    pub status: ContributionStatus,

    /// Estimated reward in sats (not guaranteed)
    pub estimated_reward_sats: u64,

    /// Message from marketplace
    pub message: String,
}

/// Configuration for the contribution client
#[derive(Debug, Clone)]
pub struct ContributionConfig {
    /// Database path for tracking contributions
    pub db_path: PathBuf,

    /// Relays to publish to
    pub relays: Vec<String>,

    /// Redaction level
    pub redaction_level: RedactionLevel,

    /// Minimum quality threshold
    pub min_quality: f64,

    /// Lightning address for payments
    pub lightning_address: Option<String>,
}

impl Default for ContributionConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from(".openagents/marketplace.db"),
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            redaction_level: RedactionLevel::Standard,
            min_quality: 0.5,
            lightning_address: None,
        }
    }
}

/// Client for contributing trajectories to the marketplace
pub struct ContributionClient {
    config: ContributionConfig,
    db: Connection,
}

impl ContributionClient {
    /// Create a new contribution client
    pub fn new(config: ContributionConfig) -> Result<Self> {
        let db = init_db(&config.db_path).context("Failed to initialize contribution database")?;

        Ok(Self { config, db })
    }

    /// Submit a trajectory contribution
    ///
    /// This will:
    /// 1. Load and validate the trajectory session
    /// 2. Apply redaction and anonymization
    /// 3. Calculate quality score and reward
    /// 4. Create and publish Nostr event
    /// 5. Store contribution record in database
    pub async fn submit(&mut self, session: TrajectorySession) -> Result<ContributionResponse> {
        // Validate trajectory
        let validation = validate_trajectory(&session, self.config.min_quality);

        if !validation.passed {
            anyhow::bail!(
                "Trajectory failed validation: {}",
                validation.failure_reasons.join(", ")
            );
        }

        // Load trajectory content from file
        let content =
            std::fs::read_to_string(&session.path).context("Failed to read trajectory file")?;

        // Apply redaction
        let redaction_engine = RedactionEngine::new(self.config.redaction_level.clone(), vec![])?;
        let redaction_result = redaction_engine.redact(&content)?;

        // Apply anonymization
        let mut anonymizer = Anonymizer::new(true, true, false);
        let anon_result = anonymizer.anonymize(&redaction_result.content)?;

        // Calculate reward
        let calculator = RewardCalculator::default();
        let reward = calculator.calculate_reward(
            &session,
            validation.quality_score,
            self.config.min_quality,
        );

        // Create contribution ID and hash
        let contribution_id = uuid::Uuid::new_v4().to_string();
        let trajectory_hash = sha256_hash(&anon_result.content);

        // Create Nostr event (kind 30078 - application-specific data)
        let event_content = json!({
            "type": "trajectory_contribution",
            "version": "1.0",
            "contribution_id": contribution_id,
            "session_id": session.session_id,
            "source": session.source,
            "trajectory_hash": trajectory_hash,
            "quality_score": validation.quality_score.value(),
            "token_count": session.token_count,
            "tool_calls": session.tool_calls,
            "has_ci_signal": session.ci_passed.is_some(),
            "initial_commit": session.initial_commit,
            "final_commit": session.final_commit,
            "redacted_trajectory": anon_result.content,
            "redaction_stats": {
                "secrets_redacted": redaction_result.secrets_redacted,
                "paths_anonymized": anon_result.paths_anonymized,
                "usernames_anonymized": anon_result.usernames_anonymized,
            }
        });

        let nostr_event_id = self.publish_to_relays(&event_content).await?;

        // Store in database
        let now = chrono::Utc::now().to_rfc3339();
        self.db.execute(
            r#"
            INSERT INTO trajectory_contributions (
                contribution_id, session_id, source, trajectory_hash,
                nostr_event_id, status, quality_score, estimated_reward_sats,
                lightning_address, submitted_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            rusqlite::params![
                &contribution_id,
                &session.session_id,
                &session.source,
                &trajectory_hash,
                &nostr_event_id,
                ContributionStatus::Pending.as_str(),
                validation.quality_score.value(),
                reward.total_sats as i64,
                self.config.lightning_address.as_deref().unwrap_or(""),
                &now,
                &now,
            ],
        )?;

        Ok(ContributionResponse {
            contribution_id,
            status: ContributionStatus::Pending,
            estimated_reward_sats: reward.total_sats,
            message: format!(
                "Contribution submitted to {} relays. Event ID: {}",
                self.config.relays.len(),
                nostr_event_id
            ),
        })
    }

    /// Publish trajectory event to configured relays
    async fn publish_to_relays(&self, content: &serde_json::Value) -> Result<String> {
        // For now, return a mock event ID
        // In production, this would:
        // 1. Sign the event with user's Nostr private key
        // 2. Publish to all configured relays
        // 3. Wait for OK responses
        // 4. Return the event ID

        let event_id = sha256_hash(&content.to_string());

        // Log for debugging
        tracing::info!(
            "Publishing trajectory contribution to {} relays",
            self.config.relays.len()
        );

        Ok(event_id)
    }

    /// Check status of a contribution
    pub fn check_status(&self, contribution_id: &str) -> Result<ContributionStatus> {
        let status_str: String = self.db.query_row(
            "SELECT status FROM trajectory_contributions WHERE contribution_id = ?",
            [contribution_id],
            |row| row.get(0),
        )?;

        status_str
            .parse::<ContributionStatus>()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Get all contributions with optional status filter
    pub fn list_contributions(
        &self,
        status_filter: Option<ContributionStatus>,
    ) -> Result<Vec<ContributionRecord>> {
        let mut query = "SELECT contribution_id, session_id, source, status, quality_score, \
                         estimated_reward_sats, actual_reward_sats, submitted_at, paid_at \
                         FROM trajectory_contributions"
            .to_string();

        let mut params: Vec<String> = vec![];

        if let Some(status) = status_filter {
            query.push_str(" WHERE status = ?");
            params.push(status.as_str().to_string());
        }

        query.push_str(" ORDER BY submitted_at DESC");

        let mut stmt = self.db.prepare(&query)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(&params_refs[..], |row| {
            Ok(ContributionRecord {
                contribution_id: row.get(0)?,
                session_id: row.get(1)?,
                source: row.get(2)?,
                status: row
                    .get::<_, String>(3)?
                    .parse::<ContributionStatus>()
                    .unwrap_or(ContributionStatus::Pending),
                quality_score: row.get(4)?,
                estimated_reward_sats: row.get::<_, i64>(5)? as u64,
                actual_reward_sats: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                submitted_at: row.get(7)?,
                paid_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get contribution earnings
    pub fn get_earnings(&self) -> Result<Vec<ContributionEarning>> {
        let mut stmt = self.db.prepare(
            r#"
            SELECT contribution_id, session_id, actual_reward_sats, paid_at, payment_preimage
            FROM trajectory_contributions
            WHERE status = ? AND actual_reward_sats IS NOT NULL
            ORDER BY paid_at DESC
            "#,
        )?;

        let rows = stmt.query_map([ContributionStatus::Accepted.as_str()], |row| {
            Ok(ContributionEarning {
                contribution_id: row.get(0)?,
                session_id: row.get(1)?,
                reward_sats: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                paid_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap_or_else(|_| chrono::Utc::now().fixed_offset())
                    .with_timezone(&chrono::Utc),
                payment_preimage: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Record of a contribution stored in the database
#[derive(Debug, Clone)]
pub struct ContributionRecord {
    pub contribution_id: String,
    pub session_id: String,
    pub source: String,
    pub status: ContributionStatus,
    pub quality_score: f64,
    pub estimated_reward_sats: u64,
    pub actual_reward_sats: Option<u64>,
    pub submitted_at: String,
    pub paid_at: Option<String>,
}

/// Simple SHA-256 hash function for content hashing
fn sha256_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Earning record for a contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionEarning {
    /// Contribution ID
    pub contribution_id: String,

    /// Session ID
    pub session_id: String,

    /// Reward amount in sats
    pub reward_sats: u64,

    /// When payment was received
    pub paid_at: chrono::DateTime<chrono::Utc>,

    /// Payment preimage (proof of payment)
    pub payment_preimage: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Write;

    #[test]
    fn test_contribution_status_conversion() {
        assert_eq!(ContributionStatus::Pending.as_str(), "pending");
        assert_eq!(ContributionStatus::Accepted.as_str(), "accepted");
        assert_eq!(ContributionStatus::Rejected.as_str(), "rejected");

        assert_eq!(
            ContributionStatus::from_str("pending"),
            Ok(ContributionStatus::Pending)
        );
        assert_eq!(
            ContributionStatus::from_str("accepted"),
            Ok(ContributionStatus::Accepted)
        );
        assert_eq!(
            ContributionStatus::from_str("rejected"),
            Ok(ContributionStatus::Rejected)
        );
        assert!(ContributionStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_contribution_request_creation() {
        let session = TrajectorySession {
            session_id: "test-123".to_string(),
            source: "claude".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: Some("abc".to_string()),
            final_commit: Some("def".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 1000,
            tool_calls: 10,
            quality_score: 0.8,
        };

        let request = ContributionRequest {
            session,
            content: "redacted content".to_string(),
            trajectory_hash: "abc123hash".to_string(),
            lightning_address: "user@getalby.com".to_string(),
        };

        assert_eq!(request.session.session_id, "test-123");
        assert_eq!(request.lightning_address, "user@getalby.com");
    }

    #[test]
    fn test_contribution_config_default() {
        let config = ContributionConfig::default();
        assert_eq!(
            config.db_path,
            std::path::PathBuf::from(".openagents/marketplace.db")
        );
        assert_eq!(config.relays.len(), 2);
        assert_eq!(config.min_quality, 0.5);
    }

    #[tokio::test]
    async fn test_contribution_client_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut config = ContributionConfig::default();
        config.db_path = db_path;

        let client = ContributionClient::new(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_contribution_submission() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let test_file = temp_dir.path().join("test.rlog");

        // Create test trajectory file
        let mut file = std::fs::File::create(&test_file).unwrap();
        writeln!(file, "Test trajectory content").unwrap();
        writeln!(file, "Some tool calls here").unwrap();
        writeln!(file, "Final result").unwrap();

        let mut config = ContributionConfig::default();
        config.db_path = db_path;
        config.min_quality = 0.1; // Low threshold for testing

        let mut client = ContributionClient::new(config).unwrap();

        let session = TrajectorySession {
            session_id: "test-session-1".to_string(),
            source: "claude".to_string(),
            path: test_file,
            initial_commit: Some("abc123".to_string()),
            final_commit: Some("def456".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 2000,
            tool_calls: 15,
            quality_score: 0.8,
        };

        let result = client.submit(session).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status, ContributionStatus::Pending);
        assert!(response.estimated_reward_sats > 0);

        // Verify it was stored in database
        let status = client.check_status(&response.contribution_id);
        assert!(status.is_ok());
        assert_eq!(status.unwrap(), ContributionStatus::Pending);
    }

    #[tokio::test]
    async fn test_list_contributions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut config = ContributionConfig::default();
        config.db_path = db_path;

        let client = ContributionClient::new(config).unwrap();

        // List should be empty initially
        let contribs = client.list_contributions(None);
        assert!(contribs.is_ok());
        assert_eq!(contribs.unwrap().len(), 0);

        // List by status
        let pending = client.list_contributions(Some(ContributionStatus::Pending));
        assert!(pending.is_ok());
        assert_eq!(pending.unwrap().len(), 0);
    }

    #[test]
    fn test_sha256_hash() {
        let content = "test content";
        let hash1 = sha256_hash(content);
        let hash2 = sha256_hash(content);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be hex string
        assert_eq!(hash1.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));

        // Different content should produce different hash
        let hash3 = sha256_hash("different content");
        assert_ne!(hash1, hash3);
    }
}
