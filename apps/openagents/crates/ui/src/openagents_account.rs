//! The app account control. Local workspace and model-provider logins stay separate.
use crate::theme::Theme;
use coder_account::{Controller, Phase, Snapshot};
use gpui::{
    App, AppContext, Context, Entity, Global, Render, SharedString, Task, Window, div, prelude::*,
    px,
};
use gpui_tokio::Tokio;
use std::{future::Future, path::PathBuf, sync::Arc};

struct GlobalAccount(Entity<AppAccount>);
impl Global for GlobalAccount {}
pub struct AppAccount {
    pub controller: Option<Arc<Controller>>,
    pub snapshot: Snapshot,
    _watch: Option<Task<()>>,
    expiry: Option<Task<()>>,
    opened_generation: Option<u64>,
    automatic_browser: bool,
}
impl AppAccount {
    pub fn init(data: PathBuf, cx: &mut App) {
        let configured = coder_account::desktop::configured(&data);
        let mut credential = None;
        let entity = cx.new(|cx| match configured {
            Ok((controller, supplied)) => {
                credential = supplied;
                Self::new(controller, cx)
            }
            Err(e) => Self {
                controller: None,
                snapshot: Snapshot {
                    generation: 0,
                    phase: Phase::Failed {
                        message: e.to_string(),
                        can_restore: false,
                    },
                },
                _watch: None,
                expiry: None,
                opened_generation: None,
                automatic_browser: true,
            },
        });
        cx.set_global(GlobalAccount(entity.clone()));
        entity.update(cx, |this, cx| {
            if let Some(credential) = credential {
                if let Some(controller) = &this.controller {
                    let operation = controller.import_credential(credential);
                    this.run(operation, cx);
                }
            } else {
                this.restore(cx);
            }
        });
    }
    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalAccount>().map(|a| a.0.clone())
    }
    #[cfg(feature = "account-fixture")]
    pub fn install_fixture(controller: Arc<Controller>, cx: &mut App) -> Entity<Self> {
        let account = cx.new(|cx| {
            let mut account = Self::new(controller, cx);
            account.automatic_browser = false;
            account
        });
        cx.set_global(GlobalAccount(account.clone()));
        account
    }
    pub fn new(controller: Arc<Controller>, cx: &mut Context<Self>) -> Self {
        let mut updates = controller.watch();
        let snapshot = controller.snapshot();
        let watch = cx.spawn(async move |this, cx| {
            while updates.changed().await.is_ok() {
                let snapshot = updates.borrow_and_update().clone();
                if this
                    .update(cx, |this, cx| {
                        if let Phase::Waiting { url, .. } = &snapshot.phase {
                            if this.automatic_browser
                                && this.opened_generation != Some(snapshot.generation)
                            {
                                this.opened_generation = Some(snapshot.generation);
                                cx.open_url(url);
                            }
                        }
                        this.expiry = None;
                        if matches!(snapshot.phase, Phase::SignedIn { .. }) {
                            let controller = this.controller.as_ref().unwrap().clone();
                            let generation = snapshot.generation;
                            this.expiry = Some(cx.spawn(async move |this, cx| {
                                let mut checked = std::time::Instant::now();
                                loop {
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_secs(1))
                                        .await;
                                    if controller.snapshot().generation != generation {
                                        break;
                                    }
                                    if checked.elapsed() >= std::time::Duration::from_secs(15) {
                                        checked = std::time::Instant::now();
                                        let check = controller.clone();
                                        let _ = this.update(cx, |this, cx| {
                                            this.run(check.revalidate(), cx)
                                        });
                                    }
                                    if controller.lease().is_none() {
                                        let _ = this.update(cx, |this, cx| this.restore(cx));
                                        break;
                                    }
                                }
                            }));
                        }
                        this.snapshot = snapshot;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            controller: Some(controller),
            snapshot,
            _watch: Some(watch),
            expiry: None,
            opened_generation: None,
            automatic_browser: true,
        }
    }
    fn run(&mut self, f: impl Future<Output = ()> + Send + 'static, cx: &mut Context<Self>) {
        let job = Tokio::spawn(cx, f);
        cx.spawn(async move |_, _| {
            let _ = job.await;
        })
        .detach();
        if let Some(c) = &self.controller {
            self.snapshot = c.snapshot();
        }
        cx.notify();
    }
    pub fn sign_in(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.controller {
            let computer = gethostname::gethostname()
                .to_string_lossy()
                .chars()
                .take(80)
                .collect::<String>();
            let operation = c.sign_in(if computer.is_empty() {
                "This Mac".into()
            } else {
                computer
            });
            self.run(operation, cx);
        }
    }
    fn restore(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.controller {
            let operation = c.restore();
            self.run(operation, cx);
        }
    }
    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.controller {
            let operation = c.sign_out();
            self.run(operation, cx);
        }
    }
    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        action: fn(&mut Self, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = Theme::of(cx);
        div()
            .id(id)
            .role(gpui::Role::Button)
            .aria_label(label)
            .tab_index(0)
            .rounded(px(6.))
            .px(px(8.))
            .py(px(5.))
            .bg(theme.glass_hover())
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .child(label)
    }
}
impl Render for AppAccount {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let phase = self.snapshot.phase.clone();
        let mut body = div()
            .id("openagents-account")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(6.))
            .text_size(px(12.))
            .text_color(theme.text);
        match phase {
            Phase::SignedOut { notice } => {
                if let Some(notice) = notice {
                    body = body.child(SharedString::from(notice));
                }
                body = body.child(self.button(
                    "openagents-sign-in",
                    "Sign in to OpenAgents",
                    Self::sign_in,
                    cx,
                ));
            }
            Phase::Restoring | Phase::Starting => {
                body = body.child(if matches!(phase, Phase::Restoring) {
                    "Checking your login…"
                } else {
                    "Connecting…"
                });
                body = body.child(self.button("openagents-cancel", "Cancel", Self::sign_out, cx));
            }
            Phase::Waiting { code, url } => {
                body = body
                    .child("Approve login in your browser.")
                    .child(
                        div()
                            .font_family(theme.font_mono.clone())
                            .child(SharedString::from(code)),
                    )
                    .child(
                        div()
                            .id("openagents-open-browser")
                            .role(gpui::Role::Button)
                            .aria_label("Open browser")
                            .tab_index(0)
                            .cursor_pointer()
                            .on_click(move |_, _, cx| cx.open_url(&url))
                            .child("Open browser"),
                    )
                    .child(self.button("openagents-cancel", "Cancel", Self::sign_out, cx));
            }
            Phase::SignedIn { account } => {
                body = body
                    .child(
                        div()
                            .id("openagents-account-name")
                            .truncate()
                            .child(SharedString::from(account.label)),
                    )
                    .child(
                        div()
                            .id("agent-lab")
                            .cursor_pointer()
                            .child("Agent Lab ↗")
                            .on_click(|_, _, cx| cx.open_url("https://openagents.com/labs/agent")),
                    )
                    .child(self.button("openagents-sign-out", "Sign out", Self::sign_out, cx));
            }
            Phase::Failed {
                message,
                can_restore,
            } => {
                body = body.child(SharedString::from(message));
                if self.controller.is_some() {
                    body = body.child(self.button(
                        "openagents-retry",
                        "Retry",
                        if can_restore {
                            Self::restore
                        } else {
                            Self::sign_in
                        },
                        cx,
                    ));
                }
            }
        }
        body
    }
}

impl Drop for AppAccount {
    fn drop(&mut self) {
        if let Some(controller) = &self.controller {
            controller.invalidate();
        }
    }
}
