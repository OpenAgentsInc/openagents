//! The object store every tenant shares. Objects are named by the SHA-256
//! of their bytes, so they are immutable and a shared object is stored once.

use std::path::PathBuf;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use tokio::sync::Mutex;

use crate::sha256_hex;

pub enum Store {
    /// A local directory: `<root>/objects/<hash>`.
    Dir(PathBuf),
    /// A Cloud Storage bucket: `gs://<bucket>/<prefix>/objects/<hash>`.
    Gcs(Gcs),
}

pub struct Gcs {
    bucket: String,
    prefix: String,
    client: reqwest::Client,
    /// An access token and when to stop using it.
    token: Mutex<Option<(String, Instant)>>,
}

impl Store {
    /// A directory path, or `gs://BUCKET[/PREFIX]` for Cloud Storage.
    pub fn parse(spec: &str) -> Result<Self> {
        if let Some(rest) = spec.strip_prefix("gs://") {
            let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
            if bucket.is_empty() {
                bail!("a gs:// store needs a bucket");
            }
            return Ok(Store::Gcs(Gcs {
                bucket: bucket.to_owned(),
                prefix: prefix.trim_matches('/').to_owned(),
                client: reqwest::Client::new(),
                token: Mutex::new(None),
            }));
        }
        let root = PathBuf::from(spec);
        std::fs::create_dir_all(root.join("objects"))
            .with_context(|| format!("creating store {}", root.display()))?;
        Ok(Store::Dir(root))
    }

    /// Reads an object and checks that its bytes match its name.
    pub async fn get(&self, hash: &str) -> Result<Vec<u8>> {
        check_name(hash)?;
        let bytes = match self {
            Store::Dir(root) => tokio::fs::read(root.join("objects").join(hash))
                .await
                .with_context(|| format!("object {hash} is not in the store"))?,
            Store::Gcs(gcs) => gcs.get(hash).await?,
        };
        let actual = sha256_hex(&bytes);
        if actual != hash {
            bail!("object {hash} hashes to {actual}");
        }
        Ok(bytes)
    }

    /// Stores an object unless it is already there. Returns whether bytes
    /// were written.
    pub async fn put(&self, bytes: &[u8]) -> Result<(String, bool)> {
        let hash = sha256_hex(bytes);
        let written = match self {
            Store::Dir(root) => {
                let path = root.join("objects").join(&hash);
                if tokio::fs::try_exists(&path).await? {
                    false
                } else {
                    let temp = root
                        .join("objects")
                        .join(format!(".{hash}.{}", std::process::id()));
                    tokio::fs::write(&temp, bytes).await?;
                    tokio::fs::rename(&temp, &path).await?;
                    true
                }
            }
            Store::Gcs(gcs) => gcs.put(&hash, bytes).await?,
        };
        Ok((hash, written))
    }
}

fn check_name(hash: &str) -> Result<()> {
    if hash.len() != 64 || !hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        bail!("{hash:?} is not an object name");
    }
    Ok(())
}

impl Gcs {
    fn name(&self, hash: &str) -> String {
        if self.prefix.is_empty() {
            format!("objects/{hash}")
        } else {
            format!("{}/objects/{hash}", self.prefix)
        }
    }

    /// An access token: from the metadata server on Cloud Run (`K_SERVICE`
    /// is set), else from `gcloud`, which reads `CLOUDSDK_CONFIG`. Tokens
    /// are reused until five minutes before they expire.
    async fn token(&self) -> Result<String> {
        let mut cached = self.token.lock().await;
        if let Some((token, until)) = cached.as_ref()
            && Instant::now() < *until
        {
            return Ok(token.clone());
        }
        let (token, lifetime) = if std::env::var_os("K_SERVICE").is_some() {
            self.metadata_token().await?
        } else {
            gcloud_token().await?
        };
        let until = Instant::now() + lifetime.saturating_sub(Duration::from_secs(300));
        *cached = Some((token.clone(), until));
        Ok(token)
    }

    async fn metadata_token(&self) -> Result<(String, Duration)> {
        #[derive(serde::Deserialize)]
        struct Token {
            access_token: String,
            expires_in: u64,
        }
        let token: Token = self
            .client
            .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google")
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("asking the metadata server for a token")?
            .error_for_status()?
            .json()
            .await?;
        Ok((token.access_token, Duration::from_secs(token.expires_in)))
    }

    async fn get(&self, hash: &str) -> Result<Vec<u8>> {
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
            self.bucket,
            encode(&self.name(hash))
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(self.token().await?)
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("reading gs object {hash}: {}", response.status());
        }
        Ok(response.bytes().await?.to_vec())
    }

    async fn put(&self, hash: &str, bytes: &[u8]) -> Result<bool> {
        // ifGenerationMatch=0 writes only when the object is absent, so an
        // object that is already stored costs one refused request.
        let url = format!(
            "https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&ifGenerationMatch=0&name={}",
            self.bucket,
            encode(&self.name(hash))
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(self.token().await?)
            .header("cache-control", "public, max-age=31536000, immutable")
            .body(bytes.to_vec())
            .send()
            .await?;
        match response.status().as_u16() {
            200 => Ok(true),
            412 => Ok(false),
            status => bail!("writing gs object {hash}: {status}"),
        }
    }
}

async fn gcloud_token() -> Result<(String, Duration)> {
    let output = tokio::process::Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .await
        .context("running gcloud")?;
    if !output.status.success() {
        bail!("gcloud auth print-access-token failed");
    }
    // gcloud doesn't say; its tokens last an hour.
    Ok((
        String::from_utf8(output.stdout)?.trim().to_owned(),
        Duration::from_secs(3600),
    ))
}

fn encode(name: &str) -> String {
    name.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dir_store_round_trips_and_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::parse(dir.path().to_str().unwrap()).unwrap();
        let (hash, written) = store.put(b"hello").await.unwrap();
        assert!(written);
        assert_eq!(store.put(b"hello").await.unwrap(), (hash.clone(), false));
        assert_eq!(store.get(&hash).await.unwrap(), b"hello");
        assert!(store.get("../escape").await.is_err());
        std::fs::write(dir.path().join("objects").join(&hash), b"tampered").unwrap();
        assert!(store.get(&hash).await.is_err());
    }
}
