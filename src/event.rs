use bitcoin_hashes::{sha256, Hash};
use lazy_static::lazy_static;
use actix::Message;
use serde::{Deserialize, Serialize};
use secp256k1::{schnorr, Secp256k1, VerifyOnly, XOnlyPublicKey};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

lazy_static! {
    pub static ref SECP: Secp256k1<VerifyOnly> = Secp256k1::verification_only();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Message)]
#[rtype(result = "()")]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u64,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    #[serde(skip)]
    pub tagidx: Option<HashMap<char, HashSet<String>>>,
}

impl Event {
    pub fn validate(&self) -> Result<(), &'static str> {
        // Validate event format and signature
        let canonical = self.to_canonical()
            .ok_or("Could not canonicalize event")?;

        // Compute SHA256 of canonical form
        let digest: sha256::Hash = sha256::Hash::hash(canonical.as_bytes());
        let hex_digest = format!("{digest:x}");

        // Verify ID matches computed hash
        if self.id != hex_digest {
            return Err("Event ID does not match content hash");
        }

        // Verify signature
        let sig = schnorr::Signature::from_str(&self.sig)
            .map_err(|_| "Invalid signature format")?;

        let msg = secp256k1::Message::from_slice(digest.as_ref())
            .map_err(|_| "Could not create message from digest")?;

        let pubkey = XOnlyPublicKey::from_str(&self.pubkey)
            .map_err(|_| "Invalid public key format")?;

        SECP.verify_schnorr(&sig, &msg, &pubkey)
            .map_err(|_| "Invalid signature")?;

        Ok(())
    }

    pub fn to_canonical(&self) -> Option<String> {
        let mut elements = Vec::new();
        elements.push(serde_json::Value::Number(0.into())); // id placeholder
        elements.push(serde_json::Value::String(self.pubkey.clone()));
        elements.push(serde_json::Value::Number(self.created_at.into()));
        elements.push(serde_json::Value::Number(self.kind.into()));
        elements.push(self.tags_to_canonical());
        elements.push(serde_json::Value::String(self.content.clone()));

        serde_json::to_string(&serde_json::Value::Array(elements)).ok()
    }

    fn tags_to_canonical(&self) -> serde_json::Value {
        let mut tags = Vec::new();
        for tag in &self.tags {
            let tag_array = tag.iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect();
            tags.push(serde_json::Value::Array(tag_array));
        }
        serde_json::Value::Array(tags)
    }

    pub fn build_index(&mut self) {
        if self.tags.is_empty() {
            return;
        }

        let mut idx: HashMap<char, HashSet<String>> = HashMap::new();
        
        for tag in self.tags.iter().filter(|t| t.len() > 1) {
            if let Some(tag_char) = tag.first().and_then(|s| s.chars().next()) {
                if let Some(tag_val) = tag.get(1) {
                    idx.entry(tag_char)
                        .or_default()
                        .insert(tag_val.clone());
                }
            }
        }

        self.tagidx = Some(idx);
    }

    pub fn generic_tag_val_intersect(&self, tagname: char, check: &HashSet<String>) -> bool {
        match &self.tagidx {
            Some(idx) => match idx.get(&tagname) {
                Some(valset) => !valset.is_disjoint(check),
                None => false,
            },
            None => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCmd {
    pub cmd: String,
    pub event: Event,
}

impl EventCmd {
    pub fn event_id(&self) -> &str {
        &self.event.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_validation() {
        let event = Event {
            id: "a6b6c6d6e6f6".into(),
            pubkey: "0123456789abcdef".into(), 
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".into(),
            sig: "0123456789abcdef".into(),
            tagidx: None,
        };

        // This will fail since we're using dummy values
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_canonical_serialization() {
        let event = Event {
            id: "a6b6c6d6e6f6".into(),
            pubkey: "0123456789abcdef".into(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["e".into(), "123".into()]],
            content: "test".into(),
            sig: "0123456789abcdef".into(),
            tagidx: None,
        };

        let canonical = event.to_canonical().unwrap();
        assert!(canonical.starts_with("[0,"));
        assert!(canonical.contains("test"));
    }

    #[test]
    fn test_tag_indexing() {
        let mut event = Event {
            id: "a6b6c6d6e6f6".into(),
            pubkey: "0123456789abcdef".into(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![
                vec!["e".into(), "123".into()],
                vec!["p".into(), "456".into()]
            ],
            content: "test".into(),
            sig: "0123456789abcdef".into(),
            tagidx: None,
        };

        event.build_index();
        
        let mut check = HashSet::new();
        check.insert("123".into());
        
        assert!(event.generic_tag_val_intersect('e', &check));
        
        check.clear();
        check.insert("789".into());
        assert!(!event.generic_tag_val_intersect('e', &check));
    }
}
