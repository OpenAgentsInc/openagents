//! Start a session on one of the account's computers (#12266), as the
//! desktop's new-session canvas does: pick the computer, its project and
//! branch, whether to run in the project's folder or a new worktree, the
//! harness, the model, and the reasoning. The chat's row goes to the
//! registry, its first message goes to the computer as a `QueueCommand`,
//! and the transcript streams back from the computer. The harness is fixed
//! once the chat exists: these pickers show only for a new chat.

use futures::StreamExt as _;
use gpui::{
    AnyElement, ClickEvent, Context, Focusable as _, IntoElement, ParentElement, SharedString,
    Styled, Task, Window, div, prelude::*, px,
};
use openagents_proto::{
    ChatConfig, HarnessId, Model, ReasoningLevel, RepoRef, SandboxLevel, WorktreeSpec,
};
use openagents_web_chat_view::svg;
use openagents_web_chat_view::{now_ms, theme};
use openagents_web_client::computer;
use openagents_web_client::workspace::{self, Harness, NewSession};
use serde_json::json;

use super::ChatApp;

/// Which picker is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Picker {
    Computer,
    Project,
    Branch,
    Harness,
    Model,
    Reasoning,
}

/// A new session's choices.
#[derive(Default)]
pub(super) struct Target {
    /// The computer; `None` is a chat with OpenAgents on the site.
    pub computer: Option<String>,
    /// The project (registry space) on it.
    pub project: Option<String>,
    pub refs: Vec<RepoRef>,
    pub branch: Option<String>,
    /// Run in a new worktree off the branch, not the project's folder.
    pub worktree: bool,
    pub harnesses: Vec<Harness>,
    pub harness: Option<HarnessId>,
    pub models: Vec<Model>,
    pub model: Option<String>,
    pub reasoning: Option<ReasoningLevel>,
    pub open: Option<Picker>,
    /// What's being read from the computer now.
    pub loading: Option<&'static str>,
    pub task: Option<Task<()>>,
}

fn reasoning_label(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Minimal => "Minimal",
        ReasoningLevel::Low => "Low",
        ReasoningLevel::Medium => "Medium",
        ReasoningLevel::High => "High",
        ReasoningLevel::XHigh => "Extra high",
        ReasoningLevel::Max => "Max",
        ReasoningLevel::Ultra => "Ultra",
        ReasoningLevel::Ultracode => "Ultracode",
        ReasoningLevel::Ultrathink => "Ultrathink",
    }
}

impl ChatApp {
    fn target_name(&self, device: &str) -> String {
        self.registry
            .as_ref()
            .and_then(|r| r.read(|r| r.devices().into_iter().find(|d| d.id == device)))
            .map(|d| d.name)
            .unwrap_or_else(|| "your computer".into())
    }

