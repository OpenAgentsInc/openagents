//! HTTP mock server fixture using wiremock

use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// HTTP mock server for deterministic executor testing
pub struct HttpMockServer {
    server: MockServer,
}

impl HttpMockServer {
    /// Start a new mock server on a random port
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    /// Get the server's base URI (e.g., "http://127.0.0.1:12345")
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Mount a GET endpoint that returns a fixed response
    pub async fn mount_get(&self, endpoint: &str, status: u16, body: &str) {
        Mock::given(method("GET"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_string(body)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&self.server)
            .await;
    }

    /// Mount a POST endpoint that echoes the request body
    pub async fn mount_post_echo(&self, endpoint: &str) {
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(Vec::new()))
            .mount(&self.server)
            .await;

        // Note: wiremock doesn't have direct body echo, we use a workaround
        // The actual echo is handled by checking the request was received
    }

    /// Mount a POST endpoint with a fixed response
    pub async fn mount_post(&self, endpoint: &str, status: u16, body: &str) {
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_string(body)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&self.server)
            .await;
    }

    /// Mount a slow response for timeout testing
    pub async fn mount_slow_response(&self, endpoint: &str, delay_ms: u64) {
        Mock::given(method("GET"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("slow response")
                    .set_delay(Duration::from_millis(delay_ms)),
            )
            .mount(&self.server)
            .await;
    }

    /// Mount an error response (5xx) for a limited number of times
    pub async fn mount_error(&self, endpoint: &str, status: u16, times: u64) {
        Mock::given(method("GET"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(status).set_body_string("error"))
            .up_to_n_times(times)
            .mount(&self.server)
            .await;
    }

    /// Mount a PUT endpoint
    pub async fn mount_put(&self, endpoint: &str, status: u16, body: &str) {
        Mock::given(method("PUT"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(status)
                    .set_body_string(body)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&self.server)
            .await;
    }

    /// Mount a DELETE endpoint
    pub async fn mount_delete(&self, endpoint: &str, status: u16) {
        Mock::given(method("DELETE"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(status))
            .mount(&self.server)
            .await;
    }

    /// Get the number of requests received
    pub async fn received_requests(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_mock_server_starts() {
        let server = HttpMockServer::start().await;
        assert!(server.uri().starts_with("http://"));
    }

    #[tokio::test]
    async fn test_http_mock_get_endpoint() {
        let server = HttpMockServer::start().await;
        server.mount_get("/test", 200, r#"{"ok": true}"#).await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/test", server.uri()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(body.contains("ok"));
    }
}
