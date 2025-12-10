use crate::error::*;
use crate::types::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Client for session management
pub struct SessionClient {
    base_url: String,
    http_client: Client,
}

impl SessionClient {
    pub fn new(base_url: impl Into<String>, http_client: Client) -> Self {
        Self {
            base_url: base_url.into(),
            http_client,
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        model: Option<String>,
        transcript: Option<Vec<ChatMessage>>,
    ) -> Result<CreateSessionResponse> {
        let url = format!("{}/v1/sessions", self.base_url);

        let request = CreateSessionRequest { model, transcript };

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let session = response.json::<CreateSessionResponse>().await?;
        Ok(session)
    }

    /// Create a new session without transcript
    pub async fn create_empty_session(&self) -> Result<CreateSessionResponse> {
        self.create_session(None, None).await
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<ListSessionsResponse> {
        let url = format!("{}/v1/sessions", self.base_url);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let sessions = response.json::<ListSessionsResponse>().await?;
        Ok(sessions)
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Result<SessionInfo> {
        let url = format!("{}/v1/sessions/{}", self.base_url, session_id);

        let response = self.http_client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let session = response.json::<SessionInfo>().await?;
        Ok(session)
    }

    /// Get session transcript
    pub async fn get_transcript(&self, session_id: &str) -> Result<TranscriptResponse> {
        let url = format!("{}/v1/sessions/{}/transcript", self.base_url, session_id);

        let response = self.http_client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let transcript = response.json::<TranscriptResponse>().await?;
        Ok(transcript)
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<DeleteSessionResponse> {
        let url = format!("{}/v1/sessions/{}", self.base_url, session_id);

        let response = self.http_client.delete(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<DeleteSessionResponse>().await?;
        Ok(result)
    }

    /// Complete a prompt using session context
    pub async fn complete_with_session(
        &self,
        session_id: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<CompletionResponse> {
        let url = format!("{}/v1/sessions/{}/complete", self.base_url, session_id);

        let request = CompletionRequest {
            model: None,
            messages,
            temperature: None,
            max_tokens: None,
            stream: Some(false),
            response_format: None,
        };

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let completion = response.json::<CompletionResponse>().await?;
        Ok(completion)
    }

    /// Complete a simple prompt using session context
    pub async fn complete_prompt_with_session(
        &self,
        session_id: &str,
        prompt: impl Into<String>,
    ) -> Result<CompletionResponse> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.into(),
        }];

        self.complete_with_session(session_id, messages).await
    }
}

// Session-related types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created: String,
    #[serde(rename = "lastUsed")]
    pub last_used: String,
    #[serde(rename = "messageCount")]
    pub message_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<Vec<ChatMessage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub id: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionResponse {
    pub id: String,
    pub deleted: bool,
}
