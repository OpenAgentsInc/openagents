//! Injected studio display and confirmed commands. The host owns the facts,
//! command parser, and admission; reopening this page only reads.
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Receiver;
use web_time::{Duration, Instant};

/// The mount's independently injected studio service.
pub trait Transport: Send + Sync {
    fn read_studio(&self) -> Receiver<Result<crate::studio::View, String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this mount reads no studio".into()));
        rx
    }
    fn prepare_studio(
        &self,
        source: &[u8],
        review: Option<&Review>,
        line: &str,
        workspace: Option<&str>,
    ) -> Receiver<Result<crate::studio::Prepared, String>> {
        let _ = (source, review, line, workspace);
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this mount prepares no studio commands".into()));
        rx
    }
    fn read_review(&self, task: &str) -> Receiver<Result<Review, String>> {
        let _ = task;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this mount reads no studio reviews".into()));
        rx
    }
    fn send_studio(&self, command: &crate::studio::Prepared) -> Receiver<Result<String, String>> {
        let _ = command;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this mount sends no studio commands".into()));
        rx
    }
}
impl Transport for () {}

fn ready<T>(receiver: &Receiver<Result<T, String>>) -> Option<Result<T, String>> {
    match receiver.try_recv() {
        Ok(result) => Some(result),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
            "Studio adapter ended without an answer; no automatic replay.".into(),
        )),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct View {
    #[serde(default)]
    pub source: Vec<u8>,
    pub stream: String,
    pub sequence: u64,
    pub rows: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    pub operate: bool,
    #[serde(default)]
    pub review: bool,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub local_runs: Vec<String>,
    #[serde(default)]
    pub workspaces: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepared {
    pub request: String,
    pub stream: String,
    pub description: String,
    pub bytes: Vec<u8>,
    pub review: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub stream: String,
    pub task: String,
    pub rows: Vec<String>,
    pub source: Vec<u8>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Studio,
    Decisions,
    Review,
}

#[derive(Default)]
pub struct Page {
    pub open: bool,
    pub section: Section,
    /// The world mount supplies its already admitted source instead of
    /// asking the local helper to choose a different host.
    pub external: bool,
    pub view: Option<View>,
    pub review: Option<Review>,
    pub review_task: Option<String>,
    reviewing: Option<Receiver<Result<Review, String>>>,
    pub workspace: Option<String>,
    pub scroll: usize,
    pub notice: Option<String>,
    pub pending: Option<Prepared>,
    pub prepare: Option<String>,
    pub prepare_source: Vec<u8>,
    pub prepare_review: Option<Review>,
    pub send: Option<Prepared>,
    pub ticket: Option<u64>,
    read: Option<Receiver<Result<View, String>>>,
    preparing: Option<Receiver<Result<Prepared, String>>>,
    sending: Option<Receiver<Result<String, String>>>,
    checked: Option<Instant>,
}

impl Page {
    pub fn update(&mut self, view: View) -> Result<(), String> {
        if view.tasks.len() > 256
            || view.tasks.iter().any(|id| id.is_empty() || id.len() > 128)
            || view.source.len() > 64 * 1024
            || view.stream.is_empty()
            || view.stream.len() > 64
            || view.workspaces.len() > 64
            || view
                .workspaces
                .iter()
                .any(|label| label.is_empty() || label.len() > 128)
            || view.local_runs.len() > 256
            || view
                .local_runs
                .iter()
                .any(|id| id.is_empty() || id.len() > 128)
            || view.decisions.len() > 2048
            || view.decisions.iter().any(|row| row.len() > 4096)
            || view.decisions.iter().map(String::len).sum::<usize>() > 256 * 1024
            || view.rows.len() > 2048
            || view.rows.iter().any(|row| row.len() > 4096)
            || view.rows.iter().map(String::len).sum::<usize>() > 256 * 1024
        {
            self.revoke();
            return Err("studio display exceeds its bounds".into());
        }
        if self
            .view
            .as_ref()
            .is_some_and(|old| old.stream == view.stream && old.sequence > view.sequence)
        {
            return Err("studio update is older than the displayed facts".into());
        }
        if self
            .view
            .as_ref()
            .is_some_and(|old| (old.operate && !view.operate) || (old.review && !view.review))
            || self
                .view
                .as_ref()
                .is_some_and(|old| old.stream != view.stream)
        {
            self.ticket = None;
            self.preparing = None;
            self.pending = None;
            self.prepare = None;
            self.prepare_source.clear();
            self.prepare_review = None;
            self.send = None;
        }
        if self
            .view
            .as_ref()
            .is_some_and(|old| old.stream != view.stream)
        {
            self.workspace = None;
            self.review = None;
            self.review_task = None;
            self.reviewing = None;
        }
        if self
            .workspace
            .as_ref()
            .is_some_and(|chosen| !view.workspaces.contains(chosen))
        {
            self.workspace = None;
            self.pending = None;
            self.preparing = None;
        }
        self.view = Some(view);
        Ok(())
    }

    pub fn revoke(&mut self) {
        let uncertain = self.sending.is_some() || self.ticket.is_some();
        self.read = None;
        self.preparing = None;
        self.sending = None;
        self.checked = None;
        self.view = None;
        self.review = None;
        self.review_task = None;
        self.reviewing = None;
        self.workspace = None;
        self.ticket = None;
        self.pending = None;
        self.prepare = None;
        self.prepare_source.clear();
        self.prepare_review = None;
        self.send = None;
        self.notice = Some(if uncertain {
            "Studio observation is unavailable; a sent command has an unknown outcome. No automatic replay."
        } else { "Studio observation is unavailable or revoked." }.into());
    }

    pub fn prepared(&mut self, result: Result<Prepared, String>) {
        match result {
            Ok(prepared)
                if prepared.bytes.len() <= 64 * 1024
                    && self.view.as_ref().is_some_and(|v| {
                        (if prepared.review { v.review } else { v.operate })
                            && v.stream == prepared.stream
                    }) =>
            {
                self.pending = Some(prepared);
            }
            Ok(_) => self.notice = Some("Studio command is stale or not admitted.".into()),
            Err(error) => self.notice = Some(error),
        }
    }

    pub fn reviewed(&mut self, result: Result<Review, String>) {
        match result {
            Ok(review)
                if review.source.len() <= 64 * 1024
                    && review.rows.len() <= 2048
                    && review.rows.iter().all(|row| row.len() <= 4096)
                    && review.rows.iter().map(String::len).sum::<usize>() <= 256 * 1024
                    && self.view.as_ref().is_some_and(|v| {
                        v.stream == review.stream && v.tasks.contains(&review.task)
                    }) =>
            {
                self.pending = None;
                self.review = Some(review);
                self.notice = Some("Exact-revision review loaded.".into());
            }
            Ok(_) => self.notice = Some("Review is stale or exceeds its bounds.".into()),
            Err(error) => self.notice = Some(error),
        }
    }

    pub fn enter(&mut self, line: String) {
        if self.sending.is_some()
            || self.preparing.is_some()
            || self.send.is_some()
            || self.prepare.is_some()
        {
            return;
        }
        if let Some(task) = line.strip_prefix("/review ") {
            self.section = Section::Review;
            self.pending = None;
            if self
                .view
                .as_ref()
                .is_some_and(|view| view.tasks.iter().any(|id| id == task))
            {
                self.review = None;
                self.review_task = Some(task.to_owned());
                self.notice = Some("Reading the task's exact revisions.".into());
            } else {
                self.notice = Some("That task is not in the admitted snapshot.".into());
            }
            return;
        }
        if let Some(workspace) = line.strip_prefix("/repo ") {
            self.pending = None;
            if self
                .view
                .as_ref()
                .is_some_and(|view| view.workspaces.iter().any(|label| label == workspace))
            {
                self.workspace = Some(workspace.to_owned());
                self.notice = Some(format!(
                    "Workspace selected: {workspace}; no work submitted."
                ));
            } else {
                self.notice = Some("That workspace is not in the admitted snapshot.".into());
            }
            return;
        }
        if let Some(prepared) = self.pending.take() {
            if line.is_empty()
                && self.view.as_ref().is_some_and(|v| {
                    (if prepared.review { v.review } else { v.operate })
                        && v.stream == prepared.stream
                })
            {
                self.send = Some(prepared);
            } else {
                self.notice = Some("Command rejected; enter the revised command again.".into());
            }
        } else if !line.is_empty() && self.view.as_ref().is_some_and(|v| v.operate || v.review) {
            self.prepare_source = self.view.as_ref().unwrap().source.clone();
            self.prepare_review = self.review.clone();
            self.prepare = Some(line);
        } else {
            self.notice = Some("Studio steering is not admitted.".into());
        }
    }

    pub fn poll(&mut self, transport: &dyn Transport) {
        if self.external || !self.open {
            return;
        }
        if self.read.is_none()
            && self
                .checked
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(2))
        {
            self.checked = Some(Instant::now());
            self.read = Some(transport.read_studio());
        }
        if let Some(result) = self.read.as_ref().and_then(ready) {
            self.read = None;
            match result {
                Ok(view) => {
                    let _ = self.update(view);
                }
                Err(error) => {
                    self.revoke();
                    self.notice = Some(format!(
                        "{error}. {}",
                        self.notice.as_deref().unwrap_or_default()
                    ));
                }
            }
        }
        if self.reviewing.is_none() {
            if let Some(task) = self.review_task.take() {
                self.reviewing = Some(transport.read_review(&task));
            }
        }
        if let Some(result) = self.reviewing.as_ref().and_then(ready) {
            self.reviewing = None;
            self.reviewed(result);
        }
        if let Some(line) = self.prepare.take() {
            self.preparing = Some(transport.prepare_studio(
                &self.prepare_source,
                self.prepare_review.as_ref(),
                &line,
                self.workspace.as_deref(),
            ));
        }
        if let Some(result) = self.preparing.as_ref().and_then(ready) {
            self.preparing = None;
            self.prepared(result);
        }
        if let Some(prepared) = self.send.take() {
            self.sending = Some(transport.send_studio(&prepared));
        }
        if let Some(result) = self.sending.as_ref().and_then(ready) {
            self.sending = None;
            self.notice = Some(result.unwrap_or_else(|e| e));
            self.checked = None;
        }
    }
}

