//! The Spark wallet on a computer: where it lives and how it opens.
//!
//! The home is `~/.openagents/spark` (`OPENAGENTS_SPARK_HOME` overrides). The
//! seed is BIP39 entropy as hex in `seed`, a 0600 file in a 0700 directory,
//! the way the computer keeps its other keys; it is the same seed as the
//! phone's wallet, brought here by `openagents wallet link` or typed with
//! `openagents wallet restore`. Breez's records are a JSON file
//! ([`crate::store`]) under `wallets/<fingerprint>/`, so another seed never
//! reads this one's history. The wallet always runs on Bitcoin mainnet, as
//! the phone's does; nothing here switches it to another network. The
//! x402 receiver's Lightning node (`~/.openagents/wallet`) is separate and
//! never opened here.

use crate::seed::Seed;
use crate::spark::SparkNode;
use breez_sdk_spark::Network;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The only network a computer's wallet runs on.
pub const NETWORK: Network = Network::Mainnet;
/// The environment variable that moves the home, for tests and scratch runs.
pub const HOME_ENV: &str = "OPENAGENTS_SPARK_HOME";
const SEED_FILE: &str = "seed";

/// This computer's wallet home.
#[must_use]
pub fn home() -> PathBuf {
    if let Some(home) = std::env::var_os(HOME_ENV) {
        return PathBuf::from(home);
    }
    let user = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    user.join(".openagents").join("spark")
}

/// Whether a seed is kept under `home`.
#[must_use]
pub fn has_seed(home: &Path) -> bool {
    home.join(SEED_FILE).is_file()
}

/// The seed kept under `home`, if any.
///
/// # Errors
///
/// When the file exists but cannot be read or holds no valid seed. The
/// message never contains the seed.
pub fn load_seed(home: &Path) -> Result<Option<Seed>, String> {
    let path = home.join(SEED_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("This computer's wallet key could not be read.".into()),
    };
    let entropy = unhex(text.trim())
        .ok_or_else(|| "This computer's wallet key is unreadable.".to_string())?;
    Seed::from_entropy(entropy).map(Some)
}

/// Keep `seed` under `home`: a 0600 file, written whole, in a 0700
/// directory. A different seed already there is refused unless `replace`.
///
/// # Errors
///
/// When a different seed is kept and `replace` is false, or the file
/// cannot be written.
pub fn save_seed(home: &Path, seed: &Seed, replace: bool) -> Result<(), String> {
    if let Some(held) = load_seed(home)? {
        if held.entropy == seed.entropy {
            return Ok(());
        }
        if !replace {
            return Err("This computer already has a different wallet. Run the command again with --replace to use this one instead; the old one's recovery words are the only way back to it.".into());
        }
    }
    private_dir(home)?;
    let path = home.join(SEED_FILE);
    let temporary = home.join(format!("{SEED_FILE}.tmp"));
    let write = || -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(hex(&seed.entropy).as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        std::fs::rename(&temporary, &path)
    };
    write().map_err(|_| "This computer's wallet key could not be saved.".to_string())
}

/// Open this computer's wallet on mainnet. Blocking; it reaches Spark.
///
/// # Errors
///
/// When no seed is kept here ([`NOT_SET_UP`]), the store cannot be opened,
/// or Spark cannot be reached.
pub fn open(home: &Path) -> Result<SparkNode, String> {
    let seed = load_seed(home)?.ok_or_else(|| NOT_SET_UP.to_string())?;
    open_with(home, &seed, NETWORK)
}

/// Open the wallet for `seed` under `home` on `network`. Only tests choose
/// another network than [`NETWORK`].
///
/// # Errors
///
/// When the store cannot be opened or Spark cannot be reached.
pub fn open_with(home: &Path, seed: &Seed, network: Network) -> Result<SparkNode, String> {
    let dir =
        home.join("wallets")
            .join(format!("{}-{}", network_name(network), seed.fingerprint()));
    private_dir(&home.join("wallets"))?;
    let storage = crate::store::backend(&dir.join("store.json"))?;
    SparkNode::open(storage, network, &seed.mnemonic)
}

/// What a computer without the wallet says.
pub const NOT_SET_UP: &str = "This computer doesn't have your wallet yet. Run `openagents wallet link` to bring it over from your phone, or `openagents wallet restore` to type your recovery words.";

fn network_name(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Regtest => "regtest",
        _ => "other",
    }
}

fn private_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|_| "The wallet's folder could not be made.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "The wallet's folder could not be made private.".to_string())?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_is_a_private_file_and_never_silently_replaced() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("spark");
        assert!(!has_seed(&home));
        assert!(load_seed(&home).expect("read").is_none());
        let first = Seed::from_entropy(vec![1u8; 16]).expect("seed");
        save_seed(&home, &first, false).expect("saved");
        assert!(has_seed(&home));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&home.join(SEED_FILE)), 0o600);
            assert_eq!(mode(&home), 0o700);
        }
        let read = load_seed(&home).expect("read").expect("a seed");
        assert_eq!(read.entropy, first.entropy);
        // The same seed again is fine; another is refused without replace.
        save_seed(&home, &first, false).expect("same seed");
        let second = Seed::from_entropy(vec![2u8; 32]).expect("seed");
        let refused = save_seed(&home, &second, false).unwrap_err();
        assert!(refused.contains("--replace"), "{refused}");
        assert!(!refused.contains(&second.mnemonic));
        save_seed(&home, &second, true).expect("replaced");
        assert_eq!(load_seed(&home).unwrap().unwrap().entropy, second.entropy);
        assert!(!home.join("seed.tmp").exists());
    }

    #[test]
    fn a_damaged_seed_file_is_reported_without_its_contents() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(SEED_FILE), "zz-not-hex").unwrap();
        let error = load_seed(dir.path()).err().expect("refused");
        assert!(!error.contains("zz-not-hex"), "{error}");
        let missing = open(&dir.path().join("none")).err().expect("not set up");
        assert_eq!(missing, NOT_SET_UP);
    }
}
