//! GPUI mounting of the same Rust Native Home cards the phone uses.
use crate::{
    composer::ComposerInput, openagents_account::AppAccount, state::AppState, theme::Theme,
};
use coder_account::dashboard::{Effect, Home};
use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, Render, SharedString, Subscription,
    Task, Window, div, prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_dashboard::view::Action;
use openagents_dashboard::{Computer, Work};
use rust_native::{Activation, Axis, Element, Node, View};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub enum HomeEvent {
    Open(String),
}
pub struct HomePage {
    state: Entity<AppState>,
    account: Option<Entity<AppAccount>>,
    model: Option<Arc<Home>>,
    generation: Option<u64>,
    fields: BTreeMap<String, Entity<ComposerInput>>,
    field_screen: String,
    submission: u64,
    notice: Option<String>,
    watch: Option<Task<()>>,
    _state_watch: Subscription,
    _account_watch: Option<Subscription>,
}
impl EventEmitter<HomeEvent> for HomePage {}
impl HomePage {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let account = AppAccount::global(cx);
        let account_watch = account
            .as_ref()
            .map(|a| cx.observe(a, |_, _, cx| cx.notify()));
        let state_watch = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            account,
            model: None,
            generation: None,
            fields: BTreeMap::new(),
            field_screen: String::new(),
            submission: 0,
            notice: None,
            watch: None,
            _state_watch: state_watch,
            _account_watch: account_watch,
        }
    }
    pub fn set_visible(&self, visible: bool) {
        if let Some(m) = &self.model {
            m.set_visible(visible, true);
        }
    }
    fn prepare(&mut self, cx: &mut Context<Self>) {
        let lease = self
            .account
            .as_ref()
            .and_then(|a| a.read(cx).controller.as_ref())
            .and_then(|c| c.lease());
        let generation = lease.as_ref().map(|l| l.generation);
        if self.generation != generation || self.model.is_none() && lease.is_some() {
            if let Some(m) = self.model.take() {
                m.shutdown();
            }
            self.watch = None;
            self.fields.clear();
            self.generation = generation;
            if let (Some(lease), Some(root)) = (lease, self.state.read(cx).data_dir.clone()) {
                let scope = coder_account::desktop::sync_scope(
                    lease.client().deployment(),
                    "home",
                    &lease.session().account().id,
                );
                let model = Home::new(
                    Some(lease),
                    root.join("home").join(scope).join("dashboard.json"),
                );
                let mut updates = model.watch();
                self.watch = Some(cx.spawn(async move |this, cx| {
                    while updates.changed().await.is_ok() {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
                let start = model.clone();
                let task = Tokio::spawn(cx, async move {
                    Home::start(&start);
                    start.set_visible(true, true);
                    start.refresh(true).await;
                });
                cx.spawn(async move |_, _| {
                    let _ = task.await;
                })
                .detach();
                self.model = Some(model);
            }
        }
        let Some(model) = &self.model else { return };
        let state = self.state.read(cx);
        let now = chrono::Utc::now();
        model.set_local_projects(
            state
                .spaces
                .iter()
                .map(|p| openagents_dashboard::Project {
                    id: p.id.clone(),
                    name: p.name.clone().unwrap_or_else(|| "Project".into()),
                    repositories: vec![],
                })
                .collect(),
        );
        let work = state
            .chats
            .iter()
            .filter(|c| !c.archived)
            .map(|chat| {
                let session = state.session_for(&chat.id);
                let device = state.devices.iter().find(|d| d.id == chat.device_id);
                let online = device
                    .is_some_and(|d| crate::settings::devices::device_online(d.last_seen_at, now));
                let status = match session.map(|s| s.status) {
                    Some(zeron_proto::SessionStatus::Working) => "working",
                    Some(zeron_proto::SessionStatus::AwaitingInput) => "awaiting",
                    Some(zeron_proto::SessionStatus::Errored) => "failed",
                    Some(zeron_proto::SessionStatus::Idle) => {
                        if session.is_some_and(|s| s.last_completed_turn.is_some()) {
                            "completed"
                        } else {
                            "idle"
                        }
                    }
                    None => "unknown",
                };
                Work {
                    id: chat.id.clone(),
                    session: Some(chat.id.clone()),
                    destination: Some(openagents_dashboard::WorkTarget::Session {
                        id: chat.id.clone(),
                    }),
                    parent: chat.parent_chat_id.clone(),
                    agent: chat
                        .config
                        .as_ref()
                        .map(|c| {
                            match c.harness {
                                zeron_proto::HarnessId::ClaudeCode => "Claude Code",
                                zeron_proto::HarnessId::Codex => "Codex",
                                zeron_proto::HarnessId::Cursor => "Cursor",
                                zeron_proto::HarnessId::Devin => "Devin",
                                zeron_proto::HarnessId::Grok => "Grok",
                                zeron_proto::HarnessId::Hermes => "Hermes",
                                zeron_proto::HarnessId::Pi => "Pi",
                                zeron_proto::HarnessId::Opencode => "OpenCode",
                                zeron_proto::HarnessId::Antigravity => "Antigravity",
                                zeron_proto::HarnessId::Mock => "Test agent",
                                zeron_proto::HarnessId::Sdk => "Custom agent",
                            }
                            .to_owned()
                        })
                        .unwrap_or_else(|| "Agent".into()),
                    title: chat.title.clone().unwrap_or_else(|| "New session".into()),
                    project: chat.space_id.clone(),
                    host: device
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "Computer".into()),
                    state: status.into(),
                    online: online
                        && session.is_some_and(|s| {
                            now.signed_duration_since(s.updated_at).num_seconds() < 45
                        }),
                    observed_at: session
                        .map(|s| s.updated_at)
                        .unwrap_or(chat.created_at)
                        .timestamp()
                        .max(0) as u64,
                }
            })
            .collect();
        let computers = state
            .devices
            .iter()
            .map(|d| Computer {
                name: d.name.clone(),
                online: crate::settings::devices::device_online(d.last_seen_at, now),
            })
            .collect();
        model.set_live(work, computers);
    }
    fn mount(
        &mut self,
        node: Node<Action>,
        view: &View<Action>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = Theme::of(cx).clone();
        let key = node.key.clone();
        match node.element {
            Element::Text { value, role } => div()
                .text_color(if role == rust_native::TextRole::Heading {
                    theme.text
                } else {
                    theme.text_muted
                })
                .when_some(node.style.text_size, |d, size| d.text_size(px(size as f32)))
                .when(role == rust_native::TextRole::Heading, |d| {
                    d.font_weight(gpui::FontWeight::SEMIBOLD)
                })
                .child(value)
                .into_any_element(),
            Element::Button { label, enabled, .. } => {
                let activation = Activation {
                    instance: view.instance.clone(),
                    revision: view.revision,
                    node: key.clone(),
                };
                let (title, detail) = label.split_once('\n').unwrap_or((&label, ""));
                let content = div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().child(title.to_owned()))
                    .when(!detail.is_empty(), |d| {
                        d.child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child(detail.to_owned()),
                        )
                    });
                div()
                    .id(SharedString::from(key))
                    .min_h(px(44.))
                    .p(px(8.))
                    .rounded_md()
                    .text_color(theme.text)
                    .hover(|d| d.bg(theme.surface))
                    .tab_index(0)
                    .role(gpui::Role::Button)
                    .when(enabled, |d| {
                        let keyboard = activation.clone();
                        d.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.activate(activation.clone(), cx)
                            }))
                            .on_key_down(cx.listener(
                                move |this, event: &gpui::KeyDownEvent, _, cx| {
                                    if event.keystroke.key == "enter"
                                        || event.keystroke.key == "space"
                                    {
                                        cx.stop_propagation();
                                        this.activate(keyboard.clone(), cx);
                                    }
                                },
                            ))
                    })
                    .child(content)
                    .into_any_element()
            }
            Element::Field { label, value, .. } => {
                let input = self
                    .fields
                    .entry(key)
                    .or_insert_with(|| {
                        cx.new(|cx| {
                            let mut input = ComposerInput::new(&label, cx);
                            input.set_text(value, cx);
                            input
                        })
                    })
                    .clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_color(theme.text_muted).child(label))
                    .child(input)
                    .into_any_element()
            }
            Element::Stack { axis, children } => {
                let mut out = div().flex().flex_col().gap(px(12.)).w_full();
                if matches!(node.style.radius, Some(_)) {
                    out = out
                        .p(px(20.))
                        .bg(theme.surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border);
                }
                if matches!(node.style.radius, None)
                    && matches!(axis, Axis::Horizontal | Axis::Wrap)
                {
                    out = out.flex_row().flex_wrap();
                }
                for child in children {
                    let mounted = self.mount(child, view, cx);
                    out = if axis == Axis::Horizontal {
                        out.child(div().flex_1().min_w(px(180.)).child(mounted))
                    } else {
                        out.child(mounted)
                    };
                }
                out.into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
    fn activate(&mut self, activation: Activation, cx: &mut Context<Self>) {
        let Some(model) = self.model.clone() else {
            return;
        };
        let fields = self
            .fields
            .iter()
            .map(|(key, input)| (key.clone(), input.read(cx).text().to_owned()))
            .collect::<BTreeMap<String, String>>();
        let submitted = fields.clone();
        self.submission = self.submission.wrapping_add(1);
        let ticket = self.submission;
        let task = Tokio::spawn(
            cx,
            async move { model.activate_form(activation, fields).await },
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(effect)) => {
                        if this.submission == ticket {
                            this.fields.retain(|key, input| {
                                submitted
                                    .get(key)
                                    .is_none_or(|value| input.read(cx).text() != value.as_str())
                            });
                        }
                        this.effect(effect, cx);
                    }
                    Ok(Err(e)) => this.notice = Some(e),
                    Err(_) => this.notice = Some("Couldn't update Home.".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn effect(&mut self, effect: Effect, cx: &mut Context<Self>) {
        match effect {
            Effect::Session { id } => cx.emit(HomeEvent::Open(id)),
            Effect::Url { url } => cx.open_url(&url),
            Effect::Export { name, contents } => {
                let root = self.state.read(cx).data_dir.clone().unwrap_or_default();
                let prompt = cx.prompt_for_new_path(&root, Some(&name));
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(path))) = prompt.await {
                        let result = std::fs::write(path, contents);
                        let _ = this.update(cx, |this, cx| {
                            if result.is_err() {
                                this.notice = Some("Couldn't export this layout.".into());
                            }
                            cx.notify();
                        });
                    }
                })
                .detach();
            }
            Effect::Import => {
                let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("Choose layout".into()),
                });
                let model = self.model.clone();
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = prompt.await {
                        if let (Some(path), Some(model)) = (paths.first(), model) {
                            let bytes = std::fs::metadata(path)
                                .ok()
                                .filter(|m| m.len() <= 256 * 1024)
                                .and_then(|_| std::fs::read(path).ok());
                            if let Some(bytes) = bytes {
                                let task = cx.update(|cx| {
                                    Tokio::spawn(cx, async move { model.import(&bytes).await })
                                });
                                {
                                    let result = task.await;
                                    let _ = this.update(cx, |this, cx| {
                                        if let Ok(Err(e)) = result {
                                            this.notice = Some(e);
                                        }
                                        cx.notify();
                                    });
                                }
                            }
                        }
                    }
                })
                .detach();
            }
            Effect::None => {}
        }
    }
}
impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare(cx);
        let mut out = div()
            .id("home-page")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.));
        if let Some(notice) = &self.notice {
            out = out.child(notice.clone());
        }
        if let Some(model) = &self.model {
            let view = model.view();
            let signature = view.root.key.clone();
            if self.field_screen != signature {
                self.field_screen = signature;
                self.fields.clear();
            }
            out = out.child(self.mount(view.root.clone(), &view, cx));
        } else {
            out = out.child("Sign in to open Home.");
        }
        out
    }
}
