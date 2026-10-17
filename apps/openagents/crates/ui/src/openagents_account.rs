//! The app account control. Local workspace and model-provider logins stay separate.
//!
//! Nobody signs in (#11997, [`identity`]): a fresh computer opens the
//! welcome, the identity's key signs in silently after that, and the sidebar
//! row shows the person's ID, linked devices, and Back up your account.
use crate::{
    icons::{self, icon},
    motion, popover,
    theme::Theme,
};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, Global, MouseButton, Render, SharedString, Task,
    Window, div, prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_account::{Controller, Deletion, Phase, Snapshot};
use std::{future::Future, path::PathBuf, sync::Arc};

mod identity;
#[cfg(feature = "account-fixture")]
pub use identity::capture_identity;

struct GlobalAccount(Entity<AppAccount>);
impl Global for GlobalAccount {}
pub struct AppAccount {
    pub controller: Option<Arc<Controller>>,
    pub snapshot: Snapshot,
    _watch: Option<Task<()>>,
    expiry: Option<Task<()>>,
    menu: popover::Popup<()>,
    /// Where Delete account stands (#12002), while its menu is open.
    deleting: Option<DeleteStep>,
    id: identity::IdentityState,
}
/// The steps of Delete account in the account menu.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DeleteStep {
    Ask,
    PaidCredit,
    Working,
    Notice(String),
}
impl AppAccount {
    pub fn init(data: PathBuf, cx: &mut App) {
        let configured = openagents_account::desktop::configured(&data);
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
                menu: popover::Popup::default(),
                deleting: None,
                id: Default::default(),
            },
        });
        cx.set_global(GlobalAccount(entity.clone()));
        entity.update(cx, |this, cx| {
            let backend = this.controller.clone().map(|controller| {
                openagents_account::desktop::identity(&data, controller, computer_name())
            });
            this.attach_identity(data, backend, cx);
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
        let account = cx.new(|cx| Self::new(controller, cx));
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
                                            this.run(check.revalidate(), cx);
                                            // Sign in again with the key before
                                            // the session ends.
                                            if let Some(backend) = this.id.backend.clone() {
                                                this.run(backend.renew(), cx);
                                            }
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
                        this.identity_changed(cx);
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
            menu: popover::Popup::default(),
            deleting: None,
            id: Default::default(),
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
    /// Where the app used to ask for a sign-in: open the welcome instead.
    /// There is no browser sign-in or code to type.
    pub fn sign_in(&mut self, cx: &mut Context<Self>) {
        if self.identity_enabled() {
            self.open_welcome(cx);
            return;
        }
        // The account fixture drives the older browser sign-in against its
        // own test server.
        #[cfg(feature = "account-fixture")]
        if let Some(c) = &self.controller {
            let operation = c.sign_in(computer_name());
            self.run(operation, cx);
        }
    }
    /// Sign in silently with this computer's key, or restore a saved session.
    fn restore(&mut self, cx: &mut Context<Self>) {
        if let Some(backend) = self.id.backend.clone() {
            self.run(backend.start(), cx);
        } else if let Some(c) = &self.controller {
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
    /// Delete the account on the server, then sign this app out. The
    /// answer moves [`DeleteStep`] on.
    fn delete_account(&mut self, forfeit_balance: bool, cx: &mut Context<Self>) {
        let Some(controller) = self.controller.clone() else {
            return;
        };
        self.deleting = Some(DeleteStep::Working);
        cx.notify();
        let job = Tokio::spawn(cx, async move {
            controller.delete_account(forfeit_balance).await
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.deleting = match result {
                    Ok(Deletion::Deleted) => None,
                    Ok(Deletion::PaidCredit) => Some(DeleteStep::PaidCredit),
                    Ok(Deletion::SharedTeam) => Some(DeleteStep::Notice(
                        openagents_account::SHARED_TEAM_NOTICE.into(),
                    )),
                    Ok(Deletion::Failed(message)) => Some(DeleteStep::Notice(message)),
                    Err(_) => Some(DeleteStep::Notice(
                        "We couldn't delete your account. Try again.".into(),
                    )),
                };
                cx.notify();
            });
        })
        .detach();
    }
    /// The account menu's Delete account steps: the warning, the paid
    /// credit question, and the answer.
    fn delete_rows(&self, step: &DeleteStep, cx: &mut Context<Self>) -> gpui::Div {
        let theme = Theme::of(cx).for_popup();
        let words = |text: SharedString| {
            div()
                .px(px(8.))
                .py(px(4.))
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .child(text)
        };
        let mut rows = div().flex().flex_col().gap(px(2.));
        match step {
            DeleteStep::Ask | DeleteStep::PaidCredit => {
                let (warning, label, forfeit) = if *step == DeleteStep::Ask {
                    (
                        openagents_account::DELETE_WARNING,
                        "Delete my account",
                        false,
                    )
                } else {
                    (
                        openagents_account::PAID_CREDIT_WARNING,
                        "Give up the credit and delete",
                        true,
                    )
                };
                rows = rows
                    .child(words(warning.into()))
                    .child(
                        popover::menu_row(&theme, false, "openagents-delete-confirm")
                            .id("openagents-delete-confirm")
                            .text_color(theme.danger)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete_account(forfeit, cx);
                            }))
                            .child(label),
                    )
                    .child(
                        popover::menu_row(&theme, false, "openagents-delete-cancel")
                            .id("openagents-delete-cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.deleting = None;
                                cx.notify();
                            }))
                            .child("Keep my account"),
                    );
            }
            DeleteStep::Working => {
                rows = rows.child(words("Deleting your account…".into()));
            }
            DeleteStep::Notice(message) => {
                rows = rows.child(words(message.clone().into())).child(
                    popover::menu_row(&theme, false, "openagents-delete-close")
                        .id("openagents-delete-close")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.deleting = None;
                            cx.notify();
                        }))
                        .child("OK"),
                );
            }
        }
        rows
    }
    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.deleting == Some(DeleteStep::Working) {
            return;
        }
        self.deleting = None;
        if self.menu.begin_close() {
            popover::reap_popup(cx, |this: &mut Self| &mut this.menu);
            cx.notify();
        }
    }
    #[cfg(feature = "account-fixture")]
    pub fn fixture_open_menu(&mut self, cx: &mut Context<Self>) {
        self.menu.open(());
        cx.notify();
    }
    /// The signed-in account: avatar and name in the sidebar footer, matching
    /// the footer's icon buttons. The menu opens upward.
    fn signed_in_row(&mut self, label: SharedString, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).for_popup();
        let open = self.menu.is_open();
        // An npub's first letters are all `npub1`; the next one tells IDs apart.
        let initial: SharedString = label
            .trim()
            .strip_prefix("npub1")
            .unwrap_or(label.trim())
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into())
            .into();
        let mut row = div()
            .id("openagents-account")
            .debug_selector(|| "openagents-account".into())
            .role(gpui::Role::Button)
            .aria_label(format!("Account menu: {label}"))
            .aria_expanded(open)
            .tab_index(0)
            .focus_visible(|s| s.border_2().border_color(theme.accent))
            .relative()
            .h(px(28.))
            .min_w_0()
            .flex_shrink_1()
            .w_full()
            .rounded(px(8.))
            .px(px(Theme::SPACE_SM))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Theme::SPACE_SM))
            .text_size(crate::typography::ui_rems(13.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(if open {
                theme.text
            } else {
                motion::hover_blend("openagents-account", theme.text.opacity(0.8), theme.text)
            })
            .bg(if open {
                theme.glass_hover()
            } else {
                motion::hover_blend(
                    "openagents-account",
                    theme.glass_hover().opacity(0.0),
                    theme.glass_hover().opacity(0.8),
                )
            })
            .on_hover(motion::hover_listener("openagents-account"))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.menu.note_trigger_press()),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if this.menu.take_press_was_open() {
                    this.close_menu(cx);
                } else {
                    this.menu.open(());
                }
                cx.notify();
            }))
            .child(
                div()
                    .size(px(16.))
                    .flex_none()
                    .rounded_full()
                    .bg(theme.text)
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family(theme.font_mono.clone())
                    .text_size(px(10.))
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.bg)
                    .child(div().w_full().text_center().child(initial)),
            )
            .child(crate::shell::sidebar_faded_label(
                "openagents-account-label".into(),
                false,
                div().line_height(px(17.)).child(label),
            ));
        if self.menu.get().is_some()
            && let Some(step) = self.deleting.clone()
        {
            let closing = self.menu.closing_since();
            let menu = popover::popover_card(&theme)
                .w(px(260.))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_menu(cx)))
                .child(self.delete_rows(&step, cx))
                .into_any_element();
            row = row.child(popover::anchored_menu_above(
                "openagents-account-menu",
                menu,
                closing,
            ));
        } else if self.menu.get().is_some() {
            let closing = self.menu.closing_since();
            let menu = popover::popover_card(&theme)
                .w(px(220.))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_menu(cx)))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    popover::menu_row(&theme, false, "openagents-connect-phone")
                        .id("openagents-connect-phone")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_menu(cx);
                            window.dispatch_action(Box::new(crate::shell::ConnectPhone), cx);
                        }))
                        .child(
                            icon(icons::SMARTPHONE)
                                .size(px(16.))
                                .text_color(theme.text_muted),
                        )
                        .child(crate::shell::CONNECT_PHONE_TITLE),
                )
                .child(
                    popover::menu_row(&theme, false, "openagents-agent-lab")
                        .id("openagents-agent-lab")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_menu(cx);
                            cx.open_url("https://openagents.com/labs/agent");
                        }))
                        .child(icon(icons::BOT).size(px(16.)).text_color(theme.text_muted))
                        .child(div().flex_1().child("Agent Lab"))
                        .child(
                            icon(icons::ARROW_UP_RIGHT)
                                .size(px(12.))
                                .text_color(theme.text_muted),
                        ),
                )
                .child(
                    popover::menu_row(&theme, false, "openagents-sign-out")
                        .id("openagents-sign-out")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_menu(cx);
                            this.sign_out(cx);
                        }))
                        .child(
                            icon(icons::LOGOUT_2)
                                .size(px(16.))
                                .text_color(theme.text_muted),
                        )
                        .child("Sign out"),
                )
                .child(
                    popover::menu_row(&theme, false, "openagents-delete-account")
                        .id("openagents-delete-account")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.deleting = Some(DeleteStep::Ask);
                            cx.notify();
                        }))
                        .child(
                            icon(icons::TRASH_BIN_MINIMALISTIC)
                                .size(px(16.))
                                .text_color(theme.text_muted),
                        )
                        .child("Delete account…"),
                )
                .into_any_element();
            let menu = self.identity_menu(&theme, cx).into_any_element();
            row = row.child(popover::anchored_menu_above(
                "openagents-account-menu",
                menu,
                closing,
            ));
        }
        if self.needs_backup() {
            row = row.child(
                div()
                    .absolute()
                    .top(px(5.))
                    .left(px(19.))
                    .size(px(6.))
                    .rounded_full()
                    .bg(theme.accent),
            );
        }
        row.into_any_element()
    }
}
impl Render for AppAccount {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // No sign-in: the ID, or Set up your account on a fresh computer.
        match self.identity_label() {
            Some(label) => self.signed_in_row(label, cx),
            None => {
                self.menu = popover::Popup::default();
                self.set_up_row(cx)
            }
        }
    }
}

/// This computer's name as the account lists it.
fn computer_name() -> String {
    let computer = gethostname::gethostname()
        .to_string_lossy()
        .chars()
        .take(80)
        .collect::<String>();
    if computer.is_empty() {
        "This Mac".into()
    } else {
        computer
    }
}

impl Drop for AppAccount {
    fn drop(&mut self) {
        if let Some(controller) = &self.controller {
            controller.invalidate();
        }
    }
}
