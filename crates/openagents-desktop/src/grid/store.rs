//! The desktop world identity and Gym grant, separate from host credentials.
use coder_mobile::{BareGym, BarePresence};
use std::path::{Path, PathBuf};

const SERVICE: &str = "com.openagents.desktop.verse";
const WORLD_KEY: &str = "world-key";
const GYM: &str = "gym-connection";
static IDENTITY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct Launch {
    pub presence: Option<BarePresence>,
    pub gym: BareGym,
    pub publication: Option<super::fixture::Publication>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    notes: bool,
}

pub fn launch(root: &Path, relay: &str, fixture: bool) -> Result<Launch, String> {
    let directory = root.join(".openagents/desktop/grid");
    if fixture {
        let publication = super::fixture::Publication::new()?;
        return Ok(Launch {
            presence: None,
            gym: BareGym {
                panel: true,
                results_panel: true,
                evals_panel: true,
                preview: true,
                xp_preview: true,
                results_base: Some(publication.base.clone()),
                results_cache_directory: Some(publication.cache.to_string_lossy().into_owned()),
                ..BareGym::default()
            },
            publication: Some(publication),
        });
    }
    let preferences = match std::fs::read(directory.join("preferences.json")) {
        Ok(bytes) if bytes.len() <= 1024 => serde_json::from_slice(&bytes)
            .map_err(|_| "The Grid preferences are invalid".to_owned())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        _ => return Err("Could not read the Grid preferences".into()),
    };
    let _identity = IDENTITY_LOCK
        .lock()
        .map_err(|_| "The world identity store is unavailable".to_owned())?;
    let secret = load_or_create(read, write)?;
    let code = read(GYM)?;
    let check_relay = if relay.starts_with("ws:") {
        coder_connect::RelayPolicy::LoopbackTest
            .validate(relay)
            .map_err(|_| "Use a secure relay or an explicit loopback fixture".to_owned())?;
        Some(relay.to_owned())
    } else {
        None
    };
    Ok(Launch {
        publication: None,
        presence: Some(BarePresence {
            secret_hex: secret,
            relay: Some(relay.into()),
        }),
        gym: BareGym {
            code,
            panel: true,
            results_panel: true,
            evals_panel: true,
            notes: preferences.notes,
            check_relay,
            results_cache_directory: Some(directory.join("results").to_string_lossy().into_owned()),
            ..BareGym::default()
        },
    })
}

fn load_or_create(
    mut read: impl FnMut(&str) -> Result<Option<String>, String>,
    mut write: impl FnMut(&str, &str) -> Result<(), String>,
) -> Result<String, String> {
    if let Some(secret) = read(WORLD_KEY)? {
        return validate_key(secret);
    }
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng())
        .display_secret()
        .to_string();
    write(WORLD_KEY, &secret)?;
    let returned = read(WORLD_KEY)?.ok_or("The world identity was not saved")?;
    if returned != secret {
        return Err("The world identity could not be verified".into());
    }
    validate_key(returned)
}

fn validate_key(secret: String) -> Result<String, String> {
    if secret.len() != 64 || secret.parse::<secp256k1::SecretKey>().is_err() {
        return Err("The saved world identity is invalid".into());
    }
    Ok(secret)
}

pub fn save_notes(root: &Path, notes: bool) -> Result<(), String> {
    let dir = root.join(".openagents/desktop/grid");
    std::fs::create_dir_all(&dir).map_err(|_| "Could not save Compare notes".to_owned())?;
    let bytes = serde_json::to_vec(&Preferences { notes })
        .map_err(|_| "Could not save Compare notes".to_owned())?;
    let tmp = dir.join(format!("preferences-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, dir.join("preferences.json")))
        .map_err(|_| "Could not save Compare notes".to_owned())
}

pub fn pick_connection() -> Result<Option<String>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Open a gym-connect connection file")
        .pick_file()
    else {
        return Ok(None);
    };
    read_connection(&path).map(Some)
}

pub fn read_connection(path: &Path) -> Result<String, String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Could not read the Gym connection file".to_owned())?;
    let metadata = file
        .metadata()
        .map_err(|_| "Could not inspect the Gym connection file".to_owned())?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err("Use a regular Gym connection file under 64 KiB".into());
    }
    let mut text = String::new();
    (&mut file)
        .take(65_537)
        .read_to_string(&mut text)
        .map_err(|_| "The Gym connection is not text".to_owned())?;
    if text.len() > 65_536 {
        return Err("The Gym connection exceeds its size limit".into());
    }
    Ok(text.trim().into())
}

pub fn save_connection(code: &str) -> Result<(), String> {
    write(GYM, code)
}

#[cfg(target_os = "linux")]
fn read(account: &str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE, account)
        .map_err(|_| "Could not open the world identity store".to_owned())?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Could not read the world identity store. Play stays offline.".into()),
    }
}
#[cfg(target_os = "linux")]
fn write(account: &str, value: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, account)
        .and_then(|entry| entry.set_password(value))
        .map_err(|_| "Could not save the world identity or Gym connection".into())
}
#[cfg(target_os = "macos")]
fn read(account: &str) -> Result<Option<String>, String> {
    match security_framework::os::macos::passwords::find_generic_password(None, SERVICE, account) {
        Ok((value, _)) => String::from_utf8(value.to_vec())
            .map(Some)
            .map_err(|_| "The world identity store contains invalid text".into()),
        Err(error) if error.code() == -25_300 => Ok(None),
        Err(_) => Err("Could not read the world identity store. Play stays offline.".into()),
    }
}
#[cfg(target_os = "macos")]
fn write(account: &str, value: &str) -> Result<(), String> {
    security_framework::os::macos::keychain::SecKeychain::default()
        .and_then(|keychain| keychain.set_generic_password(SERVICE, account, value.as_bytes()))
        .map_err(|_| "Could not save the world identity or Gym connection".into())
}

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unreadable_or_corrupt_identity_is_never_replaced() {
        let writes = std::cell::Cell::new(0);
        let write = |_: &str, _: &str| {
            writes.set(writes.get() + 1);
            Ok(())
        };
        assert!(load_or_create(|_| Err("Locked".into()), write).is_err());
        assert!(load_or_create(|_| Ok(Some("bad".into())), write).is_err());
        assert_eq!(writes.get(), 0);
    }
    #[test]
    fn only_verified_separate_world_credentials_are_created() {
        let saved = std::cell::RefCell::new(None);
        let secret = load_or_create(
            |account| {
                assert_eq!(account, WORLD_KEY);
                Ok(saved.borrow().clone())
            },
            |account, value| {
                assert_eq!(account, WORLD_KEY);
                *saved.borrow_mut() = Some(value.to_owned());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(Some(secret), *saved.borrow());
        assert_ne!(SERVICE, "com.openagents.desktop");
    }
    #[test]
    fn connection_files_are_bounded_regular_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("code");
        std::fs::write(&path, "gym-connect:fixture").unwrap();
        assert_eq!(read_connection(&path).unwrap(), "gym-connect:fixture");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_connection(&link).is_err());
        std::fs::write(&path, vec![b'x'; 65_537]).unwrap();
        assert!(read_connection(&path).is_err());
        assert!(read_connection(dir.path()).is_err());
    }
}
