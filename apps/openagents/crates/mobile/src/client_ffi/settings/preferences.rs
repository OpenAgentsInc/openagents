//! Device presentation preferences, stored by Rust without account credentials.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum SettingsAppearance {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum SettingsWallpaperEffect {
    #[default]
    None,
    Dither,
    Ascii,
    Halftone,
    Scanlines,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct SettingsWallpaper {
    pub uri: String,
    pub name: String,
    pub effect: SettingsWallpaperEffect,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct SettingsPreferences {
    pub schema: u32,
    pub appearance: SettingsAppearance,
    pub wallpaper: Option<SettingsWallpaper>,
}
impl Default for SettingsPreferences {
    fn default() -> Self {
        Self {
            schema: 1,
            appearance: Default::default(),
            wallpaper: None,
        }
    }
}
impl SettingsPreferences {
    fn valid(&self) -> bool {
        self.schema == 1
            && self.wallpaper.as_ref().is_none_or(|wallpaper| {
                wallpaper.uri.starts_with("content://")
                    && wallpaper.uri.len() <= 2048
                    && !wallpaper.uri.chars().any(char::is_control)
                    && !wallpaper.name.is_empty()
                    && wallpaper.name.len() <= 256
                    && !wallpaper.name.chars().any(char::is_control)
            })
    }
}
#[derive(Clone)]
pub(super) struct Store {
    path: PathBuf,
}
impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn read(&self) -> Result<SettingsPreferences, String> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Default::default());
            }
            Err(_) => return Err("Couldn't read your settings. Try again.".into()),
        };
        let mut bytes = Vec::new();
        file.take(8193)
            .read_to_end(&mut bytes)
            .map_err(|_| "Couldn't read your settings. Try again.")?;
        if bytes.len() > 8192 {
            return Err("Couldn't read your settings. Try again.".into());
        }
        let prefs: SettingsPreferences = serde_json::from_slice(&bytes)
            .map_err(|_| "Couldn't read your settings. Try again.")?;
        if !prefs.valid() {
            return Err("Couldn't read your settings. Try again.".into());
        }
        Ok(prefs)
    }
    pub fn write(&self, prefs: &SettingsPreferences) -> Result<(), String> {
        if !prefs.valid() {
            return Err("Choose another wallpaper and try again.".into());
        }
        let bytes =
            serde_json::to_vec(prefs).map_err(|_| "Couldn't save your settings. Try again.")?;
        let parent = self
            .path
            .parent()
            .ok_or("Couldn't save your settings. Try again.")?;
        fs::create_dir_all(parent).map_err(|_| "Couldn't save your settings. Try again.")?;
        let temporary = parent.join(format!("settings-{}.partial", uuid::Uuid::new_v4()));
        let write = (|| -> std::io::Result<()> {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if write.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write.map_err(|_| "Couldn't save your settings. Try again.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_keeps_presentation_preferences_without_confirmation_or_credentials() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().join("settings.json"));
        assert_eq!(store.read().unwrap(), SettingsPreferences::default());
        let prefs = SettingsPreferences {
            appearance: SettingsAppearance::Dark,
            wallpaper: Some(SettingsWallpaper {
                uri: "content://fixture/image/1".into(),
                name: "Test picture".into(),
                effect: SettingsWallpaperEffect::Dither,
            }),
            ..Default::default()
        };
        store.write(&prefs).unwrap();
        assert_eq!(
            Store::new(root.path().join("settings.json"))
                .read()
                .unwrap(),
            prefs
        );
        let text = fs::read_to_string(root.path().join("settings.json")).unwrap();
        for forbidden in ["credential", "forfeit", "confirm", "token", "account"] {
            assert!(!text.contains(forbidden));
        }
    }
    #[test]
    fn invalid_or_oversized_data_is_never_replaced_with_defaults() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        let store = Store::new(path.clone());
        for bytes in [b"bad".to_vec(), vec![b'x'; 8193]] {
            fs::write(&path, &bytes).unwrap();
            assert!(store.read().is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let prefs = SettingsPreferences {
            wallpaper: Some(SettingsWallpaper {
                uri: "https://fixture.invalid/image".into(),
                name: "Image".into(),
                effect: Default::default(),
            }),
            ..Default::default()
        };
        assert!(store.write(&prefs).is_err());
        assert_eq!(fs::read(&path).unwrap(), vec![b'x'; 8193]);
    }
}
