//! The desktop identity (#11997): the first-launch welcome, the sidebar
//! identity row's menu, and Back up your account.
//!
//! Nobody signs in. The welcome offers Connect your phone, Start here, or
//! Restore from 12 words; after that the identity's key signs in silently on
//! every launch through the account controller. The words and the key stay
//! in `openagents_account::desktop::identity`'s [`DesktopIdentity`]; this
//! module only draws them and passes the person's choices on.
use super::AppAccount;
use crate::{
    composer::{ComposerInput, ComposerInputEvent},
    icons::{self, icon},
    popover,
    theme::Theme,
};
use gpui::{
    AnyElement, AppContext, ClipboardItem, Context, Entity, Focusable, Pixels, SharedString,
    Subscription, Task, Window, div, prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_account::{
    Phase,
    desktop::identity::{
        self as identity, BackupStep, DesktopIdentity, IdentityView, LinkedDevice, Pending,
        WelcomeStep, copy,
    },
};
use std::{path::PathBuf, sync::Arc, time::Duration};

/// How long a copied ID stays marked Copied.
const COPIED_SECS: u64 = 2;

/// The identity half of [`AppAccount`].
#[derive(Default)]
pub struct IdentityState {
    /// `None` only when the account could not be set up at all, and in
    /// fixtures that drive the controller directly.
    pub(super) backend: Option<Arc<dyn DesktopIdentity>>,
    pub(super) data: PathBuf,
    pub(super) view: Option<IdentityView>,
    devices: Option<Vec<LinkedDevice>>,
    /// The sign-in the device list was read for.
    devices_for: Option<u64>,
    welcome: Option<WelcomeStep>,
    /// The welcome opens by itself once per launch at most.
    welcome_offered: bool,
    words: Option<Entity<ComposerInput>>,
    focus_pending: bool,
    _words_events: Option<Subscription>,
    backup: Option<BackupStep>,
    copied: bool,
    copied_task: Option<Task<()>>,
}

impl AppAccount {
    /// Install the identity for `data`; called once from `init`.
    pub(super) fn attach_identity(
        &mut self,
        data: PathBuf,
        backend: Option<Arc<dyn DesktopIdentity>>,
        cx: &mut Context<Self>,
    ) {
        self.id.data = data;
        self.id.backend = backend;
        self.identity_changed(cx);
    }

    /// Whether this computer has the identity flow.
    pub fn identity_enabled(&self) -> bool {
        self.id.backend.is_some()
    }

    /// Whether the welcome or Back up is on screen and owns the keyboard.
    pub fn identity_overlay_open(&self) -> bool {
        self.id.welcome.is_some() || self.id.backup.is_some()
    }

    /// Read the identity again after any account change: open the welcome on
    /// a fresh computer, and list linked devices once signed in.
    pub(super) fn identity_changed(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.id.backend.clone() else {
            return;
        };
        self.id.view = backend.identity();
        if self.id.view.is_some() && matches!(self.id.welcome, Some(WelcomeStep::Choose { .. })) {
            // The phone linked this computer while the welcome was open.
            self.id.welcome = None;
        }
        if !self.id.welcome_offered
            && identity::shows_welcome(
                self.id.view.as_ref(),
                &self.snapshot.phase,
                identity::welcome_dismissed(&self.id.data),
            )
        {
            self.id.welcome_offered = true;
            self.open_welcome(cx);
        }
        if matches!(self.snapshot.phase, Phase::SignedIn { .. }) {
            let generation = self.snapshot.generation;
            if self.id.devices_for != Some(generation) {
                self.id.devices_for = Some(generation);
                self.read_devices(backend, cx);
            }
        } else {
            self.id.devices = None;
            self.id.devices_for = None;
        }
        cx.notify();
    }

    fn read_devices(&mut self, backend: Arc<dyn DesktopIdentity>, cx: &mut Context<Self>) {
        let generation = self.snapshot.generation;
        self.after(backend.linked_devices(), cx, move |this, result, _| {
            if this.snapshot.generation == generation {
                // A failed read leaves the list out of the menu; the next
                // sign-in reads it again.
                this.id.devices = result.ok();
            }
        });
    }

    /// Run identity work on Tokio and hand its result back on this entity.
    fn after<T: Send + 'static>(
        &mut self,
        work: Pending<T>,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<T, String>, &mut Context<Self>) + 'static,
    ) {
        let job = Tokio::spawn(cx, work);
        cx.spawn(async move |this, cx| {
            let result = match job.await {
                Ok(result) => result,
                Err(_) => Err("Something went wrong. Try again.".into()),
            };
            let _ = this.update(cx, |this, cx| {
                then(this, result, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_welcome(&mut self, cx: &mut Context<Self>) {
        self.close_menu(cx);
        self.id.welcome = Some(WelcomeStep::Choose { error: None });
        cx.notify();
    }

    fn not_now(&mut self, cx: &mut Context<Self>) {
        identity::dismiss_welcome(&self.id.data);
        self.close_welcome(cx);
    }

    fn close_welcome(&mut self, cx: &mut Context<Self>) {
        if matches!(self.id.welcome, Some(WelcomeStep::Working)) {
            return;
        }
        self.id.welcome = None;
        self.id.words = None;
        self.id._words_events = None;
        cx.notify();
    }

    /// Show the code, and sign in once a phone has linked this computer to
    /// its account (#12000). Until then nothing signs in.
    fn welcome_connect_phone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_welcome(cx);
        window.dispatch_action(Box::new(crate::shell::ConnectPhone), cx);
        if let Some(backend) = self.id.backend.clone() {
            self.after(backend.connect_phone(), cx, |this, result, cx| {
                // An error here means the person picked another way in.
                if let Ok(view) = result {
                    this.identity_ready(view, cx);
                }
            });
        }
    }

    fn start_here(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.id.backend.clone() else {
            return;
        };
        self.id.welcome = Some(WelcomeStep::Working);
        self.after(backend.start_new(), cx, |this, result, cx| match result {
            Ok(view) => this.identity_ready(view, cx),
            Err(error) => {
                this.id.welcome = Some(WelcomeStep::Choose { error: Some(error) });
            }
        });
        cx.notify();
    }

    fn show_restore(&mut self, cx: &mut Context<Self>) {
        let input = cx.new(|cx| ComposerInput::new(copy::RESTORE_PLACEHOLDER, cx));
        let events = cx.subscribe(&input, |this: &mut Self, _, event, cx| {
            if matches!(event, ComposerInputEvent::Submitted) {
                this.submit_restore(cx);
            } else if let Some(WelcomeStep::Restore { error }) = &mut this.id.welcome {
                // Typing clears the last complaint.
                *error = None;
                cx.notify();
            }
        });
        self.id.words = Some(input);
        self.id._words_events = Some(events);
        self.id.focus_pending = true;
        self.id.welcome = Some(WelcomeStep::Restore { error: None });
        cx.notify();
    }

    fn submit_restore(&mut self, cx: &mut Context<Self>) {
        let (Some(backend), Some(input)) = (self.id.backend.clone(), self.id.words.clone()) else {
            return;
        };
        let words = match identity::typed_words(input.read(cx).text()) {
            Ok(words) => words,
            Err(error) => {
                self.id.welcome = Some(WelcomeStep::Restore { error: Some(error) });
                cx.notify();
                return;
            }
        };
        self.id.welcome = Some(WelcomeStep::Working);
        self.after(
            backend.restore(words),
            cx,
            |this, result, cx| match result {
                Ok(view) => this.identity_ready(view, cx),
                Err(error) => {
                    this.id.welcome = Some(WelcomeStep::Restore { error: Some(error) });
                    this.id.focus_pending = true;
                }
            },
        );
        cx.notify();
    }

    fn identity_ready(&mut self, view: IdentityView, cx: &mut Context<Self>) {
        self.id.view = Some(view);
        self.id.welcome = None;
        // The typed words leave memory with the field.
        self.id.words = None;
        self.id._words_events = None;
        self.identity_changed(cx);
    }

    fn open_backup(&mut self, cx: &mut Context<Self>) {
        self.close_menu(cx);
        self.id.backup = Some(BackupStep::open(self.id.view.as_ref()));
        cx.notify();
    }

    fn show_words(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = &self.id.backend else {
            return;
        };
        self.id.backup = Some(match backend.words() {
            Ok(words) => BackupStep::Words(words),
            Err(_) => BackupStep::NoWords,
        });
        cx.notify();
    }

    fn wrote_them_down(&mut self, cx: &mut Context<Self>) {
        if let Some(backend) = &self.id.backend {
            let _ = backend.confirm_saved();
            self.id.view = backend.identity();
        }
        self.id.backup = Some(BackupStep::Done);
        cx.notify();
    }

    fn close_backup(&mut self, cx: &mut Context<Self>) {
        // Dropping the step drops the words.
        self.id.backup = None;
        cx.notify();
    }

    fn copy_id(&mut self, cx: &mut Context<Self>) {
        let Some(view) = &self.id.view else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(view.npub.clone()));
        self.id.copied = true;
        self.id.copied_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(COPIED_SECS))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.id.copied = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// The row's label: the short npub, or the signed-in account's name while
    /// an older sign-in waits for its key.
    pub(super) fn identity_label(&self) -> Option<SharedString> {
        if let Some(view) = &self.id.view {
            return Some(identity::short_npub(&view.npub).into());
        }
        match &self.snapshot.phase {
            Phase::SignedIn { account } => Some(account.label.clone().into()),
            _ => None,
        }
    }

    /// Whether the row carries a dot asking for a backup.
    pub(super) fn needs_backup(&self) -> bool {
        self.id
            .view
            .as_ref()
            .is_some_and(|view| view.has_words && !view.words_saved)
    }

    /// The sidebar footer with no identity yet: Set up your account and
    /// Connect your phone. No sign-in and no code to type.
    pub(super) fn set_up_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx);
        let entry = |id: &'static str, glyph: &'static str, label: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .role(gpui::Role::Button)
                .aria_label(label)
                .tab_index(0)
                .rounded(px(6.))
                .px(px(8.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .child(icon(glyph).size(px(14.)).text_color(theme.text_muted))
                .child(label)
        };
        let mut body = div()
            .id("openagents-account")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(px(12.))
            .text_color(theme.text);
        if let Phase::Failed { message, .. } = &self.snapshot.phase {
            body = body.child(
                div()
                    .px(px(8.))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(message.clone())),
            );
        }
        body.child(
            entry("openagents-set-up", icons::KEY_MINIMALISTIC, copy::SET_UP)
                .bg(theme.glass_hover())
                .on_click(cx.listener(|this, _, _, cx| this.open_welcome(cx))),
        )
        .child(
            entry(
                "openagents-connect-phone",
                icons::SMARTPHONE,
                crate::shell::CONNECT_PHONE_TITLE,
            )
            .text_color(theme.text_muted)
            .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(crate::shell::ConnectPhone), cx);
            }),
        )
        .into_any_element()
    }

    /// The identity row's menu: Your ID, linked devices, Back up your
    /// account, Connect your phone, and Agent Lab. There is no Sign out:
    /// the key is the account.
    pub(super) fn identity_menu(&mut self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut menu = popover::popover_card(theme)
            .w(px(260.))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_menu(cx)))
            .flex()
            .flex_col()
            .gap(px(2.));
        let heading = |text: &'static str| {
            div()
                .px(px(8.))
                .pt(px(6.))
                .pb(px(2.))
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .child(text)
        };
        if let Some(view) = &self.id.view {
            menu = menu.child(heading(copy::YOUR_ID)).child(
                popover::menu_row(theme, false, "openagents-copy-id")
                    .id("openagents-copy-id")
                    .debug_selector(|| "openagents-copy-id".into())
                    .aria_label(copy::COPY_ID)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_id(cx)))
                    .child(
                        icon(if self.id.copied {
                            icons::CHECK
                        } else {
                            icons::COPY
                        })
                        .size(px(16.))
                        .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(theme.font_mono.clone())
                            .child(SharedString::from(identity::short_npub(&view.npub))),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text_muted)
                            .child(if self.id.copied { copy::COPIED } else { "" }),
                    ),
            );
        }
        if let Some(devices) = &self.id.devices {
            menu = menu.child(heading(copy::LINKED_DEVICES));
            let rows = identity::device_rows(devices);
            let only_this = rows.len() <= 1;
            for (ix, (label, detail)) in rows.into_iter().enumerate() {
                let this_computer = label == copy::THIS_COMPUTER;
                menu = menu.child(
                    div()
                        .id(("openagents-device", ix))
                        .px(px(8.))
                        .py(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            icon(if this_computer {
                                icons::LAPTOP
                            } else {
                                icons::SMARTPHONE
                            })
                            .size(px(16.))
                            .text_color(theme.text_muted),
                        )
                        .child(
                            div()
                                .flex_1()
                                .when(!this_computer, |d| d.font_family(theme.font_mono.clone()))
                                .child(SharedString::from(label)),
                        )
                        .child(
                            div()
                                .font_family(theme.font_mono.clone())
                                .text_size(px(11.))
                                .text_color(theme.text_muted)
                                .child(SharedString::from(detail)),
                        ),
                );
            }
            if only_this {
                menu = menu.child(
                    div()
                        .px(px(8.))
                        .pb(px(4.))
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .child(copy::NO_DEVICES),
                );
            }
        }
        if let Phase::Failed { message, .. } = &self.snapshot.phase {
            menu = menu.child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .text_size(px(11.))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(message.clone())),
            );
        }
        menu = menu.child(div().my(px(4.)).h(px(1.)).bg(theme.border.opacity(0.5)));
        if self.id.view.is_some() {
            let dot = self.needs_backup();
            menu = menu.child(
                popover::menu_row(theme, false, "openagents-back-up")
                    .id("openagents-back-up")
                    .debug_selector(|| "openagents-back-up".into())
                    .on_click(cx.listener(|this, _, _, cx| this.open_backup(cx)))
                    .child(
                        icon(icons::KEY_MINIMALISTIC)
                            .size(px(16.))
                            .text_color(theme.text_muted),
                    )
                    .child(div().flex_1().child(copy::BACK_UP))
                    .when(dot, |row| {
                        row.child(div().size(px(6.)).rounded_full().bg(theme.accent))
                    }),
            );
        }
        menu.child(
            popover::menu_row(theme, false, "openagents-connect-phone")
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
            popover::menu_row(theme, false, "openagents-agent-lab")
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
    }

    /// The welcome and Back up dialogs, drawn above the shell.
    pub fn render_identity_overlays(
        &mut self,
        viewport: gpui::Size<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut overlays = Vec::new();
        if self.id.backend.is_none() {
            return overlays;
        }
        if let Some(step) = self.id.welcome.clone() {
            if std::mem::take(&mut self.id.focus_pending) {
                if let Some(input) = &self.id.words {
                    window.focus(&input.focus_handle(cx), cx);
                }
            }
            let card = self.welcome_card(step, cx);
            overlays.push(popover::modal("openagents-welcome", viewport, card));
        }
        if let Some(step) = self.id.backup.clone() {
            let card = self.backup_card(step, cx);
            overlays.push(popover::modal("openagents-backup", viewport, card));
        }
        overlays
    }

    fn welcome_card(&mut self, step: WelcomeStep, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).for_popup();
        let muted = |text: SharedString| div().mt(px(8.)).text_color(theme.text_muted).child(text);
        let mut card = popover::dialog_card(&theme)
            .w(px(420.))
            .debug_selector(|| "openagents-welcome".into())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    match this.id.welcome {
                        Some(WelcomeStep::Restore { .. }) => {
                            this.id.welcome = Some(WelcomeStep::Choose { error: None });
                            cx.notify();
                        }
                        _ => this.not_now(cx),
                    }
                    cx.stop_propagation();
                }
            }));
        match step {
            WelcomeStep::Choose { error } => {
                card = card
                    .child(popover::dialog_title(&theme, copy::WELCOME_TITLE))
                    .child(muted(copy::WELCOME_BODY.into()))
                    .child(
                        div()
                            .mt(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                self.choice(
                                    "openagents-welcome-phone",
                                    icons::SMARTPHONE,
                                    copy::CONNECT_PHONE,
                                    copy::CONNECT_PHONE_HINT,
                                    true,
                                    &theme,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| this.welcome_connect_phone(window, cx),
                                )),
                            )
                            .child(
                                self.choice(
                                    "openagents-welcome-start",
                                    icons::KEY_MINIMALISTIC,
                                    copy::START_HERE,
                                    copy::START_HERE_HINT,
                                    false,
                                    &theme,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.start_here(cx))),
                            )
                            .child(
                                self.choice(
                                    "openagents-welcome-restore",
                                    icons::KEYBOARD,
                                    copy::RESTORE,
                                    copy::RESTORE_HINT,
                                    false,
                                    &theme,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.show_restore(cx))),
                            ),
                    );
                if let Some(error) = error {
                    card = card.child(muted(error.into()).text_color(theme.danger));
                }
                card = card.child(
                    div().mt(px(16.)).flex().justify_end().child(
                        popover::btn_ghost(&theme, copy::NOT_NOW, "openagents-welcome-later")
                            .id("openagents-welcome-later")
                            .on_click(cx.listener(|this, _, _, cx| this.not_now(cx))),
                    ),
                );
            }
            WelcomeStep::Restore { error } => {
                card = card
                    .child(popover::dialog_title(&theme, copy::RESTORE))
                    .child(muted(copy::RESTORE_BODY.into()));
                if let Some(input) = self.id.words.clone() {
                    card = card.child(
                        div()
                            .mt(px(16.))
                            .font_family(theme.font_mono.clone())
                            .child(popover::dialog_field(input.into_any_element())),
                    );
                }
                if let Some(error) = error {
                    card = card.child(muted(error.into()).text_color(theme.danger));
                }
                card = card.child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            popover::btn_ghost(&theme, copy::BACK, "openagents-restore-back")
                                .id("openagents-restore-back")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.id.welcome = Some(WelcomeStep::Choose { error: None });
                                    this.id.words = None;
                                    this.id._words_events = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            popover::btn_primary(&theme, copy::RESTORE_BUTTON)
                                .id("openagents-restore-submit")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| this.submit_restore(cx))),
                        ),
                );
            }
            WelcomeStep::Working => {
                card = card
                    .child(popover::dialog_title(&theme, copy::WELCOME_TITLE))
                    .child(muted(copy::STARTING.into()));
            }
        }
        card.into_any_element()
    }

    /// One of the welcome's three choices: an icon, a title and one line.
    fn choice(
        &self,
        id: &'static str,
        glyph: &'static str,
        title: &'static str,
        hint: &'static str,
        first: bool,
        theme: &Theme,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .debug_selector(move || id.into())
            .role(gpui::Role::Button)
            .aria_label(title)
            .tab_index(0)
            .w_full()
            .p(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(if first {
                theme.text.opacity(0.35)
            } else {
                theme.border
            })
            .bg(theme.glass_hover().opacity(if first { 1.0 } else { 0.4 }))
            .hover(|row| row.bg(theme.glass_hover()))
            .cursor_pointer()
            .flex()
            .items_start()
            .gap(px(12.))
            .child(
                icon(glyph)
                    .mt(px(1.))
                    .size(px(18.))
                    .text_color(theme.text_muted),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .child(hint),
                    ),
            )
    }

    fn backup_card(&mut self, step: BackupStep, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).for_popup();
        let muted = |text: &'static str| div().mt(px(8.)).text_color(theme.text_muted).child(text);
        let buttons = div().mt(px(16.)).flex().justify_end().gap(px(8.));
        let close = |label: &'static str, id: &'static str| {
            popover::btn_ghost(&theme, label, id)
                .id(id)
                .on_click(cx.listener(|this, _, _, cx| this.close_backup(cx)))
        };
        let mut card = popover::dialog_card(&theme)
            .w(px(420.))
            .debug_selector(|| "openagents-backup".into())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_backup(cx);
                    cx.stop_propagation();
                }
            }))
            .child(popover::dialog_title(&theme, copy::BACK_UP));
        card = match step {
            BackupStep::Confirm => card.child(muted(copy::BACKUP_WARNING)).child(
                buttons
                    .child(close(copy::CANCEL, "openagents-backup-cancel"))
                    .child(
                        popover::btn_primary(&theme, copy::SHOW_WORDS)
                            .id("openagents-backup-show")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.show_words(cx))),
                    ),
            ),
            BackupStep::Words(words) => {
                let mut grid = div()
                    .mt(px(16.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_wrap()
                    .font_family(theme.font_mono.clone());
                for (ix, word) in words.iter().enumerate() {
                    grid = grid.child(
                        div()
                            .w(px(122.))
                            .py(px(4.))
                            .flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .w(px(20.))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!("{}.", ix + 1))),
                            )
                            .child(SharedString::from(word.clone())),
                    );
                }
                card.child(grid).child(
                    buttons
                        .child(close(copy::CLOSE, "openagents-backup-close"))
                        .child(
                            popover::btn_primary(&theme, copy::WROTE_THEM_DOWN)
                                .id("openagents-backup-confirm")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| this.wrote_them_down(cx))),
                        ),
                )
            }
            BackupStep::NoWords => card
                .child(muted(copy::NO_WORDS))
                .child(buttons.child(close(copy::CLOSE, "openagents-backup-close"))),
            BackupStep::Done => card
                .child(muted(copy::BACKUP_DONE))
                .child(buttons.child(close(copy::CLOSE, "openagents-backup-close"))),
        };
        card.into_any_element()
    }

    /// Open a dialog directly, for captures.
    #[cfg(feature = "account-fixture")]
    pub fn fixture_identity(
        &mut self,
        backend: Arc<dyn DesktopIdentity>,
        welcome: Option<WelcomeStep>,
        backup: Option<BackupStep>,
        devices: Option<Vec<LinkedDevice>>,
        cx: &mut Context<Self>,
    ) {
        self.id.backend = Some(backend.clone());
        self.id.view = backend.identity();
        self.id.welcome_offered = true;
        if matches!(welcome, Some(WelcomeStep::Restore { .. })) {
            self.show_restore(cx);
        }
        self.id.welcome = welcome;
        self.id.backup = backup;
        self.id.devices = devices;
        cx.notify();
    }
}

