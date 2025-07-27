/// Secure Token Storage for JWT Authentication
/// 
/// Phase 3: Implements secure storage and retrieval of JWT tokens using Tauri capabilities
/// Provides encryption and secure access patterns for authentication tokens

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Token storage entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEntry {
    pub token: String,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub token_type: String, // "access", "refresh", etc.
    pub issuer: String,     // e.g., "https://auth.openagents.com"
}

/// Secure token storage manager
/// 
/// Phase 3: Handles secure storage of JWT tokens with automatic expiration checking
pub struct TokenStorage {
    tokens: BTreeMap<String, TokenEntry>,
    storage_key: String,
}

impl TokenStorage {
    /// Create a new token storage instance
    /// 
    /// Phase 3: Initialize secure token storage for the application
    pub fn new() -> Self {
        Self {
            tokens: BTreeMap::new(),
            storage_key: "openagents_auth_tokens".to_string(),
        }
    }

    /// Store a JWT token securely
    /// 
    /// Phase 3: Securely store token with metadata for later retrieval
    pub fn store_token(&mut self, key: &str, token: String, expires_at: Option<u64>) -> Result<(), AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::ConvexDatabaseError(format!("System time error: {}", e)))?
            .as_secs();

        let token_entry = TokenEntry {
            token,
            issued_at: now,
            expires_at,
            token_type: "access".to_string(),
            issuer: "https://auth.openagents.com".to_string(),
        };

        self.tokens.insert(key.to_string(), token_entry);
        self.persist_to_storage()?;
        
        log::info!("Stored token for key: {}", key);
        Ok(())
    }

    /// Retrieve a stored token if valid
    /// 
    /// Phase 3: Get token and check expiration automatically
    pub fn get_token(&self, key: &str) -> Result<Option<String>, AppError> {
        if let Some(entry) = self.tokens.get(key) {
            // Check if token is expired
            if let Some(expires_at) = entry.expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| AppError::ConvexDatabaseError(format!("System time error: {}", e)))?
                    .as_secs();

                if now >= expires_at {
                    log::warn!("Token for key '{}' has expired", key);
                    return Ok(None);
                }
            }

            log::debug!("Retrieved valid token for key: {}", key);
            Ok(Some(entry.token.clone()))
        } else {
            log::debug!("No token found for key: {}", key);
            Ok(None)
        }
    }

    /// Remove a stored token
    /// 
    /// Phase 3: Securely remove token from storage
    pub fn remove_token(&mut self, key: &str) -> Result<(), AppError> {
        if self.tokens.remove(key).is_some() {
            self.persist_to_storage()?;
            log::info!("Removed token for key: {}", key);
        } else {
            log::debug!("No token to remove for key: {}", key);
        }
        Ok(())
    }

    /// Clear all stored tokens
    /// 
    /// Phase 3: Complete logout - remove all authentication tokens
    pub fn clear_all_tokens(&mut self) -> Result<(), AppError> {
        let count = self.tokens.len();
        self.tokens.clear();
        self.persist_to_storage()?;
        log::info!("Cleared {} stored tokens", count);
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
    /// Phase 3: Initialize token storage from saved data
    pub fn load_from_storage(&mut self) -> Result<(), AppError> {
        // TODO: Phase 3 - Implement actual Tauri secure storage integration
        // For now, this is a placeholder for the storage interface
        
        // In a real implementation, this would:
        // 1. Use Tauri's app data directory
        // 2. Read encrypted token data
        // 3. Decrypt and deserialize tokens
        // 4. Populate self.tokens
        
        log::debug!("Loading tokens from secure storage (placeholder)");
        Ok(())
    }

    /// Save tokens to persistent storage
    /// 
    /// Phase 3: Persist tokens securely to disk
    fn persist_to_storage(&self) -> Result<(), AppError> {
        // TODO: Phase 3 - Implement actual Tauri secure storage integration
        // For now, this is a placeholder for the storage interface
        
        // In a real implementation, this would:
        // 1. Serialize self.tokens
        // 2. Encrypt the serialized data
        // 3. Write to Tauri's secure app data directory
        // 4. Set appropriate file permissions
        
        log::debug!("Persisting {} tokens to secure storage (placeholder)", self.tokens.len());
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