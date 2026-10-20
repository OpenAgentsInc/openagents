//! A web chat's messages as transcript entries: each person's message is
//! one entry, and the answer after it (its thinking, text, and the site's
//! notices) is one assistant entry, drawn by the shared transcript views.

use std::collections::BTreeMap;

use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_web_chat_view::transcript::NOTICE_PREFIX;
use openagents_web_client::{FileRef, MessageRow, Project, Thread, ThreadMessage};

/// One message of the chat at its position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Msg {
    pub index: usize,
    pub role: String,
    pub text: String,
    /// Images sent with a person's message, by URL and name.
    pub images: Vec<(String, String)>,
}

/// The chat's messages from its two reads: `rows` keeps every message at its
/// position; `detail` names the files sent with each person's message, in
/// the same order once empty messages are left out.
pub fn messages(chat: &str, rows: &[MessageRow], detail: &[ThreadMessage]) -> Vec<Msg> {
    let mut sent_files = detail
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.files.as_slice());
    rows.iter()
        .enumerate()
        .map(|(at, row)| {
            let text = row.text();
            let images = if row.role == "user" && !text.trim().is_empty() {
                sent_files
                    .next()
                    .unwrap_or_default()
                    .iter()
                    .filter(|f| f.kind == "image")
                    .map(|f: &FileRef| (f.url(chat), f.name.clone()))
                    .collect()
            } else {
                Vec::new()
            };
            Msg {
                index: row.index().unwrap_or(at),
                role: row.role.clone(),
                text,
                images,
            }
        })
        .collect()
}

/// The parts of an answer from its messages, in order.
pub fn answer_parts<'a>(
    messages: impl IntoIterator<Item = (usize, &'a str, &'a str)>,
) -> Vec<MessagePart> {
    messages
        .into_iter()
        .filter(|(_, _, text)| !text.trim().is_empty())
        .map(|(index, role, text)| match role {
            "thinking" => MessagePart::Reasoning {
                id: format!("r{index}"),
                text: text.to_owned(),
            },
            "tool" => MessagePart::Text {
                id: format!("{NOTICE_PREFIX}-{index}"),
                text: text.to_owned(),
            },
            _ => MessagePart::Text {
                id: format!("m{index}"),
                text: text.to_owned(),
            },
        })
        .collect()
}

fn entry(id: String, role: MessageRole, parts: Vec<MessagePart>) -> SessionMessageEntry {
    SessionMessageEntry {
        id,
        role,
        parts,
        created_at: 0,
        device_id: String::new(),
        status: Some(MessageStatus::Complete),
        continuation_of: None,
        duration_ms: None,
    }
}

/// The transcript's entries for `messages`.
pub fn entries(messages: &[Msg]) -> Vec<SessionMessageEntry> {
    let mut out = Vec::new();
    let mut ix = 0;
    while ix < messages.len() {
        let message = &messages[ix];
        if message.role == "user" {
            if !message.text.trim().is_empty() || !message.images.is_empty() {
                let mut parts = vec![MessagePart::Text {
                    id: "t".into(),
                    text: message.text.clone(),
                }];
                parts.extend(message.images.iter().map(|(url, name)| MessagePart::Image {
                    id: url.clone(),
                    path: url.clone(),
                    name: name.clone(),
                    mime_type: String::new(),
                }));
                out.push(entry(
                    format!("u{}", message.index),
                    MessageRole::User,
                    parts,
                ));
            }
            ix += 1;
            continue;
        }
        let start = ix;
        while ix < messages.len() && messages[ix].role != "user" {
            ix += 1;
        }
        let parts = answer_parts(
            messages[start..ix]
                .iter()
                .map(|m| (m.index, m.role.as_str(), m.text.as_str())),
        );
        if !parts.is_empty() {
            out.push(entry(
                format!("a{}", messages[start].index),
                MessageRole::Assistant,
                parts,
            ));
        }
    }
    out
}

/// Splits off the answer still being written: the messages after the last
/// person's message, when the chat's last request is pending.
pub fn split_pending(
    messages: Vec<Msg>,
    pending: bool,
) -> (Vec<Msg>, BTreeMap<usize, (String, String)>) {
    if !pending {
        return (messages, BTreeMap::new());
    }
    let cut = messages
        .iter()
        .rposition(|m| m.role == "user")
        .map_or(0, |at| at + 1);
    let mut messages = messages;
    let live = messages
        .split_off(cut)
        .into_iter()
        .map(|m| (m.index, (m.role, m.text)))
        .collect();
    (messages, live)
}

/// A sidebar group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group {
    Pinned,
    Project(String, String),
    Chats,
}

impl Group {
    pub fn label(&self) -> String {
        match self {
            Group::Pinned => "Pinned".into(),
            Group::Project(_, name) => name.clone(),
            Group::Chats => "Chats".into(),
        }
    }
}

