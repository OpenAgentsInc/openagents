//! Where the person's keys live.
//!
//! - **The login keychain** on macOS (feature `keychain`), service
//!   `com.openagents.desktop`, one account per provider:
//!   `provider-key-openrouter`, `provider-key-vercel`,
//!   `provider-key-typesafe`.
//! - **Else 0600 files** in `~/.openagents` (a 0700 directory), each the
//!   file its crate already reads: `openrouter.json`
//!   (`openrouter::Config::from_env`), `ai-gateway.json`, and `jev.json`
//!   (`jev_hosted::local_key`), as `{"api_key": "..."}`.
//!
//! [`load_all`] reads every store, the keychain first. A key is never
//! written anywhere else, and nothing here prints one.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{ApiKey, Keys, PROVIDERS, Provider};

/// The keychain service the desktop app's secrets live under.
pub const KEYCHAIN_SERVICE: &str = "com.openagents.desktop";

/// The keychain account for `provider`'s key.
#[must_use]
pub const fn keychain_account(provider: Provider) -> &'static str {
    match provider {
        Provider::OpenRouter => "provider-key-openrouter",
        Provider::Vercel => "provider-key-vercel",
        Provider::TypeSafe => "provider-key-typesafe",
    }
}

/// The file under `~/.openagents` that holds `provider`'s key.
#[must_use]
pub const fn file_name(provider: Provider) -> &'static str {
    match provider {
        Provider::OpenRouter => "openrouter.json",
        Provider::Vercel => "ai-gateway.json",
        Provider::TypeSafe => "jev.json",
    }
}

/// A place keys are kept.
pub trait Store {
    /// `provider`'s key, if one is kept here.
    ///
    /// # Errors
    /// The store cannot be read; the message never carries a key.
    fn load(&self, provider: Provider) -> Result<Option<ApiKey>, String>;
    /// Keep `key` for `provider`, replacing any.
    ///
    /// # Errors
    /// The store cannot be written.
    fn save(&self, provider: Provider, key: &ApiKey) -> Result<(), String>;
    /// Remove `provider`'s key; removing none is fine.
    ///
    /// # Errors
    /// The store cannot be written.
    fn delete(&self, provider: Provider) -> Result<(), String>;
    /// Where the key is kept, for a person to read.
    fn describe(&self, provider: Provider) -> String;
}

/// `~/.openagents`, when the process has a home directory.
#[must_use]
pub fn openagents_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents"))
}

/// Keys in 0600 files in a 0700 directory.
#[derive(Clone, Debug)]
pub struct Files {
    dir: PathBuf,
}

impl Files {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, provider: Provider) -> PathBuf {
        self.dir.join(file_name(provider))
    }
}

impl Store for Files {
    fn load(&self, provider: Provider) -> Result<Option<ApiKey>, String> {
        let path = self.path(provider);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(format!("cannot read {}", path.display())),
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| format!("{} is not valid JSON", path.display()))?;
        Ok(value
            .get("api_key")
            .and_then(Value::as_str)
            .map(ApiKey::new)
            .filter(|key| !key.is_empty()))
    }

    fn save(&self, provider: Provider, key: &ApiKey) -> Result<(), String> {
        let path = self.path(provider);
        // Keep whatever else the file holds (a label, a base URL).
        let mut value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        value["api_key"] = json!(key.expose());
        let mut bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        let written = write_private(&path, &bytes);
        // Scrub the serialized copy.
        bytes.fill(0);
        written
    }

    fn delete(&self, provider: Provider) -> Result<(), String> {
        let path = self.path(provider);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(());
        };
        let mut value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
        if let Some(map) = value.as_object_mut() {
            map.remove("api_key");
            if !map.is_empty() {
                let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
                return write_private(&path, &bytes);
            }
        }
        std::fs::remove_file(&path).map_err(|_| format!("cannot remove {}", path.display()))
    }

    fn describe(&self, provider: Provider) -> String {
        self.path(provider).display().to_string()
    }
}

/// Write `bytes` to `file` readable only by this user (0600), its folder
/// 0700, through a temporary file renamed into place.
///
/// # Errors
/// A sentence naming the file.
pub fn write_private(file: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let dir = file
        .parent()
        .ok_or_else(|| format!("{} has no folder", file.display()))?;
    std::fs::create_dir_all(dir).map_err(|_| format!("cannot make {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("key"),
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let fail = || format!("cannot write {}", file.display());
    let mut handle = options.open(&tmp).map_err(|_| fail())?;
    handle.write_all(bytes).map_err(|_| fail())?;
    handle.sync_all().map_err(|_| fail())?;
    drop(handle);
    std::fs::rename(&tmp, file).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        fail()
    })
}

