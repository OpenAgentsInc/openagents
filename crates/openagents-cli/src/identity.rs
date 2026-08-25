//! Cryptographic identity generation, seed derivation, and identity management

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub name: String,
    pub npub: String,
    pub nsec: String,
    pub created_at: u64,
}

pub struct IdentityStore {
    store_path: PathBuf,
}

impl IdentityStore {
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".openagents").join("identities.json")
    }

    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            store_path: path.unwrap_or_else(Self::default_path),
        }
    }

    pub fn load(&self) -> Result<HashMap<String, IdentityRecord>, Box<dyn std::error::Error>> {
        if !self.store_path.exists() {
            return Ok(HashMap::new());
        }
        let data = fs::read_to_string(&self.store_path)?;
        let map: HashMap<String, IdentityRecord> = serde_json::from_str(&data)?;
        Ok(map)
    }

    pub fn save(&self, records: &HashMap<String, IdentityRecord>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(records)?;
        fs::write(&self.store_path, data)?;
        Ok(())
    }

    pub fn generate_identity(name: &str, seed_phrase: Option<&str>) -> IdentityRecord {
        let seed = seed_phrase.unwrap_or("openagents-entropy-seed-phrase");
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(name.as_bytes());
        let digest = format!("{:x}", hasher.finalize());

        let npub = format!("npub1{}", &digest[..32]);
        let nsec = format!("nsec1{}", &digest[32..]);

        IdentityRecord {
            name: name.to_string(),
            npub,
            nsec,
            created_at: 1724600000,
        }
    }
}
