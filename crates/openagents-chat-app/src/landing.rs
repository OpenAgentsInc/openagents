//! Where a finished change went: landed, pushed, a pull request open, a
//! refused push, or a conflict, as a badge the chat shows. Display only.
//!
//! Reimplemented from Zeron's change-request badge (public MIT
//! zeronsh/zeron at `9e1a1115`, `crates/ui/src/change_requests.rs`): one
//! small model that turns a recorded state into a label, a tone, and an
//! optional link, which a renderer draws without deciding anything. Zeron
//! reads a forge's pull-request state; this reads what Coder recorded
//! itself: the issue flow's [`IssueLink`] (and its `not_landed` reason),
//! the result's `pushed_to` branch, and a reviewed change's
//! [`Publication`]. Nothing here asks a forge, pushes, or retries.

use coder_host::access::review::{Landing, Publication, PublishState};
use openagents_chat::coder_events::{Finished, IssueLink};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};

/// Where a change is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// On the repository's branch.
    Landed,
    /// On the remote, without a pull request.
    Pushed,
    /// A pull request is open.
    PullRequest { draft: bool },
    /// The push's result is unknown.
    Uncertain,
    /// The remote refused the push, so nothing landed.
    Refused,
    /// The change conflicts with the newer branch, so nothing landed.
    Conflict,
}

/// How a badge reads at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// The change is where it was meant to go.
    Done,
    /// The change is out, and someone still has to act on it.
    Open,
    /// The change did not get out.
    Attention,
}

impl Tone {
    /// The badge's text color.
    #[must_use]
    pub fn color(self) -> Color {
        let inks = crate::visual::inks();
        match self {
            Tone::Done => inks.done,
            Tone::Open => inks.open,
            Tone::Attention => inks.attention,
        }
    }
}

/// One badge: what it says and where it links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub state: State,
    /// The badge's word or two, such as `Landed`.
    pub label: &'static str,
    /// What follows the label, on one line, such as the commit and branch.
    pub detail: String,
    /// The commit or pull request on the forge, when there is one.
    pub link: Option<String>,
}

impl Status {
    fn new(state: State, detail: impl Into<String>, link: Option<String>) -> Self {
        let label = match state {
            State::Landed => "Landed",
            State::Pushed => "Pushed",
            State::PullRequest { draft: true } => "Draft pull request open",
            State::PullRequest { draft: false } => "Pull request open",
            State::Uncertain => "Push unconfirmed",
            State::Refused => "Push refused",
            State::Conflict => "Conflict",
        };
        Self {
            state,
            label,
            detail: one_line(&detail.into()),
            link,
        }
    }

    /// The badge's tone.
    #[must_use]
    pub fn tone(&self) -> Tone {
        match self.state {
            State::Landed => Tone::Done,
            State::Pushed | State::PullRequest { .. } | State::Uncertain => Tone::Open,
            State::Refused | State::Conflict => Tone::Attention,
        }
    }

    /// The issue flow's ending. `None` while it works, or when it ended
    /// without a change to place: nothing changed, it stopped, or it
    /// failed before landing.
    #[must_use]
    pub fn from_issue(issue: &IssueLink) -> Option<Self> {
        let commit = issue.commits.last().map(|commit| short(commit).to_owned());
        let commit_link = issue
            .commits
            .last()
            .map(|commit| format!("https://github.com/{}/commit/{commit}", issue.repository));
        match issue.outcome.as_str() {
            "landed" => {
                let mut detail = match &commit {
                    Some(commit) => format!("{commit} on the default branch"),
                    None => "On the default branch".to_owned(),
                };
                if issue.closed {
                    detail.push_str(&format!(" · closed #{}", issue.number));
                }
                Some(Self::new(State::Landed, detail, commit_link))
            }
            "pull_request" => Some(Self::new(
                State::PullRequest { draft: false },
                format!("For #{}", issue.number),
                issue.pull_request.clone(),
            )),
            "failed" => match issue.not_landed.as_deref() {
                Some("conflict") => Some(Self::new(
                    State::Conflict,
                    format!("#{} stays open; nothing landed", issue.number),
                    None,
                )),
                Some("push_refused") => Some(Self::new(
                    State::Refused,
                    format!("#{} stays open; nothing landed", issue.number),
                    None,
                )),
                _ => None,
            },
            _ => None,
        }
    }

