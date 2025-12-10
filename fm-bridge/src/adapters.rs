use crate::error::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Client for adapter management
pub struct AdapterClient {
    base_url: String,
    http_client: Client,
}

impl AdapterClient {
    pub fn new(base_url: impl Into<String>, http_client: Client) -> Self {
        Self {
            base_url: base_url.into(),
            http_client,
        }
    }

    /// Load adapter from file path
    pub async fn load_from_file(
        &self,
        file_url: impl Into<String>,
        adapter_id: Option<String>,
    ) -> Result<LoadAdapterResponse> {
        let url = format!("{}/v1/adapters/load", self.base_url);

        let request = LoadAdapterRequest {
            id: adapter_id,
            file_url: Some(file_url.into()),
            name: None,
        };

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

        let result = response.json::<LoadAdapterResponse>().await?;
        Ok(result)
    }

    /// Load adapter by name
    pub async fn load_by_name(
        &self,
        name: impl Into<String>,
        adapter_id: Option<String>,
    ) -> Result<LoadAdapterResponse> {
        let url = format!("{}/v1/adapters/load", self.base_url);

        let request = LoadAdapterRequest {
            id: adapter_id,
            file_url: None,
            name: Some(name.into()),
        };

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

        let result = response.json::<LoadAdapterResponse>().await?;
        Ok(result)
    }

    /// List all loaded adapters
    pub async fn list_adapters(&self) -> Result<ListAdaptersResponse> {
        let url = format!("{}/v1/adapters", self.base_url);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<ListAdaptersResponse>().await?;
        Ok(result)
    }

    /// Get adapter info by ID
    pub async fn get_adapter(&self, adapter_id: &str) -> Result<AdapterInfo> {
        let url = format!("{}/v1/adapters/{}", self.base_url, adapter_id);

        let response = self.http_client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Adapter not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<AdapterInfo>().await?;
        Ok(result)
    }

    /// Unload adapter
    pub async fn unload_adapter(&self, adapter_id: &str) -> Result<UnloadAdapterResponse> {
        let url = format!("{}/v1/adapters/{}", self.base_url, adapter_id);

        let response = self.http_client.delete(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Adapter not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<UnloadAdapterResponse>().await?;
        Ok(result)
    }

    /// Recompile adapter
    pub async fn recompile_adapter(&self, adapter_id: &str) -> Result<CompileAdapterResponse> {
        let url = format!("{}/v1/adapters/{}/compile", self.base_url, adapter_id);

        let response = self.http_client.post(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Adapter not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<CompileAdapterResponse>().await?;
        Ok(result)
    }

    /// Get compatible adapter identifiers for a name
    pub async fn get_compatible_identifiers(
        &self,
        name: &str,
    ) -> Result<CompatibleAdaptersResponse> {
        let url = format!("{}/v1/adapters/compatible/{}", self.base_url, name);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<CompatibleAdaptersResponse>().await?;
        Ok(result)
    }

    /// Cleanup obsolete adapters from system
    pub async fn cleanup_obsolete_adapters(&self) -> Result<CleanupResponse> {
        let url = format!("{}/v1/adapters/cleanup", self.base_url);

        let response = self.http_client.post(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<CleanupResponse>().await?;
        Ok(result)
    }
}

// Adapter-related types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    pub loaded_at: String,
    pub last_used: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoadAdapterRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAdapterResponse {
    pub id: String,
    pub loaded: bool,
    pub adapter: AdapterInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAdaptersResponse {
    pub adapters: Vec<AdapterInfo>,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnloadAdapterResponse {
    pub id: String,
    pub unloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileAdapterResponse {
    pub id: String,
    pub compiled: bool,
    pub compiled_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibleAdaptersResponse {
    pub name: String,
    pub compatible_identifiers: Vec<String>,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResponse {
    pub cleaned: bool,
    pub message: String,
}
