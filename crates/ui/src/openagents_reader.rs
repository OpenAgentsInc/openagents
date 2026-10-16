//! Saved transcript presentation. No engine, composer, checkout, or execution adapter is used.
use crate::{
    markdown::{self, BlockTree, render},
    theme::Theme,
};
use coder_account::reader::text_part;
use coder_account::{
    Controller,
    history::{ExistingChat, LoadState},
    openagents::{AttachmentRef, MessageRole, SavedMessage, ToolStatus},
    reader::{Reader, Snapshot},
};
use gpui::{
    AnyElement, Context, EventEmitter, ListAlignment, ListState, Render, SharedString, Task,
    Window, div, list, prelude::*, px,
};
use gpui_tokio::Tokio;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    future::Future,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
pub enum ReaderEvent {
    Close,
    AccessLost,
}
pub struct SavedReader {
    pub model: Arc<Reader>,
    pub snapshot: Snapshot,
    account: Arc<Controller>,
    list: ListState,
    focus: gpui::FocusHandle,
    focus_pending: bool,
    cache: HashMap<String, (String, Arc<BlockTree>)>,
    cache_order: VecDeque<String>,
    parts: HashMap<String, usize>,
    details: HashSet<String>,
    save_notice: Option<SharedString>,
    notified_loss: Option<u64>,
    _watch: Task<()>,
    _auth: Task<()>,
}
impl EventEmitter<ReaderEvent> for SavedReader {}
fn launch(f: impl Future<Output = ()> + Send + 'static, cx: &mut Context<SavedReader>) {
    let job = Tokio::spawn(cx, f);
    cx.spawn(async move |_, _| {
        let _ = job.await;
    })
    .detach();
}
impl SavedReader {
    pub fn new(account: Arc<Controller>, cx: &mut Context<Self>) -> Self {
        let model = Reader::new();
        let mut updates = model.watch();
        let watch = cx.spawn({
            let account = account.clone();
            async move |this, cx| {
                while updates.changed().await.is_ok() {
                    let snapshot = updates.borrow_and_update().clone();
                    if this
                        .update(cx, |this, cx| {
                            if snapshot.login_required {
                                launch(account.sign_out(), cx);
                                this.model.clear();
                                this.sync(this.model.snapshot());
                            } else {
                                this.sync(snapshot);
                            }
                            if this.snapshot.access_lost
                                && this.notified_loss != Some(this.snapshot.epoch)
                            {
                                this.notified_loss = Some(this.snapshot.epoch);
                                cx.emit(ReaderEvent::AccessLost);
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
        let mut auth = account.watch();
        let auth_watch = cx.spawn({
            let model = model.clone();
            async move |_, _| {
                while auth.changed().await.is_ok() {
                    if !model.current() {
                        model.clear();
                    }
                }
            }
        });
        Self {
            model,
            snapshot: Snapshot::default(),
            account,
            list: ListState::new(0, ListAlignment::Top, px(300.)),
            focus: cx.focus_handle(),
            focus_pending: false,
            cache: HashMap::new(),
            cache_order: VecDeque::new(),
            parts: HashMap::new(),
            details: HashSet::new(),
            save_notice: None,
            notified_loss: None,
            _watch: watch,
            _auth: auth_watch,
        }
    }
    fn sync(&mut self, snapshot: Snapshot) {
        let old_len = self.snapshot.messages.len();
        let new_len = snapshot.messages.len();
        let same_chat = self.snapshot.summary.as_ref().map(|s| &s.chat)
            == snapshot.summary.as_ref().map(|s| &s.chat);
        let changed =
            self.snapshot.epoch != snapshot.epoch || self.snapshot.first != snapshot.first;
        if changed || new_len < old_len {
            let mut offset = self.list.logical_scroll_top();
            self.list.reset(new_len);
            if same_chat && self.snapshot.first == snapshot.first && new_len > 0 {
                offset.item_ix = offset.item_ix.min(new_len - 1);
                self.list.scroll_to(offset);
            }
            self.cache.clear();
            self.cache_order.clear();
            self.parts.clear();
            self.details.clear();
            self.save_notice = None;
        } else if old_len == 0 && new_len > 0 {
            self.list.reset(new_len);
            self.list.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            });
        } else if new_len > old_len {
            self.list.splice(old_len..old_len, new_len - old_len);
        } else if new_len > 0 && !Arc::ptr_eq(&snapshot.messages, &self.snapshot.messages) {
            self.list.remeasure_items(0..new_len);
        }
        self.snapshot = snapshot;
    }
    pub fn open(&mut self, target: ExistingChat, cx: &mut Context<Self>) -> bool {
        if !self.account.accepts(&target.lease) {
            self.clear(cx);
            return false;
        }
        if self.model.target().is_some_and(|t| t.chat == target.chat)
            && self
                .snapshot
                .summary
                .as_ref()
                .is_some_and(|s| s.revision >= target.summary.revision)
            && !self.snapshot.access_lost
        {
            return true;
        }
        let opened = self.model.open(target).is_ok();
        if opened {
            self.focus_pending = true;
            launch(self.model.more(), cx);
        }
        self.sync(self.model.snapshot());
        cx.notify();
        opened
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.model.clear();
        self.sync(self.model.snapshot());
        cx.notify();
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.clear(cx);
        cx.emit(ReaderEvent::Close);
    }
    pub fn more(&mut self, cx: &mut Context<Self>) {
        launch(self.model.more(), cx);
        self.sync(self.model.snapshot());
        cx.notify();
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.model.refresh().is_ok() {
            self.more(cx);
        }
    }
    pub fn reveal_message(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.snapshot.first && index < self.snapshot.received {
            self.list.scroll_to_reveal_item(index - self.snapshot.first);
            cx.notify();
        }
    }
    fn latest(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.received > 0 {
            self.reveal_message(self.snapshot.received - 1, cx);
        }
    }
    fn continue_window(&mut self, cx: &mut Context<Self>) {
        self.model.continue_window();
        self.more(cx);
    }
    fn smaller(&mut self, cx: &mut Context<Self>) {
        self.model.smaller_page();
        self.more(cx);
    }
    pub fn download(&mut self, reference: AttachmentRef, name: String, cx: &mut Context<Self>) {
        self.parts.remove("saved-file-text-body");
        self.save_notice = None;
        launch(self.model.download(reference, name), cx);
        self.sync(self.model.snapshot());
        cx.notify();
    }
    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let hover = Theme::of(cx).glass_hover();
        div()
            .id(id)
            .tab_index(0)
            .role(gpui::Role::Button)
            .cursor_pointer()
            .px(px(8.))
            .py(px(5.))
            .rounded(px(5.))
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .child(label.into())
    }
    fn allowed(&self, epoch: u64) -> bool {
        let s = self.model.snapshot();
        self.model.current() && s.epoch == epoch && !s.access_lost && !s.login_required
    }
    fn rich(
        &mut self,
        key: String,
        text: &str,
        code: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let epoch = self.snapshot.epoch;
        let mut body = div().flex().flex_col().gap(px(6.));
        let (shown, count, part) = text_part(text, *self.parts.get(&key).unwrap_or(&0));
        let chunked = text.len() > 256 * 1024;
        let source = if chunked { shown } else { text.to_owned() };
        if chunked {
            let previous = key.clone();
            let next = key.clone();
            let row_key = key.clone();
            body = body.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .child(SharedString::from(format!("Part {} of {count}", part + 1)))
                    .when(part > 0, |row| {
                        row.child(self.button(
                            SharedString::from(format!("{row_key}-prev")),
                            "Previous part",
                            move |this, cx| {
                                this.parts.insert(previous.clone(), part - 1);
                                this.list.remeasure_items(0..this.snapshot.messages.len());
                                cx.notify();
                            },
                            cx,
                        ))
                    })
                    .when(part + 1 < count, |row| {
                        row.child(self.button(
                            SharedString::from(format!("{row_key}-next")),
                            "Next part",
                            move |this, cx| {
                                this.parts.insert(next.clone(), part + 1);
                                this.list.remeasure_items(0..this.snapshot.messages.len());
                                cx.notify();
                            },
                            cx,
                        ))
                    }),
            );
        }
        let tree = if let Some((old, tree)) = self.cache.get(&key).filter(|(old, _)| old == &source)
        {
            let _ = old;
            tree.clone()
        } else {
            let tree = Arc::new(if code || chunked {
                BlockTree {
                    blocks: vec![Arc::new(markdown::parser::TopBlock {
                        range: 0..source.len(),
                        block: markdown::Block::CodeBlock {
                            language: None,
                            code: source.clone(),
                        },
                    })],
                }
            } else {
                markdown::parse_full(&source)
            });
            if !self.cache.contains_key(&key) {
                self.cache_order.push_back(key.clone());
            }
            self.cache.insert(key.clone(), (source, tree.clone()));
            while self.cache_order.len() > 32 {
                if let Some(old) = self.cache_order.pop_front() {
                    self.cache.remove(&old);
                }
            }
            tree
        };
        let weak = cx.weak_entity();
        let mut options = render::RenderOptions::settled(SharedString::from(key));
        options.copy = Some(render::CopyUi {
            handler: Rc::new(move |_, code, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if this.allowed(epoch) {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(code.to_string()));
                    }
                });
            }),
            copied_ix: None,
        });
        let weak = cx.weak_entity();
        options.link = Some(render::LinkUi {
            source_session: None,
            source_local: false,
            file_roots: None,
            handler: Rc::new(move |activation, _, cx| {
                weak.update(cx, |this, _| {
                    if this.allowed(epoch) {
                        activation.web_outcome(false)
                    } else {
                        render::LinkOutcome::Rejected
                    }
                })
                .unwrap_or(render::LinkOutcome::Rejected)
            }),
        });
        body.child(render::render_tree(
            &tree,
            &options,
            &theme,
            window,
            &|_| None,
        ))
        .into_any_element()
    }
    fn render_message(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(message) = self.snapshot.messages.get(ix).cloned() else {
            return gpui::Empty.into_any_element();
        };
        let theme = Theme::of(cx).clone();
        let key = format!("saved-{}-{}", self.snapshot.epoch, message.id);
        let role = match message.role {
            MessageRole::User => "You",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
            MessageRole::Thinking => "Thinking",
            MessageRole::Other(_) => "Saved content",
        };
        let mut body = div()
            .w_full()
            .px(px(24.))
            .py(px(14.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_size(px(14.))
            .text_color(theme.text)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child(role),
            );
        if matches!(message.role, MessageRole::Other(_)) {
            body = body.child("This part uses a format this app doesn't support yet.");
        }
        let duplicate_tool = message
            .tool
            .as_ref()
            .is_some_and(|t| t.output == message.text);
        if !message.text.is_empty() && !duplicate_tool {
            body = body.child(self.rich(format!("{key}-text"), &message.text, false, window, cx));
        }
        if let Some(tool) = &message.tool {
            let status: SharedString = match &tool.status {
                ToolStatus::Done => "Completed".into(),
                ToolStatus::Failed => "Failed".into(),
                ToolStatus::Running => "Was running when saved".into(),
                ToolStatus::Other(value) => format!("Saved status: {}", short_label(value)).into(),
            };
            body = body
                .child(
                    div()
                        .font_family(theme.font_mono.clone())
                        .child(SharedString::from(short_label(&tool.name))),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.text_muted)
                        .child(status),
                );
            if !tool.summary.is_empty() {
                body = body.child(self.rich(
                    format!("{key}-tool-summary"),
                    &tool.summary,
                    false,
                    window,
                    cx,
                ));
            }
            for (label, text) in [
                ("Input", tool.input.as_str()),
                ("Output", tool.output.as_str()),
            ] {
                if !text.is_empty() {
                    body = body
                        .child(div().text_color(theme.text_muted).child(label))
                        .child(self.rich(format!("{key}-{label}"), text, true, window, cx));
                }
            }
        }
        for file in &message.files {
            let name = file.name.clone();
            let label = short_label(&name);
            if let Some(reference) = file.download_ref().cloned() {
                body = body.child(self.button(
                    SharedString::from(format!("{key}-file-{}", file.id)),
                    label,
                    move |this, cx| this.download(reference.clone(), name.clone(), cx),
                    cx,
                ));
            } else {
                body = body.child(SharedString::from(format!("{label} · Unavailable")));
            }
        }
        if has_details(&message) {
            let detail_key = key.clone();
            let expanded = self.details.contains(&key);
            body = body.child(self.button(
                SharedString::from(format!("{key}-details")),
                if expanded {
                    "Hide saved details"
                } else {
                    "More saved details"
                },
                move |this, cx| {
                    if !this.details.remove(&detail_key) {
                        this.details.insert(detail_key.clone());
                    }
                    this.list.remeasure_items(0..this.snapshot.messages.len());
                    cx.notify();
                },
                cx,
            ));
            if expanded {
                body = body.child("Some saved details can't be shown as a chat message.");
                let details = serde_json::to_string_pretty(&message.extra).unwrap_or_default();
                body = body.child(self.rich(format!("{key}-extra"), &details, true, window, cx));
            }
        }
        body.into_any_element()
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(download) = self.snapshot.download.clone().filter(|d| d.bytes.is_some()) else {
            return;
        };
        let epoch = self.snapshot.epoch;
        let suggested = std::path::Path::new(&download.name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment");
        let suggested = short_label(suggested);
        let directory = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|p| p.join("Downloads"))
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            let bytes = this
                .update(cx, |this, _| {
                    this.model.bytes_for_save(epoch, &download.reference)
                })
                .ok()
                .flatten();
            let Some(bytes) = bytes else {
                return;
            };
            let job = this.update(cx, |_, cx| {
                Tokio::spawn(cx, async move {
                    tokio::task::spawn_blocking(move || {
                        use std::io::Write;
                        let mut options = std::fs::OpenOptions::new();
                        options.write(true).create(true).truncate(true);
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::OpenOptionsExt;
                            options.mode(0o600);
                        }
                        options.open(path)?.write_all(&bytes)
                    })
                    .await
                })
            });
            if let Ok(job) = job {
                let result = job.await;
                let _ = this.update(cx, |this, cx| {
                    if this.allowed(epoch) {
                        this.save_notice = Some(
                            if matches!(result, Ok(Ok(Ok(())))) {
                                "Saved."
                            } else {
                                "Couldn't save the file. Try again."
                            }
                            .into(),
                        );
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
}
impl Drop for SavedReader {
    fn drop(&mut self) {
        self.model.clear();
    }
}
impl Render for SavedReader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(self.model.snapshot());
        if self.focus_pending {
            self.focus.focus(window, cx);
            self.focus_pending = false;
        }
        let theme = Theme::of(cx).clone();
        let mut body = div()
            .id("saved-reader")
            .track_focus(&self.focus)
            .key_context("SavedHistory")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close(cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .text_color(theme.text);
        let current = self.model.current();
        let title = if current {
            self.snapshot
                .summary
                .as_ref()
                .map(|s| short_label(&s.title))
                .unwrap_or_else(|| "Saved chat".into())
        } else {
            "Saved chat".into()
        };
        body = body.child(
            div()
                .flex_none()
                .px(px(24.))
                .py(px(12.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(18.))
                        .child(SharedString::from(title)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .when(current, |row| {
                            row.child(self.button(
                                "saved-reader-refresh",
                                "Reload",
                                Self::refresh,
                                cx,
                            ))
                        })
                        .when(
                            current && !self.snapshot.has_more && self.snapshot.received > 0,
                            |row| {
                                row.child(self.button(
                                    "saved-reader-latest",
                                    "Latest",
                                    Self::latest,
                                    cx,
                                ))
                            },
                        )
                        .child(self.button(
                            "saved-reader-close",
                            "Back to workspace",
                            Self::close,
                            cx,
                        )),
                ),
        );
        if !self.model.current() {
            return body.child(div().p(px(24.)).child("Sign in to open this chat."));
        }
        body = body.child(
            div().flex_1().min_h_0().child(
                list(self.list.clone(), cx.processor(Self::render_message))
                    .size_full()
                    .with_sizing_behavior(gpui::ListSizingBehavior::Auto),
            ),
        );
        let mut footer = div()
            .flex_none()
            .px(px(24.))
            .py(px(10.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .text_size(px(12.));
        match &self.snapshot.state {
            LoadState::Loading => footer = footer.child("Loading messages…"),
            LoadState::Failed { message, stale } => {
                if *stale {
                    footer = footer
                        .child("Couldn't update this chat. Earlier messages are still shown.");
                }
                footer = footer.child(SharedString::from(message.clone()));
                if self.snapshot.has_more
                    && !self.snapshot.smaller_page
                    && !self.snapshot.reload_required
                    && !self.snapshot.access_lost
                {
                    footer =
                        footer.child(self.button("saved-reader-retry", "Retry", Self::more, cx));
                }
            }
            LoadState::Ready => {
                if self.snapshot.received == 0 {
                    footer = footer.child("No saved messages.");
                } else {
                    let total = self
                        .snapshot
                        .summary
                        .as_ref()
                        .map(|s| s.messages)
                        .unwrap_or(self.snapshot.received);
                    footer = footer.child(SharedString::from(format!(
                        "Messages {}–{} of {total}",
                        self.snapshot.first + 1,
                        self.snapshot.received
                    )));
                }
            }
            LoadState::Idle => {}
        }
        if self.snapshot.reload_required {
            footer =
                footer.child(self.button("saved-reader-reload", "Reload chat", Self::refresh, cx));
        } else if self.snapshot.smaller_page {
            footer = footer.child(self.button(
                "saved-reader-smaller",
                "Load one message",
                Self::smaller,
                cx,
            ));
        } else if self.snapshot.continue_required {
            footer = footer
                .child("Earlier messages will leave this view. Reload to see them again.")
                .child(self.button(
                    "saved-reader-continue",
                    "Continue",
                    Self::continue_window,
                    cx,
                ));
        } else if self.snapshot.has_more && self.snapshot.state == LoadState::Ready {
            footer = footer.child(self.button(
                "saved-reader-more",
                "Load more messages",
                Self::more,
                cx,
            ));
        }
        for capture in self.snapshot.captures.clone() {
            let name = capture.name.clone();
            let label = short_label(&name);
            if let Some(reference) = capture.download_ref().cloned() {
                footer = footer.child(self.button(
                    SharedString::from(format!("saved-capture-{}", capture.id)),
                    label,
                    move |this, cx| this.download(reference.clone(), name.clone(), cx),
                    cx,
                ));
            } else {
                footer = footer.child(SharedString::from(format!(
                    "{label} · {}",
                    if capture.state == coder_account::openagents::CaptureState::NotSaved {
                        "Not saved"
                    } else {
                        "Unavailable"
                    }
                )));
            }
        }
        body = body.child(footer);
        if let Some(download) = self.snapshot.download.clone() {
            let mut preview = div()
                .id("saved-file-preview")
                .flex_none()
                .max_h(px(220.))
                .px(px(24.))
                .py(px(10.))
                .border_t_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(SharedString::from(short_label(&download.name)))
                        .child(self.button(
                            "saved-file-close",
                            "Close file",
                            |this, _| this.model.close_download(),
                            cx,
                        )),
                );
            match download.state {
                LoadState::Loading => preview = preview.child("Opening file…"),
                LoadState::Failed { message, .. } => {
                    let reference = download.reference.clone();
                    let name = download.name.clone();
                    preview = preview
                        .child(SharedString::from(message))
                        .child(self.button(
                            "saved-file-retry",
                            "Retry",
                            move |this, cx| this.download(reference.clone(), name.clone(), cx),
                            cx,
                        ));
                }
                LoadState::Ready => {
                    if let Some(bytes) = download.bytes {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            preview = preview.child(
                                div()
                                    .id("saved-file-text")
                                    .max_h(px(100.))
                                    .overflow_y_scroll()
                                    .child(self.rich(
                                        "saved-file-text-body".into(),
                                        text,
                                        true,
                                        window,
                                        cx,
                                    )),
                            );
                        } else {
                            preview = preview.child("Save this file to open it.");
                        }
                        preview = preview.child(self.button(
                            "saved-file-save",
                            "Save file",
                            Self::save,
                            cx,
                        ));
                    }
                }
                _ => {}
            }
            if let Some(notice) = &self.save_notice {
                preview = preview.child(notice.clone());
            }
            body = body.child(preview);
        }
        body
    }
}
fn short_label(text: &str) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(256).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}
fn has_details(message: &SavedMessage) -> bool {
    message.extra.values().any(|v| {
        !v.is_null()
            && !v.as_array().is_some_and(Vec::is_empty)
            && !v.as_object().is_some_and(serde_json::Map::is_empty)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_unicode_text_parts_preserve_every_character_in_order() {
        let text = "🦀 code and text\n".repeat(20000);
        let (_, count, _) = text_part(&text, 0);
        let restored = (0..count)
            .map(|part| text_part(&text, part).0)
            .collect::<String>();
        assert_eq!(restored, text);
        assert!(text_part(&text, count + 3).2 < count);
    }
}