/// The sidebar's groups, each with the indexes of its chats: pinned first,
/// then each project by its newest chat, then the rest. `filter` keeps
/// chats whose title holds it, ignoring case.
pub fn groups(threads: &[Thread], projects: &[Project], filter: &str) -> Vec<(Group, Vec<usize>)> {
    let filter = filter.trim().to_lowercase();
    let mut out: Vec<(Group, Vec<usize>)> = Vec::new();
    for (ix, thread) in threads.iter().enumerate() {
        if !filter.is_empty() && !thread.title.to_lowercase().contains(&filter) {
            continue;
        }
        let group = if thread.pinned {
            Group::Pinned
        } else if let Some(project) = &thread.project {
            let name = projects
                .iter()
                .find(|p| &p.id == project)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Project".into());
            Group::Project(project.clone(), name)
        } else {
            Group::Chats
        };
        match out.iter_mut().find(|(g, _)| *g == group) {
            Some((_, members)) => members.push(ix),
            None => out.push((group, vec![ix])),
        }
    }
    let newest = |members: &Vec<usize>| {
        members
            .iter()
            .map(|ix| threads[*ix].updated_unix)
            .max()
            .unwrap_or(0)
    };
    out.sort_by_key(|(group, members)| {
        let rank = match group {
            Group::Pinned => 0,
            Group::Project(..) => 1,
            Group::Chats => 2,
        };
        (rank, std::cmp::Reverse(newest(members)))
    });
    for (_, members) in &mut out {
        members.sort_by_key(|ix| std::cmp::Reverse(threads[*ix].updated_unix));
    }
    out
}

/// A title for a chat from its first message, as the site makes one.
pub fn title_from(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    openagents_web_chat_view::transcript::truncate(line, 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize, role: &str, text: &str) -> MessageRow {
        serde_json::from_value(serde_json::json!({
            "id": format!("msg_{index}"),
            "role": role,
            "content": [{"type": "text", "text": text}],
        }))
        .unwrap()
    }

    #[test]
    fn an_answer_folds_into_one_entry() {
        let rows = [
            row(0, "user", "Hi"),
            row(1, "thinking", "Hmm"),
            row(2, "assistant", "Hello"),
            row(3, "tool", "Stopped."),
            row(4, "user", "Again"),
        ];
        let detail: Vec<ThreadMessage> = serde_json::from_value(serde_json::json!([
            {"role": "user", "text": "Hi", "files": [{"id": "f1", "name": "a.png", "kind": "image", "size": 3}]},
            {"role": "assistant", "text": "Hello"},
            {"role": "user", "text": "Again"},
        ]))
        .unwrap();
        let messages = messages("c1", &rows, &detail);
        assert_eq!(
            messages[0].images,
            vec![("/chat/c1/files/f1".into(), "a.png".into())]
        );
        assert!(messages[4].images.is_empty());
        let entries = entries(&messages);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].role, MessageRole::User);
        assert_eq!(entries[0].parts.len(), 2);
        let answer = &entries[1].parts;
        assert!(matches!(&answer[0], MessagePart::Reasoning { text, .. } if text == "Hmm"));
        assert!(matches!(&answer[1], MessagePart::Text { id, .. } if id == "m2"));
        assert!(
            matches!(&answer[2], MessagePart::Text { id, .. } if id.starts_with(NOTICE_PREFIX))
        );
    }

    #[test]
    fn a_pending_answer_is_split_off() {
        let messages = vec![
            Msg {
                index: 0,
                role: "user".into(),
                text: "Hi".into(),
                images: vec![],
            },
            Msg {
                index: 1,
                role: "assistant".into(),
                text: "Hel".into(),
                images: vec![],
            },
        ];
        let (kept, live) = split_pending(messages.clone(), true);
        assert_eq!(kept.len(), 1);
        assert_eq!(live.get(&1), Some(&("assistant".into(), "Hel".into())));
        let (kept, live) = split_pending(messages, false);
        assert_eq!(kept.len(), 2);
        assert!(live.is_empty());
    }

    #[test]
    fn the_sidebar_groups_pinned_then_projects_then_chats() {
        let thread = |id: &str, at: u64, pinned: bool, project: Option<&str>| Thread {
            id: id.into(),
            title: id.into(),
            updated_unix: at,
            pinned,
            project: project.map(Into::into),
            ..Default::default()
        };
        let threads = vec![
            thread("a", 5, false, None),
            thread("b", 9, false, Some("p1")),
            thread("c", 1, true, None),
            thread("d", 7, false, None),
        ];
        let projects = vec![Project {
            id: "p1".into(),
            name: "Site".into(),
            ..Default::default()
        }];
        let groups = groups(&threads, &projects, "");
        assert_eq!(groups[0], (Group::Pinned, vec![2]));
        assert_eq!(
            groups[1],
            (Group::Project("p1".into(), "Site".into()), vec![1])
        );
        assert_eq!(groups[2], (Group::Chats, vec![3, 0]));
        assert_eq!(super::groups(&threads, &projects, "D").len(), 1);
    }

    #[test]
    fn titles_come_from_the_first_line() {
        assert_eq!(title_from("\n  Fix the build  \nmore"), "Fix the build");
    }
}