/// The macOS login keychain.
#[cfg(all(target_os = "macos", feature = "keychain"))]
#[derive(Clone, Debug, Default)]
pub struct Keychain;

#[cfg(all(target_os = "macos", feature = "keychain"))]
impl Store for Keychain {
    fn load(&self, provider: Provider) -> Result<Option<ApiKey>, String> {
        // errSecItemNotFound.
        const NOT_FOUND: i32 = -25_300;
        match security_framework::passwords::get_generic_password(
            KEYCHAIN_SERVICE,
            keychain_account(provider),
        ) {
            Ok(mut bytes) => {
                let key = String::from_utf8(bytes.clone())
                    .map_err(|_| "the stored key is not text".to_owned())
                    .map(ApiKey::new);
                bytes.fill(0);
                key.map(Some)
            }
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(_) => Err("the keychain did not answer".into()),
        }
    }

    fn save(&self, provider: Provider, key: &ApiKey) -> Result<(), String> {
        security_framework::passwords::set_generic_password(
            KEYCHAIN_SERVICE,
            keychain_account(provider),
            key.expose().as_bytes(),
        )
        .map_err(|_| "the keychain refused the key".into())
    }

    fn delete(&self, provider: Provider) -> Result<(), String> {
        const NOT_FOUND: i32 = -25_300;
        match security_framework::passwords::delete_generic_password(
            KEYCHAIN_SERVICE,
            keychain_account(provider),
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == NOT_FOUND => Ok(()),
            Err(_) => Err("the keychain refused to remove the key".into()),
        }
    }

    fn describe(&self, provider: Provider) -> String {
        format!(
            "the login keychain ({KEYCHAIN_SERVICE}, {})",
            keychain_account(provider)
        )
    }
}

/// The store a new key goes to on this computer: the login keychain when
/// it is built in and answers, else the files in `dir`.
#[must_use]
pub fn preferred(dir: &Path) -> Box<dyn Store> {
    #[cfg(all(target_os = "macos", feature = "keychain"))]
    {
        if std::env::var("OPENAGENTS_KEY_STORE").as_deref() != Ok("file")
            && Keychain.load(Provider::OpenRouter).is_ok()
        {
            return Box::new(Keychain);
        }
    }
    Box::new(Files::new(dir))
}

/// Every store this computer may hold keys in, the preferred first.
#[must_use]
pub fn all(dir: &Path) -> Vec<Box<dyn Store>> {
    #[allow(unused_mut)]
    let mut stores: Vec<Box<dyn Store>> = Vec::new();
    #[cfg(all(target_os = "macos", feature = "keychain"))]
    {
        if std::env::var("OPENAGENTS_KEY_STORE").as_deref() != Ok("file") {
            stores.push(Box::new(Keychain));
        }
    }
    stores.push(Box::new(Files::new(dir)));
    stores
}

/// The person's stored keys: for each provider, the first store holding
/// one. A store that cannot be read is skipped.
#[must_use]
pub fn load_all(stores: &[Box<dyn Store>]) -> Keys {
    let mut keys = Keys::none();
    for provider in PROVIDERS {
        for store in stores {
            if let Ok(Some(key)) = store.load(provider) {
                keys.insert(provider, key);
                break;
            }
        }
    }
    keys
}

/// Remove `provider`'s key from every store.
///
/// # Errors
/// The first store that could not remove it.
pub fn delete_everywhere(stores: &[Box<dyn Store>], provider: Provider) -> Result<(), String> {
    for store in stores {
        store.delete(provider)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_private_and_keep_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".openagents");
        let files = Files::new(&home);
        assert!(files.load(Provider::OpenRouter).unwrap().is_none());
        files
            .save(Provider::OpenRouter, &ApiKey::new("sk-or-test"))
            .unwrap();
        assert_eq!(
            files.load(Provider::OpenRouter).unwrap().unwrap().expose(),
            "sk-or-test"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&home.join("openrouter.json")), 0o600);
            assert_eq!(mode(&home), 0o700);
        }
        std::fs::write(
            home.join("jev.json"),
            r#"{"api_key":"ts-old","base_url":"https://example.test"}"#,
        )
        .unwrap();
        files
            .save(Provider::TypeSafe, &ApiKey::new("ts-new"))
            .unwrap();
        let text = std::fs::read_to_string(home.join("jev.json")).unwrap();
        assert!(text.contains("base_url") && text.contains("ts-new"));
        files.delete(Provider::TypeSafe).unwrap();
        assert!(files.load(Provider::TypeSafe).unwrap().is_none());
        assert!(home.join("jev.json").exists(), "other fields are kept");
        files.delete(Provider::OpenRouter).unwrap();
        assert!(!home.join("openrouter.json").exists());
        let stores: Vec<Box<dyn Store>> = vec![Box::new(files)];
        assert!(load_all(&stores).is_empty());
    }
}
