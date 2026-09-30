//! Search and grouping over durable conversation summaries.
use openagents_chat::basic_chats::Summary;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Pinned,
    Project(String),
    Recent,
    Archived,
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
    rows.sort_by_key(|summary| {
        (
            group(summary),
            std::cmp::Reverse(summary.updated),
            summary.id.as_str(),
        )
    });
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
            host: "host".into(),
            task: "task".into(),
            project: Some("Rocket".into()),
        });
        assert_eq!(search(&rows, "chat").len(), 512);
        assert_eq!(search(&rows, "CHAT 21").len(), 11);
        assert_eq!(search(&rows, "rocket")[0].id, rows[3].id);
        let sorted = search(&rows, "");
        assert_eq!(group(sorted[0]), Group::Pinned);
        assert_eq!(group(sorted[1]), Group::Project("Rocket".into()));
        assert_eq!(group(sorted[511]), Group::Archived);
    }
}
