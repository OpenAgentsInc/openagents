//! Private, atomic storage for the live plugin settings.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use model_access::ApiKey;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

use crate::models::{DEFAULT_MODEL, GenerationOptions};

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
#[derive(Clone, Debug)]
pub struct SavedPlugin {
    pub enabled: bool,
    pub model: String,
    pub key: Option<ApiKey>,
    pub options: GenerationOptions,
}

impl Default for SavedPlugin {
    fn default() -> Self {
        Self {
            enabled: false,
            model: DEFAULT_MODEL.into(),
            key: None,
            options: GenerationOptions::default(),
        }
    }
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
    #[serde(default)]
    model: String,
    #[serde(deserialize_with = "read_key")]
    api_key: Option<ApiKey>,
    #[serde(default)]
    options: GenerationOptions,
}

#[derive(Serialize)]
struct PluginDocument<'a> {
    version: u32,
    enabled: bool,
    model: &'a str,
    api_key: Option<&'a str>,
    options: &'a GenerationOptions,
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
    pub fn contains_settings(&self) -> bool {
        self.root.join("plugins.json").exists()
    }

    pub(crate) fn read_extra<T: DeserializeOwned>(
        &self,
        filename: &str,
    ) -> Result<Option<T>, String> {
        let path = self.root.join(filename);
        let metadata = match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(READ_ERROR.into()),
            Ok(metadata) => metadata,
        };
        if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
            return Err(INVALID_ERROR.into());
        }
        let file = File::open(path).map_err(|_| READ_ERROR.to_owned())?;
        let mut bytes = PrivateBytes(Vec::new());
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes.0)
            .map_err(|_| READ_ERROR.to_owned())?;
        if bytes.0.len() > MAX_BYTES {
            return Err(INVALID_ERROR.into());
        }
        serde_json::from_slice(&bytes.0)
            .map(Some)
            .map_err(|_| INVALID_ERROR.into())
    }

    pub(crate) fn save_extra<T: Serialize>(
        &self,
        filename: &str,
        settings: &T,
    ) -> Result<(), String> {
        let mut bytes =
            PrivateBytes(serde_json::to_vec_pretty(settings).map_err(|_| WRITE_ERROR.to_owned())?);
        bytes.0.push(b'\n');
        if bytes.0.len() > MAX_BYTES {
            return Err(WRITE_ERROR.into());
        }
        write_private(&self.root, filename, &bytes.0).map_err(|_| WRITE_ERROR.to_owned())
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

/// A test that saves settings must use a temporary folder, never the
/// person's `~/.openagents` (a saved model there changes what their Coder
/// runs and shows).
#[cfg(test)]
fn refuse_the_real_home(root: &Path) {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let temporary = |path: &Path| {
        path.starts_with(std::env::temp_dir())
            || path.starts_with("/tmp")
            || path.starts_with("/private")
    };
    assert!(
        temporary(&home) || !root.starts_with(home.join(".openagents")),
        "a test tried to save Coder settings under the real ~/.openagents: {}",
        root.display()
    );
}

impl Store {
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
        if stored.version != VERSION || !valid_model(&stored.model) || !stored.options.valid() {
            return Err(INVALID_ERROR.into());
        }
        Ok(SavedPlugin {
            enabled: stored.enabled,
            model: normalized_model(&stored.model).into(),
            key: stored.api_key,
            options: stored.options,
        })
    }

    /// Save the settings and key together, replacing the file atomically.
    ///
    /// # Errors
    /// Invalid existing settings are preserved. A failed write preserves the previous file.
    pub fn save(&self, settings: &SavedPlugin) -> Result<(), String> {
        #[cfg(test)]
        refuse_the_real_home(&self.root);
        // Do not overwrite a damaged or newer document if loading it failed.
        self.load()?;
        if !valid_model(&settings.model)
            || !settings.options.valid()
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
            model: normalized_model(&settings.model),
            api_key: settings.key.as_ref().map(ApiKey::expose),
            options: &settings.options,
        };
        let mut bytes =
            PrivateBytes(serde_json::to_vec_pretty(&document).map_err(|_| WRITE_ERROR.to_owned())?);
        bytes.0.push(b'\n');
        if bytes.0.len() > MAX_BYTES {
            return Err(WRITE_ERROR.into());
        }
        write_private(&self.root, "plugins.json", &bytes.0).map_err(|_| WRITE_ERROR.to_owned())
    }
}

fn valid_model(model: &str) -> bool {
    model.len() <= MAX_MODEL_BYTES && !model.chars().any(char::is_control)
}

