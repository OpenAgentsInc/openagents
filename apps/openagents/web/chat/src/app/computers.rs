//! Every computer's chats in the sidebar (#12265): the account's registry
//! room, merged with the site's own chats, and the registry's writes for
//! them (rename, pin and reorder, sections, archive, delete). A computer
//! chat opens through the computer's device room: its transcript streams
//! from `WatchDocMessages`, and a message goes to it as a `QueueCommand`.

use std::collections::{HashMap, HashSet};

use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Focusable as _, FontWeight, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*, px,
};
use openagents_doc::{SessionMessageEntry, TranscriptUpdate, apply_transcript_frame};
use openagents_web_chat_view::composer::{Cancelled, Submitted, TextInput};
use openagents_web_chat_view::svg;
use openagents_web_chat_view::{now_ms, theme};
use openagents_web_client::computer;
use openagents_web_client::registry::Registry;
use openagents_web_client::workspace::{self, Organize, Sort, View};
use openagents_web_client::{Change, Thread};
use serde_json::json;

use super::{ChatApp, Menu, menu_item, menu_item_label};
use crate::model::{self, Group};

/// Where this browser keeps its sidebar view.
const VIEW_KEY: &str = "openagents.sidebar.view";

/// What the person at the computer has to do the first time.
pub(super) fn allow_line(computer: &str) -> String {
    format!(
        "Allow this browser on {computer}: in the OpenAgents app there, open Settings, then Devices."
    )
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub(super) fn load_view() -> View {
    storage()
        .and_then(|s| s.get_item(VIEW_KEY).ok().flatten())
        .map(|v| View::decode(&v))
        .unwrap_or_default()
}

fn save_view(view: View) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(VIEW_KEY, &view.encode());
    }
}

