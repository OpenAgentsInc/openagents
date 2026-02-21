use std::sync::Arc;

use base64::Engine;
use bip39::Mnemonic;
use reqwest::Client;
use serde::Deserialize;

use crate::config::{MnemonicProvider, WalletExecutorConfig};
use crate::error::SecretLoadError;

pub trait MnemonicSecretProvider: Send + Sync {
    fn load_mnemonic<'a>(
        &'a self,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<String, SecretLoadError>> + Send + 'a>,
    >;
}

#[derive(Clone)]
pub struct EnvMnemonicProvider {
    env_var: String,
}

impl EnvMnemonicProvider {
    pub fn new(env_var: impl Into<String>) -> Self {
        Self {
            env_var: env_var.into(),
        }
    }
}

impl MnemonicSecretProvider for EnvMnemonicProvider {
    fn load_mnemonic<'a>(
        &'a self,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<String, SecretLoadError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let value = std::env::var(&self.env_var).unwrap_or_default();
            if value.trim().is_empty() {
                return Err(SecretLoadError::new(
                    "env",
                    self.env_var.clone(),
                    "environment variable is missing",
                ));
            }
            validate_mnemonic("env", &self.env_var, &value)
        })
    }
}

#[derive(Clone)]
pub struct GcpSecretManagerMnemonicProvider {
    secret_version: String,
    http: Client,
}

impl GcpSecretManagerMnemonicProvider {
    pub fn new(secret_version: impl Into<String>, http: Client) -> Self {
        Self {
            secret_version: secret_version.into(),
            http,
        }
    }
}

impl MnemonicSecretProvider for GcpSecretManagerMnemonicProvider {
    fn load_mnemonic<'a>(
        &'a self,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<String, SecretLoadError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let token = gcp_access_token(&self.http).await?;

            let url = format!(
                "https://secretmanager.googleapis.com/v1/{}:access",
                self.secret_version
            );

            let response = self
                .http
                .get(&url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|error| {
                    SecretLoadError::new(
                        "gcp",
                        self.secret_version.clone(),
                        format!("failed to access secret version: {error}"),
                    )
                })?;

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(SecretLoadError::new(
                    "gcp",
                    self.secret_version.clone(),
                    format!(
                        "secret manager returned {} while accessing secret version: {}",
                        status.as_u16(),
                        body
                    ),
                ));
            }

            let payload: SecretAccessResponse = response.json().await.map_err(|error| {
                SecretLoadError::new(
                    "gcp",
                    self.secret_version.clone(),
                    format!("failed to decode secret manager response: {error}"),
                )
            })?;

            let encoded = payload
                .payload
                .and_then(|payload| payload.data)
                .ok_or_else(|| {
                    SecretLoadError::new(
                        "gcp",
                        self.secret_version.clone(),
                        "secret manager response did not include payload data",
                    )
                })?;

            let decoded = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| {
                    SecretLoadError::new(
                        "gcp",
                        self.secret_version.clone(),
                        format!("failed to decode secret payload: {error}"),
                    )
                })?;

            let mnemonic = String::from_utf8(decoded).map_err(|error| {
                SecretLoadError::new(
                    "gcp",
                    self.secret_version.clone(),
                    format!("secret payload is not utf8: {error}"),
                )
            })?;

            validate_mnemonic("gcp", &self.secret_version, &mnemonic)
        })
    }
}

#[derive(Debug, Deserialize)]
struct GcpTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct SecretAccessResponse {
    payload: Option<SecretAccessPayload>,
}

#[derive(Debug, Deserialize)]
struct SecretAccessPayload {
    data: Option<String>,
}

async fn gcp_access_token(http: &Client) -> Result<String, SecretLoadError> {
    let response = http
        .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .map_err(|error| {
            SecretLoadError::new(
                "gcp",
                "metadata.server",
                format!("failed to request metadata token: {error}"),
            )
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(SecretLoadError::new(
            "gcp",
            "metadata.server",
            format!(
                "metadata token endpoint returned {}: {}",
                status.as_u16(),
                body
            ),
        ));
    }

    let token: GcpTokenResponse = response.json().await.map_err(|error| {
        SecretLoadError::new(
            "gcp",
            "metadata.server",
            format!("failed to decode metadata token response: {error}"),
        )
    })?;

    if token.access_token.trim().is_empty() {
        return Err(SecretLoadError::new(
            "gcp",
            "metadata.server",
            "metadata token response did not include access token",
        ));
    }

    Ok(token.access_token)
}

fn validate_mnemonic(
    provider: &str,
    secret_ref: &str,
    value: &str,
) -> Result<String, SecretLoadError> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return Err(SecretLoadError::new(
            provider,
            secret_ref,
            "mnemonic secret resolved to empty value",
        ));
    }

    Mnemonic::parse_normalized(&normalized)
        .map_err(|_| SecretLoadError::new(provider, secret_ref, "mnemonic failed validation"))?;

    Ok(normalized)
}

pub fn provider_from_config(
    config: &WalletExecutorConfig,
    http: Client,
) -> Arc<dyn MnemonicSecretProvider> {
    match config.mnemonic_provider {
        MnemonicProvider::Env => {
            Arc::new(EnvMnemonicProvider::new(config.mnemonic_env_var.clone()))
        }
        MnemonicProvider::Gcp => Arc::new(GcpSecretManagerMnemonicProvider::new(
            config
                .mnemonic_secret_version
                .clone()
                .unwrap_or_else(|| "".to_string()),
            http,
        )),
    }
}
