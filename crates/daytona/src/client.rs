//! Daytona API client.
//!
//! Provides a type-safe interface to Daytona's REST API for:
//! - Sandbox management (create, start, stop, delete)
//! - Process execution (toolbox)

use crate::config::DaytonaConfig;
use crate::error::{DaytonaError, Result};
use crate::models::{
    CreateSandbox, CreateSessionRequest, ExecuteRequest, ExecuteResponse, FileInfo, GitAddRequest,
    GitBranchRequest, GitCheckoutRequest, GitCloneRequest, GitCommitInfo, GitCommitRequest,
    GitCommitResponse, GitDeleteBranchRequest, GitRepoRequest, GitStatus, ListBranchResponse,
    Match, PortPreviewUrl, ReplaceRequest, ReplaceResult, Sandbox, SandboxLabels, SandboxState,
    SearchFilesResponse, Session, SessionExecuteRequest, SessionExecuteResponse,
};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, instrument};

/// Daytona API client.
#[derive(Clone)]
pub struct DaytonaClient {
    http_client: reqwest::Client,
    config: DaytonaConfig,
}

impl DaytonaClient {
    /// Create a new Daytona client from configuration.
    pub fn new(config: DaytonaConfig) -> Result<Self> {
        // Validate that we have some form of authentication
        if config.api_key.is_none() && config.bearer_token.is_none() {
            return Err(DaytonaError::NotConfigured(
                "Either api_key or bearer_token must be provided".to_string(),
            ));
        }

        let http_client = reqwest::Client::builder().timeout(config.timeout).build()?;

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Build authorization headers for Daytona API requests.
    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // Add authorization header
        if let Some(api_key) = &self.config.api_key {
            let auth_value = format!("Bearer {}", api_key);
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).expect("valid auth header"),
            );
        } else if let Some(token) = &self.config.bearer_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).expect("valid auth header"),
            );
        }

        // Add content type
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Add organization ID if present
        if let Some(org_id) = &self.config.organization_id {
            headers.insert(
                "X-Daytona-Organization-ID",
                HeaderValue::from_str(org_id).expect("valid org id header"),
            );
        }

        headers
    }

    /// Make a GET request to the API.
    #[instrument(skip(self), fields(url = %url))]
    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        debug!("GET {}", url);

        let response = self
            .http_client
            .get(url)
            .headers(self.auth_headers())
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a POST request to the API.
    #[instrument(skip(self, body), fields(url = %url))]
    async fn post<T: DeserializeOwned, B: Serialize>(&self, url: &str, body: &B) -> Result<T> {
        debug!("POST {}", url);

        let response = self
            .http_client
            .post(url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a POST request without a body.
    #[instrument(skip(self), fields(url = %url))]
    async fn post_empty<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        debug!("POST {}", url);

        let response = self
            .http_client
            .post(url)
            .headers(self.auth_headers())
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a POST request that returns no content.
    #[instrument(skip(self), fields(url = %url))]
    async fn post_no_content(&self, url: &str) -> Result<()> {
        debug!("POST {}", url);

        let response = self
            .http_client
            .post(url)
            .headers(self.auth_headers())
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Make a POST request with a body that returns no content.
    #[instrument(skip(self, body), fields(url = %url))]
    async fn post_no_content_with_body<B: Serialize>(&self, url: &str, body: &B) -> Result<()> {
        debug!("POST {}", url);

        let response = self
            .http_client
            .post(url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Make a PUT request to the API.
    #[instrument(skip(self, body), fields(url = %url))]
    async fn put<T: DeserializeOwned, B: Serialize>(&self, url: &str, body: &B) -> Result<T> {
        debug!("PUT {}", url);

        let response = self
            .http_client
            .put(url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a DELETE request to the API.
    #[instrument(skip(self), fields(url = %url))]
    async fn delete(&self, url: &str) -> Result<()> {
        debug!("DELETE {}", url);

        let response = self
            .http_client
            .delete(url)
            .headers(self.auth_headers())
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Make a DELETE request with a JSON body.
    #[instrument(skip(self, body), fields(url = %url))]
    async fn delete_with_body<B: Serialize>(&self, url: &str, body: &B) -> Result<()> {
        debug!("DELETE {}", url);

        let response = self
            .http_client
            .delete(url)
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Make a GET request that returns raw bytes.
    #[instrument(skip(self), fields(url = %url))]
    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        debug!("GET (bytes) {}", url);

        let response = self
            .http_client
            .get(url)
            .headers(self.auth_headers())
            .send()
            .await?;

        let status = response.status();
        let status_code = status.as_u16();

        if status_code == 401 {
            return Err(DaytonaError::Unauthorized);
        }

        if status_code == 403 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::Forbidden(error_text));
        }

        if status_code == 404 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::SandboxNotFound(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("API error ({}): {}", status_code, error_text);
            return Err(DaytonaError::ApiError {
                status: status_code,
                message: error_text,
            });
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| DaytonaError::InvalidResponse(e.to_string()))
    }

    /// Handle API response with body.
    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();
        let status_code = status.as_u16();

        if status_code == 401 {
            return Err(DaytonaError::Unauthorized);
        }

        if status_code == 403 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::Forbidden(error_text));
        }

        if status_code == 404 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::SandboxNotFound(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("API error ({}): {}", status_code, error_text);
            return Err(DaytonaError::ApiError {
                status: status_code,
                message: error_text,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|e| DaytonaError::InvalidResponse(e.to_string()))
    }

    /// Handle API response without body.
    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        let status_code = status.as_u16();

        if status_code == 401 {
            return Err(DaytonaError::Unauthorized);
        }

        if status_code == 403 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::Forbidden(error_text));
        }

        if status_code == 404 {
            let error_text = response.text().await.unwrap_or_default();
            return Err(DaytonaError::SandboxNotFound(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("API error ({}): {}", status_code, error_text);
            return Err(DaytonaError::ApiError {
                status: status_code,
                message: error_text,
            });
        }

        Ok(())
    }

    // =========================================================================
    // Sandbox Lifecycle Operations
    // =========================================================================

    /// List all sandboxes.
    #[instrument(skip(self))]
    pub async fn list_sandboxes(
        &self,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<Vec<Sandbox>> {
        let mut url = format!("{}/sandbox", self.config.base_url);

        if let Some(labels) = labels {
            let labels_json = serde_json::to_string(labels)
                .map_err(|e| DaytonaError::InvalidResponse(e.to_string()))?;
            url = format!("{}?labels={}", url, urlencoding::encode(&labels_json));
        }

        self.get(&url).await
    }

    /// Create a new sandbox.
    #[instrument(skip(self, request))]
    pub async fn create_sandbox(&self, request: &CreateSandbox) -> Result<Sandbox> {
        let url = format!("{}/sandbox", self.config.base_url);
        self.post(&url, request).await
    }

    /// Get a sandbox by ID.
    #[instrument(skip(self))]
    pub async fn get_sandbox(&self, sandbox_id: &str) -> Result<Sandbox> {
        let url = format!("{}/sandbox/{}", self.config.base_url, sandbox_id);
        self.get(&url).await
    }

    /// Delete a sandbox.
    #[instrument(skip(self))]
    pub async fn delete_sandbox(&self, sandbox_id: &str, force: bool) -> Result<()> {
        let url = format!(
            "{}/sandbox/{}?force={}",
            self.config.base_url, sandbox_id, force
        );
        self.delete(&url).await
    }

    /// Start a sandbox.
    #[instrument(skip(self))]
    pub async fn start_sandbox(&self, sandbox_id: &str) -> Result<Sandbox> {
        let url = format!("{}/sandbox/{}/start", self.config.base_url, sandbox_id);
        self.post_empty(&url).await
    }

    /// Stop a sandbox.
    #[instrument(skip(self))]
    pub async fn stop_sandbox(&self, sandbox_id: &str) -> Result<()> {
        let url = format!("{}/sandbox/{}/stop", self.config.base_url, sandbox_id);
        self.post_no_content(&url).await
    }

    /// Archive a sandbox.
    #[instrument(skip(self))]
    pub async fn archive_sandbox(&self, sandbox_id: &str) -> Result<()> {
        let url = format!("{}/sandbox/{}/archive", self.config.base_url, sandbox_id);
        self.post_no_content(&url).await
    }

    /// Replace sandbox labels.
    #[instrument(skip(self))]
    pub async fn replace_labels(
        &self,
        sandbox_id: &str,
        labels: HashMap<String, String>,
    ) -> Result<SandboxLabels> {
        let url = format!("{}/sandbox/{}/labels", self.config.base_url, sandbox_id);
        self.put(&url, &SandboxLabels { labels }).await
    }

    /// Set auto-stop interval.
    #[instrument(skip(self))]
    pub async fn set_auto_stop(&self, sandbox_id: &str, minutes: i32) -> Result<()> {
        let url = format!(
            "{}/sandbox/{}/autostop/{}",
            self.config.base_url, sandbox_id, minutes
        );
        self.post_no_content(&url).await
    }

    /// Set auto-archive interval.
    #[instrument(skip(self))]
    pub async fn set_auto_archive(&self, sandbox_id: &str, minutes: i32) -> Result<()> {
        let url = format!(
            "{}/sandbox/{}/autoarchive/{}",
            self.config.base_url, sandbox_id, minutes
        );
        self.post_no_content(&url).await
    }

    /// Set auto-delete interval.
    #[instrument(skip(self))]
    pub async fn set_auto_delete(&self, sandbox_id: &str, minutes: i32) -> Result<()> {
        let url = format!(
            "{}/sandbox/{}/autodelete/{}",
            self.config.base_url, sandbox_id, minutes
        );
        self.post_no_content(&url).await
    }

    /// Get preview URL for a port.
    #[instrument(skip(self))]
    pub async fn get_preview_url(&self, sandbox_id: &str, port: u16) -> Result<PortPreviewUrl> {
        let url = format!(
            "{}/sandbox/{}/ports/{}/preview-url",
            self.config.base_url, sandbox_id, port
        );
        self.get(&url).await
    }

    /// Wait for a sandbox to reach a specific state.
    #[instrument(skip(self))]
    pub async fn wait_for_state(
        &self,
        sandbox_id: &str,
        target_state: SandboxState,
        timeout: Duration,
    ) -> Result<Sandbox> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_secs(2);

        loop {
            let sandbox = self.get_sandbox(sandbox_id).await?;

            if let Some(state) = sandbox.state {
                if state == target_state {
                    return Ok(sandbox);
                }

                // Check for error states
                if state == SandboxState::Error || state == SandboxState::BuildFailed {
                    return Err(DaytonaError::ApiError {
                        status: 500,
                        message: sandbox
                            .error_reason
                            .unwrap_or_else(|| "Sandbox entered error state".to_string()),
                    });
                }
            }

            if start.elapsed() > timeout {
                return Err(DaytonaError::StateTimeout);
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    // =========================================================================
    // Toolbox: Process Execution
    // =========================================================================

    /// Execute a command in a sandbox (synchronous).
    #[instrument(skip(self, request))]
    pub async fn execute_command(
        &self,
        sandbox_id: &str,
        request: &ExecuteRequest,
    ) -> Result<ExecuteResponse> {
        let url = format!(
            "{}/toolbox/{}/toolbox/process/execute",
            self.config.base_url, sandbox_id
        );
        self.post(&url, request).await
    }

    // =========================================================================
    // Toolbox: Git Operations
    // =========================================================================

    /// Clone a git repository into the sandbox.
    #[instrument(skip(self, request))]
    pub async fn git_clone(&self, sandbox_id: &str, request: &GitCloneRequest) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/clone",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }

    /// Get the project directory in a sandbox.
    #[instrument(skip(self))]
    pub async fn get_project_dir(&self, sandbox_id: &str) -> Result<String> {
        let url = format!(
            "{}/toolbox/{}/toolbox/project-dir",
            self.config.base_url, sandbox_id
        );

        #[derive(Deserialize)]
        struct Response {
            path: String,
        }

        let response: Response = self.get(&url).await?;
        Ok(response.path)
    }

    /// Create a new session in a sandbox.
    #[instrument(skip(self))]
    pub async fn create_session(
        &self,
        sandbox_id: &str,
        session_id: Option<&str>,
    ) -> Result<Session> {
        let url = format!(
            "{}/toolbox/{}/toolbox/process/session",
            self.config.base_url, sandbox_id
        );

        let request = CreateSessionRequest {
            session_id: session_id.map(String::from),
        };

        self.post(&url, &request).await
    }

    /// Get session information.
    #[instrument(skip(self))]
    pub async fn get_session(&self, sandbox_id: &str, session_id: &str) -> Result<Session> {
        let url = format!(
            "{}/toolbox/{}/toolbox/process/session/{}",
            self.config.base_url, sandbox_id, session_id
        );
        self.get(&url).await
    }

    /// Execute a command in a session.
    #[instrument(skip(self, request))]
    pub async fn session_execute(
        &self,
        sandbox_id: &str,
        session_id: &str,
        request: &SessionExecuteRequest,
    ) -> Result<SessionExecuteResponse> {
        let url = format!(
            "{}/toolbox/{}/toolbox/process/session/{}/exec",
            self.config.base_url, sandbox_id, session_id
        );
        self.post(&url, request).await
    }

    /// Get logs for a command in a session.
    #[instrument(skip(self))]
    pub async fn get_command_logs(
        &self,
        sandbox_id: &str,
        session_id: &str,
        command_id: &str,
    ) -> Result<String> {
        let url = format!(
            "{}/toolbox/{}/toolbox/process/session/{}/command/{}/logs",
            self.config.base_url, sandbox_id, session_id, command_id
        );

        #[derive(Deserialize)]
        struct Response {
            logs: String,
        }

        let response: Response = self.get(&url).await?;
        Ok(response.logs)
    }

    // =========================================================================
    // Toolbox: File Operations
    // =========================================================================

    /// List files in a directory.
    #[instrument(skip(self))]
    pub async fn list_files(&self, sandbox_id: &str, path: &str) -> Result<Vec<FileInfo>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.get(&url).await
    }

    /// Download/read a file from the sandbox.
    #[instrument(skip(self))]
    pub async fn download_file(&self, sandbox_id: &str, path: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/download?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.get_bytes(&url).await
    }

    /// Download a file as a string (assumes UTF-8).
    #[instrument(skip(self))]
    pub async fn read_file(&self, sandbox_id: &str, path: &str) -> Result<String> {
        let bytes = self.download_file(sandbox_id, path).await?;
        String::from_utf8(bytes).map_err(|e| DaytonaError::InvalidResponse(e.to_string()))
    }

    /// Upload/write a file to the sandbox.
    #[instrument(skip(self, content))]
    pub async fn upload_file(&self, sandbox_id: &str, path: &str, content: &[u8]) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );

        debug!("POST (upload) {}", url);

        // Use multipart form for file upload
        let part = reqwest::multipart::Part::bytes(content.to_vec())
            .file_name("file")
            .mime_str("application/octet-stream")
            .map_err(|e| DaytonaError::InvalidResponse(e.to_string()))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let mut headers = self.auth_headers();
        // Remove Content-Type header - it will be set by multipart
        headers.remove(CONTENT_TYPE);

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Write a string to a file (convenience method).
    #[instrument(skip(self, content))]
    pub async fn write_file(&self, sandbox_id: &str, path: &str, content: &str) -> Result<()> {
        self.upload_file(sandbox_id, path, content.as_bytes()).await
    }

    /// Delete a file from the sandbox.
    #[instrument(skip(self))]
    pub async fn delete_file(&self, sandbox_id: &str, path: &str) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.delete(&url).await
    }

    /// Create a folder in the sandbox.
    #[instrument(skip(self))]
    pub async fn create_folder(&self, sandbox_id: &str, path: &str, mode: &str) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/folder?path={}&mode={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path),
            urlencoding::encode(mode)
        );
        self.post_no_content(&url).await
    }

    /// Get information about a file.
    #[instrument(skip(self))]
    pub async fn get_file_info(&self, sandbox_id: &str, path: &str) -> Result<FileInfo> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/info?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.get(&url).await
    }

    /// Move/rename a file.
    #[instrument(skip(self))]
    pub async fn move_file(&self, sandbox_id: &str, source: &str, destination: &str) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/move?source={}&destination={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(source),
            urlencoding::encode(destination)
        );
        self.post_no_content(&url).await
    }

    /// Search for text/pattern in files (like grep).
    #[instrument(skip(self))]
    pub async fn find_in_files(
        &self,
        sandbox_id: &str,
        path: &str,
        pattern: &str,
    ) -> Result<Vec<Match>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/find?path={}&pattern={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path),
            urlencoding::encode(pattern)
        );
        self.get(&url).await
    }

    /// Search for files by name pattern (like find).
    #[instrument(skip(self))]
    pub async fn search_files(
        &self,
        sandbox_id: &str,
        path: &str,
        pattern: &str,
    ) -> Result<Vec<String>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/search?path={}&pattern={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path),
            urlencoding::encode(pattern)
        );
        let response: SearchFilesResponse = self.get(&url).await?;
        Ok(response.files)
    }

    /// Replace text in files.
    #[instrument(skip(self, request))]
    pub async fn replace_in_files(
        &self,
        sandbox_id: &str,
        request: &ReplaceRequest,
    ) -> Result<Vec<ReplaceResult>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/files/replace",
            self.config.base_url, sandbox_id
        );
        self.post(&url, request).await
    }

    // =========================================================================
    // Toolbox: Extended Git Operations
    // =========================================================================

    /// Get git status for a repository.
    #[instrument(skip(self))]
    pub async fn git_status(&self, sandbox_id: &str, path: &str) -> Result<GitStatus> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/status?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.get(&url).await
    }

    /// Stage files for commit.
    #[instrument(skip(self, request))]
    pub async fn git_add(&self, sandbox_id: &str, request: &GitAddRequest) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/add",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }

    /// Create a git commit.
    #[instrument(skip(self, request))]
    pub async fn git_commit(
        &self,
        sandbox_id: &str,
        request: &GitCommitRequest,
    ) -> Result<GitCommitResponse> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/commit",
            self.config.base_url, sandbox_id
        );
        self.post(&url, request).await
    }

    /// Get commit history.
    #[instrument(skip(self))]
    pub async fn git_history(&self, sandbox_id: &str, path: &str) -> Result<Vec<GitCommitInfo>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/history?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        self.get(&url).await
    }

    /// List branches in a repository.
    #[instrument(skip(self))]
    pub async fn git_list_branches(&self, sandbox_id: &str, path: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/branches?path={}",
            self.config.base_url,
            sandbox_id,
            urlencoding::encode(path)
        );
        let response: ListBranchResponse = self.get(&url).await?;
        Ok(response.branches)
    }

    /// Create a new branch.
    #[instrument(skip(self, request))]
    pub async fn git_create_branch(
        &self,
        sandbox_id: &str,
        request: &GitBranchRequest,
    ) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/branches",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }

    /// Delete a branch.
    #[instrument(skip(self, request))]
    pub async fn git_delete_branch(
        &self,
        sandbox_id: &str,
        request: &GitDeleteBranchRequest,
    ) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/branches",
            self.config.base_url, sandbox_id
        );
        self.delete_with_body(&url, request).await
    }

    /// Checkout a branch or commit.
    #[instrument(skip(self, request))]
    pub async fn git_checkout(&self, sandbox_id: &str, request: &GitCheckoutRequest) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/checkout",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }

    /// Pull changes from remote.
    #[instrument(skip(self, request))]
    pub async fn git_pull(&self, sandbox_id: &str, request: &GitRepoRequest) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/pull",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }

    /// Push changes to remote.
    #[instrument(skip(self, request))]
    pub async fn git_push(&self, sandbox_id: &str, request: &GitRepoRequest) -> Result<()> {
        let url = format!(
            "{}/toolbox/{}/toolbox/git/push",
            self.config.base_url, sandbox_id
        );
        self.post_no_content_with_body(&url, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_with_api_key() {
        let config = DaytonaConfig::with_api_key("test-key");
        assert!(config.api_key.is_some());
        assert_eq!(config.api_key.as_deref(), Some("test-key"));
    }

    #[test]
    fn test_client_requires_auth() {
        let config = DaytonaConfig::default();
        let result = DaytonaClient::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_client_with_api_key() {
        let config = DaytonaConfig::with_api_key("test-key");
        let result = DaytonaClient::new(config);
        assert!(result.is_ok());
    }
}
