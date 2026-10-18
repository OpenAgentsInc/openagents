//! Sandbox file upload and download, built on the process API.
//!
//! Uploads pipe the file into `cat` inside the sandbox, writing to a temp
//! file that is atomically renamed into place. Downloads run a small
//! `dd`-based loop that returns the file in 512 KiB length-prefixed chunks on
//! request, so the reader controls the pace.

use std::pin::Pin;

use bytes::Bytes;
use futures_util::Stream;
use futures_util::StreamExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result, SandboxFileTransferError, TransferOperation};
use crate::process::{
    ProcessResult, ProcessStartOptions, ProcessStatus, ProcessStream, SandboxProcess,
};
use crate::sandbox::Sandbox;

const DOWNLOAD_CHUNK_BYTES: usize = 512 * 1024;
const UPLOAD_READ_BYTES: usize = 256 * 1024;

const UPLOAD_COMMAND: &str = r#"set -eu
mkdir -p "$ARCHIL_FILE_PARENT"
trap 'rm -f "$ARCHIL_FILE_TEMP"' EXIT HUP INT TERM
: > "$ARCHIL_FILE_TEMP"
cat > "$ARCHIL_FILE_TEMP"
chmod "$ARCHIL_FILE_MODE" "$ARCHIL_FILE_TEMP"
mv -f "$ARCHIL_FILE_TEMP" "$ARCHIL_FILE_TARGET"
trap - EXIT HUP INT TERM"#;

const DOWNLOAD_COMMAND: &str = r#"set -eu
trap 'rm -f "$ARCHIL_FILE_TEMP"' EXIT HUP INT TERM
exec 3< "$ARCHIL_FILE_PATH"
while IFS= read -r count; do
    dd bs="$count" count=1 <&3 > "$ARCHIL_FILE_TEMP" 2>/dev/null
    size=$(wc -c < "$ARCHIL_FILE_TEMP")
    printf '%s\n' "$size"
    cat "$ARCHIL_FILE_TEMP"
    [ "$size" -eq "$count" ] || break
done"#;

/// Where upload bytes come from.
pub enum FileSource {
    Bytes(Bytes),
    Reader(Pin<Box<dyn AsyncRead + Send>>),
    Stream(Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>),
}

impl FileSource {
    /// Upload from any async reader.
    pub fn reader(r: impl AsyncRead + Send + 'static) -> Self {
        Self::Reader(Box::pin(r))
    }
    /// Upload from a stream of chunks.
    pub fn stream(s: impl Stream<Item = std::io::Result<Bytes>> + Send + 'static) -> Self {
        Self::Stream(Box::pin(s))
    }
}

impl From<Bytes> for FileSource {
    fn from(b: Bytes) -> Self {
        Self::Bytes(b)
    }
}
impl From<Vec<u8>> for FileSource {
    fn from(b: Vec<u8>) -> Self {
        Self::Bytes(b.into())
    }
}
impl From<&'static [u8]> for FileSource {
    fn from(b: &'static [u8]) -> Self {
        Self::Bytes(Bytes::from_static(b))
    }
}
impl From<String> for FileSource {
    fn from(b: String) -> Self {
        Self::Bytes(b.into())
    }
}
impl From<&'static str> for FileSource {
    fn from(b: &'static str) -> Self {
        Self::Bytes(Bytes::from_static(b.as_bytes()))
    }
}

/// Options for [`SandboxFiles::upload_file`].
#[derive(Debug, Clone, Copy)]
pub struct SandboxFileUploadOptions {
    /// Permission bits (default `0o644`); at most `0o7777`.
    pub mode: u32,
}

impl Default for SandboxFileUploadOptions {
    fn default() -> Self {
        Self { mode: 0o644 }
    }
}

/// File operations on a sandbox, from [`Sandbox::files`].
#[derive(Debug, Clone)]
pub struct SandboxFiles {
    sandbox: Sandbox,
}

impl SandboxFiles {
    pub(crate) fn new(sandbox: Sandbox) -> Self {
        Self { sandbox }
    }

