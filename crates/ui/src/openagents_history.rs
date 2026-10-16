//! Saved account chats in the sidebar. Selecting a row never selects an engine chat.
use crate::theme::Theme;
use coder_account::{
    Controller,
    history::{ExistingChat, History, LoadState, Snapshot},
    openagents::HistoryProject,
};
use gpui::{Context, EventEmitter, Render, SharedString, Task, Window, div, prelude::*, px};
use gpui_tokio::Tokio;
use std::{future::Future, sync::Arc};

#[derive(Clone)]
pub enum HistoryEvent {
    Open(ExistingChat),
    Reset,
    Close,
}
pub struct HistoryPanel {
    account: Arc<Controller>,
    pub model: Arc<History>,
    pub snapshot: Snapshot,
    pub selected: Option<ExistingChat>,
    selected_scope: Option<Option<String>>,
    archived: bool,
    projects_open: bool,
    _account_watch: Task<()>,
    _history_watch: Task<()>,
}
impl EventEmitter<HistoryEvent> for HistoryPanel {}
fn launch(f: impl Future<Output = ()> + Send + 'static, cx: &mut Context<HistoryPanel>) {
    let job = Tokio::spawn(cx, f);
    cx.spawn(async move |_, _| {
        let _ = job.await;
    })
    .detach();
}
impl HistoryPanel {
    pub fn new(account: Arc<Controller>, cx: &mut Context<Self>) -> Self {
        let model = History::new();
        model.reset(account.lease());
        if account.lease().is_some() {
            let a = model.more();
            let b = model.more_projects();
            launch(
                async move {
                    tokio::join!(a, b);
                },
                cx,
            );
        }
        let snapshot = model.snapshot();
        let mut auth = account.watch();
        let account_watch = cx.spawn({
            let model = model.clone();
            let account = account.clone();
            async move |this, cx| {
                while auth.changed().await.is_ok() {
                    let lease = account.lease();
                    let signed_in = lease.is_some();
                    model.reset(lease);
                    if this
                        .update(cx, |this, cx| {
                            this.selected = None;
                            this.selected_scope = None;
                            cx.emit(HistoryEvent::Reset);
                            if signed_in {
                                let a = model.more();
                                let b = model.more_projects();
                                launch(
                                    async move {
                                        tokio::join!(a, b);
                                    },
                                    cx,
                                );
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
        let mut updates = model.watch();
        let history_watch = cx.spawn({
            let account = account.clone();
            async move |this, cx| {
                while updates.changed().await.is_ok() {
                    let snapshot = updates.borrow_and_update().clone();
                    if this
                        .update(cx, |this, cx| {
                            if snapshot.login_required {
                                this.model.reset(None);
                                this.selected = None;
                                cx.emit(HistoryEvent::Reset);
                                launch(account.sign_out(), cx);
                            } else if !snapshot.signed_in
                                || this.selected.as_ref().is_some_and(|s| !s.is_current())
                            {
                                this.selected = None;
                                cx.emit(HistoryEvent::Reset);
                            } else if (matches!(
                                snapshot.state,
                                LoadState::Failed { stale: false, .. }
                            ) && snapshot.rows.is_empty())
                                || (snapshot.state == LoadState::Ready
                                    && !snapshot.has_more
                                    && this.selected_scope.as_ref()
                                        == Some(
                                            &snapshot.project.as_ref().map(|p| p.id().to_owned()),
                                        )
                                    && this.selected.as_ref().is_some_and(|selected| {
                                        !snapshot.rows.iter().any(|r| r.chat == selected.chat)
                                    }))
                            {
                                this.selected = None;
                                this.selected_scope = None;
                                cx.emit(HistoryEvent::Reset);
                            } else if let Some(selected) = &mut this.selected {
                                if let Some(row) =
                                    snapshot.rows.iter().find(|r| r.chat == selected.chat)
                                {
                                    selected.summary = row.clone();
                                }
                            }
                            this.snapshot = snapshot;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
        Self {
            account,
            model,
            snapshot,
            selected: None,
            selected_scope: None,
            archived: false,
            projects_open: false,
            _account_watch: account_watch,
            _history_watch: history_watch,
        }
    }
    fn more(&mut self, cx: &mut Context<Self>) {
        launch(self.model.more(), cx);
    }
    fn more_projects(&mut self, cx: &mut Context<Self>) {
        launch(self.model.more_projects(), cx);
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.model.refresh().is_ok() {
            launch(self.model.more(), cx);
        }
        if self.model.refresh_projects().is_ok() {
            launch(self.model.more_projects(), cx);
        }
    }
    pub fn project(&mut self, p: Option<&HistoryProject>, cx: &mut Context<Self>) {
        if self.model.project(p).is_ok() {
            launch(self.model.more(), cx);
        }
    }
    pub fn open(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(target) = self.model.existing(id) {
            self.selected_scope = Some(self.snapshot.project.as_ref().map(|p| p.id().to_owned()));
            self.selected = Some(target.clone());
            cx.emit(HistoryEvent::Open(target));
            cx.notify();
        }
    }
    pub fn reopen(
        &mut self,
        summary: coder_account::openagents::ChatSummary,
        cx: &mut Context<Self>,
    ) {
        let Some(lease) = self.account.lease() else {
            return;
        };
        if lease
            .client()
            .chat(lease.session(), summary.chat.id())
            .ok()
            .as_ref()
            != Some(&summary.chat)
        {
            return;
        }
        let target = ExistingChat {
            chat: summary.chat.clone(),
            summary,
            lease,
        };
        self.selected_scope = None;
        self.selected = Some(target.clone());
        cx.emit(HistoryEvent::Open(target));
        cx.notify();
    }
    pub fn toggle_archived(&mut self, cx: &mut Context<Self>) {
        self.archived = !self.archived;
        cx.notify();
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.selected_scope = None;
        cx.emit(HistoryEvent::Close);
        cx.notify();
    }
    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        text: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let hover = Theme::of(cx).glass_hover();
        div()
            .id(id)
            .tab_index(0)
            .role(gpui::Role::Button)
            .cursor_pointer()
            .px(px(6.))
            .py(px(5.))
            .rounded(px(5.))
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .child(text.into())
    }
    /// A real existing-chat target and its saved summary; the transcript renderer is separate.
    pub fn render_selected(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = Theme::of(cx).clone();
        let mut body = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(24.))
            .text_color(theme.text);
        if let Some(target) = self.selected.as_ref().filter(|t| t.is_current()) {
            body = body
                .child(
                    div()
                        .text_size(px(22.))
                        .child(SharedString::from(target.summary.title.clone())),
                )
                .child(SharedString::from(format!(
                    "{} saved messages",
                    target.summary.messages
                )));
        }
        body.child(self.button("close-saved-chat", "Back to workspace", Self::close, cx))
            .into_any_element()
    }
}
impl Drop for HistoryPanel {
    fn drop(&mut self) {
        self.model.reset(None);
    }
}
impl Render for HistoryPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let mut body = div()
            .id("openagents-history")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pb(px(12.))
            .text_size(px(12.))
            .text_color(theme.text);
        body = body.child(
            div()
                .px(px(6.))
                .text_color(theme.text_muted)
                .child("OpenAgents chats"),
        );
        if !self.snapshot.signed_in {
            return body.child(
                div()
                    .px(px(6.))
                    .py(px(8.))
                    .text_color(theme.text_muted)
                    .child("Sign in to see your chats."),
            );
        }
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(self.button("saved-refresh", "Refresh", Self::refresh, cx))
                .child(self.button(
                    "saved-archive",
                    if self.archived { "Active" } else { "Archived" },
                    Self::toggle_archived,
                    cx,
                ))
                .child(self.button(
                    "saved-projects",
                    "Projects",
                    |this, cx| {
                        this.projects_open = !this.projects_open;
                        cx.notify();
                    },
                    cx,
                )),
        );
        if self.projects_open {
            body = body.child(self.button(
                "saved-all",
                "My chats",
                |this, cx| this.project(None, cx),
                cx,
            ));
            let projects = self
                .snapshot
                .projects
                .iter()
                .filter(|p| p.shared)
                .cloned()
                .collect::<Vec<_>>();
            for p in projects {
                let name = p.name.clone();
                let key = SharedString::from(format!("saved-project-{}", p.id()));
                body = body.child(self.button(
                    key,
                    name,
                    move |this, cx| this.project(Some(&p), cx),
                    cx,
                ));
            }
            match &self.snapshot.projects_state {
                LoadState::Loading => body = body.child("Loading projects…"),
                LoadState::Failed { message, .. } => {
                    body = body
                        .child(SharedString::from(message.clone()))
                        .child(self.button(
                            "saved-projects-retry",
                            "Retry projects",
                            Self::more_projects,
                            cx,
                        ))
                }
                _ if self.snapshot.more_projects => {
                    body = body.child(self.button(
                        "saved-projects-more",
                        "More projects",
                        Self::more_projects,
                        cx,
                    ))
                }
                _ => {}
            }
        }
        if let Some(project) = &self.snapshot.project {
            body = body.child(
                div()
                    .px(px(6.))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(project.name.clone())),
            );
        }
        match &self.snapshot.state {
            LoadState::Loading => body = body.child(div().px(px(6.)).child("Loading chats…")),
            LoadState::Failed { message, stale } => {
                if *stale {
                    body = body.child(
                        div()
                            .px(px(6.))
                            .child("Couldn't refresh. Showing saved results."),
                    );
                }
                body = body
                    .child(div().px(px(6.)).child(SharedString::from(message.clone())))
                    .child(self.button("saved-retry", "Retry", Self::more, cx));
            }
            _ => {}
        }
        if !self.snapshot.rows.is_empty() {
            body = body.child(div().px(px(6.)).text_color(theme.text_faint).child(
                SharedString::from(format!("{} chats loaded", self.snapshot.rows.len())),
            ));
        }
        let mut rows = self
            .snapshot
            .rows
            .iter()
            .filter(|r| r.archived_unix.is_some() == self.archived)
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a.pinned_unix
                .is_none()
                .cmp(&b.pinned_unix.is_none())
                .then_with(|| {
                    if a.pinned_unix.is_some() && b.pinned_unix.is_some() {
                        b.pinned_unix.cmp(&a.pinned_unix)
                    } else {
                        a.project.cmp(&b.project)
                    }
                })
                .then_with(|| b.updated_unix.cmp(&a.updated_unix))
                .then_with(|| a.chat.id().cmp(b.chat.id()))
        });
        let mut group = None;
        for row in rows {
            let next = (
                row.pinned_unix.is_some(),
                if row.pinned_unix.is_some() {
                    None
                } else {
                    row.project.clone()
                },
            );
            if group.as_ref() != Some(&next) {
                let label = if next.0 {
                    "Pinned".to_owned()
                } else {
                    row.project
                        .as_ref()
                        .and_then(|id| self.snapshot.projects.iter().find(|p| p.id() == id))
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Chats".into())
                };
                body = body.child(
                    div()
                        .px(px(6.))
                        .pt(px(8.))
                        .text_color(theme.text_faint)
                        .child(SharedString::from(label)),
                );
                group = Some(next);
            }
            let id = row.chat.id().to_owned();
            let selected = self
                .selected
                .as_ref()
                .is_some_and(|s| s.chat == row.chat && s.is_current());
            let source = match row.source.as_str() {
                "web" => "Web",
                "terminal" => "Terminal",
                "phone" => "Phone",
                _ => "OpenAgents",
            };
            let activity =
                chrono::DateTime::from_timestamp(row.updated_unix.min(i64::MAX as u64) as i64, 0)
                    .filter(|_| row.updated_unix != 0)
                    .map(|d| d.format("%b %e, %Y").to_string())
                    .unwrap_or_default();
            let detail = if activity.is_empty() {
                source.to_owned()
            } else {
                format!("{source} · {activity}")
            };
            body =
                body.child(
                    div()
                        .id(SharedString::from(format!(
                            "saved:{}:{}:{}:{}:{}",
                            row.chat.deployment().api_base(),
                            row.chat.deployment().website(),
                            row.chat.account(),
                            row.source.as_str(),
                            id
                        )))
                        .tab_index(0)
                        .role(gpui::Role::Button)
                        .aria_label(SharedString::from(row.title.clone()))
                        .cursor_pointer()
                        .px(px(8.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .when(selected, |s| s.bg(theme.glass_hover()))
                        .hover(|s| s.bg(theme.glass_hover()))
                        .on_click(cx.listener(move |this, _, _, cx| this.open(&id, cx)))
                        .child(div().truncate().child(SharedString::from(
                            if row.title.is_empty() {
                                "Untitled chat".into()
                            } else {
                                row.title
                            },
                        )))
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(theme.text_muted)
                                .child(SharedString::from(detail)),
                        ),
                );
        }
        match &self.snapshot.state {
            LoadState::Loading | LoadState::Failed { .. } => {}
            LoadState::Ready => {
                let count = self
                    .snapshot
                    .rows
                    .iter()
                    .filter(|r| r.archived_unix.is_some() == self.archived)
                    .count();
                if count == 0 {
                    body = body.child(div().px(px(6.)).child(if self.archived {
                        "No archived chats in these results."
                    } else {
                        "No chats in these results."
                    }));
                }
                if self.snapshot.has_more {
                    body = body.child(self.button("saved-more", "Load more chats", Self::more, cx));
                } else {
                    body = body.child(
                        div()
                            .px(px(6.))
                            .text_color(theme.text_faint)
                            .child("All chats loaded."),
                    );
                }
            }
            LoadState::Idle => {}
        }
        body
    }
}