/// An identity that lives in memory, for tests and captures. Its words are
/// the BIP-39 test phrase and its npub is NIP-06's published test vector.
#[cfg(any(test, feature = "account-fixture"))]
pub mod fake {
    use super::*;
    use openagents_account::desktop::identity::Work;
    use std::sync::Mutex;

    pub const NPUB: &str = "npub1sg6plzptd64u62a878hep2kev88swjh3tw00gjsfl8f237lmu63q0uf63m";
    pub const PHONE: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
    pub const WORDS: &str = "leader monkey parrot ring guide accident before fence cannon \
                             height naive bean";

    #[derive(Default)]
    pub struct State {
        pub view: Option<IdentityView>,
        pub restored: Vec<String>,
        pub words_read: usize,
        pub refuse: Option<String>,
    }

    #[derive(Default)]
    pub struct FakeIdentity(pub Mutex<State>);

    impl FakeIdentity {
        /// A computer that has made its words and not backed them up.
        pub fn made() -> Self {
            let fake = Self::default();
            fake.0.lock().unwrap().view = Some(view());
            fake
        }
    }

    fn view() -> IdentityView {
        IdentityView {
            npub: NPUB.into(),
            words_saved: false,
            has_words: true,
        }
    }

    pub fn devices() -> Vec<LinkedDevice> {
        vec![
            LinkedDevice {
                npub: PHONE.into(),
                this_device: false,
            },
            LinkedDevice {
                npub: NPUB.into(),
                this_device: true,
            },
        ]
    }

