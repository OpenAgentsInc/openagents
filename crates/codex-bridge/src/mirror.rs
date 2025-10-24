use anyhow::*;
use serde::Serialize;
use std::{fs, io::Write, path::{Path, PathBuf}, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum MirrorEvent<'a> {
    #[serde(rename = "thread_upsert")]
    ThreadUpsert { thread_id: &'a str, title: Option<&'a str>, project_id: Option<&'a str>, created_at: Option<u64>, updated_at: Option<u64> },
    #[serde(rename = "message_create")]
    MessageCreate { thread_id: &'a str, role: &'a str, text: &'a str, ts: u64 },
}

#[derive(Debug, Clone)]
pub struct ConvexMirror {
    dir: PathBuf,
    file: Arc<Mutex<Option<fs::File>>>,
}

impl ConvexMirror {
    pub fn new(dir: PathBuf) -> Self {
        if let Some(parent) = dir.parent() { let _ = fs::create_dir_all(parent); }
        let _ = fs::create_dir_all(&dir);
        Self { dir: dir.clone(), file: Arc::new(Mutex::new(None)) }
    }

    fn spool_path(&self) -> PathBuf { self.dir.join("spool.jsonl") }

    fn ensure_file(&self) -> Result<fs::File> {
        let path = self.spool_path();
        let f = fs::OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(f)
    }

    pub async fn append(&self, ev: &MirrorEvent<'_>) -> Result<()> {
        let mut lock = self.file.lock().await;
        if lock.is_none() {
            *lock = Some(self.ensure_file()?);
        }
        if let Some(f) = lock.as_mut() {
            let line = serde_json::to_string(ev)?;
            f.write_all(line.as_bytes())?;
            f.write_all(b"\n")?;
            f.flush()?;
        }
        Ok(())
    }
}

pub fn default_mirror_dir() -> PathBuf {
    if let Some(home) = std::env::var("HOME").ok() {
        Path::new(&home).join(".openagents/convex/mirror")
    } else {
        Path::new(".openagents/convex/mirror").to_path_buf()
    }
}