pub fn lines(page: &Page) -> Vec<(String, crate::paper::Tone)> {
    use crate::paper::Tone;
    let mut rows = vec![(
        match page.section {
            Section::Studio => "STUDIO: GOALS, TASK WALL, SEATS, AND MEMORY",
            Section::Decisions => "STUDIO: QUESTIONS AND TOOL APPROVALS",
            Section::Review => "STUDIO: EXACT-REVISION REVIEW",
        }
        .into(),
        Tone::Loud,
    )];
    if let Some(view) = &page.view {
        rows.push((
            format!("Host stream {} sequence {}", view.stream, view.sequence),
            Tone::Quiet,
        ));
        if let Some(workspace) = &page.workspace {
            rows.push((format!("Selected workspace: {workspace}"), Tone::Loud));
        }
        let facts: &[String] = match page.section {
            Section::Studio => &view.rows,
            Section::Decisions => &view.decisions,
            Section::Review => &[],
        };
        rows.extend(facts.iter().map(|row| (row.clone(), Tone::Present)));
    } else {
        rows.push(("Studio facts unavailable.".into(), Tone::Quiet));
    }
    if page.section == Section::Review
        && let Some(review) = &page.review
    {
        rows.extend(review.rows.iter().map(|row| (row.clone(), Tone::Present)));
        rows.push((
            if page.view.as_ref().is_some_and(|view| view.review) {
                "/merge, /changes TEXT, or /reject TEXT prepare a verdict; ENTER confirms."
            } else {
                "Read-only review; Review right is not admitted."
            }
            .into(),
            Tone::Loud,
        ));
    }
    if let Some(notice) = &page.notice {
        rows.push((notice.clone(), Tone::Loud));
    }
    if let Some(pending) = &page.pending {
        rows.push((format!("CONFIRM: {}", pending.description), Tone::Loud));
        rows.push(("ENTER sends once; ESC rejects.".into(), Tone::Loud));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(stream: &str, operate: bool) -> View {
        View {
            source: Vec::new(),
            stream: stream.into(),
            sequence: 4,
            rows: vec!["retained facts".into()],
            decisions: Vec::new(),
            operate,
            review: false,
            tasks: Vec::new(),
            local_runs: Vec::new(),
            workspaces: vec!["scratch".into()],
        }
    }
    fn prepared() -> Prepared {
        Prepared {
            request: "a".repeat(64),
            stream: "ab".into(),
            description: "pause ada".into(),
            bytes: b"exact typed command".to_vec(),
            review: false,
        }
    }

    #[test]
    fn confirmation_dispatches_exactly_once_and_reopen_does_not_dispatch() {
        let mut page = Page::default();
        page.update(view("ab", true)).unwrap();
        page.enter("/pause @ada".into());
        assert_eq!(page.prepare.take().as_deref(), Some("/pause @ada"));
        page.prepared(Ok(prepared()));
        assert!(page.send.is_none());
        page.open = false;
        page.open = true;
        assert!(page.send.is_none());
        page.enter(String::new());
        assert_eq!(page.send.take().unwrap().bytes, b"exact typed command");
        page.enter(String::new());
        assert!(page.send.is_none());
    }

    #[test]
    fn revocation_restart_and_edits_never_send_an_armed_command() {
        for next in [view("ab", false), view("cd", true)] {
            let mut page = Page::default();
            page.update(view("ab", true)).unwrap();
            page.prepared(Ok(prepared()));
            page.update(next).unwrap();
            page.enter(String::new());
            assert!(page.send.is_none());
        }
        let mut page = Page::default();
        page.update(view("ab", true)).unwrap();
        page.prepared(Ok(prepared()));
        page.enter("edited".into());
        assert!(page.send.is_none());
        page.prepared(Ok(prepared()));
        page.revoke();
        assert!(page.view.is_none() && page.pending.is_none());
    }

    #[test]
    fn old_or_oversized_display_is_not_accepted() {
        let mut page = Page::default();
        page.update(view("ab", true)).unwrap();
        let mut old = view("ab", true);
        old.sequence = 3;
        assert!(page.update(old).is_err());
        assert_eq!(page.view.as_ref().unwrap().sequence, 4);
        let mut huge = view("ab", true);
        huge.rows = vec!["x".repeat(4097)];
        assert!(page.update(huge).is_err());
        assert!(page.view.is_none());
    }

    #[test]
    fn workspace_selection_only_uses_admitted_labels_and_creates_no_work() {
        let mut page = Page::default();
        page.update(view("ab", false)).unwrap();
        page.enter("/repo scratch".into());
        assert_eq!(page.workspace.as_deref(), Some("scratch"));
        assert!(page.prepare.is_none() && page.send.is_none());
        page.enter("/repo private".into());
        assert_eq!(page.workspace.as_deref(), Some("scratch"));
        page.update(view("cd", false)).unwrap();
        assert!(page.workspace.is_none());
    }
    #[test]
    fn review_permission_is_independent_and_revocation_disarms_confirmation() {
        let mut view = view("ab", false);
        view.review = true;
        view.source = b"displayed snapshot".to_vec();
        view.tasks = vec!["studio-g-a".into()];
        let mut page = Page::default();
        page.update(view.clone()).unwrap();
        page.enter("/review studio-g-a".into());
        assert_eq!(page.review_task.take().as_deref(), Some("studio-g-a"));
        page.enter("/merge".into());
        assert_eq!(page.prepare_source, b"displayed snapshot");
        page.prepare = None;
        let mut command = prepared();
        command.review = true;
        page.prepared(Ok(command));
        assert!(page.pending.is_some());
        view.review = false;
        page.update(view).unwrap();
        page.enter(String::new());
        assert!(page.pending.is_none() && page.send.is_none());
    }
}
