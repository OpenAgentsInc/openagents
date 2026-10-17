//! The account's saved chats behind the sidebar's one Sessions list
//! (`shell::account_chats` draws them). Selecting a row never selects an
//! engine chat.
use gpui::{Context, EventEmitter, Task};
use gpui_tokio::Tokio;
use openagents_account::{
    Controller,
    history::{ExistingChat, History, LoadState, Snapshot},
};
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum HistoryEvent {
    Open(ExistingChat),
    Reset,
    Close,
}
/// How often the newest chats are read again while a window is active.
const REFRESH_EVERY: Duration = Duration::from_secs(60);
/// How often the panel checks whether the app came back to the front.
const FOCUS_POLL: Duration = Duration::from_secs(3);

pub struct HistoryPanel {
    account: Arc<Controller>,
    pub model: Arc<History>,
    pub snapshot: Snapshot,
    pub selected: Option<ExistingChat>,
    selected_scope: Option<Option<String>>,
    _account_watch: Task<()>,
    _history_watch: Task<()>,
    _refresh: Task<()>,
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
        // Read the newest chats again when the app comes back to the front
        // and every minute while it stays there; a failed page is retried.
        let refresh = cx.spawn(async move |this, cx| {
            let mut was_active = true;
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(FOCUS_POLL).await;
                let Ok(()) = this.update(cx, |this, cx| {
                    let active = cx.active_window().is_some();
                    if active && (!was_active || last.elapsed() >= REFRESH_EVERY) {
                        last = Instant::now();
                        this.refresh(cx);
                    }
                    was_active = active;
                }) else {
                    break;
                };
            }
        });
        Self {
            account,
            model,
            snapshot,
            selected: None,
            selected_scope: None,
            _account_watch: account_watch,
            _history_watch: history_watch,
            _refresh: refresh,
        }
    }
    /// The next page of older chats.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        launch(self.model.more(), cx);
    }
    /// Read the newest chats and project names again, keeping loaded pages.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.snapshot.signed_in {
            return;
        }
        if matches!(self.snapshot.state, LoadState::Failed { .. }) {
            launch(self.model.more(), cx);
        } else {
            launch(self.model.refresh_recent(), cx);
        }
        if self.model.refresh_projects().is_ok() {
            launch(self.model.more_projects(), cx);
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
        summary: openagents_account::openagents::ChatSummary,
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
    pub fn access_lost(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.selected_scope = None;
        self.model.reset(self.account.lease());
        self.snapshot = self.model.snapshot();
        if self.snapshot.signed_in {
            let a = self.model.more();
            let b = self.model.more_projects();
            launch(
                async move {
                    tokio::join!(a, b);
                },
                cx,
            );
        }
        cx.notify();
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.selected_scope = None;
        cx.emit(HistoryEvent::Close);
        cx.notify();
    }
}
impl Drop for HistoryPanel {
    fn drop(&mut self) {
        self.model.reset(None);
    }
}