    impl DesktopIdentity for FakeIdentity {
        fn start(&self) -> Work {
            Box::pin(async {})
        }
        fn renew(&self) -> Work {
            Box::pin(async {})
        }
        fn identity(&self) -> Option<IdentityView> {
            self.0.lock().unwrap().view.clone()
        }
        fn start_new(&self) -> Pending<IdentityView> {
            let mut state = self.0.lock().unwrap();
            let result = match state.refuse.clone() {
                Some(error) => Err(error),
                None => {
                    state.view = Some(view());
                    Ok(view())
                }
            };
            Box::pin(async move { result })
        }
        fn restore(&self, words: String) -> Pending<IdentityView> {
            let mut state = self.0.lock().unwrap();
            state.restored.push(words);
            let result = match state.refuse.clone() {
                Some(error) => Err(error),
                None => {
                    let mut restored = view();
                    restored.words_saved = true;
                    state.view = Some(restored.clone());
                    Ok(restored)
                }
            };
            Box::pin(async move { result })
        }
        fn words(&self) -> Result<Vec<String>, String> {
            self.0.lock().unwrap().words_read += 1;
            Ok(WORDS.split_whitespace().map(str::to_owned).collect())
        }
        fn confirm_saved(&self) -> Result<(), String> {
            if let Some(view) = &mut self.0.lock().unwrap().view {
                view.words_saved = true;
            }
            Ok(())
        }
        fn connect_phone(&self) -> Pending<IdentityView> {
            Box::pin(async { Err(String::new()) })
        }
        fn linked_devices(&self) -> Pending<Vec<LinkedDevice>> {
            Box::pin(async { Ok(devices()) })
        }
    }
}

