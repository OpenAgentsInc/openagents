//! The mod-K palette: the registry's chats and the selected computer's files.
use super::*;
use crate::palette::{self, Tab};
use gpui::{KeyBinding, MouseButton, ScrollHandle, actions};
use openagents_chat_view::settings_forms::{Page, Shortcut, shortcuts};
use openagents_proto::{
    WorkspaceContentSearchResult, WorkspaceEntryKind, WorkspaceFileSearchMatch,
};
use openagents_web_chat_view::composer::{CompletionKey, Edited};
use openagents_web_client::computer;
use serde_json::json;

actions!(command_palette, [NextTab, PreviousTab]);

#[derive(Clone)]
enum Pick {
    Action(Shortcut),
    Settings,
    Chat(String),
    File { path: String, line: Option<u64> },
}

#[derive(Clone)]
struct Row {
    title: String,
    detail: String,
    pick: Pick,
}

pub(super) struct Palette {
    search: Entity<TextInput>,
    previous_focus: Option<FocusHandle>,
    host: Option<HostChat>,
    query: String,
    tab: Tab,
    active: usize,
    scroll: ScrollHandle,
    files: Vec<WorkspaceFileSearchMatch>,
    content: Vec<openagents_proto::WorkspaceContentMatch>,
    status: Option<String>,
    // Replacing this task prevents an old query from updating the palette.
    request: Option<Task<()>>,
}

impl ChatApp {
    pub(crate) fn palette_key(
        &mut self,
        ctrl: bool,
        alt: bool,
        shift: bool,
        cmd: bool,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let combo =
            shortcuts::combo_from_keystroke_on(self.settings.mac, ctrl, alt, shift, cmd, key);
        if combo.as_deref() != Some(self.settings.prefs.keymap.get(Shortcut::CommandPalette)) {
            return false;
        }
        self.toggle_palette(window, cx);
        true
    }

