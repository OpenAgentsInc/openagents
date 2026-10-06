//! Private, atomic storage for the live plugin settings.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use model_access::ApiKey;
use serde::{Deserialize, Deserializer, Serialize};

const VERSION: u32 = 1;
const MAX_BYTES: usize = 64 * 1024;
const MAX_MODEL_BYTES: usize = 1024;
const MAX_KEY_BYTES: usize = 16 * 1024;
const READ_ERROR: &str = "Cannot read the saved plugin settings.";
const INVALID_ERROR: &str = "The saved plugin settings are invalid. The file was not changed.";
const WRITE_ERROR: &str =
    "Cannot save the plugin settings. The previous settings were not changed.";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// The settings saved for OpenRouter BYOK. Debug output hides the key.
#[derive(Clone, Debug, Default)]
pub struct SavedPlugin {
    pub enabled: bool,
    pub model: String,
    pub key: Option<ApiKey>,
}

/// A settings directory provided by the caller. This type never discovers a home directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPlugin {
    version: u32,
    enabled: bool,
    model: String,
    #[serde(deserialize_with = "read_key")]
    api_key: Option<ApiKey>,
}

#[derive(Serialize)]
struct PluginDocument<'a> {
    version: u32,
    enabled: bool,
    model: &'a str,
    api_key: Option<&'a str>,
}

fn read_key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<ApiKey>, D::Error> {
    struct KeyValue(ApiKey);

    impl<'de> Deserialize<'de> for KeyValue {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct KeyVisitor;

            impl serde::de::Visitor<'_> for KeyVisitor {
                type Value = KeyValue;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("an API key")
                }

                fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<KeyValue, E> {
                    if text.is_empty()
                        || text.len() > MAX_KEY_BYTES
                        || text.chars().any(char::is_whitespace)
                    {
                        return Err(E::custom("Invalid API key."));
                    }
                    Ok(KeyValue(ApiKey::new(text)))
                }

                fn visit_string<E: serde::de::Error>(self, text: String) -> Result<KeyValue, E> {
                    let bytes = PrivateBytes(text.into_bytes());
                    let text =
                        std::str::from_utf8(&bytes.0).map_err(|_| E::custom("Invalid API key."))?;
                    self.visit_str(text)
                }
            }

            deserializer.deserialize_str(KeyVisitor)
        }
    }

    Option::<KeyValue>::deserialize(deserializer).map(|key| key.map(|key| key.0))
}

impl Store {
    #[must_use]
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Load settings, or return defaults when no file has been saved.
    ///
    /// # Errors
    /// The file cannot be read, exceeds the size limit, or has an invalid schema.
    pub fn load(&self) -> Result<SavedPlugin, String> {
        let path = self.root.join("plugins.json");
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SavedPlugin::default());
            }
            Err(_) => return Err(READ_ERROR.into()),
            Ok(metadata) if !metadata.is_file() => return Err(READ_ERROR.into()),
            Ok(metadata) if metadata.len() > MAX_BYTES as u64 => {
                return Err(INVALID_ERROR.into());
            }
            Ok(_) => {}
        }
        let file = File::open(&path).map_err(|_| READ_ERROR.to_owned())?;
        let mut bytes = PrivateBytes(Vec::new());
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes.0)
            .map_err(|_| READ_ERROR.to_owned())?;
        if bytes.0.len() > MAX_BYTES {
            return Err(INVALID_ERROR.into());
        }
        let stored: StoredPlugin =
            serde_json::from_slice(&bytes.0).map_err(|_| INVALID_ERROR.to_owned())?;
        if stored.version != VERSION || !valid_model(&stored.model) {
            return Err(INVALID_ERROR.into());
        }
        Ok(SavedPlugin {
            enabled: stored.enabled,
            model: stored.model,
            key: stored.api_key,
        })
    }

    /// Save the settings and key together, replacing the file atomically.
    ///
    /// # Errors
    /// Invalid existing settings are preserved. A failed write preserves the previous file.
    pub fn save(&self, settings: &SavedPlugin) -> Result<(), String> {
        // Do not overwrite a damaged or newer document if loading it failed.
        self.load()?;
        if !valid_model(&settings.model)
            || settings.key.as_ref().is_some_and(|key| {
                key.is_empty()
                    || key.expose().len() > MAX_KEY_BYTES
                    || key.expose().chars().any(char::is_whitespace)
            })
        {
            return Err(WRITE_ERROR.into());
        }
        let document = PluginDocument {
            version: VERSION,
            enabled: settings.enabled,
            model: &settings.model,
            api_key: settings.key.as_ref().map(ApiKey::expose),
        };
        let mut bytes =
            PrivateBytes(serde_json::to_vec_pretty(&document).map_err(|_| WRITE_ERROR.to_owned())?);
        bytes.0.push(b'\n');
        if bytes.0.len() > MAX_BYTES {
            return Err(WRITE_ERROR.into());
        }
        write_private(&self.root, &bytes.0).map_err(|_| WRITE_ERROR.to_owned())
    }
}

