//! Native Kiln jobs use the portable Kiln loop, remote checked execution,
//! and the existing NIP-34 bridge. WORK objectives remain independently owned.
pub mod objectives;
pub mod worker;

use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;
pub const BUILD: &str = match option_env!("OPENAGENTS_COMMIT") {
    Some(s) => s,
    None => "development",
};
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
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("native host state exceeds its bound".into());
    }
    let mut name = path.file_name().ok_or("file name missing")?.to_os_string();
    name.push(format!(".{:032x}.new", secp256k1::rand::random::<u128>()));
    let tmp = path.with_file_name(name);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    let mut f = opts.open(&tmp).map_err(|e| e.to_string())?;
    let written = f
        .write_all(&bytes)
        .and_then(|_| f.sync_all())
        .and_then(|_| std::fs::rename(&tmp, path));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.to_string());
    }
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::sync::Barrier;

    #[test]
    fn concurrent_job_and_cancel_saves_keep_their_own_values() {
        fn writing(path: &Path, value: &Value, barrier: &Barrier) -> Vec<String> {
            let mut errors = Vec::new();
            for _ in 0..32 {
                barrier.wait();
                if let Err(error) = save(path, value) {
                    errors.push(error);
                }
                barrier.wait();
            }
            errors
        }
        let dir = tempfile::tempdir().unwrap();
        let job = dir.path().join("attempt.json");
        let cancel = dir.path().join("attempt.cancel");
        let content = json!({"task":"fixture", "payload":"x".repeat(4096)});
        let cancellation = json!(true);
        let barrier = Barrier::new(2);
        std::thread::scope(|scope| {
            let left = scope.spawn(|| writing(&job, &content, &barrier));
            let right = scope.spawn(|| writing(&cancel, &cancellation, &barrier));
            assert!(left.join().unwrap().is_empty());
            assert!(right.join().unwrap().is_empty());
        });
        assert_eq!(load::<Value>(&job).unwrap(), content);
        assert_eq!(load::<Value>(&cancel).unwrap(), cancellation);
    }
}
