//! The desktop world identity and Gym grant, separate from host credentials.
use coder_mobile::{BareGym, BarePresence};
use std::path::{Path, PathBuf};

/// The release app's Verse items.
const RELEASE_SERVICE: &str = "com.openagents.desktop.verse";
/// A dev build's own Verse items, so it never reads, prompts for, or
/// replaces the signed app's (#10096).
const DEV_SERVICE: &str = "com.openagents-dev.desktop.verse";
/// The keychain service this build keeps its world key and Gym grant
/// under.
pub const SERVICE: &str = service(crate::RELEASE);

const fn service(release: bool) -> &'static str {
    if release {
        RELEASE_SERVICE
    } else {
        DEV_SERVICE
    }
}

/// What a keychain read or write answered this session. Once one fails
/// (denied, cancelled, locked, or broken) every later one answers the same
/// without asking, until the app restarts: at most one prompt (#10096).
struct Latch(std::sync::Mutex<Option<String>>);

impl Latch {
    const fn new() -> Self {
        Self(std::sync::Mutex::new(None))
    }

    fn guard<T>(&self, call: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut failed = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(why) = failed.as_ref() {
            return Err(why.clone());
        }
        let answer = call();
        if let Err(why) = &answer {
            *failed = Some(format!(
                "{why} It won't be asked for again until OpenAgents restarts."
            ));
        }
        answer
    }
}

static KEYCHAIN: Latch = Latch::new();
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
            .map_err(|_| "Your Grid settings file is damaged".to_owned())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        _ => return Err("Could not read the Grid preferences".into()),
    };
    let _identity = IDENTITY_LOCK
        .lock()
        .map_err(|_| "Couldn't open your Grid key. Try again.".to_owned())?;
    let secret = load_or_create(read, write)?;
    let code = read(GYM)?;
    let check_relay = if relay.starts_with("ws:") {
        coder_connect::RelayPolicy::LoopbackTest
            .validate(relay)
            .map_err(|_| "This Grid server address isn't secure".to_owned())?;
        Some(relay.to_owned())
    } else {
        None
    };
    Ok(Launch {
        publication: None,
        presence: Some(BarePresence {
            secret_hex: secret,
            relay: Some(relay.into()),
            name: None,
        }),
        gym: BareGym {
            code,
            panel: true,
            results_panel: true,
            evals_panel: true,
            notes: preferences.notes,
            check_relay,
            results_cache_directory: Some(directory.join("results").to_string_lossy().into_owned()),
            // The lists `openagents verse block` and the Verse app keep.
            blocklist_directory: Some(
                root.join(".openagents/verse")
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..BareGym::default()
        },
    })
}

/// The world key, as the chat's Gym uses it for its trainer (#10060): the
/// same verified key Play uses, made once if there is none, and never
/// replaced when it can't be read.
pub fn world_key() -> Result<secp256k1::SecretKey, String> {
    let _identity = IDENTITY_LOCK
        .lock()
        .map_err(|_| "Couldn't open your Grid key. Try again.".to_owned())?;
    load_or_create(read, write)?
        .parse()
        .map_err(|_| "Your saved Grid key is damaged".to_owned())
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
    let returned = read(WORLD_KEY)?.ok_or("Couldn't save your Grid key")?;
    if returned != secret {
        return Err("Your Grid key didn't save correctly".into());
    }
    validate_key(returned)
}

fn validate_key(secret: String) -> Result<String, String> {
    if secret.len() != 64 || secret.parse::<secp256k1::SecretKey>().is_err() {
        return Err("Your saved Grid key is damaged".into());
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
        .map_err(|_| "Could not read the Gym connection file".to_owned())?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err("Use a regular Gym connection file under 64 KiB".into());
    }
    let mut text = String::new();
    (&mut file)
        .take(65_537)
        .read_to_string(&mut text)
        .map_err(|_| "The Gym connection is not text".to_owned())?;
    if text.len() > 65_536 {
        return Err("The Gym connection file is too large".into());
    }
    Ok(text.trim().into())
}

pub fn save_connection(code: &str) -> Result<(), String> {
    write(GYM, code)
}

fn read(account: &str) -> Result<Option<String>, String> {
    KEYCHAIN.guard(|| platform_read(account))
}

fn write(account: &str, value: &str) -> Result<(), String> {
    KEYCHAIN.guard(|| platform_write(account, value))
}

#[cfg(target_os = "linux")]
fn platform_read(account: &str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE, account)
        .map_err(|_| "Couldn't open your keychain".to_owned())?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Couldn't read your keychain, so Play is offline.".into()),
    }
}
#[cfg(target_os = "linux")]
fn platform_write(account: &str, value: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, account)
        .and_then(|entry| entry.set_password(value))
        .map_err(|_| "Couldn't save to your keychain".into())
}
#[cfg(target_os = "macos")]
fn platform_read(account: &str) -> Result<Option<String>, String> {
    match security_framework::os::macos::passwords::find_generic_password(None, SERVICE, account) {
        Ok((value, _)) => String::from_utf8(value.to_vec())
            .map(Some)
            .map_err(|_| "Your saved Grid key is damaged".into()),
        Err(error) if error.code() == -25_300 => Ok(None),
        Err(_) => Err("Couldn't read your keychain, so Play is offline.".into()),
    }
}
#[cfg(target_os = "macos")]
fn platform_write(account: &str, value: &str) -> Result<(), String> {
    security_framework::os::macos::keychain::SecKeychain::default()
        .and_then(|keychain| keychain.set_generic_password(SERVICE, account, value.as_bytes()))
        .map_err(|_| "Couldn't save to your keychain".into())
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
    /// A dev build (every test build, and any build without the packaging
    /// scripts' flag) keeps its own items, never the release app's.
    #[test]
    fn a_dev_build_uses_its_own_keychain_service() {
        // Tests are dev builds unless built with the packaging flag.
        let release = crate::RELEASE;
        assert!(!release, "tests are never built as the release");
        assert_eq!(SERVICE, "com.openagents-dev.desktop.verse");
        assert!(!SERVICE.starts_with("com.openagents.desktop"));
        assert_eq!(service(true), "com.openagents.desktop.verse");
        assert!(crate::release_flag(Some("1")));
        for other in [None, Some(""), Some("0"), Some("true"), Some("11")] {
            assert!(!crate::release_flag(other));
        }
    }
    /// A denied or cancelled read is remembered: nothing asks again.
    #[test]
    fn a_failed_keychain_call_is_never_repeated_this_session() {
        let latch = Latch::new();
        let calls = std::cell::Cell::new(0);
        let call = |answer: Result<Option<String>, String>| {
            calls.set(calls.get() + 1);
            answer
        };
        assert_eq!(latch.guard(|| call(Ok(None))), Ok(None));
        assert!(latch.guard(|| call(Err("Denied.".into()))).is_err());
        let again = latch.guard(|| call(Ok(Some("key".into()))));
        assert!(again.unwrap_err().starts_with("Denied."));
        assert_eq!(calls.get(), 2, "the call after a denial never ran");
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
