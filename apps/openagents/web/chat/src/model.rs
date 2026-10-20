//! A web chat's messages as transcript entries: each person's message is
//! one entry, and the answer after it (its thinking, text, and the site's
//! notices) is one assistant entry, drawn by the shared transcript views.

use std::collections::{BTreeMap, HashMap};

use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_proto::SidebarSection;
use openagents_web_chat_view::transcript::NOTICE_PREFIX;
use openagents_web_client::workspace::{ChatRow, Organize, Sort, View};
use openagents_web_client::{FileRef, MessageRow, Project, Thread, ThreadMessage, file_url};

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
                    .map(|f: &FileRef| (file_url(chat, f), f.name.clone()))
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
    /// A section the person made, by ID and name, and whether it's folded.
    Section(String, String, bool),
    Project(String, String),
    /// A project on a computer (a registry space), by ID and name.
    Space(String, String),
    /// A computer's chats, by device ID and name.
    Computer(String, String),
    Chats,
    Archived,
}

impl Group {
    pub fn label(&self) -> String {
        match self {
            Group::Pinned => "Pinned".into(),
            Group::Section(_, name, _)
            | Group::Project(_, name)
            | Group::Space(_, name)
            | Group::Computer(_, name) => name.clone(),
            Group::Chats => "Chats".into(),
            Group::Archived => "Archived".into(),
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Group::Pinned => 0,
            Group::Section(..) => 1,
            Group::Project(..) | Group::Space(..) | Group::Computer(..) => 2,
            Group::Chats => 3,
            Group::Archived => 4,
        }
    }
}

/// What the sidebar knows about a chat on a computer, beside its row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Its place among the pins.
    pub pin: Option<usize>,
    /// Its section: ID, name, folded, and the section's place.
    pub section: Option<(String, String, bool, usize)>,
    /// Its computer: device ID and name.
    pub computer: (String, String),
    /// Its project on that computer: ID and name.
    pub space: Option<(String, String)>,
    pub created_unix: u64,
    pub archived: bool,
}

/// A chat on a computer as a sidebar row; `sections` are the account's.
pub fn computer_thread(row: &ChatRow, sections: &[SidebarSection]) -> (Thread, Meta) {
    let chat = &row.chat;
    let mut line = row.computer.clone();
    if let Some(project) = &row.project {
        line = format!("{project} · {line}");
    }
    if row.needs_you {
        line = format!("Needs you · {line}");
    }
    let thread = Thread {
        id: chat.id.clone(),
        title: chat.title.clone().unwrap_or_default(),
        line,
        surface: COMPUTER.into(),
        computer: Some(row.computer.clone()),
        updated_unix: (row.updated_ms / 1000).max(0) as u64,
        working: row.working || row.needs_you,
        online: row.online,
        can_reply: row.online,
        pinned: row.pin.is_some(),
        outcome: row.failed.then(|| "failed".into()),
        ..Default::default()
    };
    let meta = Meta {
        pin: row.pin,
        section: row.section.as_ref().and_then(|id| {
            let at = sections.iter().position(|s| &s.id == id)?;
            let section = &sections[at];
            Some((
                section.id.clone(),
                section.name.clone(),
                section.collapsed,
                at,
            ))
        }),
        computer: (chat.device_id.clone(), row.computer.clone()),
        space: chat.space_id.clone().zip(row.project.clone()),
        created_unix: (row.created_ms / 1000).max(0) as u64,
        archived: chat.archived,
    };
    (thread, meta)
}

/// The surface of a chat that runs on one of the account's computers and
/// lives in the account's registry.
pub const COMPUTER: &str = "computer";