    /// Upload to an absolute file path inside the sandbox. Parent directories
    /// are created; the file appears atomically.
    pub async fn upload_file(
        &self,
        source: impl Into<FileSource>,
        remote_path: &str,
        options: SandboxFileUploadOptions,
    ) -> Result<()> {
        let (path, parent) = remote_file_path(remote_path)?;
        if options.mode > 0o7777 {
            return Err(Error::InvalidArgument(
                "mode must be between 0 and 0o7777".into(),
            ));
        }
        let start = ProcessStartOptions::new()
            .env("ARCHIL_FILE_PARENT", parent.clone())
            .env("ARCHIL_FILE_TARGET", path.clone())
            .env(
                "ARCHIL_FILE_TEMP",
                format!("{parent}/.archil-upload-{}", transfer_id()),
            )
            .env("ARCHIL_FILE_MODE", format!("{:o}", options.mode));
        let mut process = None;
        let outcome = async {
            let p = self.sandbox.run(UPLOAD_COMMAND, start).await?;
            let p = process.insert(p);
            send_source(p, source.into()).await?;
            p.close_stdin().await?;
            let result = p.wait().await?;
            assert_successful(&result, None)
        }
        .await;
        if let Some(p) = &process {
            if outcome.is_err() {
                let _ = p.kill().await;
            }
            p.disconnect().await;
        }
        outcome.map_err(|e| wrap(TransferOperation::Upload, &path, e))
    }

    /// Download an absolute file path, streaming chunks into `writer`.
    pub async fn download_file<W: AsyncWrite + Unpin>(
        &self,
        remote_path: &str,
        writer: &mut W,
    ) -> Result<()> {
        let mut download = self.download(remote_path).await?;
        while let Some(chunk) = download.next_chunk().await? {
            if let Err(e) = writer.write_all(&chunk).await {
                let err = wrap(
                    TransferOperation::Download,
                    &download.path,
                    Error::process(e.to_string()),
                );
                download.abort().await;
                return Err(err);
            }
        }
        Ok(())
    }

    /// Download a whole file into memory.
    pub async fn download_bytes(&self, remote_path: &str) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.download_file(remote_path, &mut out).await?;
        Ok(out)
    }

    /// Start a chunked download and pull chunks yourself with
    /// [`FileDownload::next_chunk`].
    pub async fn download(&self, remote_path: &str) -> Result<FileDownload> {
        let (path, _) = remote_file_path(remote_path)?;
        let start = ProcessStartOptions::new()
            .env("ARCHIL_FILE_PATH", path.clone())
            .env(
                "ARCHIL_FILE_TEMP",
                format!("/tmp/.archil-download-{}", transfer_id()),
            )
            .collect_output(false)
            .stream_output(true);
        let process = self
            .sandbox
            .run(DOWNLOAD_COMMAND, start)
            .await
            .map_err(|e| wrap(TransferOperation::Download, &path, e))?;
        Ok(FileDownload {
            process,
            path,
            buffer: Vec::new(),
            cursor: 0,
            stderr: String::new(),
            last: false,
            finished: false,
        })
    }
}

/// An in-progress chunked download.
#[derive(Debug)]
pub struct FileDownload {
    process: SandboxProcess,
    path: String,
    buffer: Vec<u8>,
    cursor: u64,
    stderr: String,
    last: bool,
    finished: bool,
}

impl FileDownload {
    /// The next chunk (up to 512 KiB), or `None` at end of file.
    pub async fn next_chunk(&mut self) -> Result<Option<Bytes>> {
        if self.finished {
            return Ok(None);
        }
        match self.next_inner().await {
            Ok(chunk) => {
                if chunk.is_none() {
                    self.finished = true;
                    self.process.disconnect().await;
                }
                Ok(chunk)
            }
            Err(e) => {
                self.abort().await;
                Err(wrap(TransferOperation::Download, &self.path, e))
            }
        }
    }

    async fn abort(&mut self) {
        self.finished = true;
        let _ = self.process.kill().await;
        self.process.disconnect().await;
    }

    async fn next_inner(&mut self) -> Result<Option<Bytes>> {
        if self.last {
            self.finish().await?;
            return Ok(None);
        }
        if let Err(e) = self
            .process
            .send_input(format!("{DOWNLOAD_CHUNK_BYTES}\n"))
            .await
        {
            // The reader can exit before the next request (a missing file);
            // report its exit and stderr rather than the closed stdin.
            if self.process.status() != ProcessStatus::Running {
                loop {
                    self.fill().await?;
                }
            }
            return Err(e);
        }
        let line = self.read_line().await?;
        let size_text = String::from_utf8_lossy(&line).trim().to_string();
        let size: usize = match size_text.parse() {
            Ok(n) if n <= DOWNLOAD_CHUNK_BYTES => n,
            _ => {
                return Err(Error::process(format!(
                    "Sandbox returned an invalid chunk size: {size_text:?}"
                )));
            }
        };
        if size == 0 {
            self.finish().await?;
            return Ok(None);
        }
        let data = self.read(size).await?;
        if size < DOWNLOAD_CHUNK_BYTES {
            self.last = true;
        }
        Ok(Some(data))
    }

