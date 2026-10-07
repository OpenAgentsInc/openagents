//! The local account operator checks and explicitly publishes immutable terms.
use serde::Deserialize;
use std::path::Path;
use tenancy::accounts::referrals::attribution::Policy;
use tenancy::accounts::referrals::commission::Terms;

const INPUT_LIMIT: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: String,
    terms: String,
}
fn run() -> Result<(), String> {
    let words: Vec<_> = std::env::args().skip(1).collect();
    if words.first().is_some_and(|w| w.starts_with("commission-")) {
        return commission(&words);
    }
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
fn commission(words: &[String]) -> Result<(), String> {
    let usage = "usage: tenant-referrals commission-check --input FILE | commission-show --registry DIR [--digest DIGEST] | commission-publish --registry DIR --input FILE --approve DIGEST --expected DIGEST|none";
    let mut options = std::collections::BTreeMap::new();
    if !(words.len() - 1).is_multiple_of(2) {
        return Err(usage.into());
    }
    for pair in words[1..].chunks_exact(2) {
        if !pair[0].starts_with("--")
            || pair[1].is_empty()
            || options.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(usage.into());
        }
    }
    let command = words[0].as_str();
    let allowed: &[&str] = match command {
        "commission-check" => &["--input"],
        "commission-show" => &["--registry", "--digest"],
        "commission-publish" => &["--registry", "--input", "--approve", "--expected"],
        _ => return Err(usage.into()),
    };
    if options.keys().any(|name| !allowed.contains(name)) {
        return Err(usage.into());
    }
    let required = |name| options.get(name).copied().ok_or_else(|| usage.to_string());
    let terms = if command != "commission-show" {
        let path = Path::new(required("--input")?);
        if !path.is_absolute() {
            return Err("Use an absolute private terms input path.".into());
        }
        let t: Terms = serde_json::from_slice(&private_input(path)?)
            .map_err(|_| "Invalid commission terms input.")?;
        Some(if command == "commission-check" && t.digest.is_empty() {
            t.seal().map_err(|e| e.to_string())?
        } else {
            t.validate().map_err(|e| e.to_string())?;
            t
        })
    } else {
        None
    };
    let value = if command == "commission-check" {
        serde_json::json!({"terms":terms,"published":false,"accrual_enabled":false})
    } else {
        let dir = Path::new(required("--registry")?);
        private_registry(dir)?;
        let accounts = tenancy::Accounts::open(dir)
            .map_err(|_| "The canonical account store is unavailable.")?;
        if command == "commission-show" {
            serde_json::json!({"publication":accounts.commission_publication(options.get("--digest").copied()).map_err(|e| e.to_string())?,"accrual_enabled":false})
        } else {
            let expected = required("--expected")?;
            let published = accounts
                .publish_commission_terms(
                    terms.as_ref().unwrap(),
                    required("--approve")?,
                    if expected == "none" {
                        None
                    } else {
                        Some(expected)
                    },
                )
                .map_err(|e| e.to_string())?;
            serde_json::json!({"publication":published,"accrual_enabled":false})
        }
    };
    println!("{value}");
    Ok(())
}
#[cfg(unix)]
fn private_registry(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path).map_err(|_| "The account registry is unavailable.")?;
    if !path.is_absolute()
        || !m.is_dir()
        || m.mode() & 0o777 != 0o700
        || m.uid() != unsafe { libc::geteuid() }
    {
        return Err("Use an absolute owned account directory with mode 0700.".into());
    }
    Ok(())
}
#[cfg(not(unix))]
fn private_registry(_: &Path) -> Result<(), String> {
    Err("Private commission publication is unavailable on this platform.".into())
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
