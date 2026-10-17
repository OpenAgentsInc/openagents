//! Connect your phone (#11998): a sheet that shows a QR code the phone
//! scans to pair with this computer.
//!
//! It follows NIP-HOST, "Showing a connect code": the QR image is drawn here
//! from the code's link, a new code appears every minute, the code it
//! replaced is cancelled a minute later, and closing the sheet, leaving the
//! window, or ten minutes without a click cancels every code it still holds.
//! The engine makes and cancels the codes
//! ([`openagents_rpc::connect_phone`]).

use super::*;
use std::sync::Arc;
use openagents_rpc::connect_phone::{
    CANCEL_CONNECT_CODES, CANCEL_REPLACED_AFTER_SECS, CREATE_CONNECT_CODE, CancelConnectCodes,
    ConnectCode, IDLE_STOP_SECS, ROTATE_EVERY_SECS,
};

pub(crate) const TITLE: &str = "Connect your phone";
pub(crate) const SCAN_HINT: &str =
    "Open the camera or the OpenAgents app on your phone and scan this code.";
pub(crate) const ROTATE_HINT: &str = "The code changes every minute and works once.";
pub(crate) const PAUSED: &str = "The code is hidden so nobody else can use it.";
pub(crate) const SHOW_AGAIN: &str = "Show a new code";
pub(crate) const COPY: &str = "Copy code";
pub(crate) const COPIED: &str = "Copied";
const NOT_READY: &str = "OpenAgents isn't ready yet. Try again in a moment.";
const UNREACHABLE: &str = "We couldn't reach OpenAgents. Try again.";

/// How long a copied code stays on the clipboard.
const CLIPBOARD_SECS: u64 = 60;
/// Size of one QR module on screen.
const MODULE_PX: f32 = 4.0;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The codes a sheet holds and when each is cancelled. Pure, so the timing
/// rules are tested without a window or an engine.
#[derive(Debug, Default)]
pub(crate) struct Rotation {
    /// Invitation IDs, oldest first, with the time a newer code replaced
    /// each one.
    held: Vec<(String, Option<u64>)>,
    /// The last time the person did something in the sheet.
    active_at: u64,
}

impl Rotation {
    pub(crate) fn new(now: u64) -> Self {
        Self {
            held: Vec::new(),
            active_at: now,
        }
    }

    /// A new code is on screen; the one before it starts its last minute.
    pub(crate) fn shown(&mut self, invitation: String, now: u64) {
        for (_, replaced) in &mut self.held {
            replaced.get_or_insert(now);
        }
        self.held.push((invitation, None));
    }

    /// Codes whose last minute has passed: cancel them now.
    pub(crate) fn due(&mut self, now: u64) -> Vec<String> {
        let mut due = Vec::new();
        self.held.retain(|(invitation, replaced)| {
            let expired =
                replaced.is_some_and(|at| now >= at.saturating_add(CANCEL_REPLACED_AFTER_SECS));
            if expired {
                due.push(invitation.clone());
            }
            !expired
        });
        due
    }

    /// Every code still held, for a sheet that closes or pauses.
    pub(crate) fn take_all(&mut self) -> Vec<String> {
        self.held
            .drain(..)
            .map(|(invitation, _)| invitation)
            .collect()
    }

    pub(crate) fn touch(&mut self, now: u64) {
        self.active_at = now;
    }

    /// Ten minutes without a click.
    pub(crate) fn idle(&self, now: u64) -> bool {
        now >= self.active_at.saturating_add(IDLE_STOP_SECS)
    }
}

/// The QR modules of a connect link, with a four-module quiet zone; `true`
/// is dark. Drawn on this computer: the code never goes to a QR service.
pub(crate) fn qr_modules(link: &str) -> Option<Vec<Vec<bool>>> {
    let qr = qrcodegen::QrCode::encode_text(link, qrcodegen::QrCodeEcc::Medium).ok()?;
    let side = qr.size() + 8;
    Some(
        (0..side)
            .map(|y| (0..side).map(|x| qr.get_module(x - 4, y - 4)).collect())
            .collect(),
    )
}

