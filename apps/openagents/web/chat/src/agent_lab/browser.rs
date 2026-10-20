//! GPUI mounting and cookie-authenticated I/O for the shared Lab workbench.
use super::*;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, KeyDownEvent, Render, Role, SharedString, Task,
    Window, div, prelude::*, px,
};
use openagents_chat_view::rust_native_gpui;
use openagents_web_chat_view::{
    composer::{Edited, TextInput},
    theme,
};
use openagents_web_client::{Refusal, web::Client};
use rust_native::{Element, Node};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, time::Duration};

#[derive(Clone)]
pub struct OpenTask {
    pub chat: String,
    pub device: String,
}

struct Input {
    entity: Entity<TextInput>,
    value: String,
}

pub struct AgentLab {
    state: State,
    client: Client,
    tab: Area,
    inputs: HashMap<String, Input>,
    task: Option<Task<()>>,
    _poll: Task<()>,
    loaded: bool,
}
impl EventEmitter<OpenTask> for AgentLab {}

fn encode(value: &str) -> String {
    js_sys::encode_uri_component(value).into()
}

fn request_id() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

async fn pages<T: DeserializeOwned>(client: &Client, path: &str) -> anyhow::Result<Vec<T>> {
    let mut data = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let url = match after {
            Some(ref id) => format!("{path}?after={}", encode(id)),
            None => path.to_owned(),
        };
        let page: Page<T> = client.get(&url).await?;
        data.extend(page.data);
        after = page.next;
        if after.is_none() {
            return Ok(data);
        }
    }
}

async fn refresh(client: &Client, state: &mut State) -> anyhow::Result<()> {
    let s = &mut state.workbench;
    let mut documents = pages(client, &format!("{API}/documents")).await?;
    if let Some(id) = &s.document {
        let full: Document = client
            .get(&format!("{API}/documents/{}", encode(id)))
            .await?;
        documents.retain(|d: &Document| &d.id != id);
        documents.push(full);
    }
    let runs = pages(client, &format!("{API}/jobs")).await?;
    let hosts = client.get(&format!("{API}/hosts")).await?;
    let agents = pages(client, "/v1/agent-definitions").await?;
    let mut published = vec![];
    for (id, _) in agent_lab::templates::offered() {
        if let Ok(p) = client
            .get(&format!("/v1/published-agents/{}", encode(&id)))
            .await
        {
            published.push(p);
        }
    }
    s.documents = documents;
    s.runs = runs;
    s.hosts = hosts;
    s.agents = agents;
    s.published = published;
    if s.draft.host == "choose" {
        if let Some(host) = s.hosts.first() {
            s.draft.host = host.id.clone();
            s.draft.project = host
                .projects
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| "choose".into());
        }
    }
    s.bump();
    Ok(())
}

