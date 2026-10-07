//! The local account operator publishes immutable attribution terms.
use serde::Deserialize;
use std::path::Path;
use tenancy::accounts::referrals::attribution::Policy;

const INPUT_LIMIT: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: String,
    terms: String,
}
fn run() -> Result<(), String> {
    let words: Vec<_> = std::env::args().skip(1).collect();
    if words.len() != 5
        || words[0] != "publish"
        || words[1] != "--registry"
        || words[3] != "--input"
    {
        return Err("usage: tenant-referrals publish --registry DIR --input FILE".into());
    }
    let registry = Path::new(&words[2]);
    let input = Path::new(&words[4]);
    if !registry.is_absolute() || !input.is_absolute() {
        return Err("Use absolute account and private input paths.".into());
    }
    let bytes = private_input(input)?;
    let input: Input =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid private attribution policy input.")?;
    let policy = Policy::new(input.version, input.terms).map_err(|e| e.to_string())?;
    let accounts = tenancy::Accounts::open(registry)
        .map_err(|_| "The canonical account store is unavailable.")?;
    let published = accounts
        .publish_attribution_policy(&policy)
        .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::json!({"version":published.version,"digest":published.digest,"rule":published.rule,"commission_eligibility":false})
    );
    Ok(())
}
#[cfg(unix)]
fn private_input(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| "The private policy file is unavailable.")?;
    if !metadata.is_file()
        || metadata.mode() & 0o777 != 0o600
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.nlink() != 1
        || metadata.len() > INPUT_LIMIT
    {
        return Err("Use one bounded private regular policy file with mode 0600.".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "The private policy file is unavailable.")?;
    let opened = file
        .metadata()
        .map_err(|_| "The private policy file is unavailable.")?;
    if opened.dev() != metadata.dev()
        || opened.ino() != metadata.ino()
        || !opened.is_file()
        || opened.mode() & 0o777 != 0o600
        || opened.uid() != metadata.uid()
        || opened.nlink() != 1
        || opened.len() > INPUT_LIMIT
    {
        return Err("The private policy file changed.".into());
    }
    let mut bytes = vec![];
    file.by_ref()
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The private policy file is unavailable.")?;
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("The private policy file exceeds its bound.".into());
    }
    Ok(bytes)
}
#[cfg(not(unix))]
fn private_input(_: &Path) -> Result<Vec<u8>, String> {
    Err("Private attribution publication is unavailable on this platform.".into())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn publication_refuses_shared_or_linked_policy_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        std::fs::write(&path, b"private terms").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(private_input(&path).unwrap(), b"private terms");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private_input(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let alias = dir.path().join("alias.json");
        symlink(&path, &alias).unwrap();
        assert!(private_input(&alias).is_err());

        let shared = dir.path().join("shared.json");
        std::fs::hard_link(&path, &shared).unwrap();
        assert!(private_input(&path).is_err());
        assert!(private_input(&shared).is_err());
    }

    #[test]
    fn native_policy_bounds_survive_private_json_encoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("encoded-policy.json");
        let policy = Policy::new("encoded-bound".into(), "\"".repeat(4096)).unwrap();
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": policy.version,
            "terms": policy.terms,
        }))
        .unwrap();
        assert!(bytes.len() > 8192);
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let decoded: Input = serde_json::from_slice(&private_input(&path).unwrap()).unwrap();
        assert_eq!(Policy::new(decoded.version, decoded.terms).unwrap(), policy);
    }
}
