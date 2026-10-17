//! Account history loading around the normal session transcript.
use crate::{
    markdown::{self, BlockTree, render},
    theme::Theme,
};
use gpui::{
    AnyElement, Context, EventEmitter, Render, SharedString, Task, Window, div, prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_account::reader::text_part;
use openagents_account::{
    Controller,
    history::{ExistingChat, LoadState},
    openagents::AttachmentRef,
    reader::{Reader, Snapshot},
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone)]
pub enum ReaderEvent {
    Continue(ExistingChat),
    ContinueOn(ExistingChat, String),
    OpenComputer(String),
    Close,
    AccessLost,
}
pub struct SavedReader {
    pub model: Arc<Reader>,
    cloud_turns: Arc<openagents_account::turns::Turns>,
    composer: gpui::Entity<crate::openagents_composer::AccountComposer>,
    pub snapshot: Snapshot,
    state: gpui::Entity<crate::state::AppState>,
    doc_id: String,
    transcript: gpui::Entity<crate::transcript::Transcript>,
    account: Arc<Controller>,
    focus: gpui::FocusHandle,
    focus_pending: bool,
    cache: HashMap<String, (String, Arc<BlockTree>)>,
    cache_order: VecDeque<String>,
    parts: HashMap<String, usize>,
    save_notice: Option<SharedString>,
    show_files: bool,
    notified_loss: Option<u64>,
    home_choices: Option<rust_native::ValidatedView<chat_home::Home>>,
    part_choices: Option<rust_native::ValidatedView<chat_home::PartRef>>,
    home_revision: u64,
    cloud_target: Option<(String, String)>,
    home_fingerprint: String,
    parts_fingerprint: String,
    _watch: Task<()>,
    _auth: Task<()>,
    _follow: Task<()>,
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
    fn move_to_cloud(&mut self, target: ExistingChat, cx: &mut Context<Self>) {
        let source_id = target.chat.id().to_owned();
        let id = match &self.cloud_target {
            Some((source, id)) if source == &source_id => id.clone(),
            _ => {
                let id = uuid::Uuid::new_v4().to_string();
                self.cloud_target = Some((source_id, id.clone()));
                id
            }
        };
        let source = target.chat.clone();
        let lease = target.lease.clone();
        let revision = target.summary.revision;
        let operation = Tokio::spawn(cx, async move {
            openagents_account::openagents::continuations::transfer(
                lease.client(),
                lease.session(),
                &source,
                &chat_home::TransferClaim {
                    revision,
                    target: chat_home::PartRef {
                        id,
                        home: chat_home::Home::Cloud,
                    },
                    seed: None,
                },
                &lease.cancel,
            )
            .await
        });
        cx.spawn(async move |this, cx| {
            let receipt = operation.await;
            this.update(cx, |this, cx| {
                let Some(mut current) = this.model.target().filter(ExistingChat::is_current) else {
                    return;
                };
                if current.chat != target.chat {
                    return;
                }
                match receipt {
                    Ok(Ok(receipt)) => {
                        let Ok(chat) = current
                            .lease
                            .client()
                            .chat(current.lease.session(), &receipt.target.id)
                        else {
                            return;
                        };
                        current.chat = chat.clone();
                        current.summary.chat = chat;
                        current.summary.home = chat_home::Home::Cloud;
                        current.summary.links = chat_home::Links {
                            continues: Some(receipt.source),
                            continued_by: None,
                        };
                        current.summary.revision = 0;
                        this.open(current, cx);
                    }
                    _ => {
                        this.save_notice =
                            Some("Couldn't move this chat. Open it again and try again.".into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_part(&mut self, part: chat_home::PartRef, cx: &mut Context<Self>) {
        let Some(mut target) = self.model.target().filter(ExistingChat::is_current) else {
            return;
        };
        if !self.snapshot.summary.as_ref().is_some_and(|s| {
            s.chat == target.chat
                && (s.links.continues.as_ref() == Some(&part)
                    || s.links.continued_by.as_ref() == Some(&part))
        }) {
            return;
        }
        if matches!(&part.home, chat_home::Home::Computer(device) if !device.starts_with("legacy-"))
        {
            cx.emit(ReaderEvent::OpenComputer(part.id));
            return;
        }
        let Ok(chat) = target.lease.client().chat(target.lease.session(), &part.id) else {
            return;
        };
        target.chat = chat.clone();
        target.summary.chat = chat;
        target.summary.home = part.home;
        target.summary.links = Default::default();
        target.summary.revision = 0;
        self.open(target, cx);
    }

    fn part_controls(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let target = self.model.target().filter(ExistingChat::is_current)?;
        let lease = target.lease.clone();
        let expected = target.chat.clone();
        let links = self.snapshot.summary.as_ref()?.links.clone();
        let fingerprint =
            serde_json::to_string(&(expected.id(), expected.account(), lease.generation, &links))
                .ok()?;
        if fingerprint != self.parts_fingerprint {
            self.home_revision = self.home_revision.wrapping_add(1);
            self.part_choices = chat_home::view::parts(
                &format!("parts-{}", self.doc_id),
                self.home_revision,
                &links,
            )
            .ok();
            self.parts_fingerprint = fingerprint;
        }
        Some(crate::native_actions::mount(
            self.part_choices.as_ref()?,
            move |this: &mut Self, event, cx| {
                if !lease.is_current() || this.model.target().is_none_or(|t| t.chat != expected) {
                    return;
                }
                if let Some(part) = this
                    .part_choices
                    .as_ref()
                    .and_then(|v| v.activate(&event).ok())
                    .cloned()
                {
                    this.open_part(part, cx);
                }
            },
            cx,
        ))
    }

    fn home_control(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use chat_home::{Home, view::Destination};
        let target = self.model.target().filter(ExistingChat::is_current)?;
        let lease = target.lease;
        let expected = target.chat;
        let summary = self.snapshot.summary.as_ref()?;
        if summary.revision == 0 || summary.links.continued_by.is_some() {
            return None;
        }
        let state = self.state.read(cx);
        let engine = state.engine()?;
        let local = state.local_device_id.as_deref();
        let current = Destination {
            home: summary.home.clone(),
            available: true,
            label: match &summary.home {
                Home::Cloud => "OpenAgents".into(),
                Home::Computer(device) => state
                    .devices
                    .iter()
                    .find(|d| &d.id == device)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Computer".into()),
            },
        };
        let mut destinations = state
            .devices
            .iter()
            .filter(|device| !matches!(device.platform.as_str(), "ios" | "android" | "ipados"))
            .filter(|d| d.supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1))
            .filter_map(|device| {
                Some(Destination {
                    home: Home::computer(&device.id)?,
                    label: if local == Some(device.id.as_str()) {
                        "This Mac".into()
                    } else {
                        device.name.clone()
                    },
                    available: true,
                })
            })
            .collect::<Vec<_>>();
        if matches!(&summary.home, Home::Computer(device) if device == &format!("legacy-{}", summary.chat.id()))
        {
            destinations.insert(
                0,
                Destination {
                    home: Home::Cloud,
                    label: "OpenAgents".into(),
                    available: true,
                },
            );
        }
        if let Some(local) = local.filter(|_| {
            engine
                .engine_info()
                .supports(openagents_proto::capabilities::OPENAGENTS_CONTINUATIONS_V1)
        }) {
            if !destinations
                .iter()
                .any(|d| d.home == Home::Computer(local.into()))
            {
                destinations.insert(
                    0,
                    Destination {
                        home: Home::Computer(local.into()),
                        label: "This Mac".into(),
                        available: true,
                    },
                );
            }
        }
        let blocked = if self.cloud_turns.snapshot().busy {
            Some("Stop the current answer before continuing elsewhere.")
        } else if self.composer.read(cx).has_draft(cx) {
            Some("Send or clear your draft before moving this chat.")
        } else {
            None
        };
        let fingerprint = serde_json::to_string(&(
            expected.id(),
            expected.account(),
            lease.generation,
            summary.revision,
            &current.home,
            &current.label,
            blocked,
            destinations
                .iter()
                .map(|d| (&d.home, &d.label, d.available))
                .collect::<Vec<_>>(),
        ))
        .ok()?;
        if fingerprint != self.home_fingerprint {
            self.home_revision = self.home_revision.wrapping_add(1);
            self.home_choices = chat_home::view::choices(
                &self.doc_id,
                self.home_revision,
                &current,
                &destinations,
                blocked,
            )
            .ok();
            self.home_fingerprint = fingerprint;
        }
        let view = self.home_choices.as_ref()?;
        Some(crate::native_actions::mount(
            view,
            move |this: &mut Self, activation, cx| {
                if !lease.is_current()
                    || this.model.target().is_none_or(|t| t.chat != expected)
                    || this.cloud_turns.snapshot().busy
                    || this.composer.read(cx).has_draft(cx)
                {
                    return;
                }
                let Some(home) = this
                    .home_choices
                    .as_ref()
                    .and_then(|v| v.activate(&activation).ok())
                    .cloned()
                else {
                    return;
                };
                let Some(mut target) = this.model.target().filter(ExistingChat::is_current) else {
                    return;
                };
                if let Some(summary) = this.snapshot.summary.clone() {
                    target.summary = summary;
                }
                match home {
                    Home::Computer(device) => cx.emit(ReaderEvent::ContinueOn(target, device)),
                    Home::Cloud => this.move_to_cloud(target, cx),
                }
            },
            cx,
        ))
    }
    pub fn new(
        account: Arc<Controller>,
        state: gpui::Entity<crate::state::AppState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let model = Reader::new();
        let cloud_turns = openagents_account::turns::Turns::new(model.clone());
        let composer = cx.new(|cx| {
            crate::openagents_composer::AccountComposer::new(model.clone(), cloud_turns.clone(), cx)
        });
        let doc_id = format!("openagents-saved-chat-{}", uuid::Uuid::new_v4());
        let transcript = cx.new(|cx| {
            crate::transcript::Transcript::for_account_history(
                state.clone(),
                model.clone(),
                doc_id.clone(),
                cx,
            )
        });
        cx.on_release(|this: &mut Self, cx| {
            this.state.update(cx, |state, cx| {
                state.unwatch_subagent_doc(&this.doc_id);
                cx.notify();
            });
        })
        .detach();
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
                                this.sync(this.model.snapshot(), cx);
                            } else {
                                this.sync(snapshot, cx);
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
        let follow = cx.spawn({
            let model = model.clone();
            async move |_, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    if Tokio::spawn(cx, model.poll()).await.is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            cloud_turns,
            composer,
            model,
            snapshot: Snapshot::default(),
            account,
            state,
            doc_id,
            transcript,
            focus: cx.focus_handle(),
            focus_pending: false,
            cache: HashMap::new(),
            cache_order: VecDeque::new(),
            parts: HashMap::new(),
            save_notice: None,
            show_files: false,
            notified_loss: None,
            home_choices: None,
            part_choices: None,
            home_revision: 0,
            cloud_target: None,
            home_fingerprint: String::new(),
            parts_fingerprint: String::new(),
            _watch: watch,
            _auth: auth_watch,
            _follow: follow,
        }
    }
    #[cfg(feature = "account-fixture")]
    pub fn fixture_continue(&mut self, cx: &mut Context<Self>) {
        let mut target = self.model.target().expect("open the saved chat");
        target.summary = self
            .snapshot
            .summary
            .clone()
            .expect("load its latest revision");
        cx.emit(ReaderEvent::Continue(target));
    }
    #[cfg(feature = "account-fixture")]
    pub fn fixture_reply(&mut self, text: String, cx: &mut Context<Self>) {
        self.composer
            .update(cx, |composer, cx| composer.fixture_submit(text, cx));
    }
    #[cfg(feature = "account-fixture")]
    pub fn fixture_turn(&self) -> openagents_account::turns::Snapshot {
        self.cloud_turns.snapshot()
    }
    fn sync(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        let changed = self.snapshot.epoch != snapshot.epoch;
        let new_content = !Arc::ptr_eq(&snapshot.messages, &self.snapshot.messages);
        if changed {
            self.cache.clear();
            self.cache_order.clear();
            self.parts.clear();
            self.save_notice = None;
            self.show_files = false;
            self.transcript = cx.new(|cx| {
                crate::transcript::Transcript::for_account_history(
                    self.state.clone(),
                    self.model.clone(),
                    self.doc_id.clone(),
                    cx,
                )
            });
        }
        if new_content || changed {
            let target = self.model.target();
            let entries = openagents_account::chat_entries::entries_with_lab_rows(
                &snapshot.messages,
                snapshot.epoch,
                target.as_ref().map(|target| &target.chat),
            );
            self.state.update(cx, |state, cx| {
                state.set_subagent_snapshot(self.doc_id.clone(), entries);
                cx.notify();
            });
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
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        let opened = self.model.open_latest(target).is_ok();
        if opened {
            self.model.set_following(true);
            self.focus_pending = true;
            launch(self.model.more(), cx);
        }
        self.sync(self.model.snapshot(), cx);
        cx.notify();
        opened
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.composer.update(cx, |composer, cx| composer.clear(cx));
        self.model.clear();
        self.sync(self.model.snapshot(), cx);
        cx.notify();
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.clear(cx);
        cx.emit(ReaderEvent::Close);
    }
    pub fn more(&mut self, cx: &mut Context<Self>) {
        launch(self.model.more(), cx);
        self.sync(self.model.snapshot(), cx);
        cx.notify();
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.model.refresh().is_ok() {
            self.more(cx);
        }
    }
    pub fn reveal_message(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(message) = index
            .checked_sub(self.snapshot.first)
            .and_then(|index| self.snapshot.messages.get(index))
        {
            let id = format!("account-{}-{}", self.snapshot.epoch, message.id);
            self.transcript
                .update(cx, |view, cx| view.reveal_account_message(&id, cx));
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
        self.sync(self.model.snapshot(), cx);
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
        self.cloud_turns.clear();
        self.model.clear();
    }
}
impl Render for SavedReader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(self.model.snapshot(), cx);
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
                ),
        );
        if !self.model.current() {
            return body.child(div().p(px(24.)).child("Sign in to open this chat."));
        }
        body = body.child(div().flex_1().min_h_0().child(self.transcript.clone()));
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
                .child("The messages on screen will be replaced by earlier messages.")
                .child(self.button(
                    "saved-reader-continue",
                    "Continue",
                    Self::continue_window,
                    cx,
                ));
        } else if self.snapshot.has_more && self.snapshot.state == LoadState::Ready {
            footer = footer.child(self.button(
                "saved-reader-more",
                "Load earlier messages",
                Self::more,
                cx,
            ));
        }
        let file_count = self
            .snapshot
            .messages
            .iter()
            .map(|message| message.files.len())
            .sum::<usize>()
            + self.snapshot.captures.len();
        if file_count > 0 {
            footer = footer.child(self.button(
                "chat-files",
                if self.show_files {
                    "Hide files"
                } else {
                    "Files"
                },
                |this, cx| {
                    this.show_files = !this.show_files;
                    cx.notify();
                },
                cx,
            ));
        }
        if self.show_files {
            let mut files = div()
                .id("chat-file-list")
                .max_h(px(160.))
                .overflow_y_scroll()
                .flex()
                .flex_col();
            for message in self.snapshot.messages.as_ref().clone() {
                for file in message.files {
                    if let Some(reference) = file.download_ref().cloned() {
                        let name = file.name.clone();
                        files = files.child(self.button(
                            SharedString::from(format!("saved-file-{}-{}", message.id, file.id)),
                            short_label(&name),
                            move |this, cx| this.download(reference.clone(), name.clone(), cx),
                            cx,
                        ));
                    }
                }
            }
            for capture in self.snapshot.captures.clone() {
                let name = capture.name.clone();
                let label = short_label(&name);
                if let Some(reference) = capture.download_ref().cloned() {
                    files = files.child(self.button(
                        SharedString::from(format!("saved-capture-{}", capture.id)),
                        label,
                        move |this, cx| this.download(reference.clone(), name.clone(), cx),
                        cx,
                    ));
                } else {
                    files = files.child(SharedString::from(format!(
                        "{label} · {}",
                        if capture.state == openagents_account::openagents::CaptureState::NotSaved {
                            "Not saved"
                        } else {
                            "Unavailable"
                        }
                    )));
                }
            }
            footer = footer.child(files);
        }
        if self
            .snapshot
            .summary
            .as_ref()
            .is_some_and(|s| s.links.continued_by.is_some())
        {
            footer = footer.child("This chat continues in another conversation.");
        }
        body = body.child(footer).child(self.composer.clone());
        if let Some(parts) = self.part_controls(cx) {
            body = body.child(parts);
        }
        if let Some(choices) = self.home_control(cx) {
            body = body.child(choices);
        }
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
