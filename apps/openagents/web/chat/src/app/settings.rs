//! Settings in the GPUI web chat (#12288, #12289, #12290), the chat's
//! half: the choices that act on the chat (the theme, the conversation
//! width, motion, the send key, sounds and banners, the shortcuts, and the
//! new-thread background), read from this browser's storage at start and
//! again whenever Settings saves (`oa-settings-changed`), and the
//! `?settings=<page>` address ([`route::settings_path`]).
//!
//! The pages themselves are their own module (`apps/openagents/web/settings`),
//! which the start script loads over the chat the first time Settings
//! opens (`oa-settings-open`), so this module stays under its budget.

use std::sync::Arc;

use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Image, ImageFormat, IntoElement, KeyBinding, ParentElement,
    Styled, Task, Window, actions, div, prelude::*, px, relative,
};
use openagents_chat_view::settings_forms::{Page, Prefs, Shortcut, shortcuts};
use openagents_web_chat_view::labels::ago;
use openagents_web_chat_view::svg;
use openagents_web_chat_view::{composer, now_ms, selection, theme};
use openagents_web_client::{Change, store};
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, closure::Closure};

use super::ChatApp;
use crate::notifications::browser as alerts;
use crate::route;

actions!(
    openagents_settings,
    [
        CommandPalette,
        ToggleSidebar,
        ToggleFiles,
        NewSession,
        NextSession,
        PrevSession,
        ArchiveSession
    ]
);

/// The composer's own send-key switch keeps its choice here too.
const SEND_KEY: &str = "openagents.composer.send-with-modifier";

/// Settings' state in the chat window.
pub(super) struct Settings {
    /// The open page; `None` is the chat.
    pub page: Option<Page>,
    pub prefs: Prefs,
    /// The new-thread background image, read from IndexedDB.
    pub background: Option<Arc<Image>>,
    system_dark: bool,
    pub(super) mac: bool,
    /// The wide window's sidebar is hidden (Toggle left sidebar).
    pub sidebar_hidden: bool,
    _tasks: Vec<Task<()>>,
}

impl Settings {
    pub fn load() -> Self {
        Self {
            page: None,
            prefs: Prefs::from_json(store::get(Prefs::STORAGE_KEY).as_deref()),
            background: None,
            system_dark: prefers_dark(),
            mac: is_mac(),
            sidebar_hidden: false,
            _tasks: Vec::new(),
        }
    }
}

