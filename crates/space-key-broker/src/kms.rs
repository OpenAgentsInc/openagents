//! Cloud KMS REST calls with authenticated context and integrity checks.
use crate::{Failure, Kms};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Client, Response};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
use tokio::{sync::Mutex, time::Instant};
use zeroize::Zeroizing;

pub fn valid_key_name(name: &str) -> bool {
    let parts: Vec<_> = name.split('/').collect();
    parts.len() == 8
        && parts[0] == "projects"
        && parts[2] == "locations"
        && parts[4] == "keyRings"
        && parts[6] == "cryptoKeys"
        && [1, 3, 5, 7].iter().all(|i| {
            !parts[*i].is_empty()
                && ![".", ".."].contains(&parts[*i])
                && parts[*i].len() <= 128
                && parts[*i]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
struct Token {
    value: Zeroizing<String>,
    until: Instant,
}
pub struct GoogleKms {
    http: Client,
    routes: BTreeMap<String, String>,
    token: Mutex<Option<Token>>,
}
impl GoogleKms {
    pub fn new(routes: BTreeMap<String, String>) -> Result<Self, Failure> {
        if routes.is_empty() || routes.values().any(|key| !valid_key_name(key)) {
            return Err(Failure::Unavailable);
        }
        let http = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self {
            http,
            routes,
            token: Mutex::new(None),
        })
    }
    async fn token(&self) -> Result<Zeroizing<String>, Failure> {
        let mut cache = self.token.lock().await;
        if let Some(token) = cache.as_ref().filter(|token| Instant::now() < token.until) {
            return Ok(Zeroizing::new(token.value.to_string()));
        }
        let response=self.http.get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor","Google").send().await.map_err(|_|Failure::Unavailable)?;
        if response.status() != 200
            || response
                .headers()
                .get("Metadata-Flavor")
                .and_then(|h| h.to_str().ok())
                != Some("Google")
        {
            return Err(Failure::Unavailable);
        }
        let body = read(response).await?;
        if body["token_type"] != "Bearer" {
            return Err(Failure::Unavailable);
        }
        let token = body["access_token"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 8192)
            .ok_or(Failure::Unavailable)?;
        let lifetime = body["expires_in"]
            .as_u64()
            .filter(|s| *s > 30)
            .ok_or(Failure::Unavailable)?
            .min(3600)
            - 30;
        let value = Zeroizing::new(token.to_string());
        *cache = Some(Token {
            value: Zeroizing::new(value.to_string()),
            until: Instant::now() + Duration::from_secs(lifetime),
        });
        Ok(value)
    }
    async fn call(&self, route: &str, action: &str, body: Value) -> Result<Value, Failure> {
        let key = self.routes.get(route).ok_or(Failure::Unavailable)?;
        let token = self.token().await?;
        let response = self
            .http
            .post(format!("https://cloudkms.googleapis.com/v1/{key}:{action}"))
            .bearer_auth(token.as_str())
            .json(&body)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)?;
        if action == "decrypt" && response.status() == 400 {
            return Err(Failure::InvalidKey);
        }
        if !response.status().is_success() {
            return Err(Failure::Unavailable);
        }
        read(response).await
    }
}
async fn read(mut response: Response) -> Result<Value, Failure> {
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
        if bytes.len() + chunk.len() > 65536 {
            return Err(Failure::Unavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)
}
fn crc(bytes: &[u8]) -> String {
    crc32c::crc32c(bytes).to_string()
}
fn checksum(value: &Value, bytes: &[u8]) -> bool {
    value
        .as_str()
        .and_then(|v| v.parse::<u32>().ok())
        .or_else(|| value.as_u64().and_then(|v| u32::try_from(v).ok()))
        == Some(crc32c::crc32c(bytes))
}
fn encrypted(body: &Value, key: &str) -> Result<Vec<u8>, Failure> {
    if body["verifiedPlaintextCrc32c"] != true
        || body["verifiedAdditionalAuthenticatedDataCrc32c"] != true
        || !body["name"].as_str().is_some_and(|name| {
            name.strip_prefix(&format!("{key}/cryptoKeyVersions/"))
                .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        })
    {
        return Err(Failure::Unavailable);
    }
    let ciphertext = BASE64
        .decode(body["ciphertext"].as_str().ok_or(Failure::Unavailable)?)
        .map_err(|_| Failure::Unavailable)?;
    if ciphertext.is_empty() || !checksum(&body["ciphertextCrc32c"], &ciphertext) {
        return Err(Failure::Unavailable);
    }
    Ok(ciphertext)
}
fn decrypted(body: &Value) -> Result<Zeroizing<Vec<u8>>, Failure> {
    let plain = Zeroizing::new(
        BASE64
            .decode(body["plaintext"].as_str().ok_or(Failure::Unavailable)?)
            .map_err(|_| Failure::Unavailable)?,
    );
    if plain.len() != 32 || !checksum(&body["plaintextCrc32c"], &plain) {
        return Err(Failure::Unavailable);
    }
    Ok(plain)
}
#[async_trait]
impl Kms for GoogleKms {
    async fn encrypt(&self, route: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, Failure> {
        let encoded = Zeroizing::new(BASE64.encode(plain));
        let body=self.call(route,"encrypt",json!({"plaintext":encoded.as_str(),"additionalAuthenticatedData":BASE64.encode(aad),"plaintextCrc32c":crc(plain),"additionalAuthenticatedDataCrc32c":crc(aad)})).await?;
        encrypted(&body, self.routes.get(route).ok_or(Failure::Unavailable)?)
    }
    async fn decrypt(
        &self,
        route: &str,
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, Failure> {
        let body=self.call(route,"decrypt",json!({"ciphertext":BASE64.encode(ciphertext),"additionalAuthenticatedData":BASE64.encode(aad),"ciphertextCrc32c":crc(ciphertext),"additionalAuthenticatedDataCrc32c":crc(aad)})).await?;
        decrypted(&body)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integrity_checks_reject_wrong_keys_missing_acknowledgements_and_corruption() {
        let key = "projects/fixture/locations/us-central1/keyRings/space/cryptoKeys/intermediate";
        let cipher = b"ciphertext";
        let valid = json!({"name":format!("{key}/cryptoKeyVersions/1"),"verifiedPlaintextCrc32c":true,"verifiedAdditionalAuthenticatedDataCrc32c":true,"ciphertext":BASE64.encode(cipher),"ciphertextCrc32c":crc(cipher)});
        assert!(encrypted(&valid, key).is_ok());
        for field in [
            "name",
            "verifiedPlaintextCrc32c",
            "verifiedAdditionalAuthenticatedDataCrc32c",
            "ciphertextCrc32c",
        ] {
            let mut changed = valid.clone();
            changed[field] = Value::Null;
            assert_eq!(encrypted(&changed, key), Err(Failure::Unavailable));
        }
        assert_eq!(
            encrypted(&valid, &key.replace("fixture", "another")),
            Err(Failure::Unavailable)
        );
        assert_eq!(crc32c::crc32c(b"123456789"), 0xe3069283);
    }
    #[test]
    fn plaintext_must_have_the_right_size_and_checksum() {
        let plain = [9u8; 32];
        let valid = json!({"plaintext":BASE64.encode(plain),"plaintextCrc32c":crc(&plain)});
        assert!(decrypted(&valid).is_ok());
        let mut changed = valid;
        changed["plaintextCrc32c"] = json!(0);
        assert!(decrypted(&changed).is_err());
        assert!(
            decrypted(
                &json!({"plaintext":BASE64.encode([0u8;31]),"plaintextCrc32c":crc(&[0u8;31])})
            )
            .is_err()
        );
    }
}
