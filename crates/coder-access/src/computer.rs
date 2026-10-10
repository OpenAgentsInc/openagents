//! The owner's own computer as a device reaches it (`computer`): a picture
//! of its screen, its running apps, and files copied to and from it.
//!
//! Every request needs the `terminal` right, because a device that can open
//! a shell on the computer can already do each of these; the operation only
//! does them without a terminal, with bounds, and with every byte checked.
//!
//! - `screenshot` captures one screen (or an attached Android device's
//!   screen) into a PNG the host keeps for a while and answers its
//!   [`FileInfo`]; the device then reads it like any file.
//! - `apps` lists the windows open on the computer's screen.
//! - `stat` answers a regular file's size and SHA-256 digest, and `read`
//!   one chunk of it at an offset. A device may keep several reads in
//!   flight ([`WINDOW`]), puts the chunks together in order, and checks the
//!   digest of the whole against `stat`'s, so a file that changed while it
//!   was read is refused rather than delivered torn.
//! - `write` sends one chunk of a file, each naming the whole file's size
//!   and digest. The host keeps the chunks beside the destination in any
//!   order they arrive, checks the digest once every one is there, and only
//!   then puts the file in place. It never replaces an existing file unless
//!   the request says `overwrite`. A chunk is idempotent: sending one again
//!   changes nothing, and the answer says how many bytes the host holds
//!   from the start, so a device resumes there. A host that takes chunks
//!   only in order answers the same way, so [`send_many`] works with both.
//!
//! A path is absolute or starts with `~/` (the host user's home). A file
//! is at most [`MAX_FILE_BYTES`] and a chunk at most [`CHUNK_BYTES`], small
//! enough that the signed, encrypted request stays far below every
//! NIP-HOST binding's message bound.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Code, Error, Result};

/// The bytes of every chunk but a file's last.
pub const CHUNK_BYTES: u64 = 32 * 1024;
/// The most bytes of one file copied either way.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// The most bytes of one screenshot.
pub const MAX_SCREENSHOT_BYTES: u64 = 64 * 1024 * 1024;
/// The longest path, in bytes.
pub const MAX_PATH: usize = 4096;
/// The most apps one answer lists.
pub const MAX_APPS: usize = 512;
/// The longest app name or window title an answer carries, in characters.
pub const MAX_LABEL: usize = 512;
/// The longest screen name or Android serial, in bytes.
pub const MAX_NAME: usize = 128;

/// One `computer` request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Capture a screen into a PNG on the host; answered by
    /// [`Answer::File`].
    Screenshot { source: Source },
    /// The windows open on the computer's screen; answered by
    /// [`Answer::Apps`].
    Apps {},
    /// A regular file's size and digest; answered by [`Answer::File`].
    Stat { path: String },
    /// At most [`CHUNK_BYTES`] of a file from `offset`; answered by
    /// [`Answer::Chunk`].
    Read {
        path: String,
        offset: u64,
        length: u64,
    },
    /// One chunk of a file on its way to the host; answered by
    /// [`Answer::Written`].
    Write { put: FilePut },
}

/// Which screen a screenshot captures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    /// The computer's own screen: the named one (a Wayland output, an X11
    /// display, or a macOS display number), or the main one.
    Screen { screen: Option<String> },
    /// An Android device attached to the computer over `adb`: the named
    /// serial, or the only one attached.
    Android { serial: Option<String> },
}

/// One chunk of a file a device sends (`write`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePut {
    /// Where the file goes on the host.
    pub path: String,
    /// The whole file's size.
    pub size: u64,
    /// The whole file's `sha256:<hex>` digest.
    pub digest: String,
    /// Where this chunk starts: a multiple of [`CHUNK_BYTES`].
    pub offset: u64,
    /// The chunk's bytes, standard base64 with padding.
    pub data: String,
    /// Replace a file already at `path`. Without it the host refuses an
    /// existing file as `conflict`.
    pub overwrite: bool,
}

