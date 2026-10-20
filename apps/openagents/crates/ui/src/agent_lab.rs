//! Desktop mounting of the shared Agent Lab workbench and account controller.
use crate::{
    composer::{ComposerInput, ComposerInputEvent},
    theme::Theme,
};
use agent_graph::edit::{Edit, Op};
use agent_lab::view::Action;
use gpui::{
    AnyElement, App, AppContext, Context, Empty, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, KeyDownEvent, Render, Role, SharedString, Subscription, Task, Window, div,
    prelude::*, px,
};
use gpui_tokio::Tokio;
use openagents_account::{
    Controller,
    agent_lab::{Effect, Lab},
};
use openagents_agent_graph_view::{GraphEvent, GraphView};
use rust_native::{Activation, Axis, Element, FieldChange, Node, ValidatedView};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Agent,
    Graph,
    Tests,
    Results,
}
impl Tab {
    fn label(self) -> &'static str {
        match self {
            Self::Agent => "Agent",
            Self::Graph => "Graph",
            Self::Tests => "Tests",
            Self::Results => "Results",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Area {
    Library,
    Agent,
    Tests,
    Results,
    Actions,
    Notice,
}
/// Presentation grouping only. All controls and their enabled states come from Rust Native.
fn area(key: &str) -> Area {
    if matches!(key, "heading" | "refresh" | "new")
        || ["template:", "open:", "agent:", "publish:", "run-on:"]
            .iter()
            .any(|p| key.starts_with(p))
    {
        Area::Library
    } else if matches!(key, "save" | "start" | "promote" | "retry") {
        Area::Actions
    } else if key == "notice" {
        Area::Notice
    } else if [
        "version:", "run:", "counts:", "stop:", "archive:", "review", "result:", "answer:",
        "session:",
    ]
    .iter()
    .any(|p| key.starts_with(p))
        || matches!(key, "cost" | "comparison" | "project-files-report")
    {
        Area::Results
    } else if [
        "check",
        "case-title:",
        "task:",
        "expected:",
        "pointer:",
        "remove:",
        "repeat",
        "compare:",
    ]
    .iter()
    .any(|p| key.starts_with(p))
        || matches!(
            key,
            "add-case"
                | "seconds"
                | "partition"
                | "development"
                | "confirmation"
                | "comparison-choice"
                | "compare-none"
                | "planned"
        )
    {
        Area::Tests
    } else {
        Area::Agent
    }
}
struct Input {
    entity: Entity<ComposerInput>,
    value: String,
    _events: Subscription,
}
pub struct AgentLab {
    controller: Arc<Controller>,
    /// The computer's key, which signs a published agent's owner tag.
    owner_key: Option<openagents_account::public::KeySource>,
    root: PathBuf,
    account: Option<String>,
    lab: Option<Arc<Lab>>,
    view: Option<ValidatedView<Action>>,
    inputs: HashMap<String, Input>,
    tab: Tab,
    error: Option<String>,
    busy: bool,
    focus: FocusHandle,
    _auth: Task<()>,
    watch: Option<Task<()>>,
    poll: Option<Task<()>>,
    operation: Option<Task<()>>,
    /// The draft as AgentGraph (#12231): made when the Graph tab first
    /// opens, then kept in step with the draft the form edits.
    graph: Option<(Entity<GraphView>, Subscription)>,
    /// A draft to draw without an account, for previews.
    #[cfg(feature = "account-fixture")]
    fixture_draft: Option<::agent_lab::Experiment>,
}
#[derive(Clone)]
pub enum Event {
    Session(String),
}
impl EventEmitter<Event> for AgentLab {}

impl AgentLab {
    pub fn new(controller: Arc<Controller>, root: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut auth = controller.watch();
        let auth_task = cx.spawn(async move |this, cx| {
            while auth.changed().await.is_ok() {
                if this
                    .update(cx, |this, cx| this.account_changed(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            controller,
            owner_key: None,
            root,
            account: None,
            lab: None,
            view: None,
            inputs: HashMap::new(),
            tab: Tab::Agent,
            error: None,
            busy: false,
            focus: cx.focus_handle(),
            _auth: auth_task,
            watch: None,
            poll: None,
            operation: None,
            graph: None,
            #[cfg(feature = "account-fixture")]
            fixture_draft: None,
        };
        this.account_changed(cx);
        this
    }
    /// Offer Publish, with `key` signing each published agent's owner tag
    /// (#12301).
    pub fn with_owner_key(mut self, key: Option<openagents_account::public::KeySource>) -> Self {
        if let (Some(lab), Some(key)) = (&self.lab, &key) {
            lab.set_owner_key(key.clone());
        }
        self.owner_key = key;
        self
    }
    fn account_changed(&mut self, cx: &mut Context<Self>) {
        let account = self
            .controller
            .lease()
            .map(|l| l.session().account().id.clone());
        if account == self.account {
            return;
        }
        self.account = account;
        self.operation = None;
        self.poll = None;
        self.watch = None;
        self.lab = None;
        self.view = None;
        self.inputs.clear();
        self.error = None;
        self.busy = false;
        if self.account.is_some() {
            let lab = Lab::new(self.controller.clone(), self.root.clone());
            if let Some(key) = &self.owner_key {
                lab.set_owner_key(key.clone());
            }
            let mut updates = lab.watch();
            self.lab = Some(lab.clone());
            self.watch = Some(cx.spawn(async move |this, cx| {
                while updates.changed().await.is_ok() {
                    if this
                        .update(cx, |this, cx| {
                            this.read_view();
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
            self.poll = Some(cx.spawn(async move |this, cx| {
                loop {
                    let lab = match this.update(
                        cx,
                        |this, _| {
                            if this.busy { None } else { this.lab.clone() }
                        },
                    ) {
                        Ok(lab) => lab,
                        Err(_) => break,
                    };
                    if let Some(lab) = lab {
                        let result = Tokio::spawn(cx, async move { lab.refresh().await }).await;
                        if this
                            .update(cx, |this, cx| {
                                match result {
                                    Ok(Err(message)) => this.error = Some(message),
                                    Ok(Ok(())) => this.error = None,
                                    Err(_) => {
                                        this.error = Some(
                                            "Couldn't load your experiments. Try again.".into(),
                                        )
                                    }
                                }
                                this.read_view();
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    cx.background_executor().timer(Duration::from_secs(5)).await;
                }
            }));
            self.read_view();
        }
        cx.notify();
    }
    fn read_view(&mut self) {
        self.view = self.lab.as_ref().and_then(|l| l.view().validate().ok());
    }
    /// The draft the form shows, as a graph and what the graph may change.
    fn draft_graph(&self) -> Option<(agent_graph::GraphSpec, agent_graph::edit::Editable)> {
        #[cfg(feature = "account-fixture")]
        let fixture = self.fixture_draft.clone();
        #[cfg(not(feature = "account-fixture"))]
        let fixture: Option<::agent_lab::Experiment> = None;
        let draft = &match &self.lab {
            Some(lab) => lab.snapshot().draft,
            None => fixture?,
        };
        let id = draft.title.to_lowercase().replace(' ', "-");
        Some((
            agent_graph::lab::of(draft, &id, &Default::default()),
            agent_graph::edit::editable(draft),
        ))
    }
    /// Keeps the graph on the form's draft.
    fn sync_graph(&mut self, cx: &mut Context<Self>) {
        let Some((spec, editable)) = self.draft_graph() else {
            return;
        };
        let editable = (!self.busy).then_some(editable);
        if let Some((graph, _)) = &self.graph {
            graph.update(cx, |g, cx| g.set_graph(spec, editable, cx));
            return;
        }
        let graph = cx.new(|_| {
            let view = GraphView::new(spec, false, vec![]);
            match editable {
                Some(e) => view.with_editable(e),
                None => view,
            }
        });
        let events = cx.subscribe(&graph, |this: &mut Self, _, event: &GraphEvent, cx| {
            let GraphEvent::Edit(edit) = event;
            this.graph_edit(edit.clone(), cx);
        });
        self.graph = Some((graph, events));
    }
    /// Makes a change asked for on the graph with the form's own steps, so
    /// it goes through the same store and checks as typing in the form.
    fn graph_edit(&mut self, edit: Edit, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(lab) = self.lab.clone() else {
            return;
        };
        let ops = match agent_graph::edit::plan(&lab.snapshot().draft, &edit) {
            Ok(ops) => ops,
            Err(message) => {
                self.error = Some(message);
                cx.notify();
                return;
            }
        };
        self.error = None;
        for op in ops {
            self.read_view();
            match op {
                Op::Field { key, value } => {
                    let Some(activation) = self.activation(&key) else {
                        self.error = Some("This field changed. Try it again.".into());
                        break;
                    };
                    if let Err(message) = lab.change_field(FieldChange { activation, value }) {
                        self.error = Some(message);
                        break;
                    }
                }
                Op::Action { key } => {
                    if let Some(activation) = self.activation(&key) {
                        self.activate(activation, cx);
                    }
                }
            }
        }
        self.read_view();
        self.sync_graph(cx);
        cx.notify();
    }
    fn activation(&self, key: &str) -> Option<Activation> {
        self.view.as_ref().map(|v| Activation {
            instance: v.view().instance.clone(),
            revision: v.view().revision,
            node: key.into(),
        })
    }
    fn edit(&mut self, key: &str, value: String, cx: &mut Context<Self>) {
        let (Some(lab), Some(activation)) = (self.lab.clone(), self.activation(key)) else {
            return;
        };
        let Some(node) = find(&self.view.as_ref().unwrap().view().root, key) else {
            return;
        };
        if matches!(&node.element, Element::Field { value: old, .. } if old == &value) {
            return;
        }
        self.error = lab.change_field(FieldChange { activation, value }).err();
        self.read_view();
        if self.error.is_some() {
            let accepted = self
                .view
                .as_ref()
                .and_then(|v| find(&v.view().root, key))
                .and_then(|n| {
                    if let Element::Field { value, .. } = &n.element {
                        Some(value.clone())
                    } else {
                        None
                    }
                });
            if let (Some(value), Some(input)) = (accepted, self.inputs.get(key)) {
                input
                    .entity
                    .update(cx, |input, cx| input.set_text(value, cx));
            }
        }
        cx.notify();
    }
    fn activate(&mut self, event: Activation, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(lab) = self.lab.clone() else {
            return;
        };
        let is_start = event.node == "start";
        self.busy = true;
        self.error = None;
        let job = Tokio::spawn(cx, async move { lab.activate(event).await });
        self.operation = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(Effect::Session { id })) => cx.emit(Event::Session(id)),
                    Ok(Ok(Effect::None)) => {
                        if is_start {
                            this.tab = Tab::Results;
                        }
                    }
                    Ok(Err(message)) => this.error = Some(message),
                    Err(_) => this.error = Some("Couldn't update Agent Lab. Try again.".into()),
                }
                this.read_view();
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn sync_inputs(&mut self, nodes: &[Node<Action>], cx: &mut Context<Self>) {
        self.inputs.retain(|key, _| {
            nodes
                .iter()
                .any(|n| n.key == *key && matches!(n.element, Element::Field { .. }))
        });
        for node in nodes {
            let Element::Field {
                label,
                value,
                placeholder,
                multiline,
                secret,
                enabled,
                ..
            } = &node.element
            else {
                continue;
            };
            // Agent Lab has no secret fields. Never mount a secret as ordinary text.
            if *secret {
                continue;
            }
            if !self.inputs.contains_key(&node.key) {
                let entity = cx.new(|cx| {
                    let input = ComposerInput::with_context(
                        if placeholder.is_empty() {
                            label.clone()
                        } else {
                            placeholder.clone()
                        },
                        if *multiline {
                            "AgentLabInput"
                        } else {
                            "AgentLabSingleLine"
                        },
                        cx,
                    )
                    .with_text_metrics(13., 20.)
                    .with_tab_stop()
                    .with_accessibility_role(if *multiline {
                        Role::MultilineTextInput
                    } else {
                        Role::TextInput
                    });
                    let mut input = if *multiline {
                        input.with_viewport_height(140.)
                    } else {
                        input.with_single_line()
                    };
                    input.set_text(value.clone(), cx);
                    input
                });
                let key = node.key.clone();
                let events = cx.subscribe(&entity, move |this: &mut Self, input, event, cx| {
                    if matches!(event, ComposerInputEvent::Edited) {
                        this.edit(&key, input.read(cx).text().into(), cx);
                    }
                });
                self.inputs.insert(
                    node.key.clone(),
                    Input {
                        entity,
                        value: value.clone(),
                        _events: events,
                    },
                );
            }
            let field = self.inputs.get_mut(&node.key).unwrap();
            // An edit event may still be queued. Only a changed model value
            // replaces text, so a repaint cannot overwrite a pending edit.
            let changed = field.value != *value;
            field.value = value.clone();
            let input = field.entity.clone();
            let read_only = !*enabled || self.busy;
            if changed || input.read(cx).read_only != read_only {
                input.update(cx, |i, cx| {
                    i.read_only = read_only;
                    if changed && i.text() != value {
                        i.set_text(value.clone(), cx);
                    }
                    cx.notify();
                });
            }
        }
    }
    fn mount(&mut self, node: &Node<Action>, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::of(cx).clone();
        match &node.element {
            Element::Stack { axis, children } => div()
                .flex()
                .gap(px(10.))
                .when(*axis != Axis::Horizontal, |d| d.flex_col())
                .children(children.iter().map(|n| self.mount(n, cx)))
                .into_any_element(),
            Element::Text { value, .. } => div()
                .text_color(theme.text_muted)
                .text_size(crate::typography::ui_rems(13.))
                .child(value.clone())
                .into_any_element(),
            Element::Button { label, enabled, .. } => {
                let event = self.activation(&node.key).unwrap();
                let primary = node.key == "save" || node.key == "start";
                let selector = node.key.clone();
                div()
                    .id(SharedString::from(node.key.clone()))
                    .debug_selector(move || selector.clone())
                    .role(Role::Button)
                    .aria_label(label.clone())
                    .tab_index(0)
                    .border_1()
                    .border_color(theme.border.opacity(0.0))
                    .focus_visible(|s| s.border_color(theme.accent))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded_md()
                    .text_size(crate::typography::ui_rems(13.))
                    .text_color(if primary { theme.accent } else { theme.text })
                    .bg(if primary {
                        theme.accent.opacity(0.12)
                    } else {
                        theme.surface
                    })
                    .when(!*enabled || self.busy, |d| d.opacity(0.45))
                    .when(*enabled && !self.busy, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(theme.element_hover))
                            .on_click(cx.listener({
                                let event = event.clone();
                                move |this, _, _, cx| this.activate(event.clone(), cx)
                            }))
                            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                                if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                                    this.activate(event.clone(), cx);
                                    cx.stop_propagation();
                                }
                            }))
                    })
                    .child(label.clone())
                    .into_any_element()
            }
            Element::Field {
                label,
                value,
                multiline,
                enabled,
                max_bytes,
                secret,
                ..
            } => {
                let input = self.inputs.get(&node.key).map(|i| i.entity.clone());
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .w_full()
                    .child(div().text_color(theme.text_muted).child(label.clone()))
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded_md()
                            .bg(theme.input_bg)
                            .border_1()
                            .border_color(theme.border)
                            .when(*multiline, |d| d.h(px(156.)))
                            .when(!*multiline, |d| d.min_h(px(38.)))
                            .when(!*secret, |d| d.children(input))
                            .when(!*enabled || self.busy, |d| d.opacity(0.65)),
                    )
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(11.))
                            .text_color(theme.text_faint)
                            .child(format!("{} / {} bytes", value.len(), max_bytes)),
                    )
                    .into_any_element()
            }
            _ => Empty.into_any_element(),
        }
    }
}
fn find<'a>(node: &'a Node<Action>, key: &str) -> Option<&'a Node<Action>> {
    if node.key == key {
        return Some(node);
    }
    if let Element::Stack { children, .. } = &node.element {
        return children.iter().find_map(|n| find(n, key));
    }
    None
}
impl Focusable for AgentLab {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for AgentLab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.account_changed(cx);
        if self.lab.is_some() {
            self.read_view();
        }
        let theme = Theme::of(cx).clone();
        let nodes = self
            .view
            .as_ref()
            .and_then(|v| match &v.view().root.element {
                Element::Stack { children, .. } => Some(children.clone()),
                _ => None,
            })
            .unwrap_or_default();
        self.sync_inputs(&nodes, cx);
        let mut library = div()
            .id("agent-lab-library")
            .w(px(240.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .border_r_1()
            .border_color(theme.border)
            .child(div().text_color(theme.text_faint).child("Your experiments"));
        for n in nodes
            .iter()
            .filter(|n| area(&n.key) == Area::Library && n.key != "heading")
        {
            library = library.child(self.mount(n, cx));
        }
        let mut tabs = div().flex().gap(px(8.));
        for tab in [Tab::Agent, Tab::Graph, Tab::Tests, Tab::Results] {
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("agent-lab-tab-{}", tab.label())))
                    .debug_selector(move || format!("agent-lab-tab-{}", tab.label()))
                    .role(Role::Button)
                    .aria_label(tab.label())
                    .aria_selected(self.tab == tab)
                    .tab_index(0)
                    .border_1()
                    .border_color(theme.border.opacity(0.0))
                    .focus_visible(|s| s.border_color(theme.accent))
                    .cursor_pointer()
                    .px(px(16.))
                    .py(px(10.))
                    .rounded_md()
                    .when(self.tab == tab, |d| {
                        d.bg(theme.element_hover).text_color(theme.accent)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = tab;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                            this.tab = tab;
                            cx.notify();
                            cx.stop_propagation();
                        }
                    }))
                    .child(tab.label()),
            );
        }
        let chosen = match self.tab {
            Tab::Agent | Tab::Graph => Area::Agent,
            Tab::Tests => Area::Tests,
            Tab::Results => Area::Results,
        };
        if self.tab == Tab::Graph {
            self.sync_graph(cx);
        }
        let mut content = div()
            .id(SharedString::from(format!(
                "agent-lab-{}",
                self.tab.label()
            )))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(14.));
        let selected: Vec<_> = if self.tab == Tab::Graph {
            vec![]
        } else {
            nodes.iter().filter(|n| area(&n.key) == chosen).collect()
        };
        if self.tab == Tab::Graph {
            content = match &self.graph {
                Some((graph, _)) => content.p(px(0.)).overflow_hidden().child(graph.clone()),
                None => content.child("Connect your account to use Agent Lab."),
            };
        } else if selected.is_empty() {
            content = content.child(if self.lab.is_none() {
                "Connect your account to use Agent Lab."
            } else if self.tab == Tab::Results {
                "Run your tests to see results here."
            } else {
                "Loading your experiments…"
            });
        }
        let mut index = 0;
        while index < selected.len() {
            if matches!(selected[index].element, Element::Button { .. }) {
                let mut row = div().flex().flex_wrap().gap(px(8.));
                while index < selected.len()
                    && matches!(selected[index].element, Element::Button { .. })
                {
                    row = row.child(self.mount(selected[index], cx));
                    index += 1;
                }
                content = content.child(row);
            } else {
                content = content.child(self.mount(selected[index], cx));
                index += 1;
            }
        }
        let mut footer = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .px(px(24.))
            .py(px(16.))
            .border_t_1()
            .border_color(theme.border);
        for n in nodes.iter().filter(|n| area(&n.key) == Area::Actions) {
            footer = footer.child(self.mount(n, cx));
        }
        let mut notices = div().px(px(24.)).flex().flex_col().gap(px(6.));
        if let Some(error) = &self.error {
            notices = notices.child(div().text_color(theme.text_muted).child(error.clone()));
        }
        for n in nodes.iter().filter(|n| area(&n.key) == Area::Notice) {
            notices = notices.child(self.mount(n, cx));
        }
        div()
            .id("agent-lab")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                {
                    if modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .bg(theme.bg)
            .text_size(crate::typography::ui_rems(13.))
            .child(
                div()
                    .px(px(24.))
                    .py(px(20.))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(crate::typography::ui_rems(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Agent Lab"),
                    )
                    .child(
                        div().text_color(theme.text_muted).child(
                            "Build an agent. Try it on real work. Keep the version you trust.",
                        ),
                    ),
            )
            .child(
                div().flex_1().min_h_0().flex().child(library).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().px(px(24.)).py(px(12.)).child(tabs))
                        .child(notices)
                        .child(content)
                        .child(footer),
                ),
            )
    }
}

#[cfg(test)]
#[path = "agent_lab_tests.rs"]
mod tests;

#[cfg(feature = "account-fixture")]
impl AgentLab {
    /// Mount example documents without an account or execution host.
    pub fn fixture(
        controller: Arc<Controller>,
        root: PathBuf,
        workbench: ::agent_lab::view::Workbench,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new(controller, root, cx);
        this.fixture_draft = Some(workbench.draft.clone());
        this.view = Some(
            workbench
                .view()
                .validate()
                .expect("valid fixture workbench"),
        );
        this
    }
    /// Draws `draft` on the Graph tab instead of the fixture's own.
    pub fn fixture_graph(&mut self, draft: ::agent_lab::Experiment, cx: &mut Context<Self>) {
        self.fixture_draft = Some(draft);
        self.tab = Tab::Graph;
        cx.notify();
    }
    pub fn fixture_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = [Tab::Agent, Tab::Tests, Tab::Results, Tab::Graph][tab];
        cx.notify();
    }
}
