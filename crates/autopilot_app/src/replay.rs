use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::AppEvent;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayKind {
    AppEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRecord {
    pub kind: ReplayKind,
    pub timestamp_ms: u128,
    pub payload: AppEvent,
}

impl ReplayRecord {
    pub fn event(payload: AppEvent) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Self {
            kind: ReplayKind::AppEvent,
            timestamp_ms,
            payload,
        }
    }
}

pub struct EventRecorder {
    writer: BufWriter<File>,
}

impl EventRecorder {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::create(path.as_ref())
            .with_context(|| format!("create replay file {}", path.as_ref().display()))?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn record_event(&mut self, event: &AppEvent) -> Result<()> {
        let record = ReplayRecord::event(event.clone());
        serde_json::to_writer(&mut self.writer, &record).context("serialize replay record")?;
        self.writer.write_all(b"\n").context("write newline")?;
        self.writer.flush().context("flush replay writer")
    }
}

pub struct ReplayReader {
    reader: BufReader<File>,
    buffer: String,
}

impl ReplayReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("open replay file {}", path.as_ref().display()))?;
        Ok(Self {
            reader: BufReader::new(file),
            buffer: String::new(),
        })
    }

    pub fn next_record(&mut self) -> Result<Option<ReplayRecord>> {
        self.buffer.clear();
        let read = self.reader.read_line(&mut self.buffer)?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = self.buffer.trim_end();
        let record: ReplayRecord = serde_json::from_str(trimmed).context("parse replay record")?;
        Ok(Some(record))
    }

    pub fn read_all(mut self) -> Result<Vec<ReplayRecord>> {
        let mut records = Vec::new();
        while let Some(record) = self.next_record()? {
            records.push(record);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SessionId, UserAction, WorkspaceId};
    use std::fs;

    #[test]
    fn record_and_replay_events() {
        let tmp = std::env::temp_dir();
        let path = tmp.join(format!("autopilot-replay-{}.jsonl", uuid::Uuid::new_v4()));

        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let event = AppEvent::UserActionDispatched {
            workspace_id,
            action: UserAction::Command {
                session_id,
                name: "status".to_string(),
                args: vec![],
            },
        };

        {
            let mut recorder = EventRecorder::create(&path).expect("recorder");
            recorder.record_event(&event).expect("record event");
        }

        let mut reader = ReplayReader::open(&path).expect("reader");
        let record = reader.next_record().expect("read record").expect("record");
        assert_eq!(record.kind, ReplayKind::AppEvent);
        assert_eq!(record.payload, event);

        let _ = fs::remove_file(path);
    }
}
