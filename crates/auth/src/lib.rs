//! Authentication for local API routes
//!
//! Provides token-based authentication for localhost-only services.
//! Generates a random token on startup and stores it securely.

use actix_web::error::ErrorUnauthorized;
use actix_web::{Error, dev::ServiceRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::{Context, Result};
use rand::Rng;
use std::path::{Path, PathBuf};

/// Length of authentication tokens in bytes
const TOKEN_LENGTH: usize = 32;

/// Authentication token manager
pub struct AuthToken {
    token: String,
    token_file: PathBuf,
}

impl AuthToken {
    /// Generate a new random token or load existing one
    pub async fn init() -> Result<Self> {
        let token_file = Self::get_token_path()?;

        // Try to load existing token
        if token_file.exists()
            && let Ok(token) = tokio::fs::read_to_string(&token_file).await
        {
            let token = token.trim().to_string();
            if Self::is_valid_token(&token) {
                return Ok(Self { token, token_file });
            }
        }

        // Generate new token
        let token = Self::generate_token();

        // Ensure parent directory exists
        if let Some(parent) = token_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write token to file
        tokio::fs::write(&token_file, &token).await?;

        // Set secure permissions (owner read/write only)
        #[cfg(unix)]
        {
            use nix::sys::stat::{FchmodatFlags, Mode, fchmodat};
            use std::os::unix::ffi::OsStrExt;

            fchmodat(
                None,
                token_file.as_os_str().as_bytes(),
                Mode::S_IRUSR | Mode::S_IWUSR,
                FchmodatFlags::FollowSymlink,
            )
            .context("Failed to set token file permissions")?;
        }

        #[cfg(windows)]
        {
            tracing::warn!(
                "Token file permissions not set on Windows. File may be readable by other users."
            );
        }

        Ok(Self { token, token_file })
    }

    /// Generate a random authentication token
    fn generate_token() -> String {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..TOKEN_LENGTH).map(|_| rng.r#gen()).collect();
        hex::encode(bytes)
    }

    /// Validate token format
    fn is_valid_token(token: &str) -> bool {
        token.len() == TOKEN_LENGTH * 2 && token.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Get the path to the token file
    fn get_token_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir().context("Failed to get local data directory")?;
        Ok(data_dir.join("openagents").join("auth_token"))
    }

    /// Get the token value
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get the path to the token file (for user reference)
    pub fn token_file_path(&self) -> &Path {
        &self.token_file
    }

    /// Validate a provided token
    pub fn validate(&self, provided: &str) -> bool {
        // Use constant-time comparison to prevent timing attacks
        constant_time_eq(self.token.as_bytes(), provided.as_bytes())
    }
}

/// Constant-time equality check to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }

    result == 0
}

/// Validator function for actix-web-httpauth middleware
pub async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // Get token from app data - need to check before moving req
    let is_valid = match req.app_data::<actix_web::web::Data<AuthToken>>() {
        Some(token) => token.validate(credentials.token()),
        None => {
            return Err((ErrorUnauthorized("Authentication not configured"), req));
        }
    };

    // Validate token
    if is_valid {
        Ok(req)
    } else {
        Err((ErrorUnauthorized("Invalid authentication token"), req))
    }
}

/// Type alias for the authentication validator future
type AuthValidatorFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>>,
>;

/// Create authentication middleware
///
/// Returns HttpAuthentication middleware that requires Bearer token authentication.
/// Use with `.wrap()` on Actix services or scopes.
pub fn auth_middleware()
-> HttpAuthentication<BearerAuth, impl Fn(ServiceRequest, BearerAuth) -> AuthValidatorFuture> {
    HttpAuthentication::bearer(|req, creds| -> AuthValidatorFuture {
        Box::pin(async move {
            // Get token from app data - need to check before moving req
            let is_valid = match req.app_data::<actix_web::web::Data<AuthToken>>() {
                Some(token) => token.validate(creds.token()),
                None => {
                    return Err((ErrorUnauthorized("Authentication not configured"), req));
                }
            };

            // Validate token
            if is_valid {
                Ok(req)
            } else {
                Err((ErrorUnauthorized("Invalid authentication token"), req))
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token = AuthToken::generate_token();
        assert_eq!(token.len(), TOKEN_LENGTH * 2);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_is_valid_token() {
        assert!(AuthToken::is_valid_token(&"0".repeat(64)));
        assert!(AuthToken::is_valid_token(
            &("abc123def456".repeat(5) + "abcd")
        ));
        assert!(!AuthToken::is_valid_token("short"));
        assert!(!AuthToken::is_valid_token(&"x".repeat(64)));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hello!"));
    }

    #[tokio::test]
    async fn test_token_validation() {
        let token = AuthToken::init().await.unwrap();
        let token_str = token.token().to_string();

        assert!(token.validate(&token_str));
        assert!(!token.validate("wrong"));
        assert!(!token.validate(""));
    }
}
