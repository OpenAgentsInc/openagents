use bytes::Bytes;
use reqwest::{Client, Url};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Object {
    pub bytes: Bytes,
    pub content_type: String,
    pub etag: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Storage is unavailable. Try again.")]
pub struct StorageError;

#[derive(Clone)]
pub struct Bucket(Backend);

#[derive(Clone)]
enum Backend {
    Memory(Arc<Mutex<HashMap<String, Object>>>),
    Google {
        name: String,
        client: Client,
        token: Arc<Mutex<Option<(String, Instant)>>>,
    },
}

impl Bucket {
    pub fn memory() -> Self {
        Self(Backend::Memory(Arc::default()))
    }

    pub fn google(name: String) -> Result<Self, StorageError> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b))
        {
            return Err(StorageError);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| StorageError)?;
        Ok(Self(Backend::Google {
            name,
            client,
            token: Arc::default(),
        }))
    }

    async fn bearer(&self) -> Result<String, StorageError> {
        let Backend::Google { client, token, .. } = &self.0 else {
            return Err(StorageError);
        };
        let mut token = token.lock().await;
        if let Some((value, until)) = &*token {
            if *until > Instant::now() {
                return Ok(value.clone());
            }
        }
        #[derive(serde::Deserialize)]
        struct Token {
            access_token: String,
            expires_in: u64,
        }
        let fresh: Token = client.get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google").send().await.map_err(|_| StorageError)?
            .error_for_status().map_err(|_| StorageError)?.json().await.map_err(|_| StorageError)?;
        let until = Instant::now() + Duration::from_secs(fresh.expires_in.saturating_sub(60));
        *token = Some((fresh.access_token.clone(), until));
        Ok(fresh.access_token)
    }

    fn url(bucket: &str, key: &str, upload: bool) -> Result<Url, StorageError> {
        let prefix = if upload {
            "https://storage.googleapis.com/upload/storage/v1/b/"
        } else {
            "https://storage.googleapis.com/storage/v1/b/"
        };
        let mut url = Url::parse(prefix).map_err(|_| StorageError)?;
        let mut segments = url.path_segments_mut().map_err(|_| StorageError)?;
        segments.pop_if_empty().push(bucket).push("o");
        if !upload {
            segments.push(key);
        }
        drop(segments);
        if upload {
            url.query_pairs_mut()
                .append_pair("uploadType", "media")
                .append_pair("name", key);
        }
        Ok(url)
    }

    pub async fn put(
        &self,
        key: &str,
        bytes: Bytes,
        content_type: &str,
    ) -> Result<(), StorageError> {
        match &self.0 {
            Backend::Memory(objects) => {
                objects.lock().await.insert(
                    key.into(),
                    Object {
                        bytes,
                        content_type: content_type.into(),
                        etag: uuid::Uuid::new_v4().to_string(),
                    },
                );
            }
            Backend::Google { name, client, .. } => {
                client
                    .post(Self::url(name, key, true)?)
                    .bearer_auth(self.bearer().await?)
                    .header("content-type", content_type)
                    .body(bytes)
                    .send()
                    .await
                    .map_err(|_| StorageError)?
                    .error_for_status()
                    .map_err(|_| StorageError)?;
            }
        }
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<Object>, StorageError> {
        match &self.0 {
            Backend::Memory(objects) => Ok(objects.lock().await.get(key).cloned()),
            Backend::Google { name, client, .. } => {
                let mut url = Self::url(name, key, false)?;
                url.query_pairs_mut().append_pair("alt", "media");
                let response = client
                    .get(url)
                    .bearer_auth(self.bearer().await?)
                    .send()
                    .await
                    .map_err(|_| StorageError)?;
                if response.status() == 404 {
                    return Ok(None);
                }
                let response = response.error_for_status().map_err(|_| StorageError)?;
                let content_type = response
                    .headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                let etag = response
                    .headers()
                    .get("etag")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                Ok(Some(Object {
                    bytes: response.bytes().await.map_err(|_| StorageError)?,
                    content_type,
                    etag,
                }))
            }
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match &self.0 {
            Backend::Memory(objects) => {
                objects.lock().await.remove(key);
            }
            Backend::Google { name, client, .. } => {
                let response = client
                    .delete(Self::url(name, key, false)?)
                    .bearer_auth(self.bearer().await?)
                    .send()
                    .await
                    .map_err(|_| StorageError)?;
                if response.status() != 404 {
                    response.error_for_status().map_err(|_| StorageError)?;
                }
            }
        }
        Ok(())
    }

    pub async fn probe(&self) -> Result<(), StorageError> {
        let key = format!("_health/{}", uuid::Uuid::new_v4());
        self.put(&key, Bytes::from_static(b"ready"), "text/plain")
            .await?;
        let result = self.get(&key).await;
        self.delete(&key).await?;
        match result? {
            Some(o) if o.bytes == b"ready"[..] => Ok(()),
            _ => Err(StorageError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_keys_are_one_encoded_segment() {
        let url = Bucket::url("test-bucket", "account/a?b#c", false).unwrap();
        assert!(url.as_str().ends_with("/o/account%2Fa%3Fb%23c"));
        assert!(url.query().is_none());
    }
    #[tokio::test]
    async fn fake_bucket_round_trip_and_probe() {
        let bucket = Bucket::memory();
        bucket.probe().await.unwrap();
        bucket
            .put("account/file", Bytes::from_static(b"image"), "image/png")
            .await
            .unwrap();
        let object = bucket.get("account/file").await.unwrap().unwrap();
        assert_eq!(object.bytes, b"image"[..]);
        assert_eq!(object.content_type, "image/png");
        assert!(bucket.get("other/file").await.unwrap().is_none());
        bucket.delete("account/file").await.unwrap();
        assert!(bucket.get("account/file").await.unwrap().is_none());
    }
}
