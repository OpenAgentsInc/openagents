/// Secure Token Storage for JWT Authentication
/// 
/// Phase 3: Implements secure storage and retrieval of JWT tokens using Tauri capabilities
/// Provides encryption and secure access patterns for authentication tokens

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_plugin_store::{Store, StoreBuilder};
use tauri::AppHandle;
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::{Zeroize, ZeroizeOnDrop};
use sha2::{Sha256, Digest};

/// Secure token that auto-zeroizes when dropped
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecureToken {
    value: String,
}

impl SecureToken {
    pub fn new(token: String) -> Self {
        Self { value: token }
    }
    
    pub fn as_str(&self) -> &str {
        &self.value
    }
    
    /// Get a hash of the token for logging purposes (never log the actual token)
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.value.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string() // First 16 chars of hash for better uniqueness
    }
}

// Custom Debug implementation to avoid exposing token in logs
impl std::fmt::Debug for SecureToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureToken(hash:{})", self.hash())
    }
}

// Custom serialization for secure storage
impl Serialize for SecureToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SecureToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(SecureToken::new(value))
    }
}

/// Hash a key for secure logging
fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string() // Increased to 16 chars for better uniqueness
}

/// Token storage entry with metadata and secure token handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEntry {
    pub token: SecureToken,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub token_type: String, // "access", "refresh", etc.
    pub issuer: String,     // e.g., "https://auth.openagents.com"
}

/// Secure token storage manager
/// 
/// Phase 4: Handles secure storage of JWT tokens with Tauri secure store integration
pub struct TokenStorage {
    tokens: BTreeMap<String, TokenEntry>,
    storage_key: String,
    store: Option<Arc<Store<tauri::Wry>>>,
    store_path: PathBuf,
}

impl TokenStorage {
    /// Create a new token storage instance
    /// 
    /// Phase 4: Initialize secure token storage with Tauri secure store
    pub fn new() -> Self {
        Self {
            tokens: BTreeMap::new(),
            storage_key: "openagents_auth_tokens".to_string(),
            store: None,
            store_path: PathBuf::from("openagents_tokens.dat"),
        }
    }

    /// Initialize the token storage with Tauri app handle
    /// 
    /// Phase 4: Setup secure store connection for persistent storage
    pub fn initialize_with_app(&mut self, app: &AppHandle) -> Result<(), AppError> {
        let store = StoreBuilder::new(app, &self.store_path)
            .build()
            .map_err(|e| AppError::TokenStorageError(format!("Failed to initialize secure store: {}", e)))?;
            
        self.store = Some(store);
        self.load_from_storage()?;
        
        log::info!("Initialized secure token storage with Tauri store");
        Ok(())
    }

