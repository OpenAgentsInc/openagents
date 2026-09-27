//! The owner key file.
//!
//! The owner key is the Nostr key whose authority every host serves
//! (NIP-HOST) and the only key that can read or publish the owner host
//! directory (NIP-REACH). It lives in one private file on the owner's own
//! computer: 64 lowercase hexadecimal characters, a regular file owned by
//! this user with mode `0600`, in a directory with mode `0700`. A host is set
//! up with the public half only; the secret never travels to a host, over
//! SSH, in an argument, or in a log line.

use std::path::{Path, PathBuf};

use secp256k1::SecretKey;

use crate::{Error, Result};

/// The environment variable that names the owner key file.
pub const ENV: &str = "OPENAGENTS_OWNER_KEY_FILE";

/// `$OPENAGENTS_OWNER_KEY_FILE`, else `~/.openagents/coder-owner/owner.key`.
///
/// # Errors
/// Reports an unset `HOME` when the variable is unset too.
pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    crate::home(".openagents/coder-owner/owner.key")
}

/// Read the owner secret key.
///
/// # Errors
/// Refuses a missing file, a file that is not a regular file owned by this
/// user, one that group or others can read or write, and malformed contents.
/// No message repeats the contents.
pub fn load(path: &Path) -> Result<SecretKey> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).map_err(|_| {
        Error::new(format!(
            "no owner key at {}; create one with `coder link owner init`",
            path.display()
        ))
    })?;
    if !meta.file_type().is_file() {
        return Err(Error::new("the owner key is not a regular file"));
    }
    // SAFETY: `getuid` has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    if meta.uid() != uid {
        return Err(Error::new("the owner key file belongs to another user"));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(Error::new(format!(
            "the owner key file is open to group or others; run chmod 600 {}",
            path.display()
        )));
    }
    let text =
        std::fs::read_to_string(path).map_err(|_| Error::new("the owner key cannot be read"))?;
    parse(text.trim())
}

/// Parse a hex or `nsec` secret key.
///
/// # Errors
/// Refuses anything else, without repeating it.
pub fn parse(text: &str) -> Result<SecretKey> {
    let bytes: Vec<u8> = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text)
            .map_err(|_| Error::new("the owner key is malformed"))?
            .to_vec()
    } else {
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new("the owner key is malformed"));
        }
        (0..32)
            .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::new("the owner key is malformed"))?
    };
    SecretKey::from_byte_array(
        bytes
            .try_into()
            .map_err(|_| Error::new("the owner key is malformed"))?,
    )
    .map_err(|_| Error::new("the owner key is malformed"))
}

/// Create an owner key at `path` unless one is there. Returns the public key
/// and whether this call created it. The directory is made `0700` and the
/// file `0600`, created exclusively, so an existing key is never replaced.
///
/// # Errors
/// Reports a failed write, and an existing key that does not load.
pub fn create(path: &Path) -> Result<(String, bool)> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if std::fs::symlink_metadata(path).is_ok() {
        return Ok((coder_reach::pubkey(&load(path)?), false));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("the owner key path has no directory"))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|_| Error::new("cannot create the owner key directory"))?;
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| Error::new("cannot create the owner key file"))?;
    file.write_all(format!("{}\n", secret.display_secret()).as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| Error::new("cannot write the owner key file"))?;
    Ok((coder_reach::pubkey(&secret), true))
}

/// A public key as lowercase hex from hex or `npub`.
///
/// # Errors
/// Refuses anything that is not a valid x-only public key.
pub fn public_key(text: &str) -> Result<String> {
    let hex = if text.starts_with("npub1") {
        nostr::nip19::decode_npub(text)
            .map_err(|_| Error::new("the owner public key is not a valid npub"))?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        text.to_ascii_lowercase()
    };
    coder_reach::parse_pubkey(&hex)
        .map_err(|_| Error::new("the owner public key is not a public key"))?;
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn create_is_private_idempotent_and_never_replaces_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner").join("owner.key");
        let (public, created) = create(&path).unwrap();
        assert!(created);
        let (again, created) = create(&path).unwrap();
        assert!(!created);
        assert_eq!(public, again);
        assert_eq!(coder_reach::pubkey(&load(&path).unwrap()), public);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn an_open_or_malformed_key_refuses_without_echoing_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.key");
        create(&path).unwrap();
        let secret = std::fs::read_to_string(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let error = load(&path).unwrap_err().to_string();
        assert!(error.contains("chmod 600"), "{error}");
        assert!(!error.contains(secret.trim()));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, "not-a-key-but-secret-looking").unwrap();
        let error = load(&path).unwrap_err().to_string();
        assert!(!error.contains("secret-looking"), "{error}");
        assert!(load(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn public_keys_accept_hex_and_npub() {
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        let hex = coder_reach::pubkey(&secret);
        assert_eq!(public_key(&hex).unwrap(), hex);
        assert_eq!(public_key(&hex.to_ascii_uppercase()).unwrap(), hex);
        assert!(public_key("npub1nope").is_err());
        assert!(public_key("00").is_err());
    }
}
