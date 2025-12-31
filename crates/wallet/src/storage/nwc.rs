//! Nostr Wallet Connect connection storage.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const NWC_CONNECTIONS_ENV: &str = "OPENAGENTS_NWC_CONNECTIONS";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NwcConnection {
    pub id: String,
    pub name: Option<String>,
    pub wallet_pubkey: String,
    pub wallet_secret: String,
    pub client_pubkey: String,
    pub relays: Vec<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NwcConnectionStore {
    pub connections: Vec<NwcConnection>,
}

impl NwcConnectionStore {
    pub fn load() -> Result<Self> {
        let path = connections_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read NWC connections {}", path.display()))?;
        if contents.trim().is_empty() {
            return Ok(Self::default());
        }
        let store = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse NWC connections {}", path.display()))?;
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let path = connections_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let contents =
            serde_json::to_string_pretty(self).context("Failed to serialize NWC connections")?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write NWC connections {}", path.display()))?;
        Ok(())
    }

    pub fn add(&mut self, connection: NwcConnection) -> Result<()> {
        if self
            .connections
            .iter()
            .any(|entry| entry.id == connection.id)
        {
            anyhow::bail!("NWC connection '{}' already exists.", connection.id);
        }

        if let Some(name) = &connection.name {
            if self
                .connections
                .iter()
                .any(|entry| entry.name.as_deref() == Some(name.as_str()))
            {
                anyhow::bail!("NWC connection name '{}' already exists.", name);
            }
        }

        self.connections.push(connection);
        Ok(())
    }

    pub fn remove(&mut self, id_or_name: &str) -> Result<NwcConnection> {
        if let Some(index) = self
            .connections
            .iter()
            .position(|entry| entry.id == id_or_name || entry.name.as_deref() == Some(id_or_name))
        {
            return Ok(self.connections.remove(index));
        }

        anyhow::bail!("NWC connection '{}' not found.", id_or_name);
    }

    pub fn find_by_wallet_pubkey(&self, pubkey: &str) -> Option<&NwcConnection> {
        self.connections
            .iter()
            .find(|entry| entry.wallet_pubkey == pubkey)
    }
}

fn connections_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var(NWC_CONNECTIONS_ENV) {
        return Ok(PathBuf::from(path));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home.join(".openagents").join("nwc_connections.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static NWC_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sample_connection(id: &str, name: Option<&str>) -> NwcConnection {
        NwcConnection {
            id: id.to_string(),
            name: name.map(|value| value.to_string()),
            wallet_pubkey: "wallet_pubkey".to_string(),
            wallet_secret: "wallet_secret".to_string(),
            client_pubkey: "client_pubkey".to_string(),
            relays: vec!["wss://relay.example.com".to_string()],
            created_at: 1_700_000_000,
        }
    }

    #[test]
    fn test_add_remove_connections() {
        let _guard = NWC_ENV_LOCK.lock().unwrap();
        let temp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        let original = std::env::var(NWC_CONNECTIONS_ENV).ok();

        unsafe {
            std::env::set_var(NWC_CONNECTIONS_ENV, temp.path());
        }

        let mut store = NwcConnectionStore::load().unwrap();
        assert!(store.connections.is_empty());

        store
            .add(sample_connection("conn-1", Some("default")))
            .unwrap();
        store.save().unwrap();

        let store = NwcConnectionStore::load().unwrap();
        assert_eq!(store.connections.len(), 1);
        assert_eq!(store.connections[0].id, "conn-1");

        let mut store = store;
        let removed = store.remove("conn-1").unwrap();
        assert_eq!(removed.id, "conn-1");
        store.save().unwrap();

        let store = NwcConnectionStore::load().unwrap();
        assert!(store.connections.is_empty());

        if let Some(value) = original {
            unsafe {
                std::env::set_var(NWC_CONNECTIONS_ENV, value);
            }
        } else {
            unsafe {
                std::env::remove_var(NWC_CONNECTIONS_ENV);
            }
        }
    }

    #[test]
    fn test_reject_duplicate_name() {
        let mut store = NwcConnectionStore::default();
        store
            .add(sample_connection("conn-1", Some("alpha")))
            .unwrap();
        let err = store
            .add(sample_connection("conn-2", Some("alpha")))
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
}