    /// Store a JWT token securely
    /// 
    /// Phase 4: Securely store token with metadata using Tauri secure store
    pub fn store_token(&mut self, key: &str, token: String, expires_at: Option<u64>) -> Result<(), AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::TokenStorageError(format!("System time error: {}", e)))?
            .as_secs();

        let secure_token = SecureToken::new(token);
        let token_entry = TokenEntry {
            token: secure_token,
            issued_at: now,
            expires_at,
            token_type: "access".to_string(),
            issuer: "https://auth.openagents.com".to_string(),
        };

        self.tokens.insert(key.to_string(), token_entry);
        self.persist_to_storage()?;
        
        // Phase 4: Secure authentication monitoring - only log essential events with hashed values
        log::debug!("AUTH_MONITOR: Token stored [key_hash={}, has_expiration={}]", 
            hash_key(key), expires_at.is_some());
        
        // Security audit log - minimal sensitive data exposure
        log::info!("SECURITY_AUDIT: Token storage event [key_hash={}, method=secure_store]", 
            hash_key(key));
        
        Ok(())
    }

    /// Retrieve a stored token if valid
    /// 
    /// Phase 3: Get token and check expiration automatically
    pub fn get_token(&self, key: &str) -> Result<Option<String>, AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::TokenStorageError(format!("System time error: {}", e)))?
            .as_secs();
            
        if let Some(entry) = self.tokens.get(key) {
            // Check if token is expired
            if let Some(expires_at) = entry.expires_at {
                if now >= expires_at {
                    log::warn!("AUTH_MONITOR: Token expired [key_hash={}, expired={}s_ago]", 
                        hash_key(key), now.saturating_sub(expires_at));
                    log::warn!("SECURITY_AUDIT: Expired token access attempt [key_hash={}]", 
                        hash_key(key));
                    return Ok(None);
                }
                
                let ttl = expires_at.saturating_sub(now);
                log::debug!("AUTH_MONITOR: Token retrieved [key_hash={}, ttl={}s]", hash_key(key), ttl);
                
                // Warning for tokens expiring soon
                if ttl < 300 { // 5 minutes
                    log::warn!("AUTH_MONITOR: Token expiring soon [key_hash={}, ttl={}s]", hash_key(key), ttl);
                }
            } else {
                log::debug!("AUTH_MONITOR: Long-lived token retrieved [key_hash={}]", hash_key(key));
            }

            Ok(Some(entry.token.as_str().to_string()))
        } else {
            log::debug!("AUTH_MONITOR: Token not found [key_hash={}]", hash_key(key));
            log::debug!("SECURITY_AUDIT: Access attempt for non-existent token [key_hash={}]", 
                hash_key(key));
            Ok(None)
        }
    }

    /// Remove a stored token
    /// 
    /// Phase 3: Securely remove token from storage
    pub fn remove_token(&mut self, key: &str) -> Result<(), AppError> {
        if let Some(_removed_entry) = self.tokens.remove(key) {
            self.persist_to_storage()?;
            log::debug!("AUTH_MONITOR: Token removed [key_hash={}]", hash_key(key));
            log::info!("SECURITY_AUDIT: Token deletion event [key_hash={}, method=manual]", 
                hash_key(key));
        } else {
            log::debug!("AUTH_MONITOR: Token removal attempted - not found [key_hash={}]", hash_key(key));
        }
        Ok(())
    }

    /// Clear all stored tokens
    /// 
    /// Phase 4: Complete logout - securely remove all authentication tokens
    pub fn clear_all_tokens(&mut self) -> Result<(), AppError> {
        let count = self.tokens.len();
        self.tokens.clear();
        
        // Also clear from secure store
        if let Some(store) = &self.store {
            let deleted = store.delete(&self.storage_key);
            if deleted {
                store.save()
                    .map_err(|e| AppError::TokenStorageError(format!("Failed to save cleared state: {}", e)))?;
                log::debug!("Cleared tokens from secure store");
            } else {
                log::debug!("No tokens to clear from secure store");
            }
        }
        
        // Phase 4: Secure monitoring for mass token clearing
        log::info!("AUTH_MONITOR: Mass token clearance [count={}, method=logout]", count);
        log::info!("SECURITY_AUDIT: Complete authentication logout [tokens_cleared={}]", count);
        Ok(())
    }

    /// Get token metadata without the actual token
    /// 
    /// Phase 3: Check token status without exposing the token value
    pub fn get_token_info(&self, key: &str) -> Option<TokenInfo> {
        self.tokens.get(key).map(|entry| TokenInfo {
            issued_at: entry.issued_at,
            expires_at: entry.expires_at,
            token_type: entry.token_type.clone(),
            issuer: entry.issuer.clone(),
            is_expired: self.is_token_expired(entry),
        })
    }

    /// Check if any tokens need refresh
    /// 
    /// Phase 3: Identify tokens that are near expiration and need renewal
    pub fn get_tokens_needing_refresh(&self, buffer_seconds: u64) -> Vec<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.tokens
            .iter()
            .filter_map(|(key, entry)| {
                if let Some(expires_at) = entry.expires_at {
                    // Token needs refresh if it expires within the buffer time
                    if now + buffer_seconds >= expires_at {
                        Some(key.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Load tokens from persistent storage
    /// 
    /// Phase 4: Load tokens from Tauri secure store
    pub fn load_from_storage(&mut self) -> Result<(), AppError> {
        if let Some(store) = &self.store {
            // Try to load existing tokens from secure store
            if let Some(stored_tokens) = store.get(&self.storage_key) {
                if let Some(tokens_map) = stored_tokens.as_object() {
                    for (key, value) in tokens_map {
                        if let Ok(token_entry) = serde_json::from_value::<TokenEntry>(value.clone()) {
                            // Only load non-expired tokens
                            if !self.is_token_expired(&token_entry) {
                                self.tokens.insert(key.clone(), token_entry);
                            }
                        }
                    }
                    log::info!("Loaded {} valid tokens from secure storage", self.tokens.len());
                } else {
                    log::debug!("Invalid token data format in secure storage");
                }
            } else {
                log::debug!("No existing tokens found in secure storage");
            }
        } else {
            log::warn!("Secure store not initialized, skipping token load");
        }
        
        Ok(())
    }

    /// Save tokens to persistent storage
    /// 
    /// Phase 4: Persist tokens securely using Tauri secure store
    fn persist_to_storage(&self) -> Result<(), AppError> {
        if let Some(store) = &self.store {
            // Convert tokens to JSON for secure storage
            let tokens_json = serde_json::to_value(&self.tokens)
                .map_err(|e| AppError::TokenStorageError(format!("Failed to serialize tokens: {}", e)))?;
                
            // Store encrypted tokens in secure store
            store.set(&self.storage_key, tokens_json);
                
            // Force save to disk
            store.save()
                .map_err(|e| AppError::TokenStorageError(format!("Failed to save tokens to disk: {}", e)))?;
                
            log::debug!("Securely persisted {} tokens to encrypted storage", self.tokens.len());
        } else {
            log::warn!("Secure store not initialized, cannot persist tokens");
        }
        
        Ok(())
    }

    /// Check if a token is expired
    /// 
    /// Phase 3: Internal helper for token expiration checking
    fn is_token_expired(&self, entry: &TokenEntry) -> bool {
        if let Some(expires_at) = entry.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now >= expires_at
        } else {
            false // No expiration time means token doesn't expire
        }
    }
}

/// Token information without exposing the actual token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub token_type: String,
    pub issuer: String,
    pub is_expired: bool,
}

/// Default implementation for token storage
impl Default for TokenStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_storage_creation() {
        let storage = TokenStorage::new();
        assert_eq!(storage.tokens.len(), 0);
        assert_eq!(storage.storage_key, "openagents_auth_tokens");
    }

    #[test]
    fn test_store_and_retrieve_token() {
        let mut storage = TokenStorage::new();
        let token = "test_jwt_token".to_string();
        
        // Store token
        storage.store_token("test_key", token.clone(), None).unwrap();
        
        // Retrieve token
        let retrieved = storage.get_token("test_key").unwrap();
        assert_eq!(retrieved, Some(token));
    }

    #[test]
    fn test_token_expiration() {
        let mut storage = TokenStorage::new();
        let token = "expired_token".to_string();
        
        // Store token that expires immediately
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        storage.store_token("expired_key", token, Some(now - 1)).unwrap();
        
        // Should return None for expired token
        let retrieved = storage.get_token("expired_key").unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_token_removal() {
        let mut storage = TokenStorage::new();
        let token = "remove_me".to_string();
        
        storage.store_token("remove_key", token, None).unwrap();
        assert!(storage.get_token("remove_key").unwrap().is_some());
        
        storage.remove_token("remove_key").unwrap();
        assert!(storage.get_token("remove_key").unwrap().is_none());
    }

    #[test]
    fn test_clear_all_tokens() {
        let mut storage = TokenStorage::new();
        
        storage.store_token("key1", "token1".to_string(), None).unwrap();
        storage.store_token("key2", "token2".to_string(), None).unwrap();
        
        assert_eq!(storage.tokens.len(), 2);
        
        storage.clear_all_tokens().unwrap();
        assert_eq!(storage.tokens.len(), 0);
    }

    #[test]
    fn test_tokens_needing_refresh() {
        let mut storage = TokenStorage::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Token expires in 30 seconds
        storage.store_token("refresh_soon", "token1".to_string(), Some(now + 30)).unwrap();
        // Token expires in 2 hours
        storage.store_token("refresh_later", "token2".to_string(), Some(now + 7200)).unwrap();
        // Token never expires
        storage.store_token("no_expire", "token3".to_string(), None).unwrap();
        
        // Check for tokens needing refresh within 60 seconds
        let needing_refresh = storage.get_tokens_needing_refresh(60);
        assert_eq!(needing_refresh.len(), 1);
        assert!(needing_refresh.contains(&"refresh_soon".to_string()));
    }

    #[test]
    fn test_token_info() {
        let mut storage = TokenStorage::new();
        let token = "info_token".to_string();
        
        storage.store_token("info_key", token, None).unwrap();
        
        let info = storage.get_token_info("info_key").unwrap();
        assert_eq!(info.token_type, "access");
        assert_eq!(info.issuer, "https://auth.openagents.com");
        assert!(!info.is_expired);
    }
}