#[cfg(any(test, feature = "account-fixture"))]
impl AppAccount {
    /// An account row over `backend` with no network, for tests and captures.
    pub fn with_identity(
        controller: Arc<openagents_account::Controller>,
        backend: Arc<dyn DesktopIdentity>,
        data: PathBuf,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut account = Self::new(controller, cx);
        account.attach_identity(data, Some(backend), cx);
        account
    }

    /// A signed-out controller for `data` that never reaches a server.
    pub fn offline_controller(data: &std::path::Path) -> Arc<openagents_account::Controller> {
        use openagents_account::openagents::{Client, Deployment};
        let deployment = Deployment::new("http://127.0.0.1:1/v1", "http://127.0.0.1:1")
            .expect("a local deployment");
        openagents_account::Controller::new(
            Client::new(deployment).expect("a client"),
            Arc::new(openagents_account::desktop::MemoryStore::default()),
            data.join("openagents-account/signed-out"),
        )
    }
}

/// Draw the welcome, the restore field, the identity row and its menu, and
/// Back up your account offscreen into `out` as PNGs (#11997). Nothing is
/// shown on screen.
#[cfg(feature = "account-fixture")]
pub fn capture_identity(
    cx: &mut gpui::HeadlessAppContext,
    data: &std::path::Path,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    struct Host(Entity<AppAccount>);
    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::of(cx).clone();
            let viewport = window.viewport_size();
            let overlays = self.0.update(cx, |account, cx| {
                account.render_identity_overlays(viewport, window, cx)
            });
            div()
                .size_full()
                .relative()
                .bg(theme.bg)
                .text_color(theme.text)
                .font_family(theme.font_sans.clone())
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(260.))
                        .border_r_1()
                        .border_color(theme.border)
                        .flex()
                        .flex_col()
                        .justify_end()
                        .p(px(10.))
                        .child(self.0.clone()),
                )
                .children(overlays)
        }
    }
    std::fs::create_dir_all(out)?;
    cx.update(|cx| {
        gpui_base::init(cx);
        cx.set_global(Theme::default());
        crate::settings::init(crate::settings::UiSettings::default(), data, cx);
        let settings = crate::settings::UiSettings::default();
        let fonts = crate::typography::register_fonts(cx);
        crate::typography::init(
            settings.ui_font_family.clone(),
            settings.ui_font_size,
            settings.terminal_font_family.clone(),
            settings.terminal_font_size,
            settings.code_font_family.clone(),
            settings.code_font_size,
            fonts,
            cx,
        );
        crate::composer::init(cx, settings.composer_send_behavior);
    });
    type Setup = fn(&mut AppAccount, &mut Context<AppAccount>);
    let shots: [(&str, bool, Setup); 7] = [
        ("welcome", false, |a, cx| {
            a.fixture_identity(
                Arc::new(fake::FakeIdentity::default()),
                Some(WelcomeStep::Choose { error: None }),
                None,
                None,
                cx,
            )
        }),
        ("restore", false, |a, cx| {
            a.fixture_identity(
                Arc::new(fake::FakeIdentity::default()),
                Some(WelcomeStep::Restore { error: None }),
                None,
                None,
                cx,
            );
            if let Some(input) = a.id.words.clone() {
                input.update(cx, |input, cx| {
                    input.set_text("leader monkey parrot ring guide accident before fence", cx)
                });
            }
            a.submit_restore(cx);
        }),
        ("set-up-row", false, |a, cx| {
            a.fixture_identity(
                Arc::new(fake::FakeIdentity::default()),
                None,
                None,
                None,
                cx,
            )
        }),
        ("identity-menu", true, |a, cx| {
            a.fixture_identity(
                Arc::new(fake::FakeIdentity::made()),
                None,
                None,
                Some(fake::devices()),
                cx,
            )
        }),
        ("backup-confirm", false, |a, cx| {
            a.fixture_identity(
                Arc::new(fake::FakeIdentity::made()),
                None,
                Some(BackupStep::Confirm),
                None,
                cx,
            )
        }),
        ("backup-words", false, |a, cx| {
            a.fixture_identity(Arc::new(fake::FakeIdentity::made()), None, None, None, cx);
            a.id.backup = Some(BackupStep::Confirm);
            a.show_words(cx);
        }),
        ("identity-row", false, |a, cx| {
            a.fixture_identity(Arc::new(fake::FakeIdentity::made()), None, None, None, cx)
        }),
    ];
    for (name, open_menu, setup) in shots {
        let shot = data.join(name);
        std::fs::create_dir_all(&shot)?;
        let account = cx.update(|cx| {
            let controller = AppAccount::offline_controller(&shot);
            cx.new(|cx| {
                let mut account = AppAccount::new(controller, cx);
                account.id.data = shot.clone();
                setup(&mut account, cx);
                if open_menu {
                    account.menu.open(());
                }
                account
            })
        });
        let window = cx.open_window(gpui::size(px(760.0), px(560.0)), |_, cx| {
            cx.new(|_| Host(account))
        })?;
        for _ in 0..2 {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear())?;
            cx.run_until_parked();
        }
        cx.capture_screenshot(window.into())?
            .save(out.join(format!("{name}.png")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeIdentity, NPUB, WORDS};
    use super::*;
    use gpui::TestAppContext;

    fn account(
        cx: &mut TestAppContext,
        backend: Arc<FakeIdentity>,
        data: &std::path::Path,
    ) -> Entity<AppAccount> {
        cx.update(|cx| {
            gpui_base::init(cx);
            gpui_tokio::init(cx);
            cx.set_global(Theme::default());
            crate::composer::init(cx, Default::default());
        });
        let data = data.to_path_buf();
        cx.new(|cx| {
            AppAccount::with_identity(AppAccount::offline_controller(&data), backend, data, cx)
        })
    }

    /// Let Tokio finish the fake's work and the entity take its result.
    fn settle(cx: &mut TestAppContext, done: impl Fn(&mut TestAppContext) -> bool) {
        cx.executor().allow_parking();
        for _ in 0..200 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the identity work never finished");
    }

    #[gpui::test]
    fn a_fresh_computer_opens_the_welcome_and_start_here_makes_the_account(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeIdentity::default());
        let account = account(cx, backend.clone(), dir.path());
        account.read_with(cx, |a, _| {
            assert!(a.identity_enabled());
            assert_eq!(a.id.welcome, Some(WelcomeStep::Choose { error: None }));
            assert!(a.identity_overlay_open());
            assert_eq!(a.identity_label(), None, "no sign-in, no name yet");
        });
        account.update(cx, |a, cx| a.start_here(cx));
        settle(cx, |cx| account.read_with(cx, |a, _| a.id.view.is_some()));
        account.read_with(cx, |a, _| {
            assert_eq!(a.id.welcome, None);
            assert_eq!(
                a.identity_label().unwrap().to_string(),
                identity::short_npub(NPUB)
            );
            assert!(a.needs_backup());
        });
    }

    #[gpui::test]
    fn a_failed_start_stays_on_the_welcome_with_the_reason(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeIdentity::default());
        backend.0.lock().unwrap().refuse = Some("We couldn't reach OpenAgents. Try again.".into());
        let account = account(cx, backend, dir.path());
        account.update(cx, |a, cx| a.start_here(cx));
        settle(cx, |cx| {
            account.read_with(cx, |a, _| {
                matches!(a.id.welcome, Some(WelcomeStep::Choose { error: Some(_) }))
            })
        });
    }

    #[gpui::test]
    fn restore_checks_the_count_then_hands_over_the_words(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeIdentity::default());
        let account = account(cx, backend.clone(), dir.path());
        account.update(cx, |a, cx| {
            a.show_restore(cx);
            let input = a.id.words.clone().unwrap();
            input.update(cx, |input, cx| input.set_text("leader monkey", cx));
            a.submit_restore(cx);
            assert_eq!(
                a.id.welcome,
                Some(WelcomeStep::Restore {
                    error: Some("That's 2 words. Your backup has 12.".into())
                })
            );
            input.update(cx, |input, cx| input.set_text(WORDS.to_uppercase(), cx));
            a.submit_restore(cx);
            assert_eq!(a.id.welcome, Some(WelcomeStep::Working));
        });
        settle(cx, |cx| {
            account.read_with(cx, |a, _| a.id.welcome.is_none())
        });
        assert_eq!(
            backend.0.lock().unwrap().restored,
            vec![WORDS.split_whitespace().collect::<Vec<_>>().join(" ")]
        );
        account.read_with(cx, |a, _| {
            assert!(a.id.words.is_none(), "the typed words are dropped");
            assert!(
                !a.needs_backup(),
                "restored words were written down already"
            );
        });
    }

    #[gpui::test]
    fn not_now_is_remembered_and_set_up_reopens_it(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let account1 = account(cx, Arc::new(FakeIdentity::default()), dir.path());
        account1.update(cx, |a, cx| a.not_now(cx));
        account1.read_with(cx, |a, _| assert!(!a.identity_overlay_open()));
        let data = dir.path().to_path_buf();
        let account2 = cx.new(|cx| {
            AppAccount::with_identity(
                AppAccount::offline_controller(&data),
                Arc::new(FakeIdentity::default()),
                data.clone(),
                cx,
            )
        });
        account2.read_with(cx, |a, _| assert_eq!(a.id.welcome, None));
        account2.update(cx, |a, cx| a.open_welcome(cx));
        account2.read_with(cx, |a, _| assert!(a.identity_overlay_open()));
    }

    #[gpui::test]
    fn backup_shows_words_only_after_the_warning(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeIdentity::made());
        let account = account(cx, backend.clone(), dir.path());
        account.update(cx, |a, cx| {
            assert_eq!(
                a.id.welcome, None,
                "a computer with an identity skips the welcome"
            );
            a.open_backup(cx);
            assert_eq!(a.id.backup, Some(BackupStep::Confirm));
        });
        assert_eq!(backend.0.lock().unwrap().words_read, 0);
        account.update(cx, |a, cx| {
            a.show_words(cx);
            assert!(matches!(&a.id.backup, Some(BackupStep::Words(words)) if words.len() == 12));
            a.wrote_them_down(cx);
            assert_eq!(a.id.backup, Some(BackupStep::Done));
            assert!(!a.needs_backup());
            a.close_backup(cx);
            assert!(!a.identity_overlay_open());
        });
    }
}
