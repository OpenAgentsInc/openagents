//! Read-only access to CAD's existing chat-bucket layout and checked copies.
use crate::{Error, Result, backend::Backend, require, sha};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[async_trait]
pub trait LegacySource: Send + Sync {
    async fn read(&self, key: &str) -> Result<Vec<u8>>;
    async fn keys(&self, prefix: &str) -> Result<Vec<String>>;
}

/// Explicit fixture/export root. No account home or production bucket is inferred.
pub struct DirectorySource {
    root: PathBuf,
}
impl DirectorySource {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        require(
            root.as_ref().is_absolute(),
            "Choose an absolute source folder.",
        )?;
        let root = std::fs::canonicalize(root)?;
        require(root.is_dir(), "Choose a source folder.")?;
        Ok(Self { root })
    }
    fn path(&self, key: &str) -> Result<PathBuf> {
        let mut path = self.root.clone();
        for segment in key.trim_end_matches('/').split('/') {
            component(segment)?;
            path.push(segment);
            let meta = std::fs::symlink_metadata(&path)?;
            require(
                !meta.file_type().is_symlink(),
                "Choose a source without file shortcuts.",
            )?;
        }
        Ok(path)
    }
}
#[async_trait]
impl LegacySource for DirectorySource {
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.path(key)?;
        let metadata = std::fs::metadata(&path)?;
        require(
            metadata.is_file() && metadata.len() <= crate::backend::MAX_OBJECT_BYTES as u64,
            "Choose an ordinary file within the lab's size limit.",
        )?;
        let bytes = std::fs::read(path)?;
        require(
            bytes.len() <= crate::backend::MAX_OBJECT_BYTES,
            "This saved file is too large.",
        )?;
        Ok(bytes)
    }
    async fn keys(&self, prefix: &str) -> Result<Vec<String>> {
        let mut pending = vec![(self.path(prefix)?, prefix.trim_end_matches('/').to_owned())];
        let mut keys = vec![];
        while let Some((folder, key)) = pending.pop() {
            for entry in std::fs::read_dir(folder)? {
                let entry = entry?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| Error::Corrupt)?;
                component(&name)?;
                let kind = entry.file_type()?;
                let key = format!("{key}/{name}");
                if kind.is_dir() {
                    pending.push((entry.path(), key));
                } else {
                    require(kind.is_file(), "Choose a source without file shortcuts.")?;
                    keys.push(key);
                }
                require(
                    keys.len() + pending.len() <= 100_000,
                    "This source has too many files.",
                )?;
            }
        }
        keys.sort();
        Ok(keys)
    }
}
fn component(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= 256
            && value != "."
            && value != ".."
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b)),
        "Choose a valid CAD file address.",
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LegacyIndex {
    pub schema: String,
    pub docs: BTreeMap<String, LegacyDocument>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LegacyDocument {
    pub title: String,
    #[serde(default)]
    pub kind: String,
    pub versions: u64,
    #[serde(default)]
    pub history: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopyFile {
    pub key: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopyReport {
    pub scope: String,
    pub documents: usize,
    pub versions: u64,
    pub files: Vec<CopyFile>,
    pub total_bytes: u64,
}
impl CopyReport {
    pub fn checksum(&self) -> Result<String> {
        Ok(lab_kit::digest(&serde_json::to_value(self)?))
    }
}

pub struct CadLegacy {
    source: Arc<dyn LegacySource>,
    prefix: String,
}
impl CadLegacy {
    pub fn personal(source: Arc<dyn LegacySource>, owner: &str) -> Result<Self> {
        component(owner)?;
        Ok(Self {
            source,
            prefix: format!("{owner}/cad/me"),
        })
    }
    pub fn project(source: Arc<dyn LegacySource>, owner: &str, project: &str) -> Result<Self> {
        component(owner)?;
        component(project)?;
        Ok(Self {
            source,
            prefix: format!("{owner}/cad/p-{project}"),
        })
    }
    pub async fn index(&self) -> Result<LegacyIndex> {
        let bytes = self
            .source
            .read(&format!("{}/index.json", self.prefix))
            .await?;
        require(
            bytes.len() <= 2 * 1024 * 1024,
            "This CAD index is too large.",
        )?;
        let index: LegacyIndex = serde_json::from_slice(&bytes)?;
        require(
            index.schema == "openagents.web.cad-index.v1" && index.docs.len() <= 100,
            "This CAD index uses an unsupported format.",
        )?;
        for (id, entry) in &index.docs {
            component(id)?;
            require(
                (1..=300).contains(&entry.versions),
                "This CAD document has an invalid version count.",
            )?;
        }
        Ok(index)
    }
    pub async fn version(&self, document: &str, number: u64) -> Result<(Value, Value)> {
        component(document)?;
        let index = self.index().await?;
        let doc = index.docs.get(document).ok_or(Error::NotFound)?;
        if number == 0 || number > doc.versions {
            return Err(Error::NotFound);
        }
        let prefix = format!("{}/{document}/v{number}", self.prefix);
        let content = self.source.read(&format!("{prefix}/doc.json")).await?;
        require(
            content.len() <= 512 * 1024,
            "This CAD document is too large.",
        )?;
        let record = self.source.read(&format!("{prefix}/version.json")).await?;
        Ok((
            serde_json::from_slice(&content)?,
            serde_json::from_slice(&record)?,
        ))
    }
    pub async fn dry_run(&self) -> Result<CopyReport> {
        let index = self.index().await?;
        let mut versions = 0;
        for (id, doc) in &index.docs {
            for n in 1..=doc.versions {
                self.version(id, n).await?;
                versions += 1;
            }
        }
        let keys = self.source.keys(&format!("{}/", self.prefix)).await?;
        let mut files = vec![];
        let mut total_bytes = 0u64;
        for key in keys {
            require(
                key.starts_with(&format!("{}/", self.prefix)),
                "This file belongs to a different CAD space.",
            )?;
            let bytes = self.source.read(&key).await?;
            total_bytes = total_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(Error::Corrupt)?;
            files.push(CopyFile {
                key,
                sha256: sha(&bytes),
                bytes: bytes.len() as u64,
            });
        }
        files.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(CopyReport {
            scope: self.prefix.clone(),
            documents: index.docs.len(),
            versions,
            files,
            total_bytes,
        })
    }
    /// Copy only after reviewing the dry-run checksum. Source files are never changed.
    /// Repeating a copy verifies the same immutable objects and manifest.
    pub async fn copy(&self, target: &dyn Backend, expected_checksum: &str) -> Result<CopyReport> {
        let report = self.dry_run().await?;
        if report.checksum()? != expected_checksum {
            return Err(Error::Conflict);
        }
        let namespace =
            lab_kit::digest(&json!({"cad_copy":self.prefix,"report":expected_checksum}));
        for file in &report.files {
            let bytes = self.source.read(&file.key).await?;
            if sha(&bytes) != file.sha256 || bytes.len() as u64 != file.bytes {
                return Err(Error::Conflict);
            }
            target.put(&namespace, &file.sha256, bytes).await?;
            if sha(&target.get(&namespace, &file.sha256, None).await?) != file.sha256 {
                return Err(Error::Corrupt);
            }
        }
        if self.dry_run().await? != report {
            return Err(Error::Conflict);
        }
        let bytes = serde_json::to_vec(&report)?;
        if !target
            .compare_and_swap(&namespace, 0, bytes.clone())
            .await?
        {
            let (_, existing) = target.load(&namespace).await?;
            if existing
                .as_deref()
                .map(serde_json::from_slice::<Value>)
                .transpose()?
                != Some(serde_json::to_value(&report)?)
            {
                return Err(Error::Conflict);
            }
        }
        Ok(report)
    }
}
