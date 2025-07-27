use std::collections::BTreeMap;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::error::AppError;

/// JWT claims structure for OpenAuth tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    /// Subject (GitHub ID)
    pub sub: String,
    /// Issued at time
    pub iat: u64,
    /// Expiration time
    pub exp: u64,
    /// Issuer (OpenAuth server)
    pub iss: String,
    /// Audience 
    pub aud: String,
    /// User email
    pub email: Option<String>,
    /// User name
    pub name: Option<String>,
    /// User avatar URL
    pub avatar: Option<String>,
    /// GitHub username
    pub github_username: Option<String>,
}

/// User authentication context
#[derive(Debug, Clone, Serialize)]
pub struct AuthContext {
    pub user_id: String,
    pub github_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub github_username: Option<String>,
    pub token: String,
    pub expires_at: u64,
}

/// Authentication manager for Convex operations
pub struct ConvexAuth {
    validation: Validation,
    decoding_key: Option<DecodingKey>,
    openauth_domain: String,
}

impl ConvexAuth {
    /// Create a new authentication manager
    pub fn new(openauth_domain: String) -> Self {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&openauth_domain]);
        validation.set_audience(&["openagents"]);
        
        Self {
            validation,
            decoding_key: None,
            openauth_domain,
        }
    }

    /// Set the decoding key for JWT validation (would be fetched from OpenAuth JWKS endpoint)
    pub fn set_decoding_key(&mut self, key: DecodingKey) {
        self.decoding_key = Some(key);
    }

    /// Validate a JWT token and extract user context
    pub fn validate_token(&self, token: &str) -> Result<AuthContext, AppError> {
        let decoding_key = self.decoding_key.as_ref()
            .ok_or_else(|| AppError::ConvexAuthError("No decoding key configured".to_string()))?;

        let token_data = decode::<AuthClaims>(token, decoding_key, &self.validation)
            .map_err(|e| AppError::JwtValidationError(e))?;

        let claims = token_data.claims;

        // Check if token is expired
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AppError::ConvexAuthError("System time error".to_string()))?
            .as_secs();

        if claims.exp < now {
            return Err(AppError::ConvexAuthError("Token expired".to_string()));
        }

        Ok(AuthContext {
            user_id: claims.sub.clone(),
            github_id: claims.sub,
            email: claims.email,
            name: claims.name,
            avatar: claims.avatar,
            github_username: claims.github_username,
            token: token.to_string(),
            expires_at: claims.exp,
        })
    }

    /// Extract basic user info from token without full validation (DEVELOPMENT ONLY)
    /// 
    /// WARNING: This method bypasses signature verification and should NEVER be used in production
    /// It's only intended for development/testing when JWKS is not available
    #[cfg(debug_assertions)]
    pub fn extract_user_info_unsafe(&self, token: &str) -> Result<AuthContext, AppError> {
        log::warn!("Using unsafe token extraction - DO NOT USE IN PRODUCTION");
        
        // This is for development only - decodes without signature verification
        let token_parts: Vec<&str> = token.split('.').collect();
        if token_parts.len() != 3 {
            return Err(AppError::ConvexAuthError("Invalid JWT format".to_string()));
        }

        let payload = general_purpose::URL_SAFE_NO_PAD.decode(token_parts[1])
            .map_err(|e| AppError::Base64DecodeError(e))?;

        let claims: AuthClaims = serde_json::from_slice(&payload)
            .map_err(|e| AppError::Json(e))?;

        // Still validate expiration even in unsafe mode
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AppError::ConvexAuthError("System time error".to_string()))?
            .as_secs();

        if claims.exp < now {
            return Err(AppError::ConvexAuthError("Token expired".to_string()));
        }

        Ok(AuthContext {
            user_id: claims.sub.clone(),
            github_id: claims.sub,
            email: claims.email,
            name: claims.name,
            avatar: claims.avatar,
            github_username: claims.github_username,
            token: token.to_string(),
            expires_at: claims.exp,
        })
    }

    /// Fetch JWKS (JSON Web Key Set) from OpenAuth server
    pub async fn fetch_jwks(&mut self) -> Result<(), AppError> {
        let jwks_url = format!("{}/.well-known/jwks.json", self.openauth_domain);
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::Http(e))?;
            
        let response = client.get(&jwks_url)
            .send()
            .await
            .map_err(|e| AppError::Http(e))?;

        if !response.status().is_success() {
            return Err(AppError::ConvexAuthError(
                format!("Failed to fetch JWKS: {}", response.status())
            ));
        }

        let jwks: serde_json::Value = response.json()
            .await
            .map_err(|e| AppError::Http(e))?;

        // Parse JWKS and extract the first RSA key
        if let Some(keys) = jwks.get("keys").and_then(|k| k.as_array()) {
            for key in keys {
                if let (Some(kty), Some(n), Some(e)) = (
                    key.get("kty").and_then(|v| v.as_str()),
                    key.get("n").and_then(|v| v.as_str()),
                    key.get("e").and_then(|v| v.as_str()),
                ) {
                    if kty == "RSA" {
                        // Decode the RSA components
                        let n_bytes = general_purpose::URL_SAFE_NO_PAD.decode(n)
                            .map_err(|_| AppError::ConvexAuthError("Invalid RSA modulus".to_string()))?;
                        let e_bytes = general_purpose::URL_SAFE_NO_PAD.decode(e)
                            .map_err(|_| AppError::ConvexAuthError("Invalid RSA exponent".to_string()))?;
                        
                        // Create RSA key from components
                        let decoding_key = DecodingKey::from_rsa_components(&n_bytes, &e_bytes)
                            .map_err(|_| AppError::ConvexAuthError("Failed to create RSA key".to_string()))?;
                        
                        self.decoding_key = Some(decoding_key);
                        return Ok(());
                    }
                }
            }
        }

        Err(AppError::ConvexAuthError("No valid RSA keys found in JWKS".to_string()))
    }

    /// Create authentication headers for Convex requests
    pub fn create_auth_headers(&self, auth_context: &AuthContext) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", auth_context.token));
        headers
    }

    /// Get user claims from token for Convex operations
    pub fn get_user_claims(&self, auth_context: &AuthContext) -> BTreeMap<String, serde_json::Value> {
        let mut claims = BTreeMap::new();
        
        claims.insert("userId".to_string(), serde_json::Value::String(auth_context.user_id.clone()));
        claims.insert("githubId".to_string(), serde_json::Value::String(auth_context.github_id.clone()));
        
        if let Some(email) = &auth_context.email {
            claims.insert("email".to_string(), serde_json::Value::String(email.clone()));
        }
        
        if let Some(name) = &auth_context.name {
            claims.insert("name".to_string(), serde_json::Value::String(name.clone()));
        }
        
        if let Some(avatar) = &auth_context.avatar {
            claims.insert("avatar".to_string(), serde_json::Value::String(avatar.clone()));
        }
        
        if let Some(github_username) = &auth_context.github_username {
            claims.insert("githubUsername".to_string(), serde_json::Value::String(github_username.clone()));
        }

        claims
    }
}