fn valid_model(model: &str) -> bool {
    model.len() <= MAX_MODEL_BYTES && !model.chars().any(char::is_control)
}

struct PrivateBytes(Vec<u8>);

impl Drop for PrivateBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_private(root: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(root)?;
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    for _ in 0..16 {
        let path = root.join(format!(
            ".plugins.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let temporary = TemporaryFile(path);
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary.0, root.join("plugins.json"))?;
        return Ok(());
    }
    Err(std::io::ErrorKind::AlreadyExists.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> SavedPlugin {
        SavedPlugin {
            enabled: true,
            model: "example/model".into(),
            key: Some(ApiKey::new("fake-plugin-key")),
        }
    }

    #[test]
    fn missing_settings_do_not_create_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("coder-new");
        let settings = Store::under(&root).load().unwrap();
        assert!(!settings.enabled);
        assert!(settings.model.is_empty());
        assert!(settings.key.is_none());
        assert!(!root.exists());
    }

    #[test]
    fn settings_and_key_round_trip_and_can_be_removed() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("coder-new");
        let store = Store::under(&root);
        store.save(&settings()).unwrap();
        let loaded = store.load().unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.model, "example/model");
        assert_eq!(loaded.key.unwrap().expose(), "fake-plugin-key");
        assert!(!format!("{:?}", settings()).contains("fake-plugin-key"));
        store.save(&SavedPlugin::default()).unwrap();
        assert!(store.load().unwrap().key.is_none());
        assert!(
            !fs::read_to_string(root.join("plugins.json"))
                .unwrap()
                .contains("fake-plugin-key")
        );
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn saved_files_and_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("coder-new");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        let store = Store::under(&root);
        store.save(&settings()).unwrap();
        let file = root.join("plugins.json");
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        store.save(&settings()).unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn malformed_or_newer_settings_are_preserved_without_secret_errors() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("plugins.json");
        let store = Store::under(temporary.path());
        let examples = [
            r#"{"api_key":"fake-plugin-key",bad}"#,
            r#"{"version":2,"enabled":true,"model":"","api_key":"fake-plugin-key"}"#,
            r#"{"version":1,"enabled":true,"model":"","api_key": ["fake-plugin-key"]}"#,
            r#"{"version":1,"enabled":true,"model":"","api_key":"fake plugin key"}"#,
        ];
        for example in examples {
            fs::write(&path, example).unwrap();
            let error = store.load().unwrap_err();
            assert_eq!(error, INVALID_ERROR);
            assert!(!error.contains("fake"));
            assert!(store.save(&settings()).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), example);
        }
    }

    #[test]
    fn oversized_files_and_failed_saves_preserve_existing_contents() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        store.save(&settings()).unwrap();
        let path = temporary.path().join("plugins.json");
        let original = fs::read(&path).unwrap();
        let invalid = SavedPlugin {
            model: "x".repeat(MAX_MODEL_BYTES + 1),
            ..settings()
        };
        assert_eq!(store.save(&invalid).unwrap_err(), WRITE_ERROR);
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);

        let oversized = vec![b'x'; MAX_BYTES + 1];
        fs::write(&path, &oversized).unwrap();
        assert_eq!(store.load().unwrap_err(), INVALID_ERROR);
        assert!(store.save(&settings()).is_err());
        assert_eq!(fs::read(&path).unwrap(), oversized);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_or_replaced() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let outside = temporary.path().join("other-settings.json");
        fs::write(&outside, "keep this file").unwrap();
        let root = temporary.path().join("coder-new");
        fs::create_dir(&root).unwrap();
        symlink(&outside, root.join("plugins.json")).unwrap();
        let store = Store::under(&root);
        assert_eq!(store.load().unwrap_err(), READ_ERROR);
        assert!(store.save(&settings()).is_err());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep this file");
    }
}
