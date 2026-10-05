//! A pinned file fetched over HTTPS and kept in a content-addressed cache.
//!
//! These are the Ruins loader's rules in a reusable form: HTTPS only, no
//! redirects, an exact length and SHA-256, a bounded transfer, cancellation,
//! and an atomic, verified cache install that never follows symbolic links.
//! Cleanup removes only this file's named earlier revisions and its own
//! abandoned temporary files.

#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use sha2::{Digest, Sha256};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);
#[cfg(unix)]
const STALE_TEMP_SECONDS: i64 = 24 * 60 * 60;

/// One reviewed file and where to fetch it.
#[derive(Clone, Debug)]
pub struct PinnedFile {
    /// Names the file in error messages, such as `Everglade pack`.
    pub label: &'static str,
    /// Lowercase hexadecimal SHA-256.
    pub sha256: &'static str,
    /// Exact length in bytes. Zero means the file is not published yet.
    pub bytes: u64,
    /// The HTTPS source.
    pub url: String,
    /// The cache file extension, without a dot.
    pub extension: &'static str,
    /// The cache temporary prefix, such as `.everglade-`.
    pub temp_prefix: &'static str,
    /// Earlier reviewed digests that cleanup may remove.
    pub history: &'static [&'static str],
}

impl PinnedFile {
    fn error(&self, message: &str) -> String {
        format!("{} {message}", self.label)
    }

    /// The cache entry's file name.
    pub fn cache_name(&self) -> String {
        format!("{}.{}", self.sha256, self.extension)
    }

    /// Checks the exact length and digest.
    pub fn verify(&self, bytes: &[u8]) -> Result<(), String> {
        if self.bytes == 0 {
            return Err(self.error("is not published yet"));
        }
        if bytes.len() as u64 != self.bytes {
            return Err(self.error("has an unexpected size"));
        }
        if format!("{:x}", Sha256::digest(bytes)) != self.sha256 {
            return Err(self.error("failed its content check"));
        }
        Ok(())
    }

    /// Reads and verifies a local copy without following a symbolic link.
    #[cfg(unix)]
    pub fn read_bounded(&self, path: &Path) -> Result<Vec<u8>, String> {
        use std::os::unix::fs::OpenOptionsExt;
        // Inspect the opened file, not a path checked before a racing open.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| self.error("cache could not be opened"))?;
        let metadata = file
            .metadata()
            .map_err(|_| self.error("cache is unavailable"))?;
        if !metadata.is_file() || metadata.len() != self.bytes {
            return Err(self.error("cache has an unexpected size or file type"));
        }
        let mut bytes = Vec::with_capacity(self.bytes as usize);
        file.take(self.bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| self.error("cache could not be read"))?;
        self.verify(&bytes)?;
        Ok(bytes)
    }

    /// Local reads require no-follow file support.
    #[cfg(not(unix))]
    pub fn read_bounded(&self, _path: &Path) -> Result<Vec<u8>, String> {
        Err(self.error("file loading requires no-follow file support on this platform"))
    }