/// Authentication service for managing user sessions
pub struct AuthService {
    convex_auth: ConvexAuth,
    current_auth: Option<AuthContext>,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(openauth_domain: String) -> Self {
        Self {
            convex_auth: ConvexAuth::new(openauth_domain),
            current_auth: None,
        }
    }

    /// Authenticate with a JWT token
    pub async fn authenticate(&mut self, token: String) -> Result<AuthContext, AppError> {
        // Try secure validation first
        let auth_context = match self.convex_auth.validate_token(&token) {
            Ok(context) => {
                log::info!("Token validated securely");
                context
            }
            Err(AppError::ConvexAuthError(msg)) if msg.contains("No decoding key") => {
                log::warn!("No JWKS key available, attempting to fetch...");
                
                // Try to fetch JWKS first
                if let Err(e) = self.convex_auth.fetch_jwks().await {
                    log::warn!("JWKS fetch failed: {}", e);
                    
                    // Only fall back to unsafe mode in debug builds
                    #[cfg(debug_assertions)]
                    {
                        log::warn!("Falling back to unsafe token extraction for development");
                        self.convex_auth.extract_user_info_unsafe(&token)?
                    }
                    
                    #[cfg(not(debug_assertions))]
                    {
                        return Err(AppError::ConvexAuthError(
                            "Cannot validate token: JWKS unavailable and unsafe mode disabled in release".to_string()
                        ));
                    }
                } else {
                    // Retry validation with fetched JWKS
                    self.convex_auth.validate_token(&token)?
                }
            }
            Err(e) => return Err(e),
        };
        
        self.current_auth = Some(auth_context.clone());
        Ok(auth_context)
    }

