use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigDetectParams {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_home: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigDetectResponse {
    #[serde(default)]
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportParams {
    #[serde(default)]
    pub migration_items: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportResponse {}