    /// A reviewed change's publication.
    #[must_use]
    pub fn from_publication(publication: &Publication) -> Self {
        let commit = publication.commit.as_deref().map(short).unwrap_or_default();
        let branch = publication.branch.as_deref().unwrap_or_default();
        let at = match (commit.is_empty(), branch.is_empty()) {
            (false, false) => format!("{commit} on {branch}"),
            (false, true) => commit.to_owned(),
            (true, false) => branch.to_owned(),
            (true, true) => String::new(),
        };
        match (publication.state, publication.landing) {
            (PublishState::Published, Landing::Branch) => {
                Self::new(State::Landed, at, publication.url.clone())
            }
            (PublishState::Published, Landing::DraftPullRequest) => Self::new(
                State::PullRequest { draft: true },
                at,
                publication.url.clone(),
            ),
            (PublishState::Pushed, _) => Self::new(
                State::Pushed,
                publication.note.clone(),
                publication.url.clone(),
            ),
            (PublishState::Uncertain, _) => Self::new(State::Uncertain, &publication.note, None),
            (PublishState::Refused, _) => Self::new(State::Refused, &publication.note, None),
        }
    }

    /// A finished run: its issue flow's ending when it was one, else the
    /// branch the change was pushed to, if any.
    #[must_use]
    pub fn from_result(result: &Finished) -> Option<Self> {
        if let Some(issue) = &result.issue {
            return Self::from_issue(issue);
        }
        result
            .pushed_to
            .as_deref()
            .filter(|branch| !branch.trim().is_empty())
            .map(|branch| Self::new(State::Pushed, format!("To {branch}"), None))
    }

    /// The badge as a row: the label in its tone, then the detail and
    /// link, quieter.
    #[must_use]
    pub fn node(&self, key: &str) -> Node<()> {
        let tone = self.tone().color();
        let mut children = vec![Node {
            key: format!("{key}-label"),
            style: Style {
                foreground: Some(tone),
                weight: Some(TextWeight::Bold),
                ..Style::default()
            },
            element: Element::Text {
                value: self.label.into(),
                role: TextRole::Body,
            },
        }];
        let detail = match (&self.link, self.detail.is_empty()) {
            (Some(link), true) => link.clone(),
            (Some(link), false) => format!("{} · {link}", self.detail),
            (None, _) => self.detail.clone(),
        };
        if !detail.is_empty() {
            children.push(Node {
                key: format!("{key}-detail"),
                style: Style::default(),
                element: Element::Text {
                    value: detail,
                    role: TextRole::Status,
                },
            });
        }
        Node {
            key: key.into(),
            style: Style {
                border: Some(tone),
                radius: Some(6),
                padding_top: Some(Space::Xs),
                padding_bottom: Some(Space::Xs),
                padding_start: Some(Space::Sm),
                padding_end: Some(Space::Sm),
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Horizontal,
                children,
            },
        }
    }
}

/// The first ten characters of a revision.
fn short(id: &str) -> &str {
    &id[..10.min(id.len())]
}