/// A device's answer to a `computer` request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    /// A file on the host: what `stat` read, or the screenshot taken.
    File { file: FileInfo },
    /// The windows open on the computer's screen, and what listed them.
    Apps { apps: Vec<App>, source: String },
    /// Bytes of a file from `offset`; fewer than asked only at its end.
    Chunk { offset: u64, data: String },
    /// What the host holds of a file after a `write` chunk.
    Written { received: u64, complete: bool },
    /// The host could not do it, in a sentence a person reads: no screen
    /// session, no capture tool, no such file. Authority refusals stay
    /// NIP-HOST refusal codes; this is the computer's own answer.
    Unable { reason: String },
}

/// The longest [`Answer::Unable`] reason, in characters.
pub const MAX_REASON: usize = 512;

impl Answer {
    /// This answer, or the host's reason as an error.
    ///
    /// # Errors
    /// [`Answer::Unable`] as `unavailable` with its reason.
    pub fn able(self) -> Result<Self> {
        match self {
            Self::Unable { reason } => Err(Error::new(Code::Unavailable, reason)),
            other => Ok(other),
        }
    }
}

/// A regular file on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    /// The absolute path the host read.
    pub path: String,
    pub size: u64,
    /// `sha256:<hex>`.
    pub digest: String,
    /// `image/png` for a screenshot, else none.
    pub media_type: Option<String>,
}

/// One window open on the computer's screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// The program: its app-id, class, or application name.
    pub name: String,
    /// The window's title, when the desktop gives one. A title is content.
    pub title: Option<String>,
    pub pid: Option<i64>,
    pub focused: bool,
}

/// The `sha256:<hex>` digest of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_of(&Sha256::digest(bytes)))
}

/// The `sha256:<hex>` form of a finished hasher.
#[must_use]
pub fn digest_of(hasher: Sha256) -> String {
    format!("sha256:{}", hex_of(&hasher.finalize()))
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The 64 lowercase hex digits of a `sha256:` digest.
///
/// # Errors
/// Refuses any other form.
pub fn digest_hex(digest: &str) -> Result<&str> {
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| Error::new(Code::Malformed, "file digest must be sha256"))?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::new(Code::Malformed, "file digest must be 64 hex"));
    }
    Ok(hex)
}

/// Check a path a device names: absolute, or under `~/`, without a NUL,
/// and at most [`MAX_PATH`] bytes.
///
/// # Errors
/// Refuses any other path.
pub fn path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > MAX_PATH || path.contains('\0') {
        return Err(Error::new(Code::Malformed, "path is empty or too long"));
    }
    let absolute = path.starts_with('/')
        || path == "~"
        || path.starts_with("~/")
        || (path.len() > 2
            && path.as_bytes()[1] == b':'
            && path.as_bytes()[0].is_ascii_alphabetic());
    if !absolute {
        return Err(Error::new(
            Code::Malformed,
            "path must be absolute or start with ~/",
        ));
    }
    Ok(())
}

fn name(value: Option<&String>) -> Result<()> {
    match value {
        Some(value)
            if value.is_empty()
                || value.len() > MAX_NAME
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b)) =>
        {
            Err(Error::new(
                Code::Malformed,
                "screen or serial name is malformed",
            ))
        }
        _ => Ok(()),
    }
}

fn label(text: &str) -> Result<()> {
    if text.chars().count() > MAX_LABEL {
        return Err(Error::new(Code::Bounds, "app label exceeds its bound"));
    }
    Ok(())
}

impl Request {
    /// # Errors
    /// Refuses a request outside its bounds.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Screenshot { source } => match source {
                Source::Screen { screen } => name(screen.as_ref()),
                Source::Android { serial } => name(serial.as_ref()),
            },
            Self::Apps {} => Ok(()),
            Self::Stat { path: p } => path(p),
            Self::Read {
                path: p,
                offset,
                length,
            } => {
                path(p)?;
                if *length == 0 || *length > CHUNK_BYTES || *offset >= MAX_FILE_BYTES {
                    return Err(Error::new(Code::Bounds, "read exceeds its bound"));
                }
                Ok(())
            }
            Self::Write { put } => put.validate(),
        }
    }
}