pub(super) struct ConnectPhoneSheet {
    code: Option<ConnectCode>,
    modules: Option<Arc<Vec<Vec<bool>>>>,
    rotation: Rotation,
    /// No code on screen until the person asks for one again.
    paused: bool,
    error: Option<String>,
    copied: bool,
    /// The rotation loop; dropping it stops the loop.
    rotate: Option<Task<()>>,
    copy_task: Option<Task<()>>,
    /// Fixture sheets show a fixed code and never call the engine.
    preview: bool,
}

impl ConnectPhoneSheet {
    fn new() -> Self {
        Self {
            code: None,
            modules: None,
            rotation: Rotation::new(unix_now()),
            paused: false,
            error: None,
            copied: false,
            rotate: None,
            copy_task: None,
            preview: false,
        }
    }

    fn show(&mut self, code: ConnectCode, now: u64) {
        self.rotation.shown(code.invitation.clone(), now);
        self.modules = qr_modules(&code.link).map(Arc::new);
        self.code = Some(code);
        self.error = None;
        self.copied = false;
    }
}

impl Shell {
    pub(crate) fn open_connect_phone(&mut self, cx: &mut Context<Self>) {
        if self.connect_phone.is_some() {
            return;
        }
        self.connect_phone = Some(ConnectPhoneSheet::new());
        self.start_connect_rotation(cx);
        cx.notify();
    }

    /// Show the sheet with a fixed code and no engine, for captures.
    #[cfg(feature = "account-fixture")]
    pub fn preview_connect_phone(&mut self, code: ConnectCode, cx: &mut Context<Self>) {
        let mut sheet = ConnectPhoneSheet::new();
        sheet.preview = true;
        sheet.show(code, unix_now());
        self.connect_phone = Some(sheet);
        cx.notify();
    }

    pub(super) fn connect_phone_open(&self) -> bool {
        self.connect_phone.is_some()
    }

    pub(super) fn close_connect_phone(&mut self, cx: &mut Context<Self>) {
        if let Some(mut sheet) = self.connect_phone.take() {
            let held = sheet.rotation.take_all();
            if !sheet.preview {
                self.cancel_connect_codes(held, cx);
            }
        }
        cx.notify();
    }

    /// Stop showing codes and cancel every one still held: the window lost
    /// focus, or ten minutes passed without a click.
    pub(super) fn pause_connect_phone(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.connect_phone else {
            return;
        };
        if sheet.paused || sheet.preview {
            return;
        }
        sheet.paused = true;
        sheet.rotate = None;
        sheet.code = None;
        sheet.modules = None;
        let held = sheet.rotation.take_all();
        self.cancel_connect_codes(held, cx);
        cx.notify();
    }

    fn resume_connect_phone(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.connect_phone else {
            return;
        };
        sheet.paused = false;
        sheet.error = None;
        sheet.rotation.touch(unix_now());
        self.start_connect_rotation(cx);
        cx.notify();
    }

