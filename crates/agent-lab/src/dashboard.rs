//! A minute-stamped work summary shared by check-ins and dashboard adapters.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub holder: Option<String>,
    pub last_progress_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dashboard {
    pub agent_definition_id: String,
    pub observed_ms: u64,
    pub last_check_in_ms: Option<u64>,
    pub issues: Vec<Issue>,
    pub landing_queue: Vec<String>,
    pub pool_hosts: Vec<String>,
    pub owners: Vec<String>,
    pub decisions: Vec<String>,
}

impl Dashboard {
    /// Both readers use the observation minute, never their local wall clock.
    pub fn issue_lines(&self) -> Vec<String> {
        self.issues
            .iter()
            .map(|issue| {
                let minutes =
                    (self.observed_ms / 60_000).saturating_sub(issue.last_progress_ms / 60_000);
                format!(
                    "{}#{} · {} · {} · {} minutes without progress",
                    issue.repository,
                    issue.number,
                    issue.title,
                    issue.holder.as_deref().unwrap_or("Unclaimed"),
                    minutes
                )
            })
            .collect()
    }

    pub fn check_in_context(&self) -> String {
        self.issue_lines().join("\n")
    }

    #[cfg(feature = "views")]
    pub fn view(&self) -> rust_native::View<()> {
        use rust_native::{Axis, Element, Node, TextRole, View};
        let mut lines = vec!["Agent work".to_string()];
        lines.push(match self.last_check_in_ms {
            Some(time) => format!(
                "Last check-in: {} minutes ago",
                self.observed_ms.saturating_sub(time) / 60_000
            ),
            None => "No check-in yet".into(),
        });
        lines.push("Open issues".into());
        lines.extend(self.issue_lines());
        for (title, entries) in [
            ("Landing queue", &self.landing_queue),
            ("Pool hosts", &self.pool_hosts),
            ("Owners", &self.owners),
            ("Decision log", &self.decisions),
        ] {
            lines.push(title.into());
            if entries.is_empty() {
                lines.push("None".into());
            } else {
                lines.extend(entries.iter().cloned());
            }
        }
        let children = lines
            .into_iter()
            .enumerate()
            .map(|(index, value)| Node {
                key: format!("line-{index}"),
                style: Default::default(),
                element: Element::Text {
                    value,
                    role: TextRole::Body,
                },
            })
            .collect();
        View::new_v3(
            &self.agent_definition_id,
            self.observed_ms / 60_000,
            Node {
                key: "agent-work".into(),
                style: Default::default(),
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_check_in_same_minute() {
        let mut dashboard = Dashboard {
            agent_definition_id: "a".repeat(64),
            observed_ms: 1_259_999,
            last_check_in_ms: Some(1_200_000),
            issues: vec![Issue {
                repository: "example/project".into(),
                number: 42,
                title: "Fix search".into(),
                holder: Some("chat-7".into()),
                last_progress_ms: 59_999,
            }],
            landing_queue: vec![],
            pool_hosts: vec![],
            owners: vec![],
            decisions: vec![],
        };
        let expected = "example/project#42 · Fix search · chat-7 · 20 minutes without progress";
        assert_eq!(dashboard.issue_lines(), vec![expected]);
        dashboard.observed_ms = 1_200_000;
        assert_eq!(dashboard.check_in_context(), expected);
        dashboard.issues[0].last_progress_ms = 1_300_000;
        assert!(
            dashboard
                .check_in_context()
                .ends_with("0 minutes without progress")
        );
    }
}
