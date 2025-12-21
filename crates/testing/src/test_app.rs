//! TestApp Pattern for Integration Testing
//!
//! Provides a test application instance with isolated database and configuration.

use actix_web::{App, dev::ServiceResponse, test, web, HttpServer};
use rusqlite::Connection;
use std::sync::Arc;
use tempfile::TempDir;

/// Test application for integration testing
///
/// Provides an isolated instance with:
/// - Temporary database
/// - Random port binding
/// - Clean state per test
pub struct TestApp {
    /// Temporary directory (cleaned up on drop)
    _temp_dir: TempDir,
    /// Database connection
    db: Arc<Connection>,
    /// Server port (if running)
    pub port: u16,
}

impl TestApp {
    /// Create a new test application with in-memory database
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        let db = Connection::open(&db_path).expect("Failed to open test database");

        // Initialize schema would go here
        // init_schema(&db).expect("Failed to init schema");

        Self {
            _temp_dir: temp_dir,
            db: Arc::new(db),
            port: 0, // Will be set if server is started
        }
    }

    /// Get database connection for direct queries
    pub fn db(&self) -> &Connection {
        &self.db
    }

    // HTTP request methods would go here when actix-web integration is added
    // Per d-012, we don't include stub methods that don't work
    // These will be added when HTTP testing is actually implemented
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creates_with_temp_db() {
        let app = TestApp::new().await;
        // Test that database connection works
        let result: Result<i32, _> = app.db.query_row("SELECT 1", [], |row| row.get(0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }
}
