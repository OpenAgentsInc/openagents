//! Google Secret Manager access after the broker authorizes a registered source.
//! Numeric versions prevent rotation from relabeling an existing handle.
use crate::{Failure, MAX_SECRET_BYTES, Secret, SecretProvider};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc, time::Duration};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Version(String);
impl Version {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        let p: Vec<_> = value.split('/').collect();
        fn number(s: &str) -> bool {
            s.parse::<u64>().is_ok_and(|n| n > 0 && n.to_string() == s)
        }
        if p.len() != 6
            || p[0] != "projects"
            || !number(p[1])
            || p[2] != "secrets"
            || p[3].is_empty()
            || p[3].len() > 255
            || !p[3]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || p[4] != "versions"
            || !number(p[5])
        {
            return Err("Use a numeric project and secret version.");
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Version {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<Version> for String {
    fn from(v: Version) -> Self {
        v.0
    }
}
impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Version(<registered source>)")
    }
}

#[async_trait]
pub trait AccessToken: Send + Sync {
    async fn token(&self) -> Result<Secret, Failure>;
}

/// Runs only on the secret-reading service. Workers and guests receive no
/// Google identity or Secret Manager permission through this reader.
pub struct MetadataToken {
    http: Client,
}
impl MetadataToken {
    pub fn new() -> Result<Self, Failure> {
        Ok(Self { http: client()? })
    }
}
#[async_trait]
impl AccessToken for MetadataToken {
    async fn token(&self) -> Result<Secret, Failure> {
        let response=self.http.get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor","Google").send().await.map_err(|_|Failure::Unavailable)?;
        if response.status() != 200
            || response
                .headers()
                .get("Metadata-Flavor")
                .and_then(|v| v.to_str().ok())
                != Some("Google")
        {
            return Err(Failure::Unavailable);
        }
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
            token_type: String,
            expires_in: u64,
        }
        let mut token: Token = serde_json::from_slice(&read(response, 32768).await?)
            .map_err(|_| Failure::Unavailable)?;
        if token.token_type != "Bearer"
            || token.expires_in < 30
            || token.access_token.len() > 8192
            || token.access_token.is_empty()
            || !token
                .access_token
                .bytes()
                .all(|b| (b'!'..=b'~').contains(&b))
        {
            token.access_token.zeroize();
            return Err(Failure::Unavailable);
        }
        Secret::new(std::mem::take(&mut token.access_token).into_bytes())
    }
}
fn client() -> Result<Client, Failure> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| Failure::Unavailable)
}
async fn read(mut response: Response, max: usize) -> Result<Zeroizing<Vec<u8>>, Failure> {
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(Failure::Unavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub struct GoogleSecretManager {
    http: Client,
    tokens: Arc<dyn AccessToken>,
    origin: String,
}
impl GoogleSecretManager {
    pub fn new(tokens: Arc<dyn AccessToken>) -> Result<Self, Failure> {
        Ok(Self {
            http: client()?,
            tokens,
            origin: "https://secretmanager.googleapis.com".into(),
        })
    }
    pub fn from_metadata() -> Result<Self, Failure> {
        Self::new(Arc::new(MetadataToken::new()?))
    }
    #[cfg(test)]
    fn local(tokens: Arc<dyn AccessToken>, origin: String) -> Result<Self, Failure> {
        let url = url::Url::parse(&origin).map_err(|_| Failure::Invalid)?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.path() != "/"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Failure::Invalid);
        }
        let mut reader = Self::new(tokens)?;
        reader.origin = origin.trim_end_matches('/').into();
        Ok(reader)
    }
}
#[async_trait]
impl SecretProvider for GoogleSecretManager {
    async fn access(&self, version: &Version) -> Result<Secret, Failure> {
        let token = self.tokens.token().await?;
        let token = std::str::from_utf8(token.bytes()).map_err(|_| Failure::Unavailable)?;
        let response = self
            .http
            .get(format!("{}/v1/{}:access", self.origin, version.as_str()))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)?;
        if response.status() != 200 {
            return Err(Failure::Unavailable);
        }
        let bytes = read(response, 100000).await?;
        #[derive(Deserialize)]
        struct Payload {
            data: String,
            #[serde(rename = "dataCrc32c")]
            crc: String,
        }
        impl Drop for Payload {
            fn drop(&mut self) {
                self.data.zeroize();
            }
        }
        #[derive(Deserialize)]
        struct Answer {
            name: String,
            payload: Payload,
        }
        let answer: Answer = serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)?;
        if answer.name != version.as_str()
            || answer.payload.data.len() > MAX_SECRET_BYTES.div_ceil(3) * 4
        {
            return Err(Failure::Unavailable);
        }
        let value = Zeroizing::new(
            STANDARD
                .decode(&answer.payload.data)
                .map_err(|_| Failure::Unavailable)?,
        );
        if answer.payload.crc.parse::<u32>().ok() != Some(crc32c::crc32c(&value)) {
            return Err(Failure::Unavailable);
        }
        Secret::new(value.to_vec()).map_err(|_| Failure::Unavailable)
    }
}

#[cfg(test)]
mod tests;
