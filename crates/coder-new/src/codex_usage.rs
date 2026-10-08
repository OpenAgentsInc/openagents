//! Read usage and model metadata from only the Codex session this process started.

use crate::bundled_runtime::RuntimeEvent;
use serde_json::Value;
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct Reader {
    root: Option<PathBuf>,
    file: Option<File>,
    tail: coder_delegate::tail::Reader,
    offset: u64,
    next: Instant,
}

impl Reader {
    pub(crate) fn new() -> Self {
        #[cfg(test)]
        let root = None;
        #[cfg(not(test))]
        let root = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")));
        Self {
            root,
            file: None,
            tail: coder_delegate::tail::Reader::new(64 * 1024),
            offset: 0,
            next: Instant::now(),
        }
    }

    pub(crate) fn poll(&mut self, session: Option<&str>, emit: &mut dyn FnMut(RuntimeEvent)) {
        if Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(1);
        let Some(session) = session.filter(|id| {
            id.len() == 36
                && id.bytes().enumerate().all(|(index, byte)| {
                    if matches!(index, 8 | 13 | 18 | 23) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                })
        }) else {
            return;
        };
        if self.file.is_none() {
            let Some(root) = &self.root else {
                return;
            };
            let date = coder::relay::usage::date(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            let directory = root
                .join("sessions")
                .join(&date[..4])
                .join(&date[5..7])
                .join(&date[8..10]);
            let Ok(entries) = std::fs::read_dir(directory) else {
                return;
            };
            let suffix = format!("-{session}.jsonl");
            for entry in entries.take(4096).flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(&suffix))
                    && entry.file_type().is_ok_and(|kind| kind.is_file())
                {
                    self.file = File::open(entry.path()).ok();
                    break;
                }
            }
        }
        let Some(file) = &mut self.file else {
            return;
        };
        let mut bytes = Vec::new();
        if file.take(64 * 1024).read_to_end(&mut bytes).is_err() {
            return;
        }
        for record in self.tail.feed(self.offset, &bytes) {
            if let Some(event) = decode(&record.text) {
                emit(event);
            }
        }
        self.offset = self.offset.saturating_add(bytes.len() as u64);
    }
}

fn decode(line: &str) -> Option<RuntimeEvent> {
    let value: Value = serde_json::from_str(line).ok()?;
    let payload = &value["payload"];
    if value["type"] == "turn_context" {
        let model = crate::live::model_slug(payload["model"].as_str()?)?;
        let reasoning = payload["effort"]
            .as_str()
            .or_else(|| payload["reasoning_effort"].as_str())
            .and_then(crate::live::model_slug);
        return Some(RuntimeEvent::Model(
            crate::models::GenerationOptions {
                reasoning,
                max_tokens: None,
            }
            .slug(&model),
        ));
    }
    if value["type"] == "event_msg" && payload["type"] == "token_count" {
        let usage = &payload["info"]["total_token_usage"];
        let tokens = usage["total_tokens"].as_u64().or_else(|| {
            Some(
                usage["input_tokens"]
                    .as_u64()?
                    .saturating_add(usage["output_tokens"].as_u64()?),
            )
        })?;
        return Some(RuntimeEvent::Tokens(tokens));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn live_session_log_delivers_model_reasoning_and_growing_usage() {
        let root = tempfile::tempdir().unwrap();
        let date = coder::relay::usage::date(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        let directory = root
            .path()
            .join("sessions")
            .join(&date[..4])
            .join(&date[5..7])
            .join(&date[8..10]);
        std::fs::create_dir_all(&directory).unwrap();
        let session = "12345678-1234-1234-1234-123456789abc";
        let path = directory.join(format!("rollout-fixture-{session}.jsonl"));
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", serde_json::json!({"type":"turn_context","payload":{"model":"gpt-test","effort":"high"}})).unwrap();
        let usage = |tokens| serde_json::json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":tokens}}}});
        writeln!(file, "{}", usage(1234)).unwrap();
        let mut reader = Reader::new();
        reader.root = Some(root.path().to_owned());
        let mut events = Vec::new();
        reader.poll(Some(session), &mut |event| events.push(event));
        assert!(matches!(&events[0], RuntimeEvent::Model(model) if model == "gpt-test:high"));
        assert!(matches!(events[1], RuntimeEvent::Tokens(1234)));
        writeln!(file, "{}", usage(2345)).unwrap();
        reader.next = Instant::now();
        reader.poll(Some(session), &mut |event| events.push(event));
        assert!(matches!(events[2], RuntimeEvent::Tokens(2345)));
        assert_eq!(events.len(), 3);
    }
}
