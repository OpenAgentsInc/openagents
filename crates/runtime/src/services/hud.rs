//! HUD filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
    StreamHandle, WatchEvent, WatchHandle,
};
use crate::storage::AgentStorage;
use crate::types::AgentId;
use super::LogsFs;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;

const HUD_SETTINGS_KEY: &str = "hud:settings";
const DEFAULT_REDACTION_POLICY: &str = "standard";

/// HUD settings stored per agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HudSettings {
    /// Whether HUD is public.
    pub public: bool,
    /// Whether embeds are allowed.
    pub embed_allowed: bool,
    /// Redaction policy name.
    pub redaction_policy: String,
}

impl Default for HudSettings {
    fn default() -> Self {
        Self {
            public: true,
            embed_allowed: true,
            redaction_policy: DEFAULT_REDACTION_POLICY.to_string(),
        }
    }
}

/// HUD service providing redacted stream and settings.
#[derive(Clone)]
pub struct HudFs {
    agent_id: AgentId,
    storage: Arc<dyn AgentStorage>,
    logs: Arc<LogsFs>,
    settings: Arc<RwLock<HudSettings>>,
}

impl HudFs {
    /// Create a HUD service.
    pub fn new(agent_id: AgentId, storage: Arc<dyn AgentStorage>, logs: Arc<LogsFs>) -> Self {
        let settings = Self::load_settings(&storage, &agent_id).unwrap_or_default();
        Self {
            agent_id,
            storage,
            logs,
            settings: Arc::new(RwLock::new(settings)),
        }
    }

    fn load_settings(storage: &Arc<dyn AgentStorage>, agent_id: &AgentId) -> Option<HudSettings> {
        let data = futures::executor::block_on(storage.get(agent_id, HUD_SETTINGS_KEY)).ok()??;
        serde_json::from_slice(&data).ok()
    }

    fn save_settings(&self, settings: &HudSettings) -> FsResult<()> {
        let data =
            serde_json::to_vec_pretty(settings).map_err(|err| FsError::Other(err.to_string()))?;
        futures::executor::block_on(self.storage.set(&self.agent_id, HUD_SETTINGS_KEY, &data))
            .map_err(|err| FsError::Other(err.to_string()))
    }

    fn settings_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.settings.read().map_err(|_| FsError::Other("hud settings lock poisoned".to_string()))?;
        serde_json::to_vec_pretty(&*guard).map_err(|err| FsError::Other(err.to_string()))
    }

    fn stream_receiver(&self) -> FsResult<std::sync::mpsc::Receiver<Vec<u8>>> {
        let Some(inner) = self.logs.watch("trace")? else {
            return Err(FsError::NotFound);
        };
        let settings = self.settings.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut handle = HudWatchHandle::new(inner, settings);
            loop {
                match handle.next(None) {
                    Ok(Some(WatchEvent::Data(data))) => {
                        if tx.send(data).is_err() {
                            break;
                        }
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Ok(rx)
    }
}

impl FileService for HudFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "stream" => Ok(Box::new(StreamHandle::new(self.stream_receiver()?))),
            "settings" => {
                if flags.write || flags.create {
                    Ok(Box::new(SettingsWriteHandle::new(
                        self.settings.clone(),
                        self.clone(),
                    )))
                } else {
                    Ok(Box::new(BytesHandle::new(self.settings_json()?)))
                }
            }
            "" => Err(FsError::IsDirectory),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![
                DirEntry::file("stream", 0),
                DirEntry::file("settings", self.settings_json()?.len() as u64),
            ]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "stream" => Ok(Stat::file(0)),
            "settings" => Ok(Stat::file(self.settings_json()?.len() as u64)),
            _ => Err(FsError::NotFound),
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        match path {
            "stream" => {
                let Some(inner) = self.logs.watch("trace")? else {
                    return Ok(None);
                };
                Ok(Some(Box::new(HudWatchHandle::new(
                    inner,
                    self.settings.clone(),
                ))))
            }
            _ => Ok(None),
        }
    }

    fn name(&self) -> &str {
        "hud"
    }
}

struct SettingsWriteHandle {
    settings: Arc<RwLock<HudSettings>>,
    hud: HudFs,
    buffer: Vec<u8>,
}

impl SettingsWriteHandle {
    fn new(settings: Arc<RwLock<HudSettings>>, hud: HudFs) -> Self {
        Self {
            settings,
            hud,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for SettingsWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let settings: HudSettings = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        {
            let mut guard = self
                .settings
                .write()
                .map_err(|_| FsError::Other("hud settings lock poisoned".to_string()))?;
            *guard = settings.clone();
        }
        self.hud.save_settings(&settings)?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct HudWatchHandle {
    inner: Box<dyn WatchHandle>,
    settings: Arc<RwLock<HudSettings>>,
}

impl HudWatchHandle {
    fn new(inner: Box<dyn WatchHandle>, settings: Arc<RwLock<HudSettings>>) -> Self {
        Self { inner, settings }
    }

    fn sanitize_event(&self, data: &[u8]) -> Vec<u8> {
        let text = String::from_utf8_lossy(data).to_string();
        let settings = self
            .settings
            .read()
            .map(|guard| (*guard).clone())
            .unwrap_or_default();
        if settings.redaction_policy == "none" {
            return text.into_bytes();
        }

        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) {
            sanitize_value(&mut value);
            return serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
        }

        if should_redact_text(&text) {
            return b"[REDACTED]".to_vec();
        }

        text.into_bytes()
    }
}

impl WatchHandle for HudWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        match self.inner.next(timeout)? {
            Some(WatchEvent::Data(data)) => Ok(Some(WatchEvent::Data(self.sanitize_event(&data)))),
            Some(other) => Ok(Some(other)),
            None => Ok(None),
        }
    }

    fn close(&mut self) -> FsResult<()> {
        self.inner.close()
    }
}

fn should_redact_text(value: &str) -> bool {
    let lowered = value.to_lowercase();
    SECRET_MARKERS.iter().any(|marker| lowered.contains(marker))
}

fn sanitize_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if should_redact_key(key) {
                    *val = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    sanitize_value(val);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                sanitize_value(item);
            }
        }
        serde_json::Value::String(text) => {
            if should_redact_text(text) {
                *text = "[REDACTED]".to_string();
            }
        }
        _ => {}
    }
}

fn should_redact_key(key: &str) -> bool {
    let lowered = key.to_lowercase();
    REDACTED_FIELDS.iter().any(|field| lowered == *field)
}

const REDACTED_FIELDS: &[&str] = &[
    "token",
    "secret",
    "password",
    "api_key",
    "apikey",
    "authorization",
    "bearer",
];

const SECRET_MARKERS: &[&str] = &[
    "sk-",
    "ghp_",
    "xoxb-",
    "xoxp-",
    "token",
    "secret",
    "password",
    "api_key",
    "apikey",
    "authorization",
    "bearer ",
];
