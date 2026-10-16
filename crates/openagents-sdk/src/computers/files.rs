use super::*;
use sha2::{Digest, Sha256};

/// A recovery record contains the expected hash, not the file's private bytes.
#[derive(Clone, Serialize, Deserialize)]
pub struct WriteIntent {
    v: String,
    computer: ComputerRef,
    path: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    overwrite: bool,
}
impl WriteIntent {
    /// Choose this before sending; never change a write being recovered.
    pub fn allow_overwrite(mut self) -> Self {
        self.overwrite = true;
        self
    }
    pub fn overwrites(&self) -> bool {
        self.overwrite
    }
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}
pub struct File {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub etag: String,
}
#[derive(Clone)]
pub struct WriteReceipt {
    pub intent: WriteIntent,
    pub observed_matching: bool,
}

fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn path_valid(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}
impl Computers<'_> {
    pub fn write_intent(
        &self,
        computer: &ComputerRef,
        path: &str,
        bytes: &[u8],
    ) -> Result<WriteIntent, Error> {
        self.check(computer)?;
        if !path_valid(path) {
            return Err(Error::InvalidInput);
        }
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        Ok(WriteIntent {
            v: "openagents.computers.file-write.v1".into(),
            computer: computer.clone(),
            path: path.into(),
            size: bytes.len() as u64,
            sha256: checksum(bytes),
            overwrite: false,
        })
    }
    async fn file_limit(&self, path: &str, cancel: &CancellationToken) -> Result<usize, Error> {
        if !path_valid(path) {
            return Err(Error::InvalidInput);
        }
        let files = self
            .capabilities(cancel)
            .await?
            .files
            .ok_or(Error::Unsupported)?;
        if files.max_path_bytes <= 0
            || path.len() > files.max_path_bytes as usize
            || files.max_bytes <= 0
        {
            return Err(Error::Unsupported);
        }
        Ok((files.max_bytes as usize).min(4 * 1024 * 1024))
    }
    pub async fn read_file(
        &self,
        computer: &ComputerRef,
        path: &str,
        cancel: &CancellationToken,
    ) -> Result<File, Error> {
        self.check(computer)?;
        let limit = self.file_limit(path, cancel).await?;
        let plan = generated::ReadComputerFileParams {
            id: computer.id.clone(),
            path: path.into(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let reply = self.request(plan, cancel).await?;
        if reply.bytes.len() > limit {
            return Err(Error::TooLarge);
        }
        let expected = reply
            .headers
            .get("X-Content-SHA256")
            .and_then(|v| v.to_str().ok())
            .ok_or(Error::InvalidResponse)?;
        let digest = checksum(&reply.bytes);
        if digest != expected {
            return Err(Error::InvalidResponse);
        }
        let etag = reply
            .headers
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .ok_or(Error::InvalidResponse)?
            .to_owned();
        if etag != format!("\"{digest}\"") {
            return Err(Error::InvalidResponse);
        }
        if !reply
            .headers
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|s| {
                s.split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("no-store"))
            })
        {
            return Err(Error::InvalidResponse);
        }
        Ok(File {
            bytes: reply.bytes,
            sha256: digest,
            etag,
        })
    }
    /// Send once. A lost write response requires read/hash reconciliation;
    /// it never triggers another upload, including on a 429 or 503.
    pub async fn write_file(
        &self,
        intent: &WriteIntent,
        bytes: &[u8],
        cancel: &CancellationToken,
    ) -> Result<WriteReceipt, Error> {
        self.check(&intent.computer)?;
        if intent.v != "openagents.computers.file-write.v1"
            || bytes.len() as u64 != intent.size
            || checksum(bytes) != intent.sha256
        {
            return Err(Error::InvalidInput);
        }
        let limit = self.file_limit(&intent.path, cancel).await?;
        if bytes.len() > limit {
            return Err(Error::TooLarge);
        }
        let plan = generated::WriteComputerFileParams {
            id: intent.computer.id.clone(),
            path: intent.path.clone(),
            content_length: intent.size,
            x_content_sha256: intent.sha256.clone(),
            body: bytes.to_vec(),
            overwrite: Some(intent.overwrite),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let result: generated::AppComputerFileInfo = self.json(plan, cancel).await?;
        if result.size != intent.size || result.sha256 != intent.sha256 {
            return Err(Error::Uncertain {
                operation: "writeComputerFile",
                key: None,
                cause: Box::new(Error::InvalidResponse),
            });
        }
        Ok(WriteReceipt {
            intent: intent.clone(),
            observed_matching: false,
        })
    }
    /// Observe matching bytes after an uncertain upload. This does not claim
    /// which writer produced them or submit another write.
    pub async fn reconcile_write(
        &self,
        intent: &WriteIntent,
        cancel: &CancellationToken,
    ) -> Result<Option<WriteReceipt>, Error> {
        self.check(&intent.computer)?;
        if intent.v != "openagents.computers.file-write.v1"
            || !path_valid(&intent.path)
            || intent.size > 4 * 1024 * 1024
            || intent.sha256.len() != 64
            || !intent
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::InvalidInput);
        }
        let result = self
            .read_file(&intent.computer, &intent.path, cancel)
            .await?;
        if result.bytes.len() as u64 == intent.size && result.sha256 == intent.sha256 {
            Ok(Some(WriteReceipt {
                intent: intent.clone(),
                observed_matching: true,
            }))
        } else {
            Ok(None)
        }
    }
}
opaque_debug!(WriteIntent, File, WriteReceipt);
