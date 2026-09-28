//! Device-key encrypted, atomic, bounded cache. Cached bytes never prove current access.
use secp256k1::{Keypair, Secp256k1, SecretKey, rand::RngCore};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_ITEM: usize = 192 * 1024;
const MAX_CIPHER: u64 = 400 * 1024;
const MAX_FILES: usize = 8192;
const DEFAULT_BUDGET: u64 = 64 * 1024 * 1024;

pub struct Cache {
    root: PathBuf,
    key: [u8; 32],
    budget: u64,
}

impl Cache {
    pub fn open(root: &Path, secret: &SecretKey) -> Result<Self, String> {
        if root.exists()
            && fs::symlink_metadata(root)
                .map_err(io_error)?
                .file_type()
                .is_symlink()
        {
            return Err("cache directory cannot be a symlink".into());
        }
        fs::create_dir_all(root).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
        }
        let own = Keypair::from_secret_key(&Secp256k1::new(), secret)
            .x_only_public_key()
            .0;
        Ok(Self {
            root: root.canonicalize().map_err(io_error)?,
            key: nostr::nip44::conversation_key(secret, &own),
            budget: DEFAULT_BUDGET,
        })
    }

    fn path(&self, key: &str) -> Result<PathBuf, String> {
        if key.is_empty()
            || key.len() > 180
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Err("invalid cache identity".into());
        }
        Ok(self.root.join(format!("{key}.cache")))
    }

    pub fn read<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, String> {
        let path = self.path(key)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error(e)),
        };
        if !metadata.is_file() || metadata.len() > MAX_CIPHER {
            return Err("cache entry exceeds its bounds".into());
        }
        let mut file = fs::File::open(path).map_err(io_error)?;
        let mut encrypted = String::new();
        Read::by_ref(&mut file)
            .take(MAX_CIPHER + 1)
            .read_to_string(&mut encrypted)
            .map_err(io_error)?;
        if encrypted.len() as u64 > MAX_CIPHER {
            return Err("cache entry exceeds its bounds".into());
        }
        let plaintext = nostr::nip44::decrypt(&encrypted, &self.key)
            .map_err(|_| "cache authentication failed")?;
        let value: serde_json::Value =
            serde_json::from_str(&plaintext).map_err(|_| "invalid cached value")?;
        if value["key"] != key {
            return Err("cache identity differs".into());
        }
        serde_json::from_value(value["value"].clone())
            .map(Some)
            .map_err(|_| "cache schema differs".into())
    }

    /// Persist before publishing a cursor that depends on these bytes. Eviction
    /// affects presentation only: missing pages are reported and can be reloaded.
    pub fn write<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        let path = self.path(key)?;
        let plaintext = serde_json::to_string(&serde_json::json!({"key":key,"value":value}))
            .map_err(|_| "cache encoding failed")?;
        if plaintext.len() > MAX_ITEM {
            return Err("cache item exceeds its bound".into());
        }
        let mut nonce = [0u8; 32];
        secp256k1::rand::rng().fill_bytes(&mut nonce);
        let encrypted = nostr::nip44::encrypt(&plaintext, &self.key, nonce)
            .map_err(|_| "cache encryption failed")?;
        let temp = self.root.join(format!(
            ".pending-{}",
            u64::from_le_bytes(nonce[..8].try_into().expect("fixed nonce"))
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp).map_err(io_error)?;
            file.write_all(encrypted.as_bytes()).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            fs::rename(&temp, path).map_err(io_error)?;
            fs::File::open(&self.root)
                .and_then(|d| d.sync_all())
                .map_err(io_error)?;
            self.trim(key)
        })();
        let _ = fs::remove_file(temp);
        result
    }

    pub fn keys(&self, prefix: &str) -> Result<Vec<String>, String> {
        let mut keys = Vec::new();
        for (index, entry) in fs::read_dir(&self.root)
            .map_err(io_error)?
            .take(MAX_FILES + 2)
            .enumerate()
        {
            if index > MAX_FILES {
                return Err("cache directory exceeds its entry bound".into());
            }
            let entry = entry.map_err(io_error)?;
            if let Some(name) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".cache"))
                && name.starts_with(prefix)
            {
                keys.push(name.to_owned());
            }
        }
        keys.sort();
        Ok(keys)
    }

    fn trim(&self, keep: &str) -> Result<(), String> {
        let mut files = Vec::new();
        let mut bytes = 0u64;
        for (index, entry) in fs::read_dir(&self.root)
            .map_err(io_error)?
            .take(MAX_FILES + 3)
            .enumerate()
        {
            if index > MAX_FILES + 1 {
                return Err("cache directory exceeds its entry bound".into());
            }
            let entry = entry.map_err(io_error)?;
            let meta = fs::symlink_metadata(entry.path()).map_err(io_error)?;
            if meta.is_file() {
                bytes = bytes.saturating_add(meta.len());
                files.push((meta.modified().ok(), entry.path(), meta.len()));
            }
        }
        files.sort_by_key(|f| f.0);
        let mut count = files.len();
        for (_, path, len) in files {
            if bytes <= self.budget && count <= MAX_FILES {
                return Ok(());
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if (name.starts_with("page_") || name.starts_with("catalog_"))
                && name != format!("{keep}.cache")
            {
                fs::remove_file(path).map_err(io_error)?;
                bytes = bytes.saturating_sub(len);
                count = count.saturating_sub(1);
            }
        }
        if bytes > self.budget || count > MAX_FILES {
            return Err("device cache budget is full".into());
        }
        Ok(())
    }

    pub fn erase(&self, prefix: &str) -> Result<(), String> {
        for key in self.keys(prefix)? {
            fs::remove_file(self.path(&key)?).map_err(io_error)?;
        }
        Ok(())
    }
}

fn io_error(_: std::io::Error) -> String {
    "device cache I/O failed".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_cache_survives_restart_and_refuses_wrong_key_and_swapped_entries() {
        let root = tempfile::tempdir().unwrap();
        let secret = SecretKey::from_byte_array([1; 32]).unwrap();
        let cache = Cache::open(root.path(), &secret).unwrap();
        cache.write("one", &"private transcript").unwrap();
        let bytes = fs::read_to_string(root.path().join("one.cache")).unwrap();
        assert!(!bytes.contains("private transcript"));
        let reopened = Cache::open(root.path(), &secret).unwrap();
        assert_eq!(
            reopened.read::<String>("one").unwrap().as_deref(),
            Some("private transcript")
        );
        fs::copy(root.path().join("one.cache"), root.path().join("two.cache")).unwrap();
        assert!(reopened.read::<String>("two").is_err());
        assert!(
            Cache::open(root.path(), &SecretKey::from_byte_array([2; 32]).unwrap())
                .unwrap()
                .read::<String>("one")
                .is_err()
        );
        assert!(cache.write("../escape", &0).is_err());
        cache.erase("").unwrap();
        assert!(cache.read::<String>("one").unwrap().is_none());
    }
}