fn normalized_model(model: &str) -> &str {
    if model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        model
    }
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

fn write_private(root: &Path, filename: &str, bytes: &[u8]) -> std::io::Result<()> {
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
        fs::rename(&temporary.0, root.join(filename))?;
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
            ..SavedPlugin::default()
        }
    }

    #[test]
    fn missing_settings_do_not_create_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("coder-new");
        let settings = Store::under(&root).load().unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.model, DEFAULT_MODEL);
        assert!(settings.key.is_none());
        assert_eq!(settings.options, GenerationOptions::default());
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

    #[test]
    fn version_one_settings_without_options_load_with_defaults() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("plugins.json");
        let store = Store::under(temporary.path());
        for (model, expected) in [
            ("", DEFAULT_MODEL),
            ("   ", DEFAULT_MODEL),
            ("example/model", "example/model"),
        ] {
            let document = serde_json::json!({
                "version": 1,
                "enabled": true,
                "model": model,
                "api_key": "fake-plugin-key",
            });
            let original = serde_json::to_vec(&document).unwrap();
            fs::write(&path, &original).unwrap();
            let loaded = store.load().unwrap();
            assert!(loaded.enabled);
            assert_eq!(loaded.model, expected);
            assert_eq!(loaded.options, GenerationOptions::default());
            assert_eq!(loaded.key.unwrap().expose(), "fake-plugin-key");
            assert_eq!(fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn missing_model_uses_auto_and_preserves_the_key_and_options() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("plugins.json");
        let store = Store::under(temporary.path());
        let document = serde_json::json!({
            "version": 1,
            "enabled": true,
            "api_key": "fake-plugin-key",
            "options": { "reasoning": "none", "max_tokens": null },
        });
        let original = serde_json::to_vec(&document).unwrap();
        fs::write(&path, &original).unwrap();

        let loaded = store.load().unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.model, DEFAULT_MODEL);
        assert!(!crate::models::pinned(&loaded.model));
        assert_eq!(loaded.key.as_ref().unwrap().expose(), "fake-plugin-key");
        assert_eq!(loaded.options.reasoning.as_deref(), Some("none"));
        assert_eq!(loaded.options.max_tokens, None);
        assert_eq!(fs::read(&path).unwrap(), original);

        store.save(&loaded).unwrap();
        let reopened = store.load().unwrap();
        assert_eq!(reopened.model, DEFAULT_MODEL);
        assert_eq!(reopened.key.unwrap().expose(), "fake-plugin-key");
        assert_eq!(reopened.options, loaded.options);
    }

    #[test]
    fn generation_options_round_trip_and_blank_models_use_free_router() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        let settings = SavedPlugin {
            model: "   ".into(),
            options: GenerationOptions {
                reasoning: Some("high".into()),
                max_tokens: Some(8_192),
            },
            ..settings()
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.model, DEFAULT_MODEL);
        assert_eq!(loaded.options, settings.options);
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(temporary.path().join("plugins.json")).unwrap())
                .unwrap();
        assert_eq!(document["version"], VERSION);
        assert_eq!(document["model"], DEFAULT_MODEL);
        assert_eq!(document["options"]["reasoning"], "high");
        assert_eq!(document["options"]["max_tokens"], 8_192);
    }

    #[test]
    fn invalid_generation_options_are_refused_without_overwriting_settings() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::under(temporary.path());
        let path = temporary.path().join("plugins.json");
        store.save(&settings()).unwrap();
        let original = fs::read(&path).unwrap();
        for options in [
            GenerationOptions {
                reasoning: Some("unsupported".into()),
                max_tokens: None,
            },
            GenerationOptions {
                reasoning: None,
                max_tokens: Some(0),
            },
            GenerationOptions {
                reasoning: None,
                max_tokens: Some(32_769),
            },
        ] {
            let invalid = SavedPlugin {
                options: options.clone(),
                ..settings()
            };
            assert_eq!(store.save(&invalid).unwrap_err(), WRITE_ERROR);
            assert_eq!(fs::read(&path).unwrap(), original);

            let mut document: serde_json::Value = serde_json::from_slice(&original).unwrap();
            document["options"] = serde_json::to_value(options).unwrap();
            let invalid_document = serde_json::to_vec(&document).unwrap();
            fs::write(&path, &invalid_document).unwrap();
            assert_eq!(store.load().unwrap_err(), INVALID_ERROR);
            assert!(store.save(&settings()).is_err());
            assert_eq!(fs::read(&path).unwrap(), invalid_document);
            fs::write(&path, &original).unwrap();
        }
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);
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