impl ChatApp {
    /// Opens the account's registry, signed in only, and draws again on
    /// each change.
    pub(super) fn open_registry(&mut self, cx: &mut Context<Self>) {
        if !self.signed_in {
            return;
        }
        let (registry, mut changes) = Registry::open(self.client.clone());
        self.registry = Some(registry);
        self._registry = cx.spawn(async move |this, cx| {
            while changes.next().await.is_some() {
                // Several frames often arrive together; draw once for them.
                while changes.try_recv().is_ok() {}
                if this
                    .update(cx, |this, cx| {
                        this.merge_threads();
                        // A computer chat's own address, opened before its
                        // row arrived.
                        if this.watching.is_none()
                            && this.current.as_ref().is_some_and(|id| this.is_computer(id))
                        {
                            this.load(this.current.clone(), cx);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    /// The site's chats and every computer's, as the sidebar lists them.
    pub(super) fn merge_threads(&mut self) {
        let mut meta = HashMap::new();
        let mut computer_threads = Vec::new();
        if let Some(registry) = &self.registry {
            let (rows, sections) = registry.read(|r| {
                (
                    r.rows(now_ms()),
                    r.preferences().unwrap_or_default().sections,
                )
            });
            for row in &rows {
                let (thread, found) = model::computer_thread(row, &sections);
                meta.insert(thread.id.clone(), found);
                computer_threads.push(thread);
            }
        }
        let ids: HashSet<&str> = meta.keys().map(String::as_str).collect();
        let mut threads: Vec<Thread> = self
            .web_threads
            .iter()
            .filter(|t| !ids.contains(t.id.as_str()))
            .cloned()
            .collect();
        threads.extend(computer_threads);
        self.threads = threads;
        self.meta = meta;
        if self
            .registry
            .as_ref()
            .is_some_and(|r| r.synced() || r.unavailable())
        {
            self.computers_loaded = true;
        }
    }

    /// Whether the open chat is a computer chat whose agent is working.
    pub(super) fn computer_working(&self) -> bool {
        self.current
            .as_deref()
            .filter(|id| self.is_computer(id))
            .and_then(|id| self.thread(id))
            .is_some_and(|t| t.working)
    }

    /// The composer's computer for registry chat `id`: its queue, commands,
    /// and questions go there (#12267, #12268).
    pub(super) fn host_chat_of(&self, id: &str) -> Option<crate::compose::HostChat> {
        let meta = self.meta.get(id)?;
        let harness = self
            .registry
            .as_ref()
            .and_then(|r| r.read(|r| r.chat(id)))
            .and_then(|chat| chat.config)
            .and_then(|config| serde_json::to_value(config.harness).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        Some(crate::compose::HostChat {
            device: meta.computer.0.clone(),
            chat: id.to_owned(),
            harness,
        })
    }

    /// Whether `id` is a chat on one of the account's computers.
    pub(super) fn is_computer(&self, id: &str) -> bool {
        self.meta.contains_key(id)
    }

    fn registry_write(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut workspace::Replica) -> Result<(), String>,
    ) {
        let Some(registry) = &self.registry else {
            return;
        };
        if let Err(line) = registry.write(f) {
            self.notice = Some(line);
        }
        self.merge_threads();
        cx.notify();
    }

    /// A rename, pin, or archive of a computer chat.
    pub(super) fn computer_change(&mut self, id: String, change: Change, cx: &mut Context<Self>) {
        self.menu = None;
        let leaving = change.archived == Some(true) && self.current.as_deref() == Some(&id);
        self.registry_write(cx, |r| {
            if let Some(title) = &change.title {
                r.rename(&id, title)?;
            }
            match change.pinned {
                Some(true) => r.pin(&id)?,
                Some(false) => r.unpin(&id)?,
                None => {}
            }
            if let Some(archived) = change.archived {
                r.archive(&id, archived)?;
            }
            Ok(())
        });
        if leaving {
            super::route::push(None);
            self.load(None, cx);
        }
    }

    pub(super) fn computer_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.menu = None;
        if self.current.as_deref() == Some(&id) {
            super::route::push(None);
            self.load(None, cx);
        }
        self.registry_write(cx, |r| r.delete(&id));
    }

    fn move_pin(&mut self, id: String, up: bool, cx: &mut Context<Self>) {
        self.menu = None;
        self.registry_write(cx, |r| r.move_pin(&id, up));
    }

    fn assign_section(&mut self, id: String, section: Option<String>, cx: &mut Context<Self>) {
        self.menu = None;
        self.registry_write(cx, |r| r.assign(&id, section.as_deref()));
    }

    fn toggle_section(&mut self, id: String, folded: bool, cx: &mut Context<Self>) {
        self.registry_write(cx, |r| r.collapse_section(&id, !folded));
    }

    fn delete_section(&mut self, id: String, cx: &mut Context<Self>) {
        self.menu = None;
        self.registry_write(cx, |r| r.delete_section(&id));
    }

    /// A name field for a new section that `chat` goes into, or for
    /// renaming section `rename`.
    fn section_field(
        &mut self,
        chat: Option<String>,
        rename: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = cx.new(|cx| {
            let mut field = TextInput::new("Section name", cx);
            if let Some((_, name)) = &rename {
                field.set_text(name.clone(), cx);
            }
            field
        });
        let key = match (&rename, &chat) {
            (Some((id, _)), _) => format!("section:{id}"),
            (None, Some(chat)) => chat.clone(),
            (None, None) => "section:new".into(),
        };
        cx.subscribe(&field, move |this, _, event: &Submitted, cx| {
            let name = event.0.trim().to_owned();
            this.rename = None;
            if !name.is_empty() {
                let name = openagents_web_chat_view::transcript::truncate(&name, 60);
                match &rename {
                    Some((id, _)) => {
                        let id = id.clone();
                        this.registry_write(cx, |r| r.rename_section(&id, &name));
                    }
                    None => {
                        let id = uuid::Uuid::new_v4().to_string();
                        let chat = chat.clone();
                        this.registry_write(cx, |r| {
                            r.create_section(&id, &name)?;
                            match &chat {
                                Some(chat) => r.assign(chat, Some(&id)),
                                None => Ok(()),
                            }
                        });
                    }
                }
            }
            cx.notify();
        })
        .detach();
        cx.subscribe(&field, |this, _, _: &Cancelled, cx| {
            this.rename = None;
            cx.notify();
        })
        .detach();
        window.focus(&field.focus_handle(cx), cx);
        self.menu = None;
        self.rename = Some((key, field));
        cx.notify();
    }

    fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        save_view(view);
        cx.notify();
    }

    // ── an open computer chat ────────────────────────────────────────────

    /// Opens computer chat `id`: connects to its computer, asking for this
    /// browser's access the first time, and follows its transcript.
    pub(super) fn load_computer(&mut self, id: String, generation: u64, cx: &mut Context<Self>) {
        let Some(chat) = self.registry.as_ref().and_then(|r| r.read(|r| r.chat(&id))) else {
            return;
        };
        let name = self
            .thread(&id)
            .and_then(|t| t.computer.clone())
            .unwrap_or_else(|| "your computer".into());
        super::route::set_title(chat.title.as_deref().unwrap_or("Chat"));
        if let Some(registry) = &self.registry {
            registry.write(|r| r.seen(&id, now_ms()));
        }
        self.load_computer_on(id, chat.device_id, name, generation, cx);
    }

    /// A Lab trial can finish before its chat arrives in the registry.
    pub(super) fn load_computer_on(
        &mut self,
        id: String,
        device: String,
        name: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        self.loading_chat = true;
        let client = self.client.clone();
        let (asked_tx, mut asked) = futures::channel::mpsc::unbounded::<()>();
        let asking = cx.spawn({
            let name = name.clone();
            async move |this, cx| {
                if asked.next().await.is_some() {
                    this.update(cx, |this, cx| {
                        if this.generation == generation {
                            this.notice = Some(allow_line(&name));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            }
        });
        self.watching = Some(cx.spawn(async move |this, cx| {
            let _asking = asking;
            let mut entries: Vec<SessionMessageEntry> = Vec::new();
            // A desync asks for a fresh reset, a few times at most.
            for _ in 0..3 {
                let opened = async {
                    let computer = computer::connect(&client, &device, {
                        let asked_tx = asked_tx.clone();
                        move || {
                            let _ = asked_tx.unbounded_send(());
                        }
                    })
                    .await?;
                    computer.watch(
                        openagents_web_client::computer::WATCH_DOC_MESSAGES,
                        json!({"chatId": id, "openingTail": true}),
                    )
                }
                .await;
                let mut watch = match opened {
                    Ok(watch) => watch,
                    Err(error) => {
                        this.update(cx, |this, cx| {
                            if this.generation == generation {
                                this.loading_chat = false;
                                this.notice = Some(error.to_string());
                                cx.notify();
                            }
                        })
                        .ok();
                        return;
                    }
                };
                let mut desync = false;
                while let Some(item) = watch.next().await {
                    let update = match item {
                        Ok(value) => serde_json::from_value::<TranscriptUpdate>(value).ok(),
                        Err(error) => {
                            computer::disconnect(&device);
                            this.update(cx, |this, cx| {
                                if this.generation == generation {
                                    this.notice = Some(error.to_string());
                                    cx.notify();
                                }
                            })
                            .ok();
                            return;
                        }
                    };
                    let Some(update) = update else {
                        continue;
                    };
                    if apply_transcript_frame(&mut entries, update.frame).is_err() {
                        desync = true;
                        break;
                    }
                    let shown = entries.clone();
                    let gone = this
                        .update(cx, |this, cx| {
                            if this.generation != generation {
                                return true;
                            }
                            this.loading_chat = false;
                            if this
                                .notice
                                .as_deref()
                                .is_some_and(|n| n.starts_with("Allow this browser"))
                            {
                                this.notice = None;
                            }
                            this.entries = shown;
                            this.sync_list();
                            let entries = &this.entries;
                            this.compose
                                .update(cx, |compose, cx| compose.transcript(entries, cx));
                            cx.notify();
                            false
                        })
                        .unwrap_or(true);
                    if gone {
                        return;
                    }
                }
                if !desync {
                    this.update(cx, |this, cx| {
                        if this.generation == generation {
                            this.loading_chat = false;
                            cx.notify();
                        }
                    })
                    .ok();
                    return;
                }
                entries.clear();
            }
        }));
    }

    /// Stops the open computer chat's run.
    pub(super) fn stop_computer(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(device) = self.meta.get(&id).map(|m| m.computer.0.clone()) else {
            return;
        };
        self.computer_call(
            device,
            computer::QUEUE_COMMAND,
            workspace::interrupt_command(&id),
            None,
            cx,
        );
    }

    /// One call to a computer; on failure the words go back in the composer.
    pub(super) fn computer_call(
        &mut self,
        device: String,
        method: &'static str,
        params: serde_json::Value,
        text: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.sending = true;
        self.notice = None;
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let sent = async {
                let computer = computer::connect(&client, &device, || {}).await?;
                computer.call(method, params).await
            }
            .await;
            this.update(cx, |this, cx| {
                this.sending = false;
                if let Err(error) = sent {
                    if let Some(text) = &text {
                        this.composer.update(cx, |composer, cx| {
                            if composer.is_empty() {
                                composer.set_text(text.clone(), cx);
                            }
                        });
                    }
                    this.notice = Some(error.to_string());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    // ── drawing ──────────────────────────────────────────────────────────

    /// A group's header: a section folds and has its own rename and delete;
    /// a computer says whether it's online.
    pub(super) fn render_group_header(&self, group: &Group, cx: &mut Context<Self>) -> AnyElement {
        let label = group.label();
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .pb(px(4.))
            .text_size(px(12.5))
            .text_color(theme::text_faint());
        match group {
            Group::Project(..) | Group::Space(..) => {
                row = row.child(
                    svg()
                        .path("icons/folder.svg")
                        .size(px(13.))
                        .text_color(theme::text_faint()),
                );
            }
            Group::Computer(device, _) => {
                let online = self.threads.iter().any(|t| {
                    t.online
                        && self
                            .meta
                            .get(&t.id)
                            .is_some_and(|m| &m.computer.0 == device)
                });
                row = row
                    .child(
                        svg()
                            .path("icons/laptop.svg")
                            .size(px(13.))
                            .text_color(theme::text_faint()),
                    )
                    .child(div().size(px(6.)).rounded_full().bg(if online {
                        theme::success()
                    } else {
                        theme::text_faint().opacity(0.4)
                    }));
            }
            _ => {}
        }
        let Group::Section(id, name, folded) = group else {
            return row.child(label).into_any_element();
        };
        if let Some((_, field)) = self
            .rename
            .as_ref()
            .filter(|(key, _)| *key == format!("section:{id}"))
        {
            return row
                .child(
                    div()
                        .flex_1()
                        .px(px(6.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(theme::accent().opacity(0.6))
                        .text_size(px(13.))
                        .child(field.clone()),
                )
                .into_any_element();
        }
        let (toggle, folded) = (id.clone(), *folded);
        let (rename, current) = (id.clone(), name.clone());
        let delete = id.clone();
        div()
            .id(SharedString::from(format!("section-{id}")))
            .group("section")
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.toggle_section(toggle.clone(), folded, cx)
            }))
            .child(
                row.child(
                    svg()
                        .path(if folded {
                            "icons/alt-arrow-right.svg"
                        } else {
                            "icons/alt-arrow-down.svg"
                        })
                        .size(px(12.))
                        .text_color(theme::text_faint()),
                )
                .child(div().flex_1().child(label))
                .child(
                    small_icon(format!("rename-section-{id}"), "icons/pen.svg")
                        .opacity(0.)
                        .group_hover("section", |s| s.opacity(1.))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.section_field(
                                None,
                                Some((rename.clone(), current.clone())),
                                window,
                                cx,
                            )
                        })),
                )
                .child(
                    small_icon(
                        format!("delete-section-{id}"),
                        "icons/trash-bin-minimalistic.svg",
                    )
                    .opacity(0.)
                    .group_hover("section", |s| s.opacity(1.))
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.delete_section(delete.clone(), cx)
                        },
                    )),
                ),
            )
            .into_any_element()
    }

    /// Whether a group's rows are folded away.
    pub(super) fn folded(group: &Group) -> bool {
        matches!(group, Group::Section(_, _, true))
    }

    /// The row menu's items only a computer chat has: reordering its pin,
    /// filing it in a section, and bringing it back from the archive.
    pub(super) fn computer_menu_items(
        &self,
        thread: &Thread,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let id = thread.id.clone();
        let Some(meta) = self.meta.get(&id) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        if meta.pin.is_some() {
            let up = id.clone();
            items.push(
                menu_item("menu-pin-up", "icons/arrow-up.svg", "Move up")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.move_pin(up.clone(), true, cx)
                    }))
                    .into_any_element(),
            );
            let down = id.clone();
            items.push(
                menu_item("menu-pin-down", "icons/alt-arrow-down.svg", "Move down")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.move_pin(down.clone(), false, cx)
                    }))
                    .into_any_element(),
            );
        }
        let section = id.clone();
        items.push(
            menu_item("menu-section", "icons/folder.svg", "Move to section")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.menu = Some(Menu::Section(section.clone()));
                    cx.notify();
                }))
                .into_any_element(),
        );
        if meta.archived {
            let restore = id.clone();
            items.push(
                menu_item(
                    "menu-unarchive",
                    "icons/archive-minimalistic.svg",
                    "Unarchive",
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.computer_change(
                        restore.clone(),
                        Change {
                            archived: Some(false),
                            ..Default::default()
                        },
                        cx,
                    )
                }))
                .into_any_element(),
            );
        }
        items
    }

    /// The section choices for chat `id`.
    pub(super) fn section_menu_items(&self, id: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let sections = self
            .registry
            .as_ref()
            .map(|r| r.read(|r| r.preferences().unwrap_or_default().sections))
            .unwrap_or_default();
        let current = self
            .meta
            .get(id)
            .and_then(|m| m.section.clone())
            .map(|s| s.0);
        let mut items = Vec::new();
        let none = id.to_owned();
        items.push(
            menu_item("section-none", "icons/close.svg", "No section")
                .when(current.is_none(), |b| b.bg(theme::element_active()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.assign_section(none.clone(), None, cx)
                }))
                .into_any_element(),
        );
        for section in sections {
            let chat = id.to_owned();
            let here = current.as_deref() == Some(section.id.as_str());
            let target = section.id.clone();
            items.push(
                div()
                    .id(SharedString::from(format!("section-pick-{}", section.id)))
                    .child(
                        menu_item_label("icons/folder.svg", section.name.clone())
                            .when(here, |b| b.bg(theme::element_active())),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.assign_section(chat.clone(), Some(target.clone()), cx)
                    }))
                    .into_any_element(),
            );
        }
        let chat = id.to_owned();
        items.push(
            menu_item("section-new", "icons/pen-new-square.svg", "New section")
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.section_field(Some(chat.clone()), None, window, cx)
                }))
                .into_any_element(),
        );
        items
    }

    /// The sidebar's view button.
    pub(super) fn render_view_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        super::icon_button("sidebar-view", "icons/tuning.svg").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.menu = match this.menu {
                    Some(Menu::View) => None,
                    _ => Some(Menu::View),
                };
                cx.notify();
            },
        ))
    }

    /// The sidebar's view menu: how chats are grouped and sorted, compact
    /// rows, and the archived.
    pub(super) fn render_view_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = self.view;
        let choice = |id: &'static str, label: &'static str, on: bool| {
            div().id(id).child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(9.))
                    .px(px(8.))
                    .h(px(30.))
                    .rounded(px(7.))
                    .text_size(px(13.5))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::element_hover()))
                    .child(div().w(px(14.)).text_color(theme::accent()).child(if on {
                        "✓"
                    } else {
                        ""
                    }))
                    .child(label),
            )
        };
        let heading = |label: &'static str| {
            div()
                .px(px(8.))
                .pt(px(6.))
                .pb(px(2.))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text_faint())
                .child(label)
        };
        let organize = |o: Organize| View {
            organize: o,
            ..view
        };
        let sort = |s: Sort| View { sort: s, ..view };
        let pick = |next: View| {
            cx.listener(move |this: &mut ChatApp, _: &ClickEvent, _, cx| this.set_view(next, cx))
        };
        div()
            .mx(px(10.))
            .mb(px(6.))
            .p(px(4.))
            .rounded(px(10.))
            .border_1()
            .border_color(theme::border_strong())
            .bg(theme::surface_card())
            .flex()
            .flex_col()
            .child(heading("Group"))
            .child(
                choice(
                    "view-computer",
                    "By computer",
                    view.organize == Organize::ByComputer,
                )
                .on_click(pick(organize(Organize::ByComputer))),
            )
            .child(
                choice(
                    "view-project",
                    "By project",
                    view.organize == Organize::ByProject,
                )
                .on_click(pick(organize(Organize::ByProject))),
            )
            .child(
                choice("view-list", "One list", view.organize == Organize::OneList)
                    .on_click(pick(organize(Organize::OneList))),
            )
            .child(heading("Sort"))
            .child(
                choice("sort-updated", "Last updated", view.sort == Sort::Updated)
                    .on_click(pick(sort(Sort::Updated))),
            )
            .child(
                choice("sort-created", "Created", view.sort == Sort::Created)
                    .on_click(pick(sort(Sort::Created))),
            )
            .child(heading("Show"))
            .child(
                choice("view-compact", "Compact rows", view.compact).on_click(pick(View {
                    compact: !view.compact,
                    ..view
                })),
            )
            .child(
                choice("view-archived", "Archived", view.archived).on_click(pick(View {
                    archived: !view.archived,
                    ..view
                })),
            )
            .into_any_element()
    }
}

fn small_icon(id: String, icon: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .flex_none()
        .size(px(18.))
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .hover(|s| s.bg(theme::element_active()))
        .child(
            svg()
                .path(icon)
                .size(px(12.))
                .text_color(theme::text_muted()),
        )
}