/// Text on one line, as Zeron's badge keeps a title.
fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(outcome: &str) -> IssueLink {
        IssueLink {
            repository: "acme/app".into(),
            number: 42,
            url: "https://github.com/acme/app/issues/42".into(),
            title: "Fix the docs".into(),
            outcome: outcome.into(),
            commits: vec!["0123456789abcdef".into()],
            pull_request: None,
            closed: outcome == "landed",
            not_landed: None,
        }
    }

    fn publication(state: PublishState, landing: Landing) -> Publication {
        Publication {
            operation: "a".repeat(64),
            task: "b".repeat(64),
            base: "c".repeat(40),
            head_commit: "c".repeat(40),
            head: "d".repeat(40),
            landing,
            state,
            branch: Some("main".into()),
            commit: Some("e".repeat(40)),
            url: None,
            note: "What happened.\nIn two lines.".into(),
        }
    }

    #[test]
    fn a_landed_issue_links_its_commit_and_says_it_closed() {
        let status = Status::from_issue(&issue("landed")).unwrap();
        assert_eq!(status.state, State::Landed);
        assert_eq!(status.label, "Landed");
        assert_eq!(status.tone(), Tone::Done);
        assert_eq!(
            status.detail,
            "0123456789 on the default branch · closed #42"
        );
        assert_eq!(
            status.link.as_deref(),
            Some("https://github.com/acme/app/commit/0123456789abcdef")
        );
    }

    #[test]
    fn an_issue_pull_request_links_the_pull_request() {
        let mut link = issue("pull_request");
        link.pull_request = Some("https://github.com/acme/app/pull/7".into());
        let status = Status::from_issue(&link).unwrap();
        assert_eq!(status.state, State::PullRequest { draft: false });
        assert_eq!(status.label, "Pull request open");
        assert_eq!(status.tone(), Tone::Open);
        assert_eq!(
            status.link.as_deref(),
            Some("https://github.com/acme/app/pull/7")
        );
    }

    #[test]
    fn a_conflict_and_a_refused_push_need_attention() {
        let mut link = issue("failed");
        link.not_landed = Some("conflict".into());
        let status = Status::from_issue(&link).unwrap();
        assert_eq!((status.state, status.label), (State::Conflict, "Conflict"));
        assert_eq!(status.tone(), Tone::Attention);
        assert_eq!(status.link, None);

        link.not_landed = Some("push_refused".into());
        let status = Status::from_issue(&link).unwrap();
        assert_eq!(
            (status.state, status.label),
            (State::Refused, "Push refused")
        );
        assert_eq!(status.tone(), Tone::Attention);
    }

    #[test]
    fn an_issue_with_nothing_to_place_shows_no_badge() {
        for outcome in ["working", "unchanged", "stopped", "failed"] {
            assert_eq!(Status::from_issue(&issue(outcome)), None, "{outcome}");
        }
        let mut red = issue("failed");
        red.not_landed = Some("checks_failed".into());
        assert_eq!(Status::from_issue(&red), None);
    }

    #[test]
    fn publications_map_to_each_state() {
        let landed =
            Status::from_publication(&publication(PublishState::Published, Landing::Branch));
        assert_eq!(landed.state, State::Landed);
        assert_eq!(landed.detail, "eeeeeeeeee on main");

        let mut draft = publication(PublishState::Published, Landing::DraftPullRequest);
        draft.branch = Some("coder/task".into());
        draft.url = Some("https://github.com/acme/app/pull/8".into());
        let draft = Status::from_publication(&draft);
        assert_eq!(draft.state, State::PullRequest { draft: true });
        assert_eq!(draft.label, "Draft pull request open");
        assert_eq!(
            draft.link.as_deref(),
            Some("https://github.com/acme/app/pull/8")
        );

        let pushed = Status::from_publication(&publication(
            PublishState::Pushed,
            Landing::DraftPullRequest,
        ));
        assert_eq!((pushed.state, pushed.tone()), (State::Pushed, Tone::Open));
        assert_eq!(pushed.detail, "What happened. In two lines.");

        let uncertain =
            Status::from_publication(&publication(PublishState::Uncertain, Landing::Branch));
        assert_eq!(uncertain.state, State::Uncertain);

        let refused =
            Status::from_publication(&publication(PublishState::Refused, Landing::Branch));
        assert_eq!(
            (refused.state, refused.tone()),
            (State::Refused, Tone::Attention)
        );
    }

    #[test]
    fn a_result_without_an_issue_shows_the_branch_it_pushed_to() {
        let result: Finished = serde_json::from_value(serde_json::json!({
            "turn": 1,
            "summary": "Done.",
            "files_changed": [],
            "insertions": 0,
            "deletions": 0,
            "worktree": "/w",
            "pushed_to": "origin/coder/task",
            "trajectory": "/t.json"
        }))
        .unwrap();
        let status = Status::from_result(&result).unwrap();
        assert_eq!(status.state, State::Pushed);
        assert_eq!(status.detail, "To origin/coder/task");

        let mut with_issue = result.clone();
        with_issue.issue = Some(issue("landed"));
        assert_eq!(
            Status::from_result(&with_issue).map(|s| s.state),
            Some(State::Landed)
        );

        let mut quiet = result;
        quiet.pushed_to = None;
        assert_eq!(Status::from_result(&quiet), None);
    }

    #[test]
    fn the_node_draws_the_label_in_its_tone_and_the_link() {
        let mut link = issue("pull_request");
        link.pull_request = Some("https://github.com/acme/app/pull/7".into());
        let node = Status::from_issue(&link).unwrap().node("k-status");
        assert_eq!(node.style.border, Some(Tone::Open.color()));
        let Element::Stack { children, .. } = &node.element else {
            panic!("a badge is a row");
        };
        assert_eq!(children[0].style.foreground, Some(Tone::Open.color()));
        assert!(matches!(
            &children[1].element,
            Element::Text { value, .. } if value == "For #42 · https://github.com/acme/app/pull/7"
        ));
    }
}