    fn cancel_connect_codes(&self, invitations: Vec<String>, cx: &mut Context<Self>) {
        if invitations.is_empty() {
            return;
        }
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return;
        };
        let params = serde_json::to_value(CancelConnectCodes { invitations })
            .unwrap_or(serde_json::Value::Null);
        // Detached: the cancel must finish after the sheet is gone.
        Tokio::spawn(cx, async move {
            if let Err(error) = engine.client().call(CANCEL_CONNECT_CODES, params).await {
                tracing::warn!(%error, "connect codes not cancelled");
            }
        })
        .detach();
    }

    /// Ask for a code now and every minute after, cancelling replaced codes
    /// a minute after they leave the screen.
    fn start_connect_rotation(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            if let Some(sheet) = &mut self.connect_phone {
                sheet.error = Some(NOT_READY.into());
            }
            return;
        };
        let task = cx.spawn(async move |this, cx| {
            loop {
                let call = {
                    let engine = engine.clone();
                    Tokio::spawn(cx, async move {
                        engine
                            .client()
                            .call_as::<ConnectCode>(CREATE_CONNECT_CODE, serde_json::json!({}))
                            .await
                    })
                };
                let result = call.await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        let Some(sheet) = &mut this.connect_phone else {
                            return false;
                        };
                        let now = unix_now();
                        match result {
                            Ok(Ok(code)) => sheet.show(code, now),
                            Ok(Err(error)) => sheet.error = Some(error.to_string()),
                            Err(_) => sheet.error = Some(UNREACHABLE.into()),
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(ROTATE_EVERY_SECS))
                    .await;
                let go_on = this
                    .update(cx, |this, cx| {
                        let now = unix_now();
                        let Some(sheet) = &mut this.connect_phone else {
                            return false;
                        };
                        if sheet.rotation.idle(now) {
                            this.pause_connect_phone(cx);
                            return false;
                        }
                        let due = sheet.rotation.due(now);
                        this.cancel_connect_codes(due, cx);
                        true
                    })
                    .unwrap_or(false);
                if !go_on {
                    return;
                }
            }
        });
        if let Some(sheet) = &mut self.connect_phone {
            sheet.rotate = Some(task);
        }
    }

    /// Copy the text form on an explicit click, and clear it from the
    /// clipboard a minute later if it is still there.
    fn copy_connect_code(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &mut self.connect_phone else {
            return;
        };
        let Some(text) = sheet.code.as_ref().map(|code| code.text.clone()) else {
            return;
        };
        sheet.rotation.touch(unix_now());
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        sheet.copied = true;
        sheet.copy_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(CLIPBOARD_SECS))
                .await;
            this.update(cx, |this, cx| {
                let still = cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .is_some_and(|held| held == text);
                if still {
                    cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
                }
                if let Some(sheet) = &mut this.connect_phone {
                    sheet.copied = false;
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub(super) fn render_connect_phone_overlays(
        &mut self,
        viewport: gpui::Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(sheet) = &self.connect_phone else {
            return Vec::new();
        };
        let theme = Theme::of(cx).for_popup();
        let modules = sheet.modules.clone();
        let label = sheet.code.as_ref().map(|code| code.label.clone());
        let paused = sheet.paused;
        let copied = sheet.copied;
        let error = sheet.error.clone();
        let has_code = sheet.code.is_some();

        let mut card = popover::dialog_card(&theme)
            .w(px(400.0))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_connect_phone(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(popover::dialog_title(&theme, TITLE))
                    .child(
                        div()
                            .id("connect-phone-close")
                            .size(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .role(gpui::Role::Button)
                            .aria_label("Close")
                            .on_click(cx.listener(|this, _, _, cx| this.close_connect_phone(cx)))
                            .child(
                                icon(icons::CLOSE)
                                    .size(px(16.0))
                                    .text_color(theme.text_muted),
                            ),
                    ),
            );
        let muted = |text: &str| {
            div()
                .mt(px(8.0))
                .text_color(theme.text_muted)
                .child(SharedString::from(text.to_string()))
        };
        card = card.child(muted(SCAN_HINT));

        // The QR code sits on white with its quiet zone, as phone cameras
        // expect, whatever the app's theme.
        let side = modules.as_ref().map_or(41, |rows| rows.len()) as f32 * MODULE_PX;
        let mut frame = div()
            .mt(px(16.0))
            .mx_auto()
            .size(px(side))
            .rounded(px(12.0))
            .overflow_hidden()
            .bg(gpui::white())
            .flex()
            .items_center()
            .justify_center();
        frame = match (&modules, paused) {
            (Some(modules), false) => {
                let modules = modules.clone();
                frame.child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |bounds: gpui::Bounds<Pixels>, _, window, _| {
                            let unit = px(MODULE_PX);
                            for (y, row) in modules.iter().enumerate() {
                                for (x, dark) in row.iter().enumerate() {
                                    if *dark {
                                        let origin = gpui::point(
                                            bounds.origin.x + unit * x as f32,
                                            bounds.origin.y + unit * y as f32,
                                        );
                                        window.paint_quad(gpui::fill(
                                            gpui::Bounds::new(origin, gpui::size(unit, unit)),
                                            gpui::black(),
                                        ));
                                    }
                                }
                            }
                        },
                    )
                    .size_full(),
                )
            }
            _ => frame.bg(theme.glass_hover()).child(
                div()
                    .px(px(24.0))
                    .text_center()
                    .text_color(theme.text_muted)
                    .child(SharedString::from(if paused {
                        PAUSED
                    } else if error.is_some() {
                        ""
                    } else {
                        "Getting a code…"
                    })),
            ),
        };
        card = card.child(frame);
        if let Some(label) = label.filter(|label| !label.is_empty() && !paused) {
            card = card.child(
                div()
                    .mt(px(12.0))
                    .text_center()
                    .text_color(theme.text)
                    .child(SharedString::from(label)),
            );
        }
        if !paused {
            card = card.child(muted(ROTATE_HINT).text_center());
        }
        if let Some(error) = error {
            card = card.child(div().mt(px(12.0)).text_color(theme.danger).child(error));
        }
        let mut buttons = div().mt(px(20.0)).flex().justify_end().gap(px(8.0));
        if paused {
            buttons = buttons.child(
                popover::btn_primary(&theme, SHOW_AGAIN)
                    .id("connect-phone-again")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.resume_connect_phone(cx))),
            );
        } else {
            if has_code {
                buttons = buttons.child(
                    popover::btn_ghost(&theme, if copied { COPIED } else { COPY }, "connect-copy")
                        .id("connect-phone-copy")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.copy_connect_code(cx))),
                );
            }
            buttons = buttons.child(
                popover::btn_primary(&theme, "Done")
                    .id("connect-phone-done")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.close_connect_phone(cx))),
            );
        }
        card = card.child(buttons);
        vec![popover::modal(
            "connect-phone-sheet",
            viewport,
            card.into_any_element(),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_code_is_cancelled_a_minute_after_the_next_appears() {
        let mut rotation = Rotation::new(0);
        rotation.shown("a".into(), 0);
        assert!(rotation.due(59).is_empty());
        rotation.shown("b".into(), 60);
        // "a" left the screen at 60 and stays redeemable until 120.
        assert!(rotation.due(119).is_empty());
        assert_eq!(rotation.due(120), vec!["a".to_string()]);
        rotation.shown("c".into(), 120);
        assert_eq!(rotation.due(180), vec!["b".to_string()]);
        // Closing cancels whatever is left, the code on screen included.
        assert_eq!(rotation.take_all(), vec!["c".to_string()]);
        assert!(rotation.take_all().is_empty());
    }

    #[test]
    fn ten_minutes_without_a_click_stops_the_codes() {
        let mut rotation = Rotation::new(100);
        assert!(!rotation.idle(100 + IDLE_STOP_SECS - 1));
        assert!(rotation.idle(100 + IDLE_STOP_SECS));
        rotation.touch(500);
        assert!(!rotation.idle(100 + IDLE_STOP_SECS));
    }

    #[test]
    fn the_longest_link_fits_in_a_qr_code() {
        // NIP-HOST bounds a connect link at 666 bytes.
        let link = format!("https://openagents.com/connect#{}", "A".repeat(635));
        assert_eq!(link.len(), 666);
        let rows = qr_modules(&link).unwrap();
        assert!(rows.iter().all(|row| row.len() == rows.len()));
        assert!(rows.iter().flatten().any(|dark| *dark));
        // The quiet zone is light.
        assert!(rows[0].iter().all(|dark| !dark));
    }

    #[test]
    fn the_sheet_speaks_plain_words() {
        for text in [
            TITLE,
            SCAN_HINT,
            ROTATE_HINT,
            PAUSED,
            SHOW_AGAIN,
            COPY,
            COPIED,
            NOT_READY,
            UNREACHABLE,
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
