//! Labs storage over the website's durable, conditional-write object store.

use lab_store::{Error, Result, backend::Backend};
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};

pub(super) struct ChatBackend(pub Arc<crate::chat_store::Store>);

#[derive(Serialize, Deserialize)]
struct Index {
    generation: u64,
    bytes: Vec<u8>,
}

fn key(space: &str, file: &str) -> Result<String> {
    if space.len() != 64
        || !space.bytes().all(|b| b.is_ascii_hexdigit())
        || (!matches!(file, "index.json")
            && (file.len() != 64 || !file.bytes().all(|b| b.is_ascii_hexdigit())))
    {
        return Err(Error::Invalid("Choose a valid storage address.".into()));
    }
    Ok(format!("labs/{space}/{file}"))
}

#[async_trait::async_trait]
impl Backend for ChatBackend {
    async fn load(&self, space: &str) -> Result<(u64, Option<Vec<u8>>)> {
        match self
            .0
            .read_key(&key(space, "index.json")?)
            .await
            .map_err(Error::storage)?
        {
            Some((bytes, _)) => {
                let i: Index = serde_json::from_slice(&bytes)?;
                Ok((i.generation, Some(i.bytes)))
            }
            None => Ok((0, None)),
        }
    }
    async fn compare_and_swap(&self, space: &str, generation: u64, bytes: Vec<u8>) -> Result<bool> {
        let path = key(space, "index.json")?;
        let current = self.0.read_key(&path).await.map_err(Error::storage)?;
        let old = current
            .as_ref()
            .map(|(b, _)| serde_json::from_slice::<Index>(b))
            .transpose()?;
        if old.as_ref().map_or(0, |i| i.generation) != generation {
            return Ok(false);
        }
        let bytes = serde_json::to_vec(&Index {
            generation: generation.checked_add(1).ok_or(Error::Corrupt)?,
            bytes,
        })?;
        match self
            .0
            .write_key(&path, bytes, current.as_ref().map(|(_, g)| g.as_str()))
            .await
        {
            Ok(_) => Ok(true),
            Err(crate::chat_store::Error::Conflict) => Ok(false),
            Err(e) => Err(Error::storage(e)),
        }
    }
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()> {
        if crate::chat_store::digest(&bytes) != hash {
            return Err(Error::Corrupt);
        }
        let path = key(space, hash)?;
        match self.0.write_key(&path, bytes, None).await {
            Ok(_) | Err(crate::chat_store::Error::Conflict) => Ok(()),
            Err(e) => Err(Error::storage(e)),
        }
    }
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>> {
        let (bytes, _) = self
            .0
            .read_key(&key(space, hash)?)
            .await
            .map_err(Error::storage)?
            .ok_or(Error::NotFound)?;
        if crate::chat_store::digest(&bytes) != hash {
            return Err(Error::Corrupt);
        }
        match range {
            Some(r) => bytes
                .get(r.start as usize..r.end as usize)
                .map(<[u8]>::to_vec)
                .ok_or(Error::NotFound),
            None => Ok(bytes),
        }
    }
}
