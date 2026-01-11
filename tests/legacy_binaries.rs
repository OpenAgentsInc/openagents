use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_legacy_deprecation_messages() {
    let warnings = [
        wallet::deprecation::legacy_warning(),
        marketplace::deprecation::legacy_warning(),
        gitafter::deprecation::legacy_warning(),
        autopilot_core::deprecation::autopilot_warning(),
        autopilot_core::deprecation::autopilotd_warning(),
    ];

    for warning in warnings {
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents"));
    }
}

#[cfg(unix)]
#[test]
fn test_install_legacy_symlinks_creates_links() {
    let temp_dir = std::env::temp_dir().join(format!("openagents-links-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let target = temp_dir.join("openagents");
    fs::write(&target, "#!/bin/sh\necho openagents\n").expect("write target");
    let mut perms = fs::metadata(&target)
        .expect("target metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target, perms).expect("chmod target");

    let script = repo_root().join("scripts/install-legacy-symlinks.sh");
    let status = Command::new(script)
        .arg("--bin-dir")
        .arg(&temp_dir)
        .arg("--target")
        .arg(&target)
        .status()
        .expect("run symlink script");
    assert!(status.success(), "symlink script failed");

    for name in [
        "wallet",
        "marketplace",
        "autopilot",
        "autopilotd",
        "gitafter",
    ] {
        let link = temp_dir.join(name);
        assert!(link.exists(), "missing symlink: {}", name);
        let dest = fs::read_link(&link).expect("read symlink");
        assert_eq!(dest, target);
    }

    fs::remove_dir_all(&temp_dir).expect("cleanup temp dir");
}
