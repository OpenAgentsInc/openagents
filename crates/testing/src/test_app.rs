//! TestApp Pattern for Integration Testing
//!
//! Provides a test application instance with isolated database and configuration.

use reqwest::{Client, Method, Response};
use rusqlite::Connection;
use std::sync::Arc;
use tempfile::TempDir;

/// Test application for integration testing
///
/// Provides an isolated instance with:
/// - In-memory database
/// - HTTP client for testing routes
/// - Clean state per test
#[allow(clippy::arc_with_non_send_sync)]
pub struct TestApp {
    /// Temporary directory (cleaned up on drop)
    _temp_dir: TempDir,
    /// Database connection
    db: Arc<Connection>,
    /// Server port (if running)
    pub port: u16,
    /// HTTP client
    client: Client,
    /// Base URL for requests
    base_url: String,
}

impl TestApp {
    /// Create a new test application with in-memory database
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db = Connection::open_in_memory().expect("Failed to open in-memory test database");

        // Initialize schema would go here
        // init_schema(&db).expect("Failed to init schema");

        #[allow(clippy::arc_with_non_send_sync)]
        Self {
            _temp_dir: temp_dir,
            db: Arc::new(db),
            port: 0, // Will be set when server starts
            client: Client::new(),
            base_url: String::new(),
        }
    }

    /// Set the server port and base URL
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self.base_url = format!("http://127.0.0.1:{}", port);
        self
    }

    /// Get database connection for direct queries
    pub fn db(&self) -> &Connection {
        &self.db
    }

    /// Get the base URL for the test server
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Send GET request
    pub async fn get(&self, path: &str) -> reqwest::Result<Response> {
        self.client.get(self.url(path)).send().await
    }

    /// Send POST request with JSON body
    pub async fn post<T: serde::Serialize>(
        &self,
        path: &str,
        json: &T,
    ) -> reqwest::Result<Response> {
        self.client.post(self.url(path)).json(json).send().await
    }

    /// Send PUT request with JSON body
    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        json: &T,
    ) -> reqwest::Result<Response> {
        self.client.put(self.url(path)).json(json).send().await
    }

    /// Send DELETE request
    pub async fn delete(&self, path: &str) -> reqwest::Result<Response> {
        self.client.delete(self.url(path)).send().await
    }

    /// Send PATCH request with JSON body
    pub async fn patch<T: serde::Serialize>(
        &self,
        path: &str,
        json: &T,
    ) -> reqwest::Result<Response> {
        self.client.patch(self.url(path)).json(json).send().await
    }

    /// Send custom request
    pub async fn request(&self, method: Method, path: &str) -> reqwest::Result<Response> {
        self.client.request(method, self.url(path)).send().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creates_with_in_memory_db() {
        let app = TestApp::new().await;
        // Test that database connection works
        let result: Result<i32, _> = app.db.query_row("SELECT 1", [], |row| row.get(0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        let file: Option<String> = app
            .db
            .query_row("PRAGMA database_list", [], |row| row.get(2))
            .expect("database_list");
        assert!(
            file.as_deref().is_none()
                || file.as_deref() == Some("")
                || file.as_deref() == Some(":memory:")
        );
    }

    #[tokio::test]
    async fn test_app_with_port() {
        let app = TestApp::new().await.with_port(8080);
        assert_eq!(app.port, 8080);
        assert_eq!(app.url("/test"), "http://127.0.0.1:8080/test");
    }

    #[tokio::test]
    async fn test_url_formatting() {
        let app = TestApp::new().await.with_port(3000);
        assert_eq!(app.url("/"), "http://127.0.0.1:3000/");
        assert_eq!(app.url("/api/users"), "http://127.0.0.1:3000/api/users");
        assert_eq!(
            app.url("/api/users?page=1"),
            "http://127.0.0.1:3000/api/users?page=1"
        );
    }

    #[tokio::test]
    async fn test_database_cleanup_on_drop() {
        let temp_path = {
            let app = TestApp::new().await;
            app._temp_dir.path().to_path_buf()
        };
        // After app is dropped, temp directory should be cleaned up
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn test_app_instances_use_isolated_databases() {
        let app_one = TestApp::new().await;
        let app_two = TestApp::new().await;

        app_one
            .db()
            .execute("CREATE TABLE test_isolation (id INTEGER PRIMARY KEY)", [])
            .expect("create table");
        app_one
            .db()
            .execute("INSERT INTO test_isolation (id) VALUES (1)", [])
            .expect("insert row");

        let count_two: i64 = app_two
            .db()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='test_isolation'",
                [],
                |row| row.get(0),
            )
            .expect("schema count");

        assert_eq!(count_two, 0);
    }
}
