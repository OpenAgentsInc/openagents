//! Application storage paths. Provider credentials keep their own locations.
//!
//! The app keeps its own data directory (see `coder_brand`), never Zeron's
//! `~/.zeron` or `%LOCALAPPDATA%\Zeron`, so installing it never reads a Zeron
//! login, session, or database. A data directory left under the name used
//! before the OpenAgents rename (`Coder Desktop`) is moved into place once.

use std::ffi::OsString;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let dir = resolve_data_dir(|name| std::env::var_os(name));
    if std::env::var_os("ZERON_DATA_DIR").is_none_or(|dir| dir.is_empty()) {
        adopt_legacy(&dir);
    }
    dir
}

/// Move the pre-rename data directory (same parent, legacy folder name) to
/// `dir` when only the legacy one exists. A failed rename leaves both as they
/// were; the app then starts with an empty directory and the old one stays.
fn adopt_legacy(dir: &std::path::Path) {
    let Some(parent) = dir.parent() else { return };
    let legacy_name = if cfg!(all(not(windows), not(target_os = "macos"))) {
        coder_brand::LEGACY_LINUX_DATA_DIR_NAME
    } else {
        coder_brand::LEGACY_DATA_DIR_NAME
    };
    let legacy = parent.join(legacy_name);
    if legacy != dir && !dir.exists() && legacy.is_dir() {
        if let Err(error) = std::fs::rename(&legacy, dir) {
            eprintln!(
                "couldn't move {} to {}: {error}",
                legacy.display(),
                dir.display()
            );
        }
    }
}

fn resolve_data_dir(mut env: impl FnMut(&str) -> Option<OsString>) -> PathBuf {
    if let Some(dir) = env("ZERON_DATA_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    {
        // Explorer does not set HOME. Do not let a shell-specific HOME select
        // a different workspace from a desktop launch, or migrate credentials
        // between Unix-style and native Windows directories implicitly.
        let local = env("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join("AppData").join("Local"))
            })
            .expect("LOCALAPPDATA and USERPROFILE not set; set ZERON_DATA_DIR");
        local.join(coder_brand::DATA_DIR_NAME)
    }
    #[cfg(target_os = "macos")]
    {
        let home = PathBuf::from(env("HOME").expect("HOME not set"));
        home.join("Library")
            .join("Application Support")
            .join(coder_brand::DATA_DIR_NAME)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let data_home = env("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env("HOME").expect("HOME not set"))
                    .join(".local")
                    .join("share")
            });
        data_home.join(coder_brand::LINUX_DATA_DIR_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_data_directory_moves_into_place_once() {
        let root = tempfile::tempdir().unwrap();
        let legacy_name = if cfg!(all(not(windows), not(target_os = "macos"))) {
            coder_brand::LEGACY_LINUX_DATA_DIR_NAME
        } else {
            coder_brand::LEGACY_DATA_DIR_NAME
        };
        let legacy = root.path().join(legacy_name);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("device-id"), "abc").unwrap();
        let dir = root.path().join(coder_brand::DATA_DIR_NAME);
        adopt_legacy(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("device-id")).unwrap(), "abc");
        assert!(!legacy.exists());
        // An existing directory is never replaced.
        std::fs::create_dir_all(&legacy).unwrap();
        adopt_legacy(&dir);
        assert!(legacy.exists());
    }

    fn resolve(vars: &[(&str, &str)]) -> PathBuf {
        resolve_data_dir(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.into())
        })
    }

    #[test]
    fn explicit_data_dir_needs_no_home() {
        assert_eq!(
            resolve(&[("ZERON_DATA_DIR", "custom data")]),
            PathBuf::from("custom data")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_default_is_coder_application_support() {
        assert_eq!(
            resolve(&[("HOME", "/Users/u")]),
            PathBuf::from("/Users/u/Library/Application Support/OpenAgents Desktop"),
        );
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    #[test]
    fn linux_default_is_xdg_data_home() {
        assert_eq!(
            resolve(&[("HOME", "/home/u")]),
            PathBuf::from("/home/u/.local/share/coder-desktop"),
        );
        assert_eq!(
            resolve(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data")]),
            PathBuf::from("/data/coder-desktop"),
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn default_never_uses_zeron_data() {
        let dir = resolve(&[("HOME", "/home/u")]);
        assert!(!dir.to_string_lossy().contains("zeron"));
        assert!(!dir.to_string_lossy().contains("Zeron"));
    }

    #[cfg(windows)]
    #[test]
    fn explorer_launch_without_home_uses_local_app_data() {
        assert_eq!(
            resolve(&[("LOCALAPPDATA", r"C:\Users\Test User\AppData\Local")]),
            PathBuf::from(r"C:\Users\Test User\AppData\Local\OpenAgents Desktop"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_profile_fallback_handles_unicode_and_apostrophes() {
        assert_eq!(
            resolve(&[("USERPROFILE", r"C:\Users\O'Brien 日本語")]),
            PathBuf::from(r"C:\Users\O'Brien 日本語\AppData\Local\OpenAgents Desktop"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_does_not_depend_on_shell_home() {
        assert_eq!(
            resolve(&[("HOME", r"D:\msys-home"), ("LOCALAPPDATA", r"C:\Local")]),
            PathBuf::from(r"C:\Local\OpenAgents Desktop"),
        );
    }
}
