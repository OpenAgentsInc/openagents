//! Native Kiln jobs use the portable Kiln loop, remote checked execution,
//! and the existing NIP-34 bridge. WORK objectives remain independently owned.
pub mod objectives;
pub mod worker;

use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&workspace_host::artifact::read(path, 8 * 1024 * 1024)?)
        .map_err(|e| e.to_string())
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().ok_or("state parent missing")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("new");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    let mut f = opts.open(&tmp).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("native host state exceeds its bound".into());
    }
    f.write_all(&bytes).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())?;
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