    /// Runs `call` on the chosen computer and hands its answer to `then`.
    fn target_read(
        &mut self,
        what: &'static str,
        method: &'static str,
        params: serde_json::Value,
        then: impl FnOnce(&mut Self, serde_json::Value, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(device) = self.target.computer.clone() else {
            return;
        };
        let name = self.target_name(&device);
        self.target.loading = Some(what);
        let client = self.client.clone();
        let (asked_tx, mut asked) = futures::channel::mpsc::unbounded::<()>();
        self.target.task = Some(cx.spawn(async move |this, cx| {
            let ask = async {
                asked.next().await?;
                Some(())
            };
            let call = async {
                let computer = computer::connect(&client, &device, move || {
                    let _ = asked_tx.unbounded_send(());
                })
                .await?;
                computer.call(method, params).await
            };
            futures::pin_mut!(call);
            let asked_first = futures::future::select(Box::pin(ask), &mut call).await;
            let answer = match asked_first {
                futures::future::Either::Left((None, call)) => call.await,
                futures::future::Either::Left((Some(()), call)) => {
                    this.update(cx, |this, cx| {
                        this.notice = Some(super::computers::allow_line(&name));
                        cx.notify();
                    })
                    .ok();
                    call.await
                }
                futures::future::Either::Right((answer, _)) => answer,
            };
            this.update(cx, |this, cx| {
                if this.target.computer.as_deref() != Some(device.as_str()) {
                    return;
                }
                this.target.loading = None;
                match answer {
                    Ok(value) => {
                        if this
                            .notice
                            .as_deref()
                            .is_some_and(|n| n.starts_with("Allow this browser"))
                        {
                            this.notice = None;
                        }
                        then(this, value, cx)
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Chooses where a new chat runs: a computer, or the site (`None`).
    pub(super) fn choose_computer(&mut self, device: Option<String>, cx: &mut Context<Self>) {
        let previous = self.target.harness;
        self.target = super::new_session::Target {
            computer: device.clone(),
            ..Default::default()
        };
        let Some(device) = device else {
            cx.notify();
            return;
        };
        // The project of this computer's newest chat, if any.
        self.target.project = self.registry.as_ref().and_then(|r| {
            r.read(|r| {
                r.rows(now_ms())
                    .into_iter()
                    .filter(|row| row.chat.device_id == device && row.chat.space_id.is_some())
                    .max_by_key(|row| row.updated_ms)
                    .and_then(|row| row.chat.space_id)
            })
        });
        self.target_read(
            "Reading this computer's agents…",
            computer::LIST_HARNESSES,
            json!({}),
            move |this, value, cx| {
                let harnesses: Vec<Harness> = serde_json::from_value(value).unwrap_or_default();
                let usable: Vec<Harness> = harnesses.into_iter().filter(Harness::usable).collect();
                let harness = usable
                    .iter()
                    .find(|h| Some(h.id) == previous)
                    .or(usable.first())
                    .map(|h| h.id);
                this.target.harnesses = usable;
                if this.target.harnesses.is_empty() {
                    this.notice = Some(
                        "Turn on an agent in the OpenAgents app on this computer first.".into(),
                    );
                }
                if let Some(harness) = harness {
                    this.choose_harness(harness, cx);
                }
            },
            cx,
        );
        self.load_refs(cx);
    }

    fn project_path(&self) -> Option<String> {
        let project = self.target.project.as_deref()?;
        self.registry.as_ref().and_then(|r| {
            r.read(|r| r.spaces().into_iter().find(|s| s.id == project))
                .filter(|s| s.git_detected)
                .map(|s| s.path)
        })
    }

    fn load_refs(&mut self, cx: &mut Context<Self>) {
        self.target.refs.clear();
        self.target.branch = None;
        let Some(path) = self.project_path() else {
            return;
        };
        let client = self.client.clone();
        let Some(device) = self.target.computer.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let refs = async {
                let computer = computer::connect(&client, &device, || {}).await?;
                computer
                    .call(computer::LIST_REFS, json!({"repoPath": path}))
                    .await
            }
            .await;
            this.update(cx, |this, cx| {
                if this.target.computer.as_deref() != Some(device.as_str()) {
                    return;
                }
                let refs: Vec<RepoRef> = refs
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                this.target.branch = refs.iter().find(|r| r.current).map(|r| r.name.clone());
                this.target.refs = refs;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn choose_project(&mut self, project: Option<String>, cx: &mut Context<Self>) {
        self.target.project = project;
        self.target.open = None;
        self.target.worktree = false;
        self.load_refs(cx);
        cx.notify();
    }

    fn choose_harness(&mut self, harness: HarnessId, cx: &mut Context<Self>) {
        self.target.harness = Some(harness);
        self.target.models.clear();
        self.target.model = None;
        self.target.reasoning = None;
        self.target.open = None;
        self.target_read(
            "Reading this agent's models…",
            computer::LIST_MODELS,
            json!({ "harness": harness }),
            move |this, value, _| {
                if this.target.harness != Some(harness) {
                    return;
                }
                this.target.models = serde_json::from_value(value).unwrap_or_default();
                this.target.model = this.target.models.first().map(|m| m.id.clone());
            },
            cx,
        );
    }

    /// Starts the new session with its first message.
    pub(super) fn start_session(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(device), Some(harness)) = (self.target.computer.clone(), self.target.harness)
        else {
            self.notice = Some("Choose an agent on this computer first.".into());
            cx.notify();
            return;
        };
        let Some(registry) = self.registry.clone() else {
            return;
        };
        let reference = self
            .target
            .branch
            .as_ref()
            .and_then(|b| self.target.refs.iter().find(|r| &r.name == b))
            .cloned();
        let path = self.project_path();
        let worktree = self.target.worktree && path.is_some() && reference.is_some();
        // A branch already checked out in a worktree runs there.
        let cwd = reference
            .as_ref()
            .filter(|_| !worktree)
            .and_then(|r| r.worktree_path.clone());
        let switch = reference
            .as_ref()
            .filter(|r| !worktree && !r.current && r.worktree_path.is_none())
            .cloned()
            .zip(path.clone());
        let new = NewSession {
            id: uuid::Uuid::new_v4().to_string(),
            computer: device.clone(),
            project: self.target.project.clone(),
            cwd,
            branch: self.target.branch.clone(),
            config: ChatConfig {
                harness,
                model: self.target.model.clone(),
                reasoning: self.target.reasoning,
                model_options: Default::default(),
                sandbox: SandboxLevel::WorkspaceWrite,
            },
            title: Some(crate::model::title_from(&text)),
        };
        let chat = match registry.write(|r| r.create_session(&new, now_ms())) {
            Ok(chat) => chat,
            Err(line) => {
                self.notice = Some(line);
                cx.notify();
                return;
            }
        };
        let spec = worktree.then(|| WorktreeSpec {
            repo_path: path.clone().unwrap_or_default(),
            base: self.target.branch.clone().unwrap_or_default(),
            space_id: self.target.project.clone(),
        });
        let message = uuid::Uuid::new_v4().to_string();
        let command = workspace::run_command(&chat, &message, &text, spec);
        self.sending = true;
        self.notice = None;
        self.merge_threads();
        let client = self.client.clone();
        let id = chat.id.clone();
        cx.spawn(async move |this, cx| {
            // Give the registry a moment to save the chat before the
            // computer hears of it; the computer takes it either way.
            for _ in 0..30 {
                if registry.read(|r| r.saved()) {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
            }
            let sent = async {
                let computer = computer::connect(&client, &device, || {}).await?;
                if let Some((reference, path)) = &switch {
                    computer
                        .call(
                            computer::SWITCH_REF,
                            json!({"repoPath": path, "refName": reference.name}),
                        )
                        .await?;
                }
                computer.call(computer::QUEUE_COMMAND, command).await
            }
            .await;
            this.update(cx, |this, cx| {
                this.sending = false;
                match sent {
                    Ok(_) => {
                        super::route::push(Some(&id));
                        this.merge_threads();
                        this.load(Some(id.clone()), cx);
                    }
                    Err(error) => {
                        this.composer.update(cx, |composer, cx| {
                            if composer.is_empty() {
                                composer.set_text(text.clone(), cx);
                            }
                        });
                        this.notice = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    // ── drawing ──────────────────────────────────────────────────────────

    /// The pickers row and the open picker's choices, or nothing when the
    /// account has no computers.
    pub(super) fn render_targets(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let registry = self.registry.as_ref()?;
        let (computers, spaces, online): (Vec<_>, Vec<_>, Vec<bool>) = registry.read(|r| {
            let computers = r.computers();
            let online = computers.iter().map(|d| r.online(d, now_ms())).collect();
            (computers, r.spaces(), online)
        });
        if computers.is_empty() {
            return None;
        }
        let target = &self.target;
        let computer_label = target
            .computer
            .as_deref()
            .and_then(|id| computers.iter().find(|d| d.id == id))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "OpenAgents".into());
        let mut chips: Vec<AnyElement> = vec![
            chip_button(
                "pick-computer",
                "icons/laptop.svg",
                computer_label,
                target.open == Some(Picker::Computer),
            )
            .on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_picker(Picker::Computer, cx)),
            )
            .into_any_element(),
        ];
        let mut choices: Vec<AnyElement> = Vec::new();
        if let Some(device) = target.computer.as_deref() {
            let here: Vec<_> = spaces.iter().filter(|s| s.device_id == device).collect();
            let project_label = target
                .project
                .as_deref()
                .and_then(|id| here.iter().find(|s| s.id == id))
                .map(|s| workspace::space_name(s))
                .unwrap_or_else(|| "No project".into());
            chips.push(
                chip_button(
                    "pick-project",
                    "icons/folder.svg",
                    project_label,
                    target.open == Some(Picker::Project),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_picker(Picker::Project, cx)
                }))
                .into_any_element(),
            );
            if !target.refs.is_empty() {
                let branch = target.branch.clone().unwrap_or_else(|| "Branch".into());
                chips.push(
                    chip_button(
                        "pick-branch",
                        "icons/git-branch.svg",
                        branch,
                        target.open == Some(Picker::Branch),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_picker(Picker::Branch, cx)
                    }))
                    .into_any_element(),
                );
                let worktree = target.worktree;
                chips.push(
                    chip_button(
                        "pick-checkout",
                        "icons/folder.svg",
                        if worktree {
                            "New worktree"
                        } else {
                            "Project folder"
                        }
                        .into(),
                        false,
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.target.worktree = !worktree;
                        cx.notify();
                    }))
                    .into_any_element(),
                );
            }
            if !target.harnesses.is_empty() {
                let harness = target
                    .harness
                    .and_then(|id| target.harnesses.iter().find(|h| h.id == id))
                    .map(|h| h.name.clone())
                    .unwrap_or_else(|| "Agent".into());
                chips.push(
                    chip_button(
                        "pick-harness",
                        "icons/widget.svg",
                        harness,
                        target.open == Some(Picker::Harness),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_picker(Picker::Harness, cx)
                    }))
                    .into_any_element(),
                );
            }
            if !target.models.is_empty() {
                let model = target
                    .model
                    .as_deref()
                    .and_then(|id| target.models.iter().find(|m| m.id == id))
                    .map(|m| m.label.clone())
                    .unwrap_or_else(|| "Model".into());
                chips.push(
                    chip_button(
                        "pick-model",
                        "icons/tuning.svg",
                        model,
                        target.open == Some(Picker::Model),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_picker(Picker::Model, cx)
                    }))
                    .into_any_element(),
                );
            }
            if !self.reasoning_levels().is_empty() {
                let label = target
                    .reasoning
                    .map_or("Reasoning", reasoning_label)
                    .to_owned();
                chips.push(
                    chip_button(
                        "pick-reasoning",
                        "icons/tuning.svg",
                        label,
                        target.open == Some(Picker::Reasoning),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_picker(Picker::Reasoning, cx)
                    }))
                    .into_any_element(),
                );
            }
            match target.open {
                Some(Picker::Project) => {
                    choices.push(
                        choice(
                            "project-none",
                            "No project".into(),
                            target.project.is_none(),
                        )
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.choose_project(None, cx)
                            }),
                        )
                        .into_any_element(),
                    );
                    for space in here {
                        let id = space.id.clone();
                        choices.push(
                            choice(
                                format!("project-{}", space.id),
                                workspace::space_name(space),
                                target.project.as_deref() == Some(space.id.as_str()),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.choose_project(Some(id.clone()), cx)
                            }))
                            .into_any_element(),
                        );
                    }
                }
                Some(Picker::Branch) => {
                    for reference in &target.refs {
                        let name = reference.name.clone();
                        let label = if reference.current {
                            format!("{} (checked out)", reference.name)
                        } else if reference.worktree_path.is_some() {
                            format!("{} (worktree)", reference.name)
                        } else {
                            reference.name.clone()
                        };
                        choices.push(
                            choice(
                                format!("branch-{}", reference.name),
                                label,
                                target.branch.as_deref() == Some(reference.name.as_str()),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.target.branch = Some(name.clone());
                                this.target.open = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                        );
                    }
                }
                Some(Picker::Harness) => {
                    for harness in &target.harnesses {
                        let id = harness.id;
                        choices.push(
                            choice(
                                format!("harness-{:?}", harness.id),
                                harness.name.clone(),
                                target.harness == Some(id),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.choose_harness(id, cx)
                            }))
                            .into_any_element(),
                        );
                    }
                }
                Some(Picker::Model) => {
                    for model in &target.models {
                        let id = model.id.clone();
                        choices.push(
                            choice(
                                format!("model-{}", model.id),
                                model.label.clone(),
                                target.model.as_deref() == Some(model.id.as_str()),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.target.model = Some(id.clone());
                                this.target.reasoning = None;
                                this.target.open = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                        );
                    }
                }
                Some(Picker::Reasoning) => {
                    choices.push(
                        choice(
                            "reasoning-default",
                            "Default".into(),
                            target.reasoning.is_none(),
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.target.reasoning = None;
                            this.target.open = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                    );
                    for level in self.reasoning_levels() {
                        choices.push(
                            choice(
                                format!("reasoning-{level:?}"),
                                reasoning_label(level).into(),
                                target.reasoning == Some(level),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.target.reasoning = Some(level);
                                this.target.open = None;
                                cx.notify();
                            }))
                            .into_any_element(),
                        );
                    }
                }
                _ => {}
            }
        }
        if target.open == Some(Picker::Computer) {
            choices.push(
                choice(
                    "computer-site",
                    "OpenAgents".into(),
                    target.computer.is_none(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_computer(None, cx)))
                .into_any_element(),
            );
            for (device, online) in computers.iter().zip(online) {
                let id = device.id.clone();
                let label = if online {
                    device.name.clone()
                } else {
                    format!("{} (offline)", device.name)
                };
                choices.push(
                    choice(
                        format!("computer-{}", device.id),
                        label,
                        target.computer.as_deref() == Some(device.id.as_str()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.choose_computer(Some(id.clone()), cx)
                    }))
                    .into_any_element(),
                );
            }
        }
        Some(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .max_w(px(openagents_web_chat_view::transcript::CONTENT_WIDTH))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .justify_center()
                        .gap(px(8.))
                        .children(chips),
                )
                .when(!choices.is_empty(), |col| {
                    col.child(
                        div()
                            .id("picker-choices")
                            .max_h(px(260.))
                            .overflow_y_scroll()
                            .min_w(px(240.))
                            .p(px(4.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(theme::border_strong())
                            .bg(theme::surface_card())
                            .flex()
                            .flex_col()
                            .children(choices),
                    )
                })
                .children(target.loading.map(|line| {
                    div()
                        .text_size(px(12.5))
                        .text_color(theme::text_faint())
                        .child(line)
                }))
                .into_any_element(),
        )
    }

    fn reasoning_levels(&self) -> Vec<ReasoningLevel> {
        let target = &self.target;
        let from_model = target
            .model
            .as_deref()
            .and_then(|id| target.models.iter().find(|m| m.id == id))
            .map(|m| m.reasoning_levels.clone())
            .unwrap_or_default();
        if !from_model.is_empty() {
            return from_model;
        }
        target
            .harness
            .and_then(|id| target.harnesses.iter().find(|h| h.id == id))
            .map(|h| h.reasoning_levels.clone())
            .unwrap_or_default()
    }

    fn toggle_picker(&mut self, picker: Picker, cx: &mut Context<Self>) {
        self.target.open = if self.target.open == Some(picker) {
            None
        } else {
            Some(picker)
        };
        cx.notify();
    }
}

