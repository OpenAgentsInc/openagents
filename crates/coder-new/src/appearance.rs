//! Terminal appearance preferences with caller-provided persistence.

use serde::{Deserialize, Serialize};

use crate::plugin_store::Store;

const FILENAME: &str = "appearance.json";
const VERSION: u32 = 1;
const LOAD_ERROR: &str = "Cannot load the saved appearance settings. The file was not changed.";
const SAVE_ERROR: &str =
    "Cannot save the appearance settings. The previous settings were not changed.";

/// Terminal appearance settings shared by the application and renderer.
#[derive(Default)]
pub struct Appearance {
    pub use_system_terminal_background: bool,
    store: Option<Store>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    #[serde(default)]
    use_system_terminal_background: bool,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: VERSION,
            use_system_terminal_background: false,
        }
    }
}

impl Appearance {
    /// Load saved preferences, or use defaults when no file has been saved.
    ///
    /// # Errors
    /// The file cannot be read or has an invalid or unsupported schema.
    pub fn load(&mut self, store: Store) -> Result<(), String> {
        self.store = Some(store.clone());
        let document = read(&store)?;
        self.use_system_terminal_background = document.use_system_terminal_background;
        Ok(())
    }

    /// Toggle the terminal background preference and save it when a store is attached.
    ///
    /// # Errors
    /// Invalid existing settings are preserved. A failed save leaves the preference unchanged.
    pub fn toggle_system_terminal_background(&mut self) -> Result<(), String> {
        let use_system_terminal_background = !self.use_system_terminal_background;
        if let Some(store) = &self.store {
            read(store)?;
            store
                .save_extra(
                    FILENAME,
                    &Document {
                        version: VERSION,
                        use_system_terminal_background,
                    },
                )
                .map_err(|_| SAVE_ERROR.to_owned())?;
        }
        self.use_system_terminal_background = use_system_terminal_background;
        Ok(())
    }
}

fn read(store: &Store) -> Result<Document, String> {
    let document = store
        .read_extra::<Document>(FILENAME)
        .map_err(|_| LOAD_ERROR.to_owned())?
        .unwrap_or_default();
    if document.version != VERSION {
        return Err(LOAD_ERROR.into());
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temporary() -> tempfile::TempDir {
        match std::env::var_os("OPENAGENTS_SCRATCH") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    #[test]
    fn missing_file_and_missing_preference_keep_the_existing_background() {
        let temporary = temporary();
        let root = temporary.path().join("coder-new");
        let mut appearance = Appearance::default();
        assert!(!appearance.use_system_terminal_background);
        appearance.load(Store::under(&root)).unwrap();
        assert!(!appearance.use_system_terminal_background);
        assert!(!root.exists());

        fs::create_dir(&root).unwrap();
        let path = root.join(FILENAME);
        fs::write(&path, r#"{"version":1}"#).unwrap();
        appearance.use_system_terminal_background = true;
        appearance.load(Store::under(&root)).unwrap();
        assert!(!appearance.use_system_terminal_background);
        assert_eq!(fs::read_to_string(path).unwrap(), r#"{"version":1}"#);
    }

    #[test]
    fn both_background_choices_survive_reloading() {
        let temporary = temporary();
        let store = Store::under(temporary.path().join("coder-new"));
        let mut appearance = Appearance::default();
        appearance.load(store.clone()).unwrap();
        for expected in [true, false] {
            appearance.toggle_system_terminal_background().unwrap();
            assert_eq!(appearance.use_system_terminal_background, expected);
            let mut reloaded = Appearance::default();
            reloaded.load(store.clone()).unwrap();
            assert_eq!(reloaded.use_system_terminal_background, expected);
            let document: serde_json::Value =
                serde_json::from_slice(&fs::read(store.root().join(FILENAME)).unwrap()).unwrap();
            assert_eq!(document["version"], VERSION);
            assert_eq!(document["use_system_terminal_background"], expected);
        }
    }

    #[test]
    fn toggling_preserves_documents_changed_to_invalid_or_newer_versions() {
        let temporary = temporary();
        let store = Store::under(temporary.path());
        let path = temporary.path().join(FILENAME);
        let mut appearance = Appearance::default();
        appearance.load(store).unwrap();
        appearance.toggle_system_terminal_background().unwrap();
        for document in [
            "invalid JSON",
            r#"{"version":2,"use_system_terminal_background":false}"#,
            r#"{"version":1,"use_system_terminal_background":"on"}"#,
            r#"{"version":1,"unexpected":true}"#,
        ] {
            fs::write(&path, document).unwrap();
            assert_eq!(
                appearance.toggle_system_terminal_background().unwrap_err(),
                LOAD_ERROR
            );
            assert!(appearance.use_system_terminal_background);
            assert_eq!(fs::read_to_string(&path).unwrap(), document);
        }
    }

    #[test]
    fn a_failed_load_keeps_the_store_attached_for_recovery() {
        let temporary = temporary();
        let path = temporary.path().join(FILENAME);
        fs::write(&path, "invalid JSON").unwrap();
        let mut appearance = Appearance::default();
        assert!(appearance.load(Store::under(temporary.path())).is_err());
        assert!(appearance.toggle_system_terminal_background().is_err());
        assert!(!appearance.use_system_terminal_background);
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid JSON");

        fs::write(&path, r#"{"version":1}"#).unwrap();
        appearance.toggle_system_terminal_background().unwrap();
        let mut reloaded = Appearance::default();
        reloaded.load(Store::under(temporary.path())).unwrap();
        assert!(reloaded.use_system_terminal_background);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_preserves_the_preference_and_saved_file() {
        use std::os::unix::fs::symlink;

        let temporary = temporary();
        let root = temporary.path().join("coder-new");
        fs::create_dir(&root).unwrap();
        let path = root.join(FILENAME);
        let document = r#"{"version":1,"use_system_terminal_background":true}"#;
        fs::write(&path, document).unwrap();
        let link = temporary.path().join("linked-root");
        symlink(&root, &link).unwrap();
        let mut appearance = Appearance::default();
        appearance.load(Store::under(link)).unwrap();

        assert_eq!(
            appearance.toggle_system_terminal_background().unwrap_err(),
            SAVE_ERROR
        );
        assert!(appearance.use_system_terminal_background);
        assert_eq!(fs::read_to_string(path).unwrap(), document);
    }
}