impl FilePut {
    /// # Errors
    /// Refuses a chunk outside its bounds or of the wrong length.
    pub fn validate(&self) -> Result<()> {
        self.bytes().map(|_| ())
    }

    /// The chunk's bytes, after every bound is checked.
    ///
    /// # Errors
    /// As [`FilePut::validate`].
    pub fn bytes(&self) -> Result<Vec<u8>> {
        path(&self.path)?;
        digest_hex(&self.digest)?;
        if self.size > MAX_FILE_BYTES {
            return Err(Error::new(Code::Bounds, "file size exceeds its bound"));
        }
        let expected = if self.size == 0 {
            if self.offset != 0 {
                return Err(Error::new(Code::Malformed, "chunk offset is out of place"));
            }
            0
        } else {
            if !self.offset.is_multiple_of(CHUNK_BYTES) || self.offset >= self.size {
                return Err(Error::new(Code::Malformed, "chunk offset is out of place"));
            }
            CHUNK_BYTES.min(self.size - self.offset)
        };
        if self.data.len() as u64 > CHUNK_BYTES.div_ceil(3) * 4 {
            return Err(Error::new(Code::Bounds, "chunk exceeds its bound"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|_| Error::new(Code::Malformed, "chunk is not base64"))?;
        if bytes.len() as u64 != expected {
            return Err(Error::new(Code::Malformed, "chunk has the wrong length"));
        }
        Ok(bytes)
    }
}

impl Answer {
    /// # Errors
    /// Refuses an answer outside its bounds.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::File { file } => {
                digest_hex(&file.digest)?;
                if file.path.is_empty() || file.path.len() > MAX_PATH {
                    return Err(Error::new(Code::Malformed, "file path is malformed"));
                }
                if file
                    .media_type
                    .as_ref()
                    .is_some_and(|media| media.len() > 64)
                {
                    return Err(Error::new(Code::Malformed, "media type is malformed"));
                }
                Ok(())
            }
            Self::Apps { apps, source } => {
                if apps.len() > MAX_APPS || source.len() > 64 {
                    return Err(Error::new(Code::Bounds, "app list exceeds its bound"));
                }
                for app in apps {
                    label(&app.name)?;
                    if let Some(title) = &app.title {
                        label(title)?;
                    }
                }
                Ok(())
            }
            Self::Chunk { data, .. } => {
                if data.len() as u64 > CHUNK_BYTES.div_ceil(3) * 4 {
                    return Err(Error::new(Code::Bounds, "chunk exceeds its bound"));
                }
                Ok(())
            }
            Self::Written { .. } => Ok(()),
            Self::Unable { reason } => {
                if reason.chars().count() > MAX_REASON {
                    return Err(Error::new(Code::Bounds, "reason exceeds its bound"));
                }
                Ok(())
            }
        }
    }

    /// Whether this answer is the kind `request` asks for.
    #[must_use]
    pub fn answers(&self, request: &Request) -> bool {
        match (request, self) {
            (Request::Screenshot { .. } | Request::Stat { .. }, Self::File { .. })
            | (Request::Apps {}, Self::Apps { .. })
            | (_, Self::Unable { .. }) => true,
            (Request::Read { offset, .. }, Self::Chunk { offset: at, .. }) => offset == at,
            (Request::Write { put }, Self::Written { received, .. }) => *received <= put.size,
            _ => false,
        }
    }
}

/// Chunk calls a device keeps in flight at once with [`fetch_many`] and
/// [`send_many`]. A chunk call otherwise waits out a whole round trip; with
/// eight in flight the link carries chunks while earlier answers travel.
pub const WINDOW: usize = 8;

/// Several `computer` requests sent together, each answered in its place:
/// the answer list is as long as the request list and in its order. A
/// client runs them concurrently over one link; answering them one at a
/// time is correct too, only slower.
pub type Batch<'a> = dyn FnMut(Vec<Request>) -> Vec<Result<Answer>> + 'a;

