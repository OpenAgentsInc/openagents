//! Portable account identity and owned character selection metadata.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub id: u64,
    pub epoch: u64,
    pub key: [u8; 32],
    pub characters: Vec<u64>,
}
