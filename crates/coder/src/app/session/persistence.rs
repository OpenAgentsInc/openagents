use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use super::{SessionEntry, StoredMessage};
use crate::app::chat::{ChatMessage, MessageRole};
use crate::app::config::{
    session_index_file, session_messages_dir, session_messages_file, sessions_dir,
};

fn session_metadata_file(session_id: &str) -> PathBuf {
    session_messages_dir(session_id).join("metadata.json")
}

pub(crate) fn load_session_index() -> Vec<SessionEntry> {
    let path = session_index_file();
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<SessionEntry>>(&data).unwrap_or_default()
}

pub(crate) fn save_session_index(entries: &[SessionEntry]) -> io::Result<()> {
    let dir = sessions_dir();
    fs::create_dir_all(&dir)?;
    let data = serde_json::to_string_pretty(entries).unwrap_or_else(|_| "[]".to_string());
    fs::write(session_index_file(), data)?;
    Ok(())
}

pub(crate) fn apply_session_history_limit(
    entries: &mut Vec<SessionEntry>,
    limit: usize,
) -> Vec<String> {
    entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if limit == 0 {
        return Vec::new();
    }
    if entries.len() <= limit {
        return Vec::new();
    }
    entries
        .drain(limit..)
        .map(|entry| entry.id)
        .collect()
}

pub(crate) fn write_session_messages(session_id: &str, messages: &[ChatMessage]) -> io::Result<()> {
    let dir = session_messages_dir(session_id);
    fs::create_dir_all(&dir)?;
    let mut file = fs::File::create(session_messages_file(session_id))?;
    for msg in messages {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::AssistantThought => "assistant_thought",
        };
        let stored = StoredMessage {
            role: role.to_string(),
            content: msg.content.clone(),
            uuid: msg.uuid.clone(),
        };
        serde_json::to_writer(&mut file, &stored)?;
        writeln!(&mut file)?;
    }
    Ok(())
}

pub(crate) fn write_session_metadata(session_id: &str, entry: &SessionEntry) -> io::Result<()> {
    let dir = session_messages_dir(session_id);
    fs::create_dir_all(&dir)?;
    let data = serde_json::to_string_pretty(entry)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(session_metadata_file(session_id), data)?;
    Ok(())
}

pub(crate) fn read_session_messages(session_id: &str) -> io::Result<Vec<ChatMessage>> {
    let path = session_messages_file(session_id);
    let data = fs::read_to_string(path)?;
    let mut messages = Vec::new();
    for line in data.lines() {
        let stored: StoredMessage = serde_json::from_str(line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let role = match stored.role.as_str() {
            "user" => MessageRole::User,
            "assistant_thought" => MessageRole::AssistantThought,
            _ => MessageRole::Assistant,
        };
        let document = if role == MessageRole::Assistant {
            Some(super::super::build_markdown_document(&stored.content))
        } else {
            None
        };
        messages.push(ChatMessage {
            role,
            content: stored.content,
            document,
            uuid: stored.uuid,
            metadata: None,
        });
    }
    Ok(messages)
}