/// The sidebar's groups, each with the indexes of its chats: pinned first,
/// then sections, then each project or computer by its newest chat, then
/// the rest, and the archived last when the view shows them. `filter` keeps
/// chats whose title holds it, ignoring case. `meta` holds what's known
/// about each computer chat; `view` is the person's view choices.
pub fn groups(
    threads: &[Thread],
    projects: &[Project],
    filter: &str,
    meta: &HashMap<String, Meta>,
    view: View,
) -> Vec<(Group, Vec<usize>)> {
    let filter = filter.trim().to_lowercase();
    let mut out: Vec<(Group, Vec<usize>)> = Vec::new();
    let mut section_order: HashMap<String, usize> = HashMap::new();
    for (ix, thread) in threads.iter().enumerate() {
        if !filter.is_empty() && !thread.title.to_lowercase().contains(&filter) {
            continue;
        }
        let found = meta.get(&thread.id);
        let group = if found.is_some_and(|m| m.archived) {
            if !view.archived {
                continue;
            }
            Group::Archived
        } else if thread.pinned {
            Group::Pinned
        } else if let Some((id, name, folded, at)) = found.and_then(|m| m.section.clone()) {
            section_order.insert(id.clone(), at);
            Group::Section(id, name, folded)
        } else if let Some(found) = found {
            match (view.organize, &found.space) {
                (Organize::OneList, _) => Group::Chats,
                (Organize::ByProject, Some((id, name))) => Group::Space(id.clone(), name.clone()),
                _ => Group::Computer(found.computer.0.clone(), found.computer.1.clone()),
            }
        } else if let Some(project) = thread
            .project
            .as_ref()
            .filter(|_| view.organize != Organize::OneList)
        {
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
    let at = |ix: &usize| match view.sort {
        Sort::Updated => threads[*ix].updated_unix,
        Sort::Created => meta
            .get(&threads[*ix].id)
            .map_or(threads[*ix].updated_unix, |m| m.created_unix),
    };
    let newest = |members: &Vec<usize>| members.iter().map(at).max().unwrap_or(0);
    out.sort_by_key(|(group, members)| {
        let order = match group {
            Group::Section(id, ..) => section_order.get(id).copied().unwrap_or(0),
            _ => 0,
        };
        (group.rank(), order, std::cmp::Reverse(newest(members)))
    });
    for (group, members) in &mut out {
        if *group == Group::Pinned {
            // The registry's pins keep their order; the site's follow.
            members.sort_by_key(|ix| {
                (
                    meta.get(&threads[*ix].id)
                        .and_then(|m| m.pin)
                        .unwrap_or(usize::MAX),
                    std::cmp::Reverse(at(ix)),
                )
            });
        } else {
            members.sort_by_key(|ix| std::cmp::Reverse(at(ix)));
        }
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
        let none = HashMap::new();
        let groups = groups(&threads, &projects, "", &none, View::default());
        assert_eq!(groups[0], (Group::Pinned, vec![2]));
        assert_eq!(
            groups[1],
            (Group::Project("p1".into(), "Site".into()), vec![1])
        );
        assert_eq!(groups[2], (Group::Chats, vec![3, 0]));
        assert_eq!(
            super::groups(&threads, &projects, "D", &none, View::default()).len(),
            1
        );
    }

    /// #12265: computer chats group by computer, by project, or in one
    /// list; pins keep the registry's order; sections come after pins; the
    /// archived show only when asked.
    #[test]
    fn computer_chats_group_as_the_view_says() {
        let thread = |id: &str, at: u64, pinned: bool| Thread {
            id: id.into(),
            title: id.into(),
            updated_unix: at,
            pinned,
            surface: COMPUTER.into(),
            ..Default::default()
        };
        let threads = vec![
            thread("a", 5, false),
            thread("b", 9, false),
            thread("p1", 1, true),
            thread("p2", 8, true),
            thread("s", 3, false),
            thread("old", 2, false),
            Thread {
                id: "web".into(),
                title: "web".into(),
                updated_unix: 4,
                surface: "web".into(),
                ..Default::default()
            },
        ];
        let on = |device: &str, space: Option<&str>| Meta {
            computer: (device.into(), format!("{device} name")),
            space: space.map(|s| (s.into(), format!("{s} name"))),
            ..Default::default()
        };
        let mut meta = HashMap::new();
        meta.insert("a".to_owned(), on("mac", Some("site")));
        meta.insert("b".to_owned(), on("box", None));
        meta.insert(
            "p1".to_owned(),
            Meta {
                pin: Some(0),
                ..on("mac", None)
            },
        );
        meta.insert(
            "p2".to_owned(),
            Meta {
                pin: Some(1),
                ..on("mac", None)
            },
        );
        meta.insert(
            "s".to_owned(),
            Meta {
                section: Some(("sec".into(), "Launch".into(), false, 0)),
                ..on("mac", None)
            },
        );
        meta.insert(
            "old".to_owned(),
            Meta {
                archived: true,
                ..on("mac", None)
            },
        );

        let by_computer = groups(&threads, &[], "", &meta, View::default());
        let labels: Vec<_> = by_computer.iter().map(|(g, _)| g.label()).collect();
        assert_eq!(
            labels,
            ["Pinned", "Launch", "box name", "mac name", "Chats"]
        );
        // Pins in the registry's order, not by age.
        assert_eq!(by_computer[0].1, vec![2, 3]);

        let by_project = groups(
            &threads,
            &[],
            "",
            &meta,
            View {
                organize: Organize::ByProject,
                ..View::default()
            },
        );
        assert!(by_project.contains(&(Group::Space("site".into(), "site name".into()), vec![0])));
        assert!(by_project.contains(&(Group::Computer("box".into(), "box name".into()), vec![1])));

        let one_list = groups(
            &threads,
            &[],
            "",
            &meta,
            View {
                organize: Organize::OneList,
                archived: true,
                ..View::default()
            },
        );
        assert_eq!(one_list[2], (Group::Chats, vec![1, 0, 6]));
        assert_eq!(one_list.last().unwrap(), &(Group::Archived, vec![5]));
    }

    #[test]
    fn titles_come_from_the_first_line() {
        assert_eq!(title_from("\n  Fix the build  \nmore"), "Fix the build");
    }
}
