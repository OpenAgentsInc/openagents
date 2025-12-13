//! Environment status types

use uuid::Uuid;

/// Environment execution status
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum EnvStatus {
    /// Environment created but not yet started
    Created,

    /// Environment is currently running
    Running {
        /// When execution started
        started_at: u64,
    },

    /// Environment completed successfully
    Completed {
        /// When execution finished
        finished_at: u64,
        /// Exit code (0 = success)
        exit_code: i32,
    },

    /// Environment execution failed
    Failed {
        /// When execution failed
        finished_at: u64,
        /// Error message
        error: String,
    },
}

impl EnvStatus {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, EnvStatus::Completed { .. } | EnvStatus::Failed { .. })
    }

    /// Get the status as a simple string
    pub fn as_str(&self) -> &'static str {
        match self {
            EnvStatus::Created => "created",
            EnvStatus::Running { .. } => "running",
            EnvStatus::Completed { .. } => "completed",
            EnvStatus::Failed { .. } => "failed",
        }
    }
}

impl Default for EnvStatus {
    fn default() -> Self {
        EnvStatus::Created
    }
}

/// Detailed environment status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnvStatusInfo {
    /// Environment ID
    pub id: Uuid,
    /// Current status
    pub status: EnvStatus,
    /// When environment was created
    pub created_at: u64,
    /// Number of mounted services
    pub mount_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_terminal() {
        assert!(!EnvStatus::Created.is_terminal());
        assert!(!EnvStatus::Running { started_at: 0 }.is_terminal());
        assert!(
            EnvStatus::Completed {
                finished_at: 0,
                exit_code: 0
            }
            .is_terminal()
        );
        assert!(
            EnvStatus::Failed {
                finished_at: 0,
                error: "err".into()
            }
            .is_terminal()
        );
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(EnvStatus::Created.as_str(), "created");
        assert_eq!(EnvStatus::Running { started_at: 0 }.as_str(), "running");
        assert_eq!(
            EnvStatus::Completed {
                finished_at: 0,
                exit_code: 0
            }
            .as_str(),
            "completed"
        );
        assert_eq!(
            EnvStatus::Failed {
                finished_at: 0,
                error: "".into()
            }
            .as_str(),
            "failed"
        );
    }

    #[test]
    fn test_status_serialization() {
        let status = EnvStatus::Running { started_at: 12345 };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"running\""));
        assert!(json.contains("\"started_at\":12345"));

        let status = EnvStatus::Failed {
            finished_at: 99999,
            error: "test error".into(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"failed\""));
        assert!(json.contains("test error"));
    }
}
