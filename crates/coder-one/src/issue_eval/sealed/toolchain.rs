//! Installed tools and private Cargo metadata for a read-confined evaluation.

use std::path::{Path, PathBuf};

/// Grant installed Rust tools and prefetched registry sources, not the
/// operator's Cargo credentials, configuration, workspaces, or histories.
pub(crate) fn scope(dir: &Path) -> Result<microluna::seal::ReadScope, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set")?;
    let cargo = std::env::var_os("CARGO_HOME").map_or_else(|| home.join(".cargo"), PathBuf::from);
    let rustup =
        std::env::var_os("RUSTUP_HOME").map_or_else(|| home.join(".rustup"), PathBuf::from);
    let cache = dir.join("cargo-home");
    std::fs::create_dir_all(&cache)
        .map_err(|error| format!("cannot create private Cargo home: {error}"))?;
    let cache = cache.canonicalize().map_err(|error| error.to_string())?;
    let mut readable = Vec::new();
    for path in [
        cargo.join("bin"),
        rustup.join("toolchains"),
        rustup.join("settings.toml"),
    ] {
        if path.exists() {
            readable.push(path.canonicalize().map_err(|error| error.to_string())?);
        }
    }
    // The host fetched Cargo.lock before this scope was built. Registry
    // sources stay read-only; locks and Cargo's usage database are private.
    let registry = cargo.join("registry");
    if registry.exists() {
        let registry = registry.canonicalize().map_err(|error| error.to_string())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&registry, cache.join("registry"))
            .map_err(|error| format!("cannot attach the prefetched registry: {error}"))?;
        readable.push(registry);
    }
    let mut environment = vec![("CARGO_HOME".into(), cache.clone().into_os_string())];
    #[cfg(target_os = "macos")]
    {
        let developer = std::env::var_os("DEVELOPER_DIR").map_or_else(
            || PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
            PathBuf::from,
        );
        if developer.is_dir() {
            let bundle = developer
                .ancestors()
                .find(|path| path.extension().is_some_and(|ext| ext == "app"))
                .unwrap_or(&developer);
            readable.push(bundle.canonicalize().map_err(|error| error.to_string())?);
            let sdk = developer.join("Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk");
            if sdk.exists() {
                environment.push(("SDKROOT".into(), sdk.into_os_string()));
            }
            environment.push(("DEVELOPER_DIR".into(), developer.into_os_string()));
            environment.push(("xcrun_nocache".into(), "1".into()));
        }
        // Xcode's license receipts are tool prerequisites, not the user's
        // preferences directory as a whole.
        for path in [
            "/Library/Preferences/com.apple.dt.Xcode.plist",
            "/Library/Preferences/com.apple.dt.CommandLineTools.plist",
        ] {
            let path = Path::new(path);
            if path.exists() {
                readable.push(path.canonicalize().map_err(|error| error.to_string())?);
            }
        }
    }
    if rustup.exists() {
        environment.push(("RUSTUP_HOME".into(), rustup.into_os_string()));
    }
    Ok(microluna::seal::ReadScope {
        readable,
        writable: vec![cache],
        environment,
    })
}