    /// Get current authentication context
    pub fn get_auth_context(&self) -> Option<&AuthContext> {
        self.current_auth.as_ref()
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        if let Some(auth) = &self.current_auth {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            
            auth.expires_at > now
        } else {
            false
        }
    }

    /// Clear authentication
    pub fn logout(&mut self) {
        self.current_auth = None;
    }

    /// Get authentication claims for Convex operations
    pub fn get_convex_claims(&self) -> Result<BTreeMap<String, serde_json::Value>, AppError> {
        let auth_context = self.current_auth.as_ref()
            .ok_or_else(|| AppError::ConvexAuthError("Not authenticated".to_string()))?;
        
        Ok(self.convex_auth.get_user_claims(auth_context))
    }

    /// Get authorization token
    pub fn get_token(&self) -> Option<String> {
        self.current_auth.as_ref().map(|auth| auth.token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn create_test_claims() -> AuthClaims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        AuthClaims {
            sub: "github|12345".to_string(),
            iat: now,
            exp: now + 3600, // Expires in 1 hour
            iss: "http://localhost:8787".to_string(),
            aud: "openagents".to_string(),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            avatar: Some("https://github.com/avatar.png".to_string()),
            github_username: Some("testuser".to_string()),
        }
    }

    #[test]
    fn test_extract_user_info_unsafe() {
        let auth = ConvexAuth::new("http://localhost:8787".to_string());
        let claims = create_test_claims();
        
        // Create a token using HS256 for testing (works with secret keys)
        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(b"secret");
        let token = encode(&header, &claims, &key).unwrap();
        
        let result = auth.extract_user_info_unsafe(&token);
        assert!(result.is_ok());
        
        let auth_context = result.unwrap();
        assert_eq!(auth_context.github_id, "github|12345");
        assert_eq!(auth_context.email, Some("test@example.com".to_string()));
        assert_eq!(auth_context.github_username, Some("testuser".to_string()));
    }

    #[test]
    fn test_auth_service_flow() {
        let mut service = AuthService::new("http://localhost:8787".to_string());
        
        // Initially not authenticated
        assert!(!service.is_authenticated());
        assert!(service.get_auth_context().is_none());
        
        // After logout, still not authenticated
        service.logout();
        assert!(!service.is_authenticated());
    }

    #[test]
    fn test_get_convex_claims() {
        let auth = ConvexAuth::new("http://localhost:8787".to_string());
        let auth_context = AuthContext {
            user_id: "user123".to_string(),
            github_id: "github|12345".to_string(),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            avatar: Some("https://github.com/avatar.png".to_string()),
            github_username: Some("testuser".to_string()),
            token: "token123".to_string(),
            expires_at: 9999999999, // Far future
        };

        let claims = auth.get_user_claims(&auth_context);
        
        assert_eq!(claims.get("userId").unwrap(), &serde_json::Value::String("user123".to_string()));
        assert_eq!(claims.get("githubId").unwrap(), &serde_json::Value::String("github|12345".to_string()));
        assert_eq!(claims.get("email").unwrap(), &serde_json::Value::String("test@example.com".to_string()));
    }
}