fn chip_button(
    id: &'static str,
    icon: &'static str,
    label: String,
    open: bool,
) -> gpui::Stateful<gpui::Div> {
    div().id(id).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .h(px(30.))
            .rounded(px(15.))
            .border_1()
            .border_color(if open {
                theme::accent().opacity(0.6)
            } else {
                theme::border()
            })
            .text_size(px(13.))
            .text_color(theme::text_muted())
            .cursor_pointer()
            .hover(|s| s.bg(theme::element_hover()))
            .child(
                svg()
                    .path(icon)
                    .size(px(13.))
                    .text_color(theme::text_muted()),
            )
            .child(label)
            .child(
                svg()
                    .path("icons/alt-arrow-down.svg")
                    .size(px(11.))
                    .text_color(theme::text_faint()),
            ),
    )
}

fn choice(id: impl Into<SharedString>, label: String, chosen: bool) -> gpui::Stateful<gpui::Div> {
    div().id(id.into()).child(
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
            .when(chosen, |b| b.bg(theme::element_active()))
            .child(div().w(px(14.)).when(chosen, |d| {
                d.child(
                    svg()
                        .path("icons/check.svg")
                        .size(px(14.))
                        .text_color(theme::accent()),
                )
            }))
            .child(div().truncate().child(label)),
    )
}
