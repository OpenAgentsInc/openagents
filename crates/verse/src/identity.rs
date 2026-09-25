//! Player identity: one Nostr key per profile, created on first launch.
//!
//! Signing up is creating the key. The secret lives in
//! `~/.openagents/verse/<profile>.key` as 64 lowercase hex characters,
//! readable only by the owner. `VERSE_HOME` overrides the directory.

use std::io::Read;
use std::path::{Path, PathBuf};

use nostr::domain::RelaySigner;

/// A loaded or newly created identity.
pub struct Identity {
    /// The signer for this player's events.
    pub signer: RelaySigner,
    /// The profile name, used as the display name.
    pub profile: String,
    /// True when this launch created the key.
    pub created: bool,
}

/// The directory profile keys live in.
#[must_use]
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("VERSE_HOME") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join(".openagents").join("verse")
}

/// Loads the key for `profile` from `dir`, creating it when absent.
///
/// # Errors
///
/// Returns a message when the profile name is unusable, the key cannot be
/// read or written, or the stored key is invalid.
pub fn load_or_create(dir: &Path, profile: &str) -> Result<Identity, String> {
    let valid = !profile.is_empty()
        && profile.len() <= 32
        && profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        return Err(format!(
            "profile {profile:?} must be 1 to 32 letters, digits, - or _"
        ));
    }
    let path = dir.join(format!("{profile}.key"));
    let (secret, created) = match std::fs::read_to_string(&path) {
        Ok(text) => (text.trim().to_owned(), false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let secret = fresh_secret()?;
            write_private(&path, &secret)?;
            (secret, true)
        }
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let signer = RelaySigner::from_secret_hex(&secret)
        .map_err(|e| format!("{} holds an invalid key: {e}", path.display()))?;
    Ok(Identity {
        signer,
        profile: profile.to_owned(),
        created,
    })
}

fn fresh_secret() -> Result<String, String> {
    loop {
        let mut bytes = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|e| format!("cannot read randomness: {e}"))?;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        if RelaySigner::from_secret_hex(&hex).is_ok() {
            return Ok(hex);
        }
    }
}

fn write_private(path: &Path, secret: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    writeln!(file, "{secret}").map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// A short random hex string, for session ids.
#[must_use]
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    let _ = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf));
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("verse-id-{name}-{}", random_hex(6)));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_profile_is_created_once_then_reused() {
        let dir = scratch("reuse");
        let first = load_or_create(&dir, "alice").expect("created");
        assert!(first.created);
        let again = load_or_create(&dir, "alice").expect("loaded");
        assert!(!again.created);
        assert_eq!(first.signer.pubkey(), again.signer.pubkey());
        let other = load_or_create(&dir, "bob").expect("created");
        assert_ne!(first.signer.pubkey(), other.signer.pubkey());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_key_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("mode");
        load_or_create(&dir, "carol").expect("created");
        let mode = std::fs::metadata(dir.join("carol.key"))
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_profile_name_is_refused() {
        assert!(load_or_create(&scratch("bad"), "../x").is_err());
    }
}
