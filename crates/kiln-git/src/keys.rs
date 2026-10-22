//! The key a command signs with: `--key FILE` (64 hex characters), or
//! `--as PROFILE`, the `openagents` profile key in
//! `~/.openagents/verse/PROFILE.key` (`VERSE_HOME` overrides the
//! directory). Never creates a key and never prints a secret.

use std::path::{Path, PathBuf};

use kiln::cj::Key;
use secp256k1::SecretKey;

/// A loaded signing key.
pub struct Signer {
    pub key: Key,
    pub secret: SecretKey,
}

/// The directory `openagents` profile keys live in.
#[must_use]
pub fn profile_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("VERSE_HOME") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents")
        .join("verse")
}

/// Read a hex secret from `path`.
///
/// # Errors
///
/// A plain reason when the file is missing or holds no key.
pub fn load(path: &Path) -> Result<Signer, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = text.trim();
    if text.len() != 64 {
        return Err(format!("{} does not hold a 64-hex key", path.display()));
    }
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| format!("{} does not hold a 64-hex key", path.display()))?;
    }
    let secret = SecretKey::from_byte_array(bytes).map_err(|e| e.to_string())?;
    Ok(Signer {
        key: Key::from_bytes(bytes)?,
        secret,
    })
}

/// The key `--key FILE` or `--as PROFILE` names; `default` when neither.
///
/// # Errors
///
/// A plain reason, with how to make a profile key when it is missing.
pub fn choose(file: Option<&Path>, profile: Option<&str>) -> Result<Signer, String> {
    if let Some(file) = file {
        return load(file);
    }
    let profile = profile.unwrap_or("default");
    let path = profile_dir().join(format!("{profile}.key"));
    if !path.exists() {
        return Err(format!(
            "no key for the profile {profile}; make one with `openagents key show --as {profile}` or pass --key FILE"
        ));
    }
    load(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_file_loads_and_a_bad_one_says_why() {
        let dir = std::env::temp_dir().join(format!("kiln-git-keys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.key");
        std::fs::write(&good, format!("{}\n", "11".repeat(32))).unwrap();
        assert_eq!(load(&good).unwrap().key.pubkey().len(), 64);
        let bad = dir.join("bad.key");
        std::fs::write(&bad, "nope").unwrap();
        assert!(load(&bad).is_err());
        assert!(choose(Some(&dir.join("missing.key")), None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