    async fn finish(&mut self) -> Result<()> {
        let result = self.process.wait().await?;
        assert_successful(&result, Some(&self.stderr))
    }

    async fn fill(&mut self) -> Result<()> {
        match self.process.next_output().await {
            Some(output) => {
                if output.offset != self.cursor {
                    return Err(Error::process(format!(
                        "Sandbox process output gap: expected offset {}, received {}",
                        self.cursor, output.offset
                    )));
                }
                self.cursor += output.data.len() as u64;
                match output.stream {
                    ProcessStream::Stdout => self.buffer.extend_from_slice(&output.data),
                    ProcessStream::Stderr => {
                        self.stderr.push_str(&String::from_utf8_lossy(&output.data))
                    }
                }
                Ok(())
            }
            None => {
                let result = self.process.wait().await?;
                assert_successful(&result, Some(&self.stderr))?;
                Err(Error::process(
                    "Sandbox process exited before file transfer completed",
                ))
            }
        }
    }

    async fn read_line(&mut self) -> Result<Vec<u8>> {
        loop {
            if let Some(i) = self.buffer.iter().position(|b| *b == b'\n') {
                let line = self.buffer[..i].to_vec();
                self.buffer.drain(..=i);
                return Ok(line);
            }
            self.fill().await?;
        }
    }

    async fn read(&mut self, size: usize) -> Result<Bytes> {
        while self.buffer.len() < size {
            self.fill().await?;
        }
        Ok(Bytes::from(self.buffer.drain(..size).collect::<Vec<u8>>()))
    }
}

async fn send_source(process: &SandboxProcess, source: FileSource) -> Result<()> {
    match source {
        FileSource::Bytes(b) => process.send_input(&b).await,
        FileSource::Reader(mut r) => {
            let mut buf = vec![0u8; UPLOAD_READ_BYTES];
            loop {
                let n = r
                    .read(&mut buf)
                    .await
                    .map_err(|e| Error::process(e.to_string()))?;
                if n == 0 {
                    return Ok(());
                }
                process.send_input(&buf[..n]).await?;
            }
        }
        FileSource::Stream(mut s) => {
            while let Some(chunk) = s.next().await {
                process
                    .send_input(&chunk.map_err(|e| Error::process(e.to_string()))?)
                    .await?;
            }
            Ok(())
        }
    }
}

fn wrap(operation: TransferOperation, path: &str, error: Error) -> Error {
    match error {
        Error::InvalidArgument(_) => error,
        cause => SandboxFileTransferError {
            operation,
            path: path.to_string(),
            cause: Box::new(cause),
        }
        .into(),
    }
}

fn remote_file_path(path: &str) -> Result<(String, String)> {
    if path.contains('\0') {
        return Err(Error::InvalidArgument(
            "remote path cannot contain NUL".into(),
        ));
    }
    if !path.starts_with('/') || path.ends_with('/') {
        return Err(Error::InvalidArgument(
            "remote path must be an absolute file path".into(),
        ));
    }
    let parent = match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => path[..i].to_string(),
    };
    Ok((path.to_string(), parent))
}

fn assert_successful(result: &ProcessResult, stderr: Option<&str>) -> Result<()> {
    if result.status == ProcessStatus::Completed && result.exit_code == Some(0) {
        return Ok(());
    }
    let stderr = stderr.unwrap_or(&result.stderr).trim();
    let message = if !stderr.is_empty() {
        stderr.to_string()
    } else if let Some(reason) = result.exit_reason.as_deref().filter(|r| !r.is_empty()) {
        reason.to_string()
    } else {
        match result.exit_code {
            Some(c) => format!("exit code {c}"),
            None => "exit code undefined".to_string(),
        }
    };
    Err(Error::process(message))
}

fn transfer_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{millis:x}-{}", uuid::Uuid::new_v4().simple())
}