impl ChatApp {
    /// Put the stored choices into effect before the first frame, so the
    /// page opens in the person's theme, and follow Settings' changes.
    pub(super) fn start_settings(&mut self, cx: &mut Context<Self>) {
        let look = &self.settings.prefs.appearance;
        openagents_chat_view::motion::init(look.reduce_motion, look.pause_in_background, cx);
        openagents_chat_view::motion::set_system(prefers_reduced_motion(), cx);
        // The composer's own switch keeps the send key too, and has the
        // last word.
        if let Some(on) = store::get(SEND_KEY) {
            self.settings.prefs.send_with_modifier = on == "1";
        }
        self.apply_prefs(cx);
        // The system's light or dark, followed while the page is open.
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<bool>();
        if let Some(query) = web_sys::window()
            .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
        {
            let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                let _ = tx.unbounded_send(prefers_dark());
            });
            let _ =
                query.add_event_listener_with_callback("change", listener.as_ref().unchecked_ref());
            listener.forget();
        }
        let scheme = cx.spawn(async move |this, cx| {
            while let Some(dark) = rx.next().await {
                if this
                    .update(cx, |this, cx| {
                        this.settings.system_dark = dark;
                        this.apply_appearance(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        // What the Settings module tells the page.
        let mut changed = store::listen("oa-settings-changed");
        let mut closed = store::listen("oa-settings-close");
        let mut paged = store::listen("oa-settings-page");
        let mut chats = store::listen("oa-settings-chat");
        let events = cx.spawn(async move |this, cx| {
            loop {
                let next = futures::select! {
                    _ = changed.next() => this.update(cx, |this, cx| this.settings_changed(cx)),
                    _ = closed.next() => this.update(cx, |this, cx| this.close_settings(cx)),
                    page = paged.next() => this.update(cx, |this, cx| {
                        if let Some(page) = page.as_deref().and_then(Page::from_slug) {
                            this.settings.page = Some(page);
                            route::push_settings(page);
                            route::set_title(page.label());
                        }
                        cx.notify();
                    }),
                    chat = chats.next() => this.update(cx, |this, cx| {
                        this.settings_chat(chat.as_deref().unwrap_or_default(), cx)
                    }),
                };
                if next.is_err() {
                    break;
                }
            }
        });
        self.settings._tasks = vec![scheme, events];
        self.load_background(cx);
    }

    /// Settings saved a choice: read them all again and apply them.
    fn settings_changed(&mut self, cx: &mut Context<Self>) {
        let before = self.settings.prefs.clone();
        self.settings.prefs = Prefs::from_json(store::get(Prefs::STORAGE_KEY).as_deref());
        self.apply_prefs(cx);
        if self.settings.prefs.appearance.background != before.appearance.background {
            self.load_background(cx);
        }
        cx.notify();
    }

    fn load_background(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self
            .settings
            .prefs
            .appearance
            .background
            .as_ref()
            .map(|b| b.name.clone())
        else {
            self.settings.background = None;
            return;
        };
        cx.spawn(async move |this, cx| {
            let found = store::read(store::BACKGROUND).await.ok().flatten();
            this.update(cx, |this, cx| {
                this.settings.background = found.map(|bytes| image(&name, bytes));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Settings' Archived sessions page unarchived or deleted a chat.
    fn settings_chat(&mut self, detail: &str, cx: &mut Context<Self>) {
        let asked: Value = serde_json::from_str(detail).unwrap_or_default();
        if let Some(id) = asked["unarchive"].as_str() {
            self.change(
                id.to_owned(),
                Change {
                    archived: Some(false),
                    ..Default::default()
                },
                cx,
            );
        } else if let Some(id) = asked["delete"].as_str() {
            self.delete(id.to_owned(), cx);
        }
    }

    /// The choices that act on the chat.
    fn apply_prefs(&mut self, cx: &mut Context<Self>) {
        alerts::set_prefs(&self.settings.prefs);
        let on = self.settings.prefs.send_with_modifier;
        if self.compose.read(cx).send_with_modifier() != on {
            self.compose
                .update(cx, |compose, cx| compose.set_send_with_modifier(on, cx));
        }
        self.apply_appearance(cx);
        self.bind_shortcuts(cx);
    }

    fn apply_appearance(&mut self, cx: &mut Context<Self>) {
        let look = &self.settings.prefs.appearance;
        // Settings keeps the imported themes compiled, so the chat doesn't
        // link the importer.
        let families = if look.library.is_empty() {
            Vec::new()
        } else {
            use base64::Engine as _;
            store::get(openagents_chat_view::settings_forms::Appearance::COMPILED_KEY)
                .and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok())
                .and_then(|bytes| {
                    postcard::from_bytes::<Vec<openagents_theme::ThemeFamily>>(&bytes).ok()
                })
                .unwrap_or_default()
        };
        theme::set(look.theme_with(families, self.settings.system_dark), cx);
        openagents_web_chat_view::labels::set_content_width(f32::from(look.width()));
        openagents_chat_view::motion::set_preference(look.reduce_motion, cx);
        openagents_chat_view::motion::set_pause_in_background(look.pause_in_background, cx);
        self.transcript
            .update(cx, |transcript, _| transcript.list.remeasure());
        cx.refresh_windows();
    }

    /// Bind the shortcuts, with the composer's and the transcript's keys.
    fn bind_shortcuts(&self, cx: &mut Context<Self>) {
        let mac = self.settings.mac;
        let keymap = &self.settings.prefs.keymap;
        let bind = |shortcut: Shortcut| shortcuts::platform_combo_on(mac, keymap.get(shortcut));
        cx.clear_key_bindings();
        composer::bind_keys(cx);
        selection::bind_keys(cx);
        super::palette::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new(&bind(Shortcut::CommandPalette), CommandPalette, None),
            KeyBinding::new(&bind(Shortcut::ToggleSidebar), ToggleSidebar, None),
            KeyBinding::new(&bind(Shortcut::ToggleFiles), ToggleFiles, None),
            KeyBinding::new(&bind(Shortcut::NewSession), NewSession, None),
            KeyBinding::new(&bind(Shortcut::NextSession), NextSession, None),
            KeyBinding::new(&bind(Shortcut::PrevSession), PrevSession, None),
            KeyBinding::new(&bind(Shortcut::ArchiveSession), ArchiveSession, None),
        ]);
    }

    /// The shortcuts' handlers on the window's root.
    pub(super) fn shortcut_actions(
        &self,
        root: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        root.on_action(cx.listener(|this, _: &CommandPalette, window, cx| {
            this.toggle_palette(window, cx);
        }))
        .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
            if window.viewport_size().width > px(820.) {
                this.settings.sidebar_hidden = !this.settings.sidebar_hidden;
            } else {
                this.sidebar_open = !this.sidebar_open;
            }
            cx.notify();
        }))
        .on_action(|_: &ToggleFiles, _, _| crate::compose::page_event("oa-files-toggle", ""))
        .on_action(cx.listener(|this, _: &NewSession, window, cx| {
            this.settings.page = None;
            this.open(None, window, cx);
        }))
        .on_action(cx.listener(|this, _: &NextSession, window, cx| this.step_chat(1, window, cx)))
        .on_action(cx.listener(|this, _: &PrevSession, window, cx| this.step_chat(-1, window, cx)))
        .on_action(cx.listener(|this, _: &ArchiveSession, _, cx| {
            if let Some(id) = this.current.clone() {
                this.change(
                    id,
                    Change {
                        archived: Some(true),
                        ..Default::default()
                    },
                    cx,
                );
            }
        }))
    }

    /// Open the chat `by` rows below the open one in the sidebar's order.
    fn step_chat(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order: Vec<String> =
            super::model::groups(&self.threads, &self.projects, "", &self.meta, self.view)
                .into_iter()
                .flat_map(|(_, members)| members)
                .filter_map(|ix| self.threads.get(ix).map(|t| t.id.clone()))
                .collect();
        if order.is_empty() {
            return;
        }
        let at = self
            .current
            .as_deref()
            .and_then(|id| order.iter().position(|o| o == id));
        let next = match at {
            Some(at) => (at as isize + by).rem_euclid(order.len() as isize) as usize,
            None if by > 0 => 0,
            None => order.len() - 1,
        };
        self.settings.page = None;
        self.open(Some(order[next].clone()), window, cx);
    }

    /// Escape in the composer stops a running answer when Settings says so.
    pub(super) fn escape_pressed(&mut self, cx: &mut Context<Self>) {
        if self.settings.prefs.escape_stops_agent && self.menu.is_none() {
            let computer_working = self
                .current
                .as_deref()
                .and_then(|id| self.thread(id))
                .is_some_and(|t| t.working);
            if self.live.is_some() || computer_working {
                self.stop(cx);
            }
        }
    }

    /// The address changed: a Settings page, or a chat.
    pub(super) fn arrive(&mut self, chat: Option<String>, cx: &mut Context<Self>) {
        match route::settings() {
            Some(page) => self.show_settings(page, false, cx),
            None => {
                if self.settings.page.take().is_some() {
                    store::signal("oa-settings-hide", "");
                }
                self.load(chat, cx);
            }
        }
    }

    /// Open Settings on `page`: the start script loads the pages' module
    /// the first time and shows it over the chat.
    pub(super) fn show_settings(&mut self, page: Page, push: bool, cx: &mut Context<Self>) {
        if push {
            route::push_settings(page);
        }
        route::set_title(page.label());
        self.settings.page = Some(page);
        self.sidebar_open = false;
        let detail = json!({ "page": page.slug(), "shared": self.settings_shared() });
        store::signal("oa-settings-open", &detail.to_string());
        cx.notify();
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings.page = None;
        store::signal("oa-settings-hide", "");
        route::push(self.current.as_deref());
        if let Some(title) = self
            .current
            .as_deref()
            .and_then(|id| self.thread(id))
            .map(|t| t.title.clone())
            .filter(|t| !t.is_empty())
        {
            route::set_title(&title);
        }
        cx.notify();
    }

    /// What the pages show from the chat: its computers and archived chats.
    fn settings_shared(&self) -> Value {
        let computers: Vec<Value> = self
            .registry
            .as_ref()
            .map(|r| r.read(|r| r.devices()))
            .unwrap_or_default()
            .into_iter()
            .map(|d| json!({ "id": d.id, "name": d.name }))
            .collect();
        let archived: Vec<Value> = self
            .archived()
            .into_iter()
            .map(|(id, title, detail)| json!({ "id": id, "title": title, "detail": detail }))
            .collect();
        json!({ "computers": computers, "archived": archived, "signedIn": self.signed_in })
    }

    /// The archived chats on the account's computers, newest first.
    fn archived(&self) -> Vec<(String, String, String)> {
        let now = now_ms();
        let mut rows: Vec<(u64, (String, String, String))> = self
            .threads
            .iter()
            .filter_map(|t| {
                let meta = self.meta.get(&t.id).filter(|m| m.archived)?;
                Some((
                    t.updated_unix,
                    (
                        t.id.clone(),
                        if t.title.is_empty() {
                            "Untitled session".into()
                        } else {
                            t.title.clone()
                        },
                        format!(
                            "{} · {} ago",
                            meta.computer.1,
                            ago(now - (t.updated_unix as i64) * 1000)
                        ),
                    ),
                ))
            })
            .collect();
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        rows.into_iter().map(|(_, a)| a).collect()
    }

    /// The sidebar's Settings row.
    pub(super) fn render_settings_entry(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("open-settings")
            .px(px(18.))
            .py(px(12.))
            .border_t_1()
            .border_color(theme::border())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .text_size(px(13.5))
            .text_color(theme::text_muted())
            .cursor_pointer()
            .when(self.settings.page.is_some(), |row| {
                row.text_color(theme::text())
            })
            .hover(|s| s.text_color(theme::text()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.show_settings(Page::General, true, cx)
            }))
            .child(
                svg()
                    .path("icons/settings.svg")
                    .size(px(15.))
                    .text_color(theme::text_muted()),
            )
            .child("Settings")
            .into_any_element()
    }

    /// The new chat's background image behind `content`, framed as chosen.
    pub(super) fn with_background(&self, content: AnyElement) -> AnyElement {
        let (Some(picture), Some(chosen)) = (
            self.settings.background.clone(),
            self.settings.prefs.appearance.background.as_ref(),
        ) else {
            return content;
        };
        let framing = chosen.framing.normalized();
        let extra = framing.zoom - 1.;
        div()
            .relative()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .w(relative(framing.zoom))
                    .h(relative(framing.zoom))
                    .left(relative(-extra * framing.focal_x))
                    .top(relative(-extra * framing.focal_y))
                    .child(
                        openagents_gpui_web::img(picture)
                            .size_full()
                            .object_fit(gpui::ObjectFit::Cover),
                    ),
            )
            .child(div().relative().flex_1().flex().flex_col().child(content))
            .into_any_element()
    }
}

fn image(name: &str, bytes: Vec<u8>) -> Arc<Image> {
    let lower = name.to_lowercase();
    let format = if lower.ends_with(".png") {
        ImageFormat::Png
    } else if lower.ends_with(".webp") {
        ImageFormat::Webp
    } else if lower.ends_with(".gif") {
        ImageFormat::Gif
    } else {
        ImageFormat::Jpeg
    };
    Arc::new(Image::from_bytes(format, bytes))
}

fn is_mac() -> bool {
    web_sys::window()
        .and_then(|w| w.navigator().platform().ok())
        .is_some_and(|p| p.starts_with("Mac") || p.starts_with("iP"))
}

fn media(query: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.match_media(query).ok().flatten())
        .is_some_and(|m| m.matches())
}

fn prefers_dark() -> bool {
    media("(prefers-color-scheme: dark)")
}

fn prefers_reduced_motion() -> bool {
    media("(prefers-reduced-motion: reduce)")
}