    fn canceled(&self, cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Acquire) {
            Err(self.error("entry canceled"))
        } else {
            Ok(())
        }
    }

    /// Returns the decoded file from the cache, or downloads, decodes, and
    /// caches it. `decode` runs on verified bytes only. A cache entry that
    /// fails to decode stays in place until a verified download replaces it.
    pub fn fetch<T>(
        &self,
        cache: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64),
        decode: impl Fn(&[u8]) -> Result<T, String>,
    ) -> Result<T, String> {
        self.canceled(cancel)?;
        if self.bytes == 0 {
            return Err(self.error("is not published yet"));
        }
        self.validate_cache_directory(cache)?;
        if let Ok(bytes) = self.read_bounded(&cache.join(self.cache_name())) {
            self.canceled(cancel)?;
            if let Ok(value) = decode(&bytes) {
                self.canceled(cancel)?;
                self.prune(cache);
                return Ok(value);
            }
        }
        let bytes = self.download(cancel, progress)?;
        self.canceled(cancel)?;
        let value = decode(&bytes)?;
        self.canceled(cancel)?;
        self.install_cache(cache, &bytes, cancel)?;
        Ok(value)
    }

    /// A browser build has no blocking HTTP client: the page fetches the
    /// file itself and decodes it with the caller's decoder.
    #[cfg(target_arch = "wasm32")]
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Vec<u8>, String> {
        Err(self.error("download is not available in a browser build"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn download(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Vec<u8>, String> {
        if !self.url.starts_with("https://") {
            return Err(self.error("source must use HTTPS"));
        }
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|_| self.error("download could not start"))?;
        let mut response = client
            .get(self.url.as_str())
            .send()
            .map_err(|_| self.error("download could not connect"))?;
        if !response.status().is_success() {
            return Err(self.error("download is unavailable; try again later"));
        }
        if response
            .content_length()
            .is_some_and(|size| size != self.bytes)
        {
            return Err(self.error("download has an unexpected size"));
        }
        let mut bytes = Vec::with_capacity(self.bytes as usize);
        let mut block = [0; 64 * 1024];
        let mut last_update = 0;
        loop {
            self.canceled(cancel)?;
            let count = response
                .read(&mut block)
                .map_err(|_| self.error("download was interrupted"))?;
            if count == 0 {
                break;
            }
            if (bytes.len() + count) as u64 > self.bytes {
                return Err(self.error("download exceeds its declared size"));
            }
            bytes.extend_from_slice(&block[..count]);
            // At most about 100 progress records per transfer.
            let step = (self.bytes / 100).max(256 * 1024) as usize;
            if bytes.len() - last_update >= step || bytes.len() as u64 == self.bytes {
                progress(bytes.len() as u64, self.bytes);
                last_update = bytes.len();
            }
        }
        self.verify(&bytes)?;
        Ok(bytes)
    }

    /// Verifies, then atomically installs a cache entry.
    pub fn install_cache(
        &self,
        cache: &Path,
        bytes: &[u8],
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        self.verify(bytes)?;
        self.canceled(cancel)?;
        self.validate_cache_directory(cache)?;
        std::fs::create_dir_all(cache)
            .map_err(|_| self.error("cache directory could not be created"))?;
        if !std::fs::symlink_metadata(cache).is_ok_and(|m| m.is_dir()) {
            return Err(self.error("cache directory must not be a symbolic link"));
        }
        let final_path = cache.join(self.cache_name());
        let temp: PathBuf = cache.join(format!(
            "{}{}-{}.part",
            self.temp_prefix,
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp)
                .map_err(|_| self.error("cache could not be created"))?;
            file.write_all(bytes)
                .map_err(|_| self.error("cache could not be written"))?;
            file.sync_all()
                .map_err(|_| self.error("cache could not be saved"))?;
            self.canceled(cancel)?;
            std::fs::rename(&temp, &final_path)
                .map_err(|_| self.error("cache could not be installed"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        } else {
            // Cleanup is best effort after publication.
            self.prune(cache);
        }
        result
    }

    fn validate_cache_directory(&self, cache: &Path) -> Result<(), String> {
        match std::fs::symlink_metadata(cache) {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(self.error("cache directory must be a regular directory")),
        }
    }

    /// True for this file's own temporary names: prefix, process, counter.
    pub fn is_temp_name(&self, name: &str) -> bool {
        let Some(body) = name
            .strip_prefix(self.temp_prefix)
            .and_then(|s| s.strip_suffix(".part"))
        else {
            return false;
        };
        let Some((pid, counter)) = body.split_once('-') else {
            return false;
        };
        [pid, counter]
            .iter()
            .all(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()))
    }

    /// Removes named earlier revisions and stale temporary files. Unlinking
    /// relative to an opened directory cannot follow a replaced path.
    #[cfg(unix)]
    pub fn prune(&self, cache: &Path) {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::OpenOptionsExt;
        let Ok(directory) = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(cache)
        else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs().min(i64::MAX as u64) as i64);
        let remove = |name: &str, temporary: bool| {
            let Ok(name) = std::ffi::CString::new(name) else {
                return;
            };
            let mut state = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: the directory descriptor stays live, name is NUL
            // terminated, and state points to storage for one stat record.
            let result = unsafe {
                libc::fstatat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    state.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if result != 0 {
                return;
            }
            // SAFETY: a successful fstatat initialized the whole record.
            let state = unsafe { state.assume_init() };
            if state.st_mode & libc::S_IFMT != libc::S_IFREG {
                return;
            }
            if temporary
                && (state.st_size < 0
                    || state.st_size as u64 > self.bytes
                    || now.saturating_sub(state.st_mtime) < STALE_TEMP_SECONDS)
            {
                return;
            }
            // SAFETY: name has no separators and the directory stays open;
            // flags zero unlinks one entry without following or recursing.
            let _ = unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) };
        };
        for digest in self.history.iter().take(32).filter(|&&d| d != self.sha256) {
            if digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                remove(&format!("{digest}.{}", self.extension), false);
            }
        }
        // The cap bounds cleanup work if other software fills the directory.
        if let Ok(entries) = std::fs::read_dir(cache) {
            for entry in entries.take(256).flatten() {
                if let Some(name) = entry
                    .file_name()
                    .to_str()
                    .filter(|name| self.is_temp_name(name))
                {
                    remove(name, true);
                }
            }
        }
    }

    /// Cleanup needs directory-relative unlinking.
    #[cfg(not(unix))]
    pub fn prune(&self, _cache: &Path) {}
}