/// A [`Batch`] over a one-request `call`: each request in turn, and once
/// one fails the rest are not sent.
pub fn one_at_a_time<'a>(
    call: &'a mut dyn FnMut(Request) -> Result<Answer>,
) -> impl FnMut(Vec<Request>) -> Vec<Result<Answer>> + 'a {
    move |requests| {
        let mut failed = false;
        requests
            .into_iter()
            .map(|request| {
                if failed {
                    return Err(Error::new(Code::Transport, "an earlier chunk failed"));
                }
                let answer = call(request);
                failed = answer.is_err();
                answer
            })
            .collect()
    }
}

/// Read the file at `path` on the host with `call`, chunk by chunk, into
/// `sink`, refusing a file over `limit` bytes before reading any of it.
/// Answers the file's [`FileInfo`] once every byte matched its digest.
///
/// # Errors
/// The host's refusal, a file over `limit`, a short or misplaced chunk, or
/// a digest that differs (the file changed while it was read).
pub fn fetch(
    call: &mut dyn FnMut(Request) -> Result<Answer>,
    path: &str,
    limit: u64,
    sink: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<FileInfo> {
    fetch_many(&mut one_at_a_time(call), path, limit, sink, progress)
}

/// Read a file the host already described, as [`fetch`] does.
///
/// # Errors
/// As [`fetch`].
pub fn fetch_described(
    call: &mut dyn FnMut(Request) -> Result<Answer>,
    file: &FileInfo,
    limit: u64,
    sink: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    fetch_described_many(&mut one_at_a_time(call), file, limit, sink, progress)
}

/// [`fetch`] with up to [`WINDOW`] chunk reads in flight at once. The
/// chunks still reach `sink` in order and are checked against the digest
/// as one file.
///
/// # Errors
/// As [`fetch`], or a batch that answers fewer requests than it was sent.
pub fn fetch_many(
    batch: &mut Batch<'_>,
    path: &str,
    limit: u64,
    sink: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<FileInfo> {
    let described = batch(vec![Request::Stat { path: path.into() }])
        .pop()
        .ok_or_else(|| Error::new(Code::Malformed, "the host did not describe the file"))??;
    let Answer::File { file } = described.able()? else {
        return Err(Error::new(
            Code::Malformed,
            "the host did not describe the file",
        ));
    };
    fetch_described_many(batch, &file, limit, sink, progress)?;
    Ok(file)
}

/// Read a file the host already described, as [`fetch_many`] does.
///
/// # Errors
/// As [`fetch_many`].
pub fn fetch_described_many(
    batch: &mut Batch<'_>,
    file: &FileInfo,
    limit: u64,
    sink: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    if file.size > limit.min(MAX_FILE_BYTES) {
        return Err(Error::new(
            Code::Bounds,
            format!(
                "{} is {} bytes, over the {} byte limit",
                file.path,
                file.size,
                limit.min(MAX_FILE_BYTES)
            ),
        ));
    }
    let mut hasher = Sha256::new();
    let mut offset = 0;
    while offset < file.size {
        let mut requests = Vec::with_capacity(WINDOW);
        let mut at = offset;
        while at < file.size && requests.len() < WINDOW {
            let length = CHUNK_BYTES.min(file.size - at);
            requests.push(Request::Read {
                path: file.path.clone(),
                offset: at,
                length,
            });
            at += length;
        }
        let sent = requests.len();
        let answers = batch(requests);
        if answers.len() != sent {
            return Err(Error::new(
                Code::Malformed,
                "the host did not answer every chunk",
            ));
        }
        for answer in answers {
            let length = CHUNK_BYTES.min(file.size - offset);
            let Answer::Chunk { offset: got, data } = answer?.able()? else {
                return Err(Error::new(
                    Code::Malformed,
                    "the host did not answer a chunk",
                ));
            };
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&data)
                .map_err(|_| Error::new(Code::Malformed, "chunk is not base64"))?;
            if got != offset || bytes.len() as u64 != length {
                return Err(Error::new(
                    Code::Conflict,
                    format!("{} changed while it was read", file.path),
                ));
            }
            hasher.update(&bytes);
            sink.write_all(&bytes)
                .map_err(|e| Error::new(Code::Unavailable, format!("could not write: {e}")))?;
            offset += length;
            progress(offset, file.size);
        }
    }
    if digest_of(hasher) != file.digest {
        return Err(Error::new(
            Code::Conflict,
            format!(
                "{} changed while it was read; its digest differs",
                file.path
            ),
        ));
    }
    Ok(())
}

/// Send `bytes` to `path` on the host with `call`, chunk by chunk,
/// resuming where the host says it holds. Answers the digest the host
/// checked.
///
/// # Errors
/// The host's refusal (an existing file without `overwrite` is
/// `conflict`), or bytes over [`MAX_FILE_BYTES`].
pub fn send(
    call: &mut dyn FnMut(Request) -> Result<Answer>,
    path: &str,
    bytes: &[u8],
    overwrite: bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<String> {
    send_many(&mut one_at_a_time(call), path, bytes, overwrite, progress)
}

/// [`send`] with up to [`WINDOW`] chunks in flight at once.
///
/// Each round sends the chunks from the first one the host may not hold.
/// A host takes them in any order and answers how many bytes it holds from
/// the start; the next round starts there. A host that takes chunks only
/// in order still moves on at least one chunk a round, so a device never
/// needs to know which kind it reached.
///
/// # Errors
/// As [`send`], or a batch that answers fewer requests than it was sent.
pub fn send_many(
    batch: &mut Batch<'_>,
    path: &str,
    bytes: &[u8],
    overwrite: bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<String> {
    let size = bytes.len() as u64;
    if size > MAX_FILE_BYTES {
        return Err(Error::new(
            Code::Bounds,
            format!("the file is {size} bytes, over the {MAX_FILE_BYTES} byte limit"),
        ));
    }
    let digest = digest(bytes);
    // An empty file is one empty chunk.
    let chunks = size.div_ceil(CHUNK_BYTES).max(1);
    let put = |index: u64| {
        let start = usize::try_from(index * CHUNK_BYTES)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let end = usize::try_from((index + 1) * CHUNK_BYTES)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        FilePut {
            path: path.into(),
            size,
            digest: digest.clone(),
            offset: index * CHUNK_BYTES,
            data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
            overwrite,
        }
    };
    // The first chunk the host may not hold.
    let mut next = 0;
    // A host that answers the same place again without progress is
    // refused rather than looped on.
    let mut stalls = 0;
    loop {
        let end = (next + WINDOW as u64).min(chunks);
        let requests: Vec<Request> = (next..end)
            .map(|index| Request::Write { put: put(index) })
            .collect();
        let sent = requests.len();
        let answers = batch(requests);
        if answers.len() != sent {
            return Err(Error::new(
                Code::Malformed,
                "the host did not answer every chunk",
            ));
        }
        let mut held = 0;
        let mut failed = None;
        for answer in answers {
            match answer.and_then(Answer::able) {
                Ok(Answer::Written { complete: true, .. }) => {
                    progress(size, size);
                    return Ok(digest);
                }
                Ok(Answer::Written { received, .. }) => held = held.max(received.min(size)),
                Ok(_) => {
                    return Err(Error::new(
                        Code::Malformed,
                        "the host did not answer the chunk",
                    ));
                }
                Err(error) => {
                    failed.get_or_insert(error);
                }
            }
        }
        if let Some(error) = failed {
            return Err(error);
        }
        progress(held, size);
        let reached = (held / CHUNK_BYTES).min(chunks - 1);
        if reached <= next {
            stalls += 1;
            if stalls > 3 {
                return Err(Error::new(
                    Code::Unavailable,
                    "the host stopped taking the file",
                ));
            }
        } else {
            stalls = 0;
        }
        next = reached;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_must_be_absolute_or_home() {
        assert!(path("/tmp/a").is_ok());
        assert!(path("~/a").is_ok());
        assert!(path("~").is_ok());
        assert!(path("C:\\Users\\a").is_ok());
        assert!(path("a/b").is_err());
        assert!(path("").is_err());
        assert!(path("/a\0b").is_err());
        assert!(path(&format!("/{}", "a".repeat(MAX_PATH))).is_err());
    }

    #[test]
    fn a_chunk_has_its_exact_length_and_place() {
        let bytes = vec![7u8; (CHUNK_BYTES + 10) as usize];
        let put = |offset: u64, data: &[u8]| FilePut {
            path: "/tmp/x".into(),
            size: bytes.len() as u64,
            digest: digest(&bytes),
            offset,
            data: base64::engine::general_purpose::STANDARD.encode(data),
            overwrite: false,
        };
        assert!(put(0, &bytes[..CHUNK_BYTES as usize]).validate().is_ok());
        assert!(
            put(CHUNK_BYTES, &bytes[CHUNK_BYTES as usize..])
                .validate()
                .is_ok()
        );
        assert!(put(1, &bytes[..CHUNK_BYTES as usize]).validate().is_err());
        assert!(put(0, &bytes[..10]).validate().is_err());
        let mut big = put(0, &bytes[..CHUNK_BYTES as usize]);
        big.size = MAX_FILE_BYTES + 1;
        assert_eq!(big.validate().unwrap_err().code, Code::Bounds);
        let empty = FilePut {
            path: "/tmp/e".into(),
            size: 0,
            digest: digest(b""),
            offset: 0,
            data: String::new(),
            overwrite: false,
        };
        assert!(empty.validate().is_ok());
    }

    #[test]
    fn reads_and_names_are_bounded() {
        let read = |length| Request::Read {
            path: "/a".into(),
            offset: 0,
            length,
        };
        assert!(read(CHUNK_BYTES).validate().is_ok());
        assert!(read(CHUNK_BYTES + 1).validate().is_err());
        assert!(read(0).validate().is_err());
        let shot = |screen: &str| Request::Screenshot {
            source: Source::Screen {
                screen: Some(screen.into()),
            },
        };
        assert!(shot("DP-2").validate().is_ok());
        assert!(shot(":0").validate().is_ok());
        assert!(shot("a; rm -rf /").validate().is_err());
    }

    /// A host in memory: the same chunk rules the real host keeps.
    struct Memory {
        files: std::collections::BTreeMap<String, Vec<u8>>,
        partial: Vec<u8>,
        drop_every: usize,
        calls: usize,
    }

    impl Memory {
        fn call(&mut self, request: Request) -> Result<Answer> {
            request.validate()?;
            self.calls += 1;
            match request {
                Request::Stat { path } => {
                    let bytes = self
                        .files
                        .get(&path)
                        .ok_or_else(|| Error::new(Code::Unavailable, "no such file"))?;
                    Ok(Answer::File {
                        file: FileInfo {
                            path,
                            size: bytes.len() as u64,
                            digest: digest(bytes),
                            media_type: None,
                        },
                    })
                }
                Request::Read {
                    path,
                    offset,
                    length,
                } => {
                    let bytes = &self.files[&path];
                    let start = (offset as usize).min(bytes.len());
                    let end = (start + length as usize).min(bytes.len());
                    Ok(Answer::Chunk {
                        offset,
                        data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
                    })
                }
                Request::Write { put } => {
                    let data = put.bytes()?;
                    if put.offset == 0 && self.files.contains_key(&put.path) && !put.overwrite {
                        return Err(Error::new(Code::Conflict, "exists"));
                    }
                    if put.offset == self.partial.len() as u64 {
                        self.partial.extend_from_slice(&data);
                    }
                    let received = self.partial.len() as u64;
                    if received == put.size {
                        if digest(&self.partial) != put.digest {
                            self.partial.clear();
                            return Err(Error::new(Code::Conflict, "digest differs"));
                        }
                        self.files
                            .insert(put.path.clone(), std::mem::take(&mut self.partial));
                        return Ok(Answer::Written {
                            received,
                            complete: true,
                        });
                    }
                    // Lose some replies: the device resends, and the
                    // host's held count moves it on.
                    if self.drop_every > 0 && self.calls.is_multiple_of(self.drop_every) {
                        return Ok(Answer::Written {
                            received: received.saturating_sub(CHUNK_BYTES),
                            complete: false,
                        });
                    }
                    Ok(Answer::Written {
                        received,
                        complete: false,
                    })
                }
                _ => Err(Error::new(Code::Unsupported, "not here")),
            }
        }
    }

    #[test]
    fn a_file_round_trips_with_its_digest_and_an_existing_one_is_kept() {
        let mut host = Memory {
            files: Default::default(),
            partial: Vec::new(),
            drop_every: 3,
            calls: 0,
        };
        let bytes: Vec<u8> = (0..(5 * CHUNK_BYTES + 123))
            .map(|i| (i % 251) as u8)
            .collect();
        let mut call = |request| host.call(request);
        let sent = send(&mut call, "/tmp/f", &bytes, false, &mut |_, _| {}).unwrap();
        assert_eq!(sent, digest(&bytes));
        let mut back = Vec::new();
        let info = fetch(
            &mut call,
            "/tmp/f",
            MAX_FILE_BYTES,
            &mut back,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(back, bytes);
        assert_eq!(info.digest, digest(&bytes));
        let refused = send(&mut call, "/tmp/f", b"other", false, &mut |_, _| {}).unwrap_err();
        assert_eq!(refused.code, Code::Conflict);
        send(&mut call, "/tmp/f", b"other", true, &mut |_, _| {}).unwrap();
        let mut small = Vec::new();
        fetch(
            &mut call,
            "/tmp/f",
            MAX_FILE_BYTES,
            &mut small,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(small, b"other");
        let over = fetch(&mut call, "/tmp/f", 3, &mut Vec::new(), &mut |_, _| {}).unwrap_err();
        assert_eq!(over.code, Code::Bounds);
        send(&mut call, "/tmp/empty", b"", false, &mut |_, _| {}).unwrap();
    }

    /// Run a batch the way a link that delivers the requests in reverse
    /// does, answering each in its own place.
    fn reversed(
        host: &mut dyn FnMut(Request) -> Result<Answer>,
        requests: Vec<Request>,
    ) -> Vec<Result<Answer>> {
        let mut answers: Vec<Option<Result<Answer>>> = requests.iter().map(|_| None).collect();
        for (place, request) in requests.into_iter().enumerate().rev() {
            answers[place] = Some(host(request));
        }
        answers.into_iter().map(Option::unwrap).collect()
    }

    /// A host that takes chunks in any order, as `coder-host` does: it
    /// answers how many bytes it holds from the start.
    #[derive(Default)]
    struct AnyOrder {
        files: std::collections::BTreeMap<String, Vec<u8>>,
        pieces: std::collections::BTreeMap<u64, Vec<u8>>,
    }

    impl AnyOrder {
        fn call(&mut self, request: Request) -> Result<Answer> {
            request.validate()?;
            match request {
                Request::Write { put } => {
                    let data = put.bytes()?;
                    if self.pieces.is_empty()
                        && self.files.contains_key(&put.path)
                        && !put.overwrite
                    {
                        return Err(Error::new(Code::Conflict, "exists"));
                    }
                    self.pieces.insert(put.offset, data);
                    let mut held = 0;
                    while let Some(piece) = self.pieces.get(&held) {
                        if piece.is_empty() {
                            break;
                        }
                        held += piece.len() as u64;
                    }
                    let whole = held == put.size || (put.size == 0 && self.pieces.contains_key(&0));
                    if !whole {
                        return Ok(Answer::Written {
                            received: held,
                            complete: false,
                        });
                    }
                    let bytes: Vec<u8> = std::mem::take(&mut self.pieces)
                        .into_values()
                        .flatten()
                        .collect();
                    if digest(&bytes) != put.digest {
                        return Err(Error::new(Code::Conflict, "digest differs"));
                    }
                    self.files.insert(put.path, bytes);
                    Ok(Answer::Written {
                        received: put.size,
                        complete: true,
                    })
                }
                Request::Stat { path } => {
                    let bytes = &self.files[&path];
                    Ok(Answer::File {
                        file: FileInfo {
                            path,
                            size: bytes.len() as u64,
                            digest: digest(bytes),
                            media_type: None,
                        },
                    })
                }
                Request::Read {
                    path,
                    offset,
                    length,
                } => {
                    let bytes = &self.files[&path];
                    let start = (offset as usize).min(bytes.len());
                    let end = (start + length as usize).min(bytes.len());
                    Ok(Answer::Chunk {
                        offset,
                        data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
                    })
                }
                _ => Err(Error::new(Code::Unsupported, "not here")),
            }
        }
    }

    #[test]
    fn a_window_of_chunks_lands_whole_in_any_order_in_few_rounds() {
        let mut host = AnyOrder::default();
        let bytes: Vec<u8> = (0..(20 * CHUNK_BYTES + 9))
            .map(|i| (i % 241) as u8)
            .collect();
        let rounds = std::cell::Cell::new(0);
        let mut batch = |requests: Vec<Request>| {
            rounds.set(rounds.get() + 1);
            reversed(&mut |request| host.call(request), requests)
        };
        let sent = send_many(&mut batch, "/tmp/w", &bytes, false, &mut |_, _| {}).unwrap();
        assert_eq!(sent, digest(&bytes));
        let mut back = Vec::new();
        let file = fetch_many(
            &mut batch,
            "/tmp/w",
            MAX_FILE_BYTES,
            &mut back,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(back, bytes);
        assert_eq!(file.digest, digest(&bytes));
        // 21 chunks go in three rounds of eight; the stat and 21 reads in
        // one and three.
        assert_eq!(rounds.get(), 3 + 1 + 3);
        // The window still refuses a file already there without overwrite.
        let refused = send_many(&mut batch, "/tmp/w", b"other", false, &mut |_, _| {});
        assert_eq!(refused.unwrap_err().code, Code::Conflict);
        send_many(&mut batch, "/tmp/empty", b"", false, &mut |_, _| {}).unwrap();
    }

    #[test]
    fn a_host_that_takes_chunks_only_in_order_still_gets_the_whole_file() {
        let mut host = Memory {
            files: Default::default(),
            partial: Vec::new(),
            drop_every: 0,
            calls: 0,
        };
        let bytes: Vec<u8> = (0..(10 * CHUNK_BYTES + 1))
            .map(|i| (i % 239) as u8)
            .collect();
        // Delivered in reverse, such a host keeps one chunk a round.
        let mut batch =
            |requests: Vec<Request>| reversed(&mut |request| host.call(request), requests);
        let sent = send_many(&mut batch, "/tmp/o", &bytes, false, &mut |_, _| {}).unwrap();
        assert_eq!(sent, digest(&bytes));
        assert_eq!(host.files["/tmp/o"], bytes);
    }

    #[test]
    fn a_batch_that_drops_answers_is_refused() {
        let mut short = |requests: Vec<Request>| {
            let mut answers: Vec<Result<Answer>> = requests
                .iter()
                .map(|_| {
                    Ok(Answer::Written {
                        received: 0,
                        complete: false,
                    })
                })
                .collect();
            answers.pop();
            answers
        };
        let bytes = vec![1u8; (2 * CHUNK_BYTES) as usize];
        let error = send_many(&mut short, "/tmp/s", &bytes, false, &mut |_, _| {}).unwrap_err();
        assert_eq!(error.code, Code::Malformed);
    }
}