impl AgentLab {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                let Ok(Some((client, revision))) = this.read_with(cx, |this, _| {
                    (this.loaded && !this.state.workbench.busy && this.state.pending.is_none())
                        .then(|| (this.client.clone(), this.state.workbench.revision))
                }) else {
                    continue;
                };
                let runs = pages::<Run>(&client, &format!("{API}/jobs")).await;
                if this
                    .update(cx, |this, cx| {
                        if !this.state.workbench.busy
                            && this.state.pending.is_none()
                            && this.state.workbench.revision == revision
                        {
                            match runs {
                                Ok(runs) => {
                                    this.state.workbench.runs = runs;
                                    this.state.workbench.bump();
                                }
                                Err(_) => {
                                    this.state.workbench.notice =
                                        Some("Couldn't update your results. Try Refresh.".into());
                                }
                            }
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            state: State::new(uuid::Uuid::new_v4().to_string()),
            client: Client::from_page(),
            tab: Area::Agent,
            inputs: HashMap::new(),
            task: None,
            _poll: poll,
            loaded: false,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        if !self.loaded && !self.state.workbench.busy {
            self.perform(Command::Refresh, cx);
        }
    }

    fn activate(&mut self, event: Activation, cx: &mut Context<Self>) {
        match self.state.activate(&event) {
            Ok(Some(Command::Session(chat))) => {
                let device = self
                    .state
                    .workbench
                    .runs
                    .iter()
                    .find(|r| r.trials.iter().any(|t| t.chat.as_deref() == Some(&chat)))
                    .map(|r| r.experiment.host.clone());
                if let Some(device) = device {
                    cx.emit(OpenTask { chat, device });
                }
            }
            Ok(Some(command)) => self.perform(command, cx),
            Ok(None) => {
                self.inputs.clear();
                cx.notify();
            }
            Err(message) => {
                self.state.workbench.notice = Some(message);
                cx.notify();
            }
        }
    }

    fn perform(&mut self, command: Command, cx: &mut Context<Self>) {
        if self.state.workbench.busy {
            return;
        }
        let write = !matches!(command, Command::Refresh | Command::Load(_));
        if write {
            if let Err(message) = self.state.prepare(command.clone(), request_id()) {
                self.state.workbench.notice = Some(message);
                cx.notify();
                return;
            }
        }
        self.state.workbench.busy = true;
        self.state.workbench.notice = None;
        self.state.workbench.bump();
        let mut state = self.state.clone();
        let client = self.client.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let result: anyhow::Result<()> = async {
                match &command {
                    Command::Refresh => refresh(&client, &mut state).await?,
                    Command::Load(id) => {
                        let d: Document = client
                            .get(&format!("{API}/documents/{}", encode(id)))
                            .await?;
                        let v = d.versions.last().ok_or_else(|| {
                            anyhow::anyhow!("This experiment has no saved version.")
                        })?;
                        let Content::Experiment { experiment } =
                            serde_json::from_value(v.content.clone())?
                        else {
                            anyhow::bail!("Couldn't open this experiment.");
                        };
                        let s = &mut state.workbench;
                        s.draft = experiment;
                        s.document = Some(d.id.clone());
                        s.base = v.number;
                        s.dirty = false;
                        s.inputs.clear();
                        s.documents.retain(|old| old.id != d.id);
                        s.documents.push(d);
                    }
                    _ => {
                        let op = state
                            .pending
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("There is no saved change."))?;
                        let value = client
                            .send::<Value>(op.method, &op.path, Some(&op.body))
                            .await?;
                        state.complete(value).map_err(anyhow::Error::msg)?;
                    }
                }
                Ok(())
            }
            .await;
            this.update(cx, |this, cx| {
                if let Err(error) = result {
                    // A definitive refusal discards the request. Network loss keeps the exact body for Retry.
                    let refusal = error.downcast_ref::<Refusal>();
                    if refusal.is_some_and(|r| {
                        matches!(
                            r.code.as_str(),
                            "http_400"
                                | "http_401"
                                | "http_403"
                                | "http_404"
                                | "forbidden"
                                | "not_found"
                                | "invalid_lab"
                                | "version_conflict"
                                | "invalid_request"
                                | "quota_exhausted"
                        )
                    }) {
                        state.pending = None;
                        state.workbench.pending = false;
                    }
                    state.workbench.notice = Some(
                        refusal
                            .map(|r| r.message.clone())
                            .unwrap_or_else(|| "Couldn't complete this action. Try again.".into()),
                    );
                } else {
                    this.loaded = true;
                    if matches!(command, Command::Start) {
                        this.tab = Area::Results;
                    }
                    if matches!(command, Command::Load(_) | Command::Fork(_)) {
                        this.tab = Area::Agent;
                    }
                }
                state.workbench.busy = false;
                state.workbench.bump();
                this.state = state;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn input(
        &mut self,
        node: &Node<agent_lab::view::Action>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Element::Field {
            label,
            value,
            multiline,
            enabled,
            secret,
            ..
        } = &node.element
        else {
            return None;
        };
        if *secret {
            return None;
        }
        if !self.inputs.contains_key(&node.key) {
            let initial = value.clone();
            let entity = cx.new(|cx| {
                let mut input = if *multiline {
                    TextInput::multiline(label.clone(), cx)
                } else {
                    TextInput::new(label.clone(), cx)
                };
                input.set_send_with_modifier(true);
                input.set_text(initial, cx);
                input
            });
            let key = node.key.clone();
            cx.subscribe(&entity, move |this, input, _: &Edited, cx| {
                let value = input.read(cx).text();
                let event = FieldChange {
                    activation: Activation {
                        instance: this.state.workbench.instance.clone(),
                        revision: this.state.workbench.revision,
                        node: key.clone(),
                    },
                    value,
                };
                if let Err(message) = this.state.edit(event) {
                    this.state.workbench.notice = Some(message);
                    if let Some(field) = this.inputs.get(&key) {
                        input.update(cx, |input, cx| input.set_text(field.value.clone(), cx));
                    }
                }
                cx.notify();
            })
            .detach();
            self.inputs.insert(
                node.key.clone(),
                Input {
                    entity,
                    value: value.clone(),
                },
            );
        }
        let input = self.inputs.get_mut(&node.key).unwrap();
        if input.value != *value {
            input.value = value.clone();
            if input.entity.read(cx).text() != *value {
                input
                    .entity
                    .update(cx, |input, cx| input.set_text(value.clone(), cx));
            }
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .w_full()
                .child(div().text_color(theme::text_muted()).child(label.clone()))
                .child(
                    div()
                        .p(px(10.))
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::input_bg())
                        .when(*enabled, |d| d.child(input.entity.clone()))
                        .when(!*enabled, |d| d.opacity(0.5).child(value.clone())),
                )
                .into_any_element(),
        )
    }

    fn mount(
        &mut self,
        node: &Node<agent_lab::view::Action>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(field) = self.input(node, cx) {
            return field;
        }
        rust_native_gpui::mount_root(
            node,
            &self.state.workbench.instance,
            self.state.workbench.revision,
            |this: &mut Self, event, cx| this.activate(event, cx),
            cx,
        )
    }
}

impl Render for AgentLab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nodes = self
            .state
            .workbench
            .view()
            .validate()
            .ok()
            .and_then(|v| match &v.view().root.element {
                Element::Stack { children, .. } => Some(children.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let mut library = div()
            .id("agent-lab-library")
            .w(px(210.))
            .flex_none()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.));
        let mut content = div()
            .id("agent-lab-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.));
        let mut footer = div().flex().flex_wrap().gap(px(12.)).p(px(16.));
        for node in &nodes {
            let section = area(&node.key);
            if section == Area::Library {
                library = library.child(self.mount(node, cx));
            } else if section == Area::Actions || section == Area::Notice {
                footer = footer.child(self.mount(node, cx));
            } else if section == self.tab {
                content = content.child(self.mount(node, cx));
            }
        }
        if self.tab == Area::Results && self.state.workbench.runs.is_empty() {
            content = content.child("Run your tests to see results here.");
        }
        let mut tabs = div().flex().gap(px(8.)).p(px(12.));
        for (tab, label) in [
            (Area::Agent, "Agent"),
            (Area::Tests, "Tests"),
            (Area::Results, "Results"),
        ] {
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("agent-lab-tab-{label}")))
                    .role(Role::Tab)
                    .aria_label(label)
                    .aria_selected(self.tab == tab)
                    .tab_index(0)
                    .cursor_pointer()
                    .p(px(10.))
                    .when(self.tab == tab, |d| d.bg(theme::element_active()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.tab = tab;
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }))
                    .child(label),
            );
        }
        div()
            .size_full()
            .flex()
            .text_color(theme::text())
            .text_size(px(13.))
            .child(library)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(tabs)
                    .child(content)
                    .child(footer),
            )
    }
}
