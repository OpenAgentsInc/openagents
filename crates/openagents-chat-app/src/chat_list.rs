//! Search and grouping over durable conversation summaries.
//!
//! One ordering for every chat list (the desktop sidebar, the phone's
//! Chats, the command palette): Pinned first, then every other live chat
//! newest first in one list, then Archived (#10100). A project is a label
//! on its row, not a block that sorts ahead of newer chats, so the chat
//! Cmd/Ctrl-N just made is always the first unpinned row.
use openagents_chat::basic_chats::Summary;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Pinned,
    Project(String),
    Recent,
    Archived,
}

impl Group {
    /// Where the group's rows sit in a list: Pinned above, Archived below,
    /// and a project's chats among the other recent chats by recency.
    #[must_use]
    pub fn rank(&self) -> u8 {
        match self {
            Group::Pinned => 0,
            Group::Project(_) | Group::Recent => 1,
            Group::Archived => 2,
        }
    }
}

pub fn group(summary: &Summary) -> Group {
    if summary.archived {
        Group::Archived
    } else if summary.pinned {
        Group::Pinned
    } else if let Some(project) = summary
        .coder
        .as_ref()
        .and_then(|coder| coder.project.as_ref())
    {
        Group::Project(project.clone())
    } else {
        Group::Recent
    }
}

/// Search uses display metadata only; it never opens transcripts or contacts a host.
/// Rows come back in list order ([`Group::rank`], then newest first); chats
/// changed in the same second keep the catalog's order, newest made first.
pub fn search<'a>(summaries: &'a [Summary], query: &str) -> Vec<&'a Summary> {
    let query = query.trim().to_lowercase();
    let mut rows: Vec<_> = summaries
        .iter()
        .filter(|summary| {
            query.is_empty()
                || summary.title.to_lowercase().contains(&query)
                || summary
                    .coder
                    .as_ref()
                    .and_then(|coder| coder.project.as_ref())
                    .is_some_and(|project| project.to_lowercase().contains(&query))
        })
        .collect();
    rows.sort_by_key(|summary| (group(summary).rank(), std::cmp::Reverse(summary.updated)));
    rows
}

/// The archived conversations a Settings list offers to restore, most
/// recently changed first. Display metadata only, as [`search`].
pub fn archived(summaries: &[Summary]) -> Vec<&Summary> {
    let mut rows: Vec<_> = summaries
        .iter()
        .filter(|summary| summary.archived)
        .collect();
    rows.sort_by_key(|summary| (std::cmp::Reverse(summary.updated), summary.id.as_str()));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_and_groups_share_stable_metadata() {
        let mut rows = vec![];
        for n in 0..512 {
            rows.push(Summary {
                id: format!("{n:032x}"),
                title: format!("Chat {n}"),
                started: n,
                updated: n,
                coder: None,
                archived: n == 1,
                pinned: n == 2,
                named: false,
            });
        }
        rows[3].coder = Some(openagents_chat::basic_chats::Spawned {
            at: None,
            host: "host".into(),
            task: "task".into(),
            project: Some("Rocket".into()),
        });
        assert_eq!(search(&rows, "chat").len(), 512);
        assert_eq!(search(&rows, "CHAT 21").len(), 11);
        assert_eq!(search(&rows, "rocket")[0].id, rows[3].id);
        let sorted = search(&rows, "");
        assert_eq!(group(sorted[0]), Group::Pinned);
        assert_eq!(sorted[1].title, "Chat 511");
        assert!(
            sorted
                .iter()
                .position(|row| row.id == rows[3].id)
                .is_some_and(|at| at > 500)
        );
        assert_eq!(group(sorted[511]), Group::Archived);
        rows[5].archived = true;
        let archived = archived(&rows);
        assert_eq!(
            archived
                .iter()
                .map(|row| row.title.as_str())
                .collect::<Vec<_>>(),
            ["Chat 5", "Chat 1"]
        );
    }

    fn chat(id: &str, updated: u64, project: Option<&str>) -> Summary {
        Summary {
            id: id.into(),
            title: id.into(),
            started: updated,
            updated,
            coder: project.map(|project| openagents_chat::basic_chats::Spawned {
                at: Some(updated),
                host: "local".into(),
                task: format!("task-{id}"),
                project: Some(project.into()),
            }),
            archived: false,
            pinned: false,
            named: false,
        }
    }

    /// #10100: old Coder issue-flow chats in projects no longer sit above a
    /// new chat with no project; the list is newest first across projects,
    /// with Pinned above and Archived below.
    #[test]
    fn a_new_chat_is_first_above_older_project_chats() {
        let mut rows = vec![
            chat("work on #10058", 1_000, Some("openagents")),
            chat("work on #10057", 990, Some("openagents")),
            chat("work on #10060", 1_010, Some("openagents-host-tasks")),
            chat("work on #10061", 1_020, Some("openagents-host-tasks")),
            chat("plain old", 900, None),
            chat("pinned old", 10, None),
            chat("archived new", 5_000, None),
            chat("New chat", 2_000, None),
        ];
        rows[5].pinned = true;
        rows[6].archived = true;
        let titles: Vec<&str> = search(&rows, "")
            .iter()
            .map(|row| row.title.as_str())
            .collect();
        assert_eq!(
            titles,
            [
                "pinned old",
                "New chat",
                "work on #10061",
                "work on #10060",
                "work on #10058",
                "work on #10057",
                "plain old",
                "archived new",
            ]
        );
        // Each project's own chats stay newest first, and a project that was
        // touched more recently than another is listed above it.
        let in_project = |name: &str| -> Vec<&str> {
            search(&rows, "")
                .into_iter()
                .filter(|row| group(row) == Group::Project(name.into()))
                .map(|row| row.title.as_str())
                .collect()
        };
        assert_eq!(
            in_project("openagents"),
            ["work on #10058", "work on #10057"]
        );
        // A chat made in the same second as another keeps the catalog's
        // order, where the newest made is first.
        let tied = vec![chat("made second", 7, None), chat("made first", 7, None)];
        assert_eq!(search(&tied, "")[0].title, "made second");
    }
}
