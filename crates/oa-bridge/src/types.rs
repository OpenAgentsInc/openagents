use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../docs/types/bridge.d.ts")]
pub struct SyncWatchedDirTs {
    pub provider: String,
    pub base: String,
    pub files: i64,
    pub last_read: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../docs/types/bridge.d.ts")]
pub struct SyncStatusTs {
    pub enabled: bool,
    pub two_way: bool,
    pub watched: Vec<SyncWatchedDirTs>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../docs/types/bridge.d.ts")]
pub struct TinyvexSnapshot<T: TS + Serialize + Clone> {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub stream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub rows: Vec<T>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../docs/types/bridge.d.ts")]
pub struct TinyvexQueryResult<T: TS + Serialize + Clone> {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub rows: Vec<T>,
}

