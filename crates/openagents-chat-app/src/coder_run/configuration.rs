//! The composer's choices for the next provider session, offered by its owner.

use route_contract::{
    Digest,
    session::{Selection, control as wire},
};
use rust_native::style::{Space, Style};
use rust_native::{Axis, Element, Node, TextRole};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Control {
    Toggle,
    Reasoning,
    Session,
    Previous,
    Next,
    Default,
    Model(usize),
    Effort(usize),
    Mode(wire::Mode),
    Refresh,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub offer: Digest,
    pub view: Digest,
    pub control: Control,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum Menu {
    Models,
    Reasoning,
    Session,
}
#[derive(Default)]
pub struct Choice {
    offer: Option<wire::Offer>,
    selected: Option<Selection>,
    selected_adapter: Option<String>,
    current: Option<Selection>,
    current_adapter: Option<String>,
    mode: Option<wire::Mode>,
    available: bool,
    binding: Option<(Digest, Option<String>)>,
    resume: Option<Digest>,
    expanded: Option<Menu>,
    page: usize,
}

pub struct View<I> {
    pub node: Node<I>,
}

impl Choice {
    pub fn update(&mut self, offer: Option<wire::Offer>) -> bool {
        let old = self.offer.clone();
        let available = self.available;
        let Some(offer) = offer.filter(wire::Offer::valid) else {
            self.available = false;
            return available;
        };
        if let Some(old) = &self.offer
            && old.task == offer.task
            && (offer.turn, offer.revision) < (old.turn, old.revision)
        {
            return false;
        }
        if self
            .offer
            .as_ref()
            .is_some_and(|old| old.task != offer.task || offer.turn > old.turn)
        {
            self.mode = None;
            self.resume = None;
        }
        self.page = self.page.min(offer.models.len().saturating_sub(1) / 5);
        self.offer = Some(offer);
        self.available = true;
        old != self.offer || !available
    }
    pub fn adopt(&mut self, offer: wire::Offer, request: Option<&wire::Request>) {
        if !offer.valid() {
            return;
        }
        if let Some(request) = request.filter(|request| offer.accepts(request)) {
            self.selected = Some(request.selection.clone());
            self.selected_adapter = Some(request.adapter.clone());
            self.mode = Some(request.mode);
            self.resume = request.resume.clone();
            self.binding = offer
                .models
                .iter()
                .find(|model| {
                    model.selection.provider == request.selection.provider
                        && model.selection.model == request.selection.model
                        && model.adapter == request.adapter
                })
                .map(|model| (offer.authority.clone(), model.account.clone()));
        }
        self.update(Some(offer));
    }
    pub fn current(&mut self, selection: Selection, adapter: String) {
        self.current = Some(selection);
        self.current_adapter = Some(adapter);
    }
    pub fn has_offer(&self) -> bool {
        self.offer.is_some()
    }
    pub fn open(&mut self) {
        self.expanded = Some(Menu::Models);
    }
    pub fn offer(&self) -> Option<&wire::Offer> {
        self.offer.as_ref()
    }
    pub fn selection(&self) -> Option<&Selection> {
        self.selected.as_ref()
    }
    fn model(&self) -> Option<&wire::Model> {
        let offer = self.offer.as_ref()?;
        let selection = self.selected.as_ref().or(self.current.as_ref());
        let adapter = self
            .selected_adapter
            .as_ref()
            .or(self.current_adapter.as_ref());
        selection
            .and_then(|selection| {
                offer.models.iter().find(|model| {
                    model.selection.provider == selection.provider
                        && model.selection.model == selection.model
                        && adapter.is_none_or(|adapter| adapter == &model.adapter)
                })
            })
            .or_else(|| offer.models.iter().find(|model| model.available))
    }
    fn view_id(&self) -> Digest {
        route_contract::digest_of(&(
            self.offer.as_ref().map(|offer| &offer.id),
            &self.selected,
            &self.selected_adapter,
            &self.current,
            &self.current_adapter,
            &self.mode,
            self.expanded,
            self.page,
        ))
    }
    pub fn apply(&mut self, target: &Target) -> bool {
        let Some(offer) = &self.offer else {
            return false;
        };
        if (!self.available && target.control != Control::Toggle)
            || target.offer != offer.id
            || target.view != self.view_id()
        {
            return false;
        }
        match target.control {
            Control::Toggle => {
                self.expanded = if self.expanded == Some(Menu::Models) {
                    None
                } else {
                    Some(Menu::Models)
                }
            }
            Control::Reasoning => {
                self.expanded = if self.expanded == Some(Menu::Reasoning) {
                    None
                } else {
                    Some(Menu::Reasoning)
                }
            }
            Control::Session => {
                self.expanded = if self.expanded == Some(Menu::Session) {
                    None
                } else {
                    Some(Menu::Session)
                }
            }
            Control::Previous => {
                self.page = self.page.saturating_sub(1);
            }
            Control::Next => {
                if (self.page + 1) * 5 < offer.models.len() {
                    self.page += 1;
                } else {
                    return false;
                }
            }
            Control::Default if offer.task.is_none() => {
                self.selected = None;
                self.selected_adapter = None;
                self.mode = None;
                self.resume = None;
                self.binding = None;
                self.expanded = None;
            }
            Control::Model(index) => {
                let Some(model) = offer.models.get(index).filter(|model| model.available) else {
                    return false;
                };
                if self.mode.is_some_and(|mode| !model.modes.contains(&mode)) {
                    return false;
                }
                self.binding = Some((offer.authority.clone(), model.account.clone()));
                self.selected = Some(model.selection.clone());
                self.selected_adapter = Some(model.adapter.clone());
                self.expanded = None;
            }
            Control::Effort(index) => {
                let Some(model) = self.model() else {
                    return false;
                };
                let Some(effort) = model.efforts.get(index) else {
                    return false;
                };
                let mut selected = self
                    .selected
                    .clone()
                    .unwrap_or_else(|| model.selection.clone());
                selected.effort = Some(effort.clone());
                let account = model.account.clone();
                let adapter = model.adapter.clone();
                self.binding = Some((offer.authority.clone(), account));
                self.selected = Some(selected);
                self.selected_adapter = Some(adapter);
                self.expanded = None;
            }
            Control::Mode(mode) => {
                let Some(model) = self
                    .model()
                    .filter(|model| model.available && model.modes.contains(&mode))
                else {
                    return false;
                };
                let selection = model.selection.clone();
                let account = model.account.clone();
                let adapter = model.adapter.clone();
                if self.selected.is_none() {
                    self.selected = Some(selection);
                    self.selected_adapter = Some(adapter);
                }
                self.binding = Some((offer.authority.clone(), account));
                self.resume = if mode == wire::Mode::NativeResume {
                    offer.resume.as_ref().map(|resume| resume.id.clone())
                } else {
                    None
                };
                self.mode = Some(mode);
                self.expanded = None;
            }
            Control::Refresh => return false,
            _ => return false,
        }
        true
    }
    pub fn mode(&self) -> wire::Mode {
        self.mode.unwrap_or(
            if self
                .offer
                .as_ref()
                .is_some_and(|offer| offer.task.is_none())
            {
                wire::Mode::New
            } else {
                wire::Mode::Continue
            },
        )
    }
    pub fn request(&self) -> Result<Option<wire::Request>, String> {
        let Some(selection) = self.selected.as_ref() else {
            return Ok(None);
        };
        let offer = self
            .offer
            .as_ref()
            .filter(|_| self.available)
            .ok_or("The coding choices are unavailable. Refresh them and try again.")?;
        let model = offer
            .models
            .iter()
            .find(|model| {
                model.selection.provider == selection.provider
                    && model.selection.model == selection.model
                    && self.selected_adapter.as_ref() == Some(&model.adapter)
            })
            .ok_or("This model is no longer available.")?;
        if self.binding.as_ref() != Some(&(offer.authority.clone(), model.account.clone())) {
            return Err("The coding login or settings changed. Choose a model again.".into());
        }
        let mode = self.mode();
        let request = wire::Request {
            schema: wire::SCHEMA.into(),
            offer: offer.id.clone(),
            adapter: model.adapter.clone(),
            checkout: offer.checkout.clone(),
            target: offer.target.clone(),
            selection: selection.clone(),
            mode,
            resume: self.resume.clone(),
        };
        offer
            .accepts(&request)
            .then_some(Some(request))
            .ok_or_else(|| {
                "This coding choice is no longer available. Choose a model or session again.".into()
            })
    }
    pub fn view<I>(
        &self,
        prefix: &str,
        enabled: bool,
        modes_enabled: bool,
        intent: impl Fn(Target) -> I,
    ) -> Option<View<I>> {
        let offer = self.offer.as_ref()?;
        let view = self.view_id();
        let button = |suffix: &str, label: String, control: Control, supported: bool| {
            let key = format!("{prefix}-{}-{suffix}", offer.id.as_str());
            Node {
                key,
                style: Style::default(),
                element: Element::Button {
                    label,
                    enabled: enabled
                        && (self.available
                            || matches!(control, Control::Toggle | Control::Refresh))
                        && supported,
                    intent: intent(Target {
                        offer: offer.id.clone(),
                        view: view.clone(),
                        control,
                    }),
                    shortcut: None,
                    icon: None,
                },
            }
        };
        let label = match self.selected.as_ref() {
            Some(selection) => format!(
                "{}: {} · {}",
                if offer.task.is_some() {
                    "Next turn"
                } else {
                    "Coder"
                },
                provider(&selection.provider),
                selection.model
            ),
            None => {
                if offer.task.is_some() {
                    "Coder model: current task settings".into()
                } else {
                    "Coder model: default settings".into()
                }
            }
        };
        let mut header = vec![button("toggle", label, Control::Toggle, true)];
        if let Some(model) = self.model() {
            if !model.efforts.is_empty() {
                let effort = self
                    .selected
                    .as_ref()
                    .or(self.current.as_ref())
                    .and_then(|selection| selection.effort.as_deref())
                    .unwrap_or("medium");
                header.push(button(
                    "reasoning",
                    format!("Reasoning: {}", effort_label(effort)),
                    Control::Reasoning,
                    true,
                ));
            }
            if offer.task.is_some()
                && (model.modes.len() > 1
                    || self.mode.is_some_and(|mode| mode != wire::Mode::Continue))
            {
                header.push(button(
                    "session",
                    mode_label(self.mode()).into(),
                    Control::Session,
                    true,
                ));
            }
        }
        let mut children = vec![Node {
            key: format!("{prefix}-settings-header"),
            style: Style {
                gap: Some(Space::Xs),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Wrap,
                children: header,
            },
        }];
        match self.expanded {
            Some(Menu::Models) => {
                children.push(text(
                    &format!("{prefix}-folder"),
                    &format!("Project: {}", offer.checkout),
                ));
                if offer.task.is_none() {
                    children.push(button(
                        "default",
                        "Use default coding settings".into(),
                        Control::Default,
                        true,
                    ));
                }
                for (index, model) in offer.models.iter().enumerate().skip(self.page * 5).take(5) {
                    children.push(button(
                        &format!("model-{index}"),
                        format!(
                            "{} · {}",
                            provider(&model.selection.provider),
                            if model.adapter == "claude-sdk" {
                                format!("{} · Live approvals", model.selection.model)
                            } else {
                                model.selection.model.clone()
                            }
                        ),
                        Control::Model(index),
                        model.available && self.mode.is_none_or(|mode| model.modes.contains(&mode)),
                    ));
                    if !model.available
                        && let Some(reason) = &model.reason
                    {
                        children.push(text(&format!("{prefix}-reason-{index}"), reason));
                    }
                }
                if self.page > 0 {
                    children.push(button(
                        "previous",
                        "Previous models".into(),
                        Control::Previous,
                        true,
                    ));
                }
                if (self.page + 1) * 5 < offer.models.len() {
                    children.push(button("next", "More models".into(), Control::Next, true));
                }
                children.push(button(
                    "refresh",
                    "Refresh coding choices".into(),
                    Control::Refresh,
                    true,
                ));
            }
            Some(Menu::Reasoning) => {
                if let Some(model) = self.model() {
                    for (index, effort) in model.efforts.iter().enumerate() {
                        children.push(button(
                            &format!("effort-{index}"),
                            effort_label(effort).into(),
                            Control::Effort(index),
                            model.available,
                        ));
                    }
                }
            }
            Some(Menu::Session) => {
                if let Some(model) = self.model() {
                    for mode in &model.modes {
                        children.push(button(
                            &format!("mode-{mode:?}"),
                            mode_label(*mode).into(),
                            Control::Mode(*mode),
                            model.available && modes_enabled,
                        ));
                    }
                }
            }
            None => {}
        }
        if let Err(reason) = self.request() {
            children.push(text(&format!("{prefix}-choice-error"), &reason));
        }
        Some(View {
            node: Node {
                key: format!("{prefix}-settings"),
                style: Style {
                    gap: Some(Space::Xs),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        })
    }
}

fn text<I>(key: &str, value: &str) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role: TextRole::Status,
        },
    }
}
fn provider(value: &str) -> &str {
    match value {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "grok" => "Grok Build",
        "devin" => "Devin",
        "opencode" => "OpenCode",
        _ => value,
    }
}
fn effort_label(value: &str) -> &str {
    match value {
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        _ => value,
    }
}
fn mode_label(mode: wire::Mode) -> &'static str {
    match mode {
        wire::Mode::New => "New session",
        wire::Mode::Continue => "Continue task",
        wire::Mode::NativeResume => "Resume saved session",
        wire::Mode::ContextTransfer => "New session with task summary",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn offer() -> wire::Offer {
        let mut offer = wire::Offer {
            schema: wire::SCHEMA.into(),
            id: route_contract::digest_of(&()),
            task: Some("task".into()),
            revision: 4,
            turn: 1,
            authority: route_contract::digest_of(&"rights"),
            checkout: "/fixture".into(),
            target: "inode:1".into(),
            base: "head".into(),
            models: vec![
                wire::Model {
                    selection: Selection {
                        provider: "claude".into(),
                        model: "fixture".into(),
                        effort: None,
                    },
                    adapter: "claude-cli".into(),
                    efforts: vec![],
                    available: true,
                    image_followups: false,
                    modes: vec![wire::Mode::Continue],
                    account: None,
                    reason: None,
                },
                wire::Model {
                    selection: Selection {
                        provider: "claude".into(),
                        model: "fixture".into(),
                        effort: None,
                    },
                    adapter: "claude-sdk".into(),
                    efforts: vec!["high".into()],
                    available: true,
                    image_followups: true,
                    modes: vec![
                        wire::Mode::Continue,
                        wire::Mode::New,
                        wire::Mode::NativeResume,
                    ],
                    account: Some("login".into()),
                    reason: None,
                },
            ],
            resume: Some(wire::Resume {
                id: route_contract::digest_of(&"source"),
                turn: 1,
                provider: "claude".into(),
                model: "fixture".into(),
            }),
        };
        offer.seal();
        offer
    }
    fn choose(choice: &mut Choice, control: Control) -> bool {
        let target = Target {
            offer: choice.offer().unwrap().id.clone(),
            view: choice.view_id(),
            control,
        };
        choice.apply(&target)
    }
    #[test]
    fn duplicate_model_adapters_and_stale_clicks_are_distinct() {
        let mut choice = Choice::default();
        choice.update(Some(offer()));
        let old = Target {
            offer: choice.offer().unwrap().id.clone(),
            view: choice.view_id(),
            control: Control::Model(0),
        };
        assert!(choose(&mut choice, Control::Model(1)));
        assert_eq!(choice.request().unwrap().unwrap().adapter, "claude-sdk");
        assert!(!choice.apply(&old));
        assert!(choose(&mut choice, Control::Effort(0)));
        assert_eq!(
            choice
                .request()
                .unwrap()
                .unwrap()
                .selection
                .effort
                .as_deref(),
            Some("high")
        );
        assert!(choose(&mut choice, Control::Mode(wire::Mode::NativeResume)));
        assert!(!choose(&mut choice, Control::Model(0)));
    }
    #[test]
    fn disconnected_changed_login_and_changed_resume_do_not_rebind_choice() {
        let mut choice = Choice::default();
        let mut offer = offer();
        choice.update(Some(offer.clone()));
        choose(&mut choice, Control::Model(1));
        choose(&mut choice, Control::Mode(wire::Mode::NativeResume));
        let original = choice.request().unwrap().unwrap();
        choice.update(None);
        assert!(choice.request().is_err());
        assert!(choice.selection().is_some());
        choice.update(Some(offer.clone()));
        assert_eq!(choice.request().unwrap().unwrap(), original);
        offer.resume.as_mut().unwrap().id = route_contract::digest_of(&"replacement");
        offer.seal();
        choice.update(Some(offer.clone()));
        assert!(choice.request().is_err());
        choose(&mut choice, Control::Mode(wire::Mode::New));
        assert!(choice.request().is_ok());
        offer.models[1].account = Some("another-login".into());
        offer.seal();
        choice.update(Some(offer));
        assert!(choice.request().is_err());
        choose(&mut choice, Control::Model(1));
        assert!(choice.request().is_ok());
    }
    #[test]
    fn old_poll_cannot_replace_new_options_and_pages_shrink() {
        let mut choice = Choice::default();
        let old = offer();
        let mut new = old.clone();
        new.revision += 1;
        new.models.extend(vec![old.models[1].clone(); 10]);
        new.seal();
        choice.update(Some(new.clone()));
        choose(&mut choice, Control::Next);
        assert_eq!(choice.page, 1);
        assert!(!choice.update(Some(old.clone())));
        assert_eq!(choice.offer().unwrap().id, new.id);
        new.revision += 1;
        new.models.truncate(2);
        new.seal();
        choice.update(Some(new));
        assert_eq!(choice.page, 0);
    }
}
