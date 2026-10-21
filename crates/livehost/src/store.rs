//! The object store every tenant shares. Objects are named by the SHA-256
//! of their bytes, so they are immutable and a shared object is stored once.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use tokio::sync::OnceCell;

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
    token: OnceCell<String>,
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
                token: OnceCell::new(),
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

    /// An access token from `gcloud`, which reads `CLOUDSDK_CONFIG`.
    async fn token(&self) -> Result<&str> {
        self.token
            .get_or_try_init(|| async {
                let output = tokio::process::Command::new("gcloud")
                    .args(["auth", "print-access-token"])
                    .output()
                    .await
                    .context("running gcloud")?;
                if !output.status.success() {
                    bail!("gcloud auth print-access-token failed");
                }
                Ok(String::from_utf8(output.stdout)?.trim().to_owned())
            })
            .await
            .map(String::as_str)
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
