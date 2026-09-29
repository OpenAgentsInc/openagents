//! Where secret keys live.
//!
//! A host holds an owner Nostr key, a host Nostr key, and an iroh key; a
//! device holds a device Nostr key and an iroh key. [`KeySource`] is the one
//! interface to their storage. [`FileKeySource`] keeps each key in its own
//! `0600` file in a `0700` directory, for CLI-only installs and tests; the
//! desktop app's keychain source implements the same trait.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Code, Error, Result, fail, hex, unhex32};

/// Which key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyName {
    /// The owner's Nostr key.
    Owner,
    /// The host's Nostr key.
    Host,
    /// The host's iroh key.
    HostIroh,
    /// A device's Nostr key.
    Device,
    /// A device's iroh key.
    DeviceIroh,
}

impl KeyName {
    pub const ALL: [Self; 5] = [
        Self::Owner,
        Self::Host,
        Self::HostIroh,
        Self::Device,
        Self::DeviceIroh,
    ];

    /// A stable name for storage: a file name, or a keychain account.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Host => "host",
            Self::HostIroh => "host-iroh",
            Self::Device => "device",
            Self::DeviceIroh => "device-iroh",
        }
    }
}

/// 32 secret bytes. `Debug` never prints them.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret([u8; 32]);

impl Secret {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// 32 fresh random bytes.
    #[must_use]
    pub fn random() -> Self {
        Self(coder_reach::random_bytes())
    }

    #[must_use]
    pub const fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Best effort; the compiler may still have copied the bytes.
        self.0 = [0; 32];
    }
}

/// Storage for secret keys.
pub trait KeySource: Send + Sync {
    /// The stored key, or `None` when none is stored.
    ///
    /// # Errors
    /// `unavailable` when the store cannot be read; `malformed` when the
    /// stored value is not a key or is readable by others.
    fn load(&self, name: KeyName) -> Result<Option<Secret>>;

    /// Store a key, replacing any stored under the same name.
    ///
    /// # Errors
    /// `unavailable` when the store cannot be written.
    fn store(&self, name: KeyName, secret: &Secret) -> Result<()>;

    /// Remove a key. Removing a missing key is not an error.
    ///
    /// # Errors
    /// `unavailable` when the store cannot be written.
    fn delete(&self, name: KeyName) -> Result<()>;
}

/// The stored key, or a new one from `generate`, stored before it is
/// returned. The store is read back so a key is never used unless it
/// persisted.
///
/// # Errors
/// Any error from the store, or `unavailable` when the read-back differs.
pub fn load_or_create(
    source: &dyn KeySource,
    name: KeyName,
    generate: impl FnOnce() -> Secret,
) -> Result<Secret> {
    if let Some(secret) = source.load(name)? {
        return Ok(secret);
    }
    let secret = generate();
    source.store(name, &secret)?;
    match source.load(name)? {
        Some(stored) if stored == secret => Ok(secret),
        _ => fail(Code::Unavailable, "stored key did not read back"),
    }
}

/// The iroh key under `name`, created on first use.
///
/// # Errors
/// As [`load_or_create`].
pub fn iroh_key(source: &dyn KeySource, name: KeyName) -> Result<iroh::SecretKey> {
    let secret = load_or_create(source, name, Secret::random)?;
    Ok(iroh::SecretKey::from_bytes(secret.expose()))
}

/// Keys as files: `DIR/NAME.key`, 64 lowercase hex characters and a newline.
/// On Unix the directory is `0700` and each file `0600`, and a file that
/// others can read is refused.
#[derive(Clone, Debug)]
pub struct FileKeySource {
    dir: PathBuf,
}

impl FileKeySource {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn path(&self, name: KeyName) -> PathBuf {
        self.dir.join(format!("{}.key", name.as_str()))
    }

    fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|_| unavailable("create key directory"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))
                .map_err(|_| unavailable("set key directory mode"))?;
        }
        Ok(())
    }
}

impl KeySource for FileKeySource {
    fn load(&self, name: KeyName) -> Result<Option<Secret>> {
        let path = self.path(name);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return fail(Code::Unavailable, "read key file"),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path)
                .map_err(|_| unavailable("read key file mode"))?
                .permissions()
                .mode();
            if mode & 0o077 != 0 {
                return fail(Code::Malformed, "key file is readable by others");
            }
        }
        let hex_text = text.strip_suffix('\n').unwrap_or(&text);
        unhex32(hex_text)
            .map(|bytes| Some(Secret(bytes)))
            .map_err(|_| Error::new(Code::Malformed, "key file does not hold a key"))
    }

    fn store(&self, name: KeyName, secret: &Secret) -> Result<()> {
        self.ensure_dir()?;
        let path = self.path(name);
        let tmp = self.dir.join(format!(".{}.key.tmp", name.as_str()));
        let _ = fs::remove_file(&tmp);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|_| unavailable("write key file"))?;
        let mut text = hex(secret.expose());
        text.push('\n');
        let written = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
            return fail(Code::Unavailable, "write key file");
        }
        fs::rename(&tmp, &path).map_err(|_| unavailable("write key file"))
    }

    fn delete(&self, name: KeyName) -> Result<()> {
        match fs::remove_file(self.path(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => fail(Code::Unavailable, "remove key file"),
        }
    }
}

fn unavailable(detail: &'static str) -> Error {
    Error::new(Code::Unavailable, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_source_creates_once_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let source = FileKeySource::new(dir.path().join("keys"));
        assert_eq!(source.load(KeyName::HostIroh).unwrap(), None);
        let first = iroh_key(&source, KeyName::HostIroh).unwrap();
        let second = iroh_key(&source, KeyName::HostIroh).unwrap();
        assert_eq!(first.public(), second.public());
        // Names do not collide.
        let device = iroh_key(&source, KeyName::DeviceIroh).unwrap();
        assert_ne!(first.public(), device.public());
        source.delete(KeyName::HostIroh).unwrap();
        source.delete(KeyName::HostIroh).unwrap();
        assert_eq!(source.load(KeyName::HostIroh).unwrap(), None);
    }

    #[test]
    fn stored_secret_is_the_given_one_and_debug_hides_it() {
        let dir = tempfile::tempdir().unwrap();
        let source = FileKeySource::new(dir.path());
        let secret = Secret::from_bytes([7; 32]);
        let got = load_or_create(&source, KeyName::Owner, || secret.clone()).unwrap();
        assert_eq!(got, secret);
        assert_eq!(format!("{got:?}"), "Secret(..)");
        let text = fs::read_to_string(source.path(KeyName::Owner)).unwrap();
        assert_eq!(text, format!("{}\n", "07".repeat(32)));
    }

    #[test]
    fn malformed_key_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let source = FileKeySource::new(dir.path());
        source.store(KeyName::Host, &Secret::random()).unwrap();
        fs::write(source.path(KeyName::Host), "not a key\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                source.path(KeyName::Host),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        assert_eq!(
            source.load(KeyName::Host).unwrap_err().code,
            Code::Malformed
        );
    }

    #[cfg(unix)]
    #[test]
    fn modes_are_private_and_open_files_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let source = FileKeySource::new(dir.path().join("keys"));
        source.store(KeyName::Device, &Secret::random()).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(source.dir()), 0o700);
        assert_eq!(mode(&source.path(KeyName::Device)), 0o600);
        fs::set_permissions(
            source.path(KeyName::Device),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(
            source.load(KeyName::Device).unwrap_err().code,
            Code::Malformed
        );
    }
}