    pub(super) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
            return;
        }
        if self.settings.page.is_some() {
            self.close_settings(cx);
        }
        crate::compose::page_event("oa-palette-open", "");
        let search = cx.new(|cx| {
            let mut input = TextInput::new("Search actions, chats, files and content", cx);
            input.set_completing(true);
            input
        });
        cx.subscribe(&search, |this, _, _: &Edited, cx| this.search_palette(cx))
            .detach();
        cx.subscribe_in(
            &search,
            window,
            |this, _, key: &CompletionKey, window, cx| match key {
                CompletionKey::Dismiss => this.close_palette(window, cx),
                CompletionKey::Accept => {
                    let row = this
                        .palette
                        .as_ref()
                        .and_then(|p| this.palette_rows().get(p.active).cloned());
                    if let Some(row) = row {
                        this.pick_palette(row.pick, window, cx);
                    }
                }
                CompletionKey::Up | CompletionKey::Down => {
                    let count = this.palette_rows().len();
                    if let Some(p) = &mut this.palette {
                        p.active = palette::step(
                            p.active,
                            if *key == CompletionKey::Up { -1 } else { 1 },
                            count,
                        );
                        p.scroll.scroll_to_item(p.active);
                    }
                    cx.notify();
                }
            },
        )
        .detach();
        let host = self.compose.read(cx).host().cloned();
        self.palette = Some(Palette {
            search: search.clone(),
            previous_focus: window.focused(cx),
            host,
            query: String::new(),
            tab: Tab::All,
            active: 0,
            scroll: ScrollHandle::new(),
            files: Vec::new(),
            content: Vec::new(),
            status: None,
            request: None,
        });
        self.menu = None;
        window.focus(&search.focus_handle(cx), cx);
        self.search_palette(cx);
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.palette.take() {
            crate::compose::page_event("oa-palette-close", "");
            if let Some(focus) = p.previous_focus {
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }

    fn palette_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.palette {
            p.tab = tab;
            p.active = 0;
            p.scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        cx.notify();
    }

    fn cycle_palette_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        if let Some(p) = &self.palette {
            let at = Tab::ALL.iter().position(|tab| *tab == p.tab).unwrap_or(0);
            self.palette_tab(Tab::ALL[palette::step(at, by, Tab::ALL.len())], cx);
        }
    }

    fn search_palette(&mut self, cx: &mut Context<Self>) {
        let Some(p) = &mut self.palette else {
            return;
        };
        let query = p.search.read(cx).text().trim().to_owned();
        if p.query == query && (p.request.is_some() || p.status.is_some()) {
            return;
        }
        p.query = query.clone();
        p.active = 0;
        p.files.clear();
        p.content.clear();
        p.scroll.set_offset(gpui::point(px(0.), px(0.)));
        p.request = None;
        let Some(host) = p.host.clone() else {
            p.status = Some("Open a chat on a computer to search its files.".into());
            cx.notify();
            return;
        };
        p.status = Some("Searching…".into());
        let client = self.client.clone();
        p.request = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(180))
                .await;
            let result =
                async {
                    let computer = computer::connect(&client, &host.device, || {}).await?;
                    // The gate checks this browser's grant on each request, including warmup.
                    if query.is_empty() {
                        computer
                            .call("WarmWorkspaceSearch", json!({"chatId": host.chat}))
                            .await?;
                    }
                    let files = computer.call("SearchWorkspaceFiles", json!({
                    "chatId": host.chat, "query": query, "includeIgnored": false, "limit": 40,
                }));
                    let content = async {
                        if query.chars().count() < 2 {
                            return Ok(None);
                        }
                        computer.call("SearchWorkspaceContent", json!({
                        "chatId": host.chat, "query": query, "limit": 40, "perFileLimit": 3,
                    })).await.map(Some)
                    };
                    let (files, content) = futures::join!(files, content);
                    let files = serde_json::from_value::<Vec<WorkspaceFileSearchMatch>>(files?)?;
                    let content = content?
                        .map(serde_json::from_value::<WorkspaceContentSearchResult>)
                        .transpose()?;
                    anyhow::Ok((files, content))
                }
                .await;
            this.update(cx, |this, cx| {
                let Some(p) = &mut this.palette else {
                    return;
                };
                if p.query != query {
                    return;
                }
                match result {
                    Ok((files, content)) => {
                        p.files = files;
                        p.status = content.as_ref().and_then(|r| {
                            if r.indexing {
                                Some("Files are still being indexed. Try again in a moment.".into())
                            } else if r.truncated {
                                Some("More matches exist. Refine your search.".into())
                            } else {
                                None
                            }
                        });
                        p.content = content.map(|r| r.matches).unwrap_or_default();
                    }
                    Err(error) => p.status = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn palette_rows(&self) -> Vec<Row> {
        let Some(p) = &self.palette else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        if p.tab.includes(Tab::Actions) {
            for shortcut in Shortcut::ALL
                .into_iter()
                .filter(|s| *s != Shortcut::CommandPalette)
            {
                let available = match shortcut {
                    Shortcut::ToggleFiles => p.host.is_some(),
                    Shortcut::ArchiveSession => self.current.is_some(),
                    Shortcut::NextSession | Shortcut::PrevSession => !self.threads.is_empty(),
                    _ => true,
                };
                if available && palette::score(&p.query, shortcut.label(), "", false).is_some() {
                    rows.push(Row {
                        title: shortcut.label().into(),
                        detail: shortcuts::display_combo_on(
                            self.settings.mac,
                            self.settings.prefs.keymap.get(shortcut),
                        ),
                        pick: Pick::Action(shortcut),
                    });
                }
            }
            if palette::score(&p.query, "Settings", "Preferences shortcuts", false).is_some() {
                rows.push(Row {
                    title: "Settings".into(),
                    detail: String::new(),
                    pick: Pick::Settings,
                });
            }
        }
        if p.tab.includes(Tab::Chats) {
            let mut chats = self
                .threads
                .iter()
                .filter_map(|t| {
                    let meta = self.meta.get(&t.id);
                    let archived = meta.is_some_and(|m| m.archived);
                    if p.query.is_empty() && archived {
                        return None;
                    }
                    let detail = meta
                        .map(|m| {
                            format!("{} {}", m.space.as_ref().map_or("", |s| &s.1), m.computer.1)
                        })
                        .unwrap_or_else(|| {
                            let project = t
                                .project
                                .as_ref()
                                .and_then(|id| self.projects.iter().find(|p| &p.id == id));
                            format!("{} {}", project.map_or("", |p| p.name.as_str()), t.line)
                        });
                    let detail = if archived {
                        format!("{detail} · Archived")
                    } else {
                        detail
                    };
                    let title = if t.title.is_empty() {
                        "Untitled chat".into()
                    } else {
                        t.title.clone()
                    };
                    palette::score(&p.query, &title, &detail, archived).map(|score| {
                        (
                            score,
                            t.updated_unix,
                            Row {
                                title,
                                detail,
                                pick: Pick::Chat(t.id.clone()),
                            },
                        )
                    })
                })
                .collect::<Vec<_>>();
            chats.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
            rows.extend(chats.into_iter().take(40).map(|(_, _, row)| row));
        }
        if p.tab.includes(Tab::Files) {
            rows.extend(
                p.files
                    .iter()
                    .filter(|f| f.kind != WorkspaceEntryKind::Directory)
                    .map(|f| Row {
                        title: f.name.clone(),
                        detail: f.path.clone(),
                        pick: Pick::File {
                            path: f.path.clone(),
                            line: None,
                        },
                    }),
            );
        }
        if p.tab.includes(Tab::Content) {
            rows.extend(p.content.iter().map(|m| Row {
                title: m.preview.clone(),
                detail: format!("{}:{}", m.path, m.line),
                pick: Pick::File {
                    path: m.path.clone(),
                    line: Some(m.line),
                },
            }));
        }
        rows
    }

    fn pick_palette(&mut self, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.palette.as_ref().and_then(|p| p.host.clone());
        self.close_palette(window, cx);
        match pick {
            Pick::Chat(id) => {
                self.show_computers = false;
                self.open(Some(id), window, cx);
            }
            Pick::Settings => self.show_settings(Page::General, true, cx),
            Pick::File { path, line } => {
                if let Some(host) = host {
                    crate::compose::page_event(
                        "oa-files-open",
                        &json!({
                            "device": host.device, "chat": host.chat, "path": path, "line": line,
                        })
                        .to_string(),
                    );
                }
            }
            Pick::Action(shortcut) => match shortcut {
                Shortcut::ToggleSidebar => {
                    window.dispatch_action(Box::new(super::settings::ToggleSidebar), cx)
                }
                Shortcut::ToggleFiles => {
                    window.dispatch_action(Box::new(super::settings::ToggleFiles), cx)
                }
                Shortcut::NewSession => {
                    window.dispatch_action(Box::new(super::settings::NewSession), cx)
                }
                Shortcut::NextSession => {
                    window.dispatch_action(Box::new(super::settings::NextSession), cx)
                }
                Shortcut::PrevSession => {
                    window.dispatch_action(Box::new(super::settings::PrevSession), cx)
                }
                Shortcut::ArchiveSession => {
                    window.dispatch_action(Box::new(super::settings::ArchiveSession), cx)
                }
                Shortcut::CommandPalette => {}
            },
        }
    }

    pub(super) fn render_palette(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let p = self.palette.as_ref()?;
        let rows = self.palette_rows();
        let active = p.active;
        let tab = p.tab;
        let theme = theme::desktop();
        Some(
            div()
                .id("palette-backdrop")
                .absolute()
                .inset_0()
                .size_full()
                .bg(gpui::black().opacity(0.45))
                .flex()
                .justify_center()
                .pt(px(64.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
                )
                .child(
                    openagents_chat_view::popover::palette_card(&theme, px(640.), 14.)
                        .id("command-palette")
                        .max_w_full()
                        .h(gpui::relative(0.8))
                        .flex()
                        .flex_col()
                        .key_context("CommandPalette")
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_action(
                            cx.listener(|this, _: &NextTab, _, cx| this.cycle_palette_tab(1, cx)),
                        )
                        .on_action(cx.listener(|this, _: &PreviousTab, _, cx| {
                            this.cycle_palette_tab(-1, cx)
                        }))
                        .child(div().p(px(14.)).child(p.search.clone()))
                        .child(div().flex().gap(px(6.)).px(px(12.)).pb(px(8.)).children(
                            Tab::ALL.into_iter().map(|t| {
                                button(
                                    SharedString::from(format!("palette-tab-{}", t.label())),
                                    t.label(),
                                    tab == t,
                                )
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.palette_tab(t, cx)),
                                )
                            }),
                        ))
                        .child(
                            div()
                                .id("palette-results")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .track_scroll(&p.scroll)
                                .children(rows.iter().enumerate().map(|(ix, row)| {
                                    let pick = row.pick.clone();
                                    div()
                                        .id(("palette-result", ix))
                                        .px(px(14.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .when(ix == active, |d| d.bg(theme::element_hover()))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme::element_hover()))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.pick_palette(pick.clone(), window, cx)
                                        }))
                                        .child(div().text_size(px(14.)).child(row.title.clone()))
                                        .child(footer_text(row.detail.clone()))
                                }))
                                .when(rows.is_empty(), |d| {
                                    d.child(div().p(px(16.)).child(footer_text("No results")))
                                }),
                        )
                        .children(
                            p.status.clone().map(|status| {
                                div().px(px(14.)).py(px(6.)).child(footer_text(status))
                            }),
                        )
                        .child(div().px(px(14.)).py(px(10.)).child(footer_text(
                            "↑ ↓ to select · Enter to open · Ctrl+Alt+← → for tabs · Esc to close",
                        ))),
                )
                .into_any_element(),
        )
    }
}

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-alt-right", NextTab, Some("CommandPalette")),
        KeyBinding::new("ctrl-alt-left", PreviousTab, Some("CommandPalette")),
    ]);
}
