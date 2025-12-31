use crate::error::{Error, Result};
use crate::events::EventStream;
use crate::types::*;
use reqwest::Client;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Clone)]
pub struct OpencodeClientConfig {
    pub base_url: String,
    pub directory: Option<PathBuf>,
    pub timeout_seconds: u64,
}

impl Default for OpencodeClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:4096".to_string(),
            directory: None,
            timeout_seconds: 30,
        }
    }
}

impl OpencodeClientConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.directory = Some(dir.into());
        self
    }

    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

pub struct OpencodeClient {
    config: OpencodeClientConfig,
    http: Client,
    base_url: Url,
}

impl OpencodeClient {
    pub fn new(config: OpencodeClientConfig) -> Result<Self> {
        let base_url = Url::parse(&config.base_url)?;
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(Error::Http)?;

        Ok(Self {
            config,
            http,
            base_url,
        })
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base_url.join(path).map_err(Error::Url)
    }

    fn add_directory_param(&self, url: &mut Url) {
        if let Some(dir) = &self.config.directory {
            url.query_pairs_mut()
                .append_pair("directory", &dir.to_string_lossy());
        }
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let url = self.url("/global/health")?;
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_list(&self) -> Result<Vec<Session>> {
        let mut url = self.url("/session")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_create(&self, request: SessionCreateRequest) -> Result<Session> {
        let mut url = self.url("/session")?;
        self.add_directory_param(&mut url);
        let resp = self
            .http
            .post(url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn session_get(&self, id: &str) -> Result<Session> {
        let mut url = self.url(&format!("/session/{}", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_delete(&self, id: &str) -> Result<()> {
        let mut url = self.url(&format!("/session/{}", id))?;
        self.add_directory_param(&mut url);
        self.http.delete(url).send().await?;
        Ok(())
    }

    pub async fn session_prompt(&self, id: &str, content: impl Into<String>) -> Result<()> {
        let request = PromptRequest {
            parts: vec![Part::Text {
                text: content.into(),
            }],
            ..Default::default()
        };
        self.session_prompt_with_request(id, request).await
    }

    pub async fn session_prompt_with_request(
        &self,
        id: &str,
        request: PromptRequest,
    ) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/prompt", id))?;
        self.add_directory_param(&mut url);
        self.http.post(url).json(&request).send().await?;
        Ok(())
    }

    pub async fn session_abort(&self, id: &str) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/abort", id))?;
        self.add_directory_param(&mut url);
        self.http.post(url).send().await?;
        Ok(())
    }

    pub async fn session_messages(&self, id: &str) -> Result<Vec<Message>> {
        let mut url = self.url(&format!("/session/{}/message", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_fork(&self, id: &str) -> Result<Session> {
        let mut url = self.url(&format!("/session/{}/fork", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.post(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn provider_list(&self) -> Result<Vec<Provider>> {
        let mut url = self.url("/provider")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn config_get(&self) -> Result<Config> {
        let mut url = self.url("/config")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn config_update(&self, config: &Config) -> Result<Config> {
        let mut url = self.url("/config")?;
        self.add_directory_param(&mut url);
        let resp = self
            .http
            .patch(url)
            .json(config)
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn project_list(&self) -> Result<Vec<Project>> {
        let url = self.url("/project")?;
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn events(&self) -> Result<EventStream> {
        let mut url = self.url("/global/event")?;
        self.add_directory_param(&mut url);
        EventStream::connect(url.as_str()).await
    }

    pub async fn dispose(&self) -> Result<()> {
        let url = self.url("/global/dispose")?;
        self.http.post(url).send().await?;
        Ok(())
    }

    pub async fn session_prompt_async(&self, id: &str, request: PromptRequest) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/prompt_async", id))?;
        self.add_directory_param(&mut url);
        self.http.post(url).json(&request).send().await?;
        Ok(())
    }

    pub async fn session_share(&self, id: &str) -> Result<ShareResponse> {
        let mut url = self.url(&format!("/session/{}/share", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.post(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_diff(&self, id: &str) -> Result<DiffResponse> {
        let mut url = self.url(&format!("/session/{}/diff", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_summarize(&self, id: &str, model: Option<ModelRef>) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/summarize", id))?;
        self.add_directory_param(&mut url);
        let body = SummarizeRequest { model };
        self.http.post(url).json(&body).send().await?;
        Ok(())
    }

    pub async fn session_todos(&self, id: &str) -> Result<Vec<Todo>> {
        let mut url = self.url(&format!("/session/{}/todo", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_children(&self, id: &str) -> Result<Vec<Session>> {
        let mut url = self.url(&format!("/session/{}/children", id))?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn session_revert(&self, id: &str, message_id: &str) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/revert", id))?;
        self.add_directory_param(&mut url);
        let body = RevertRequest {
            message_id: message_id.to_string(),
        };
        self.http.post(url).json(&body).send().await?;
        Ok(())
    }

    pub async fn session_unrevert(&self, id: &str) -> Result<()> {
        let mut url = self.url(&format!("/session/{}/unrevert", id))?;
        self.add_directory_param(&mut url);
        self.http.post(url).send().await?;
        Ok(())
    }

    pub async fn session_permission_respond(
        &self,
        session_id: &str,
        permission_id: &str,
        response: &str,
    ) -> Result<()> {
        let mut url = self.url(&format!(
            "/session/{}/permissions/{}",
            session_id, permission_id
        ))?;
        self.add_directory_param(&mut url);
        let body = PermissionResponse {
            response: response.to_string(),
        };
        self.http.post(url).json(&body).send().await?;
        Ok(())
    }

    pub async fn file_list(&self, path: Option<&str>) -> Result<Vec<FileInfo>> {
        let mut url = self.url("/file")?;
        self.add_directory_param(&mut url);
        if let Some(p) = path {
            url.query_pairs_mut().append_pair("path", p);
        }
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn file_content(&self, path: &str) -> Result<String> {
        let mut url = self.url("/file/content")?;
        self.add_directory_param(&mut url);
        url.query_pairs_mut().append_pair("file", path);
        let resp = self.http.get(url).send().await?.text().await?;
        Ok(resp)
    }

    pub async fn file_status(&self) -> Result<FileStatusResponse> {
        let mut url = self.url("/file/status")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn find_text(&self, pattern: &str) -> Result<Vec<TextMatch>> {
        let mut url = self.url("/find")?;
        self.add_directory_param(&mut url);
        url.query_pairs_mut().append_pair("pattern", pattern);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn find_file(&self, query: &str) -> Result<Vec<String>> {
        let mut url = self.url("/find/file")?;
        self.add_directory_param(&mut url);
        url.query_pairs_mut().append_pair("query", query);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn find_symbol(&self, query: &str) -> Result<Vec<Symbol>> {
        let mut url = self.url("/find/symbol")?;
        self.add_directory_param(&mut url);
        url.query_pairs_mut().append_pair("query", query);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn provider_auth(&self) -> Result<Vec<AuthMethod>> {
        let mut url = self.url("/provider/auth")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn project_current(&self) -> Result<Project> {
        let mut url = self.url("/project/current")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn vcs_status(&self) -> Result<VcsStatus> {
        let mut url = self.url("/vcs")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn agent_list(&self) -> Result<Vec<Agent>> {
        let mut url = self.url("/agent")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }

    pub async fn mcp_list(&self) -> Result<Vec<McpServer>> {
        let mut url = self.url("/mcp")?;
        self.add_directory_param(&mut url);
        let resp = self.http.get(url).send().await?.json().await?;
        Ok(resp)
    }
}
