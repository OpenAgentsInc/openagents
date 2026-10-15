//! Atomic writes for explicitly selected private analysis directories.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err("Choose a file name without directories.".into());
    }
    let mut at = Some(dir);
    while let Some(path) = at {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Choose a directory without symbolic links.".into());
        }
        at = path.parent();
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = dir.join(format!(
        ".write-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, dir.join(name)).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
