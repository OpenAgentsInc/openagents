//! Settings → General → Check-ins: every saved agent's check-ins, grants
//! and decision log on this computer (#11965, #12044). The grants, the
//! Decision log switch and the log are the shared autonomy card
//! (`agent_lab::autonomy`, #12045) that the phone and the website show too. Any agent whose
//! definition declares wakes appears here, keyed by its definition ID; the
//! engine that hosts its chat keeps all three, and grants go through the
//! account.

use std::collections::HashMap;
use std::time::Duration;

use gpui::{AnyElement, Context, Entity, Render, Task, Window, div, prelude::*, px};
use openagents_proto::agent_wakes::{
    AgentWakeStatus, DecisionEntry, GrantRequest, Over, RevokeGrant, Scope, SetWakes, WakeReason,
    WakeSettings, WakesStatus,
};
use openagents_rpc::methods;

use super::widgets;
use crate::popover::Loadable;
use crate::state::AppState;
use crate::theme::Theme;

/// The intervals offered, in minutes.
const EVERY: [u32; 6] = [10, 20, 30, 60, 120, 240];
/// The log lines shown per agent, newest first.
const SHOWN: usize = 8;
const REFRESH: Duration = Duration::from_secs(30);

pub(super) struct AgentWakesCard {
    state: Entity<AppState>,
    status: Loadable<WakesStatus>,
    error: Option<String>,
    saving: bool,
    /// Dropdowns, by `chat:AGENT`, `every:AGENT` and `setup`.
    selects: HashMap<String, widgets::SelectState>,
    task: Option<Task<()>>,
    _refresh: Task<()>,
}

/// The status Design Lab's offscreen tour shows instead of asking an engine.
static FIXTURE: std::sync::OnceLock<WakesStatus> = std::sync::OnceLock::new();

/// Show `status` in every Check-ins card, for Design Lab captures.
pub fn install_fixture(status: WakesStatus) {
    let _ = FIXTURE.set(status);
}

/// A saved agent that isn't Foreman, with a grant, its log on, and two
/// decisions: the autonomy card as Design Lab shows it (#12045).
pub fn fixture_status() -> WakesStatus {
    use openagents_proto::agent_wakes::{Grant, Wake};
    WakesStatus {
        agents: vec![AgentWakeStatus {
            agent: "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3".into(),
            agent_name: "Release reviewer".into(),
            settings: WakeSettings {
                chat_id: Some("fixture-chat".into()),
                every_minutes: 30,
                ..WakeSettings::default()
            },
            chat_title: Some("Release review".into()),
            wakes: vec![Wake::ChatFinished, Wake::IssueClosed],
            grants: vec![
                Grant {
                    id: "d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1".into(),
                    scopes: vec![Scope::ChatMessage, Scope::ChatSteer],
                    over: Over::Chats(vec!["a".into(), "b".into()]),
                    granted_at: 1_792_340_000_000,
                },
                Grant {
                    id: "e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1".into(),
                    scopes: vec![Scope::LogWrite],
                    over: Over::Account,
                    granted_at: 1_792_340_000_000,
                },
            ],
            next_check_in: Some(1_792_345_400_000),
            problem: None,
            turned_on_at: None,
            decisions: vec![
                DecisionEntry::Decision {
                    at: 1_792_341_000_000,
                    saw: "the release build finished".into(),
                    decided: "to review the notes".into(),
                    did: "asked the build chat for its changelog".into(),
                    judgments: vec![],
                },
                DecisionEntry::Decision {
                    at: 1_792_342_800_000,
                    saw: "the changelog".into(),
                    decided: "it reads well".into(),
                    did: "nothing".into(),
                    judgments: vec![],
                },
            ],
            log_kept_before_grant: false,
        }],
        candidates: vec![],
        problem: None,
    }
}

impl AgentWakesCard {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        if let Some(status) = FIXTURE.get() {
            return Self {
                state,
                status: Loadable::Ready(status.clone()),
                error: None,
                saving: false,
                selects: HashMap::new(),
                task: None,
                _refresh: Task::ready(()),
            };
        }
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH).await;
                if this
                    .update(cx, |card, cx| {
                        if !card.saving {
                            card.request(None, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut card = Self {
            state,
            status: Loadable::Loading,
            error: None,
            saving: false,
            selects: HashMap::new(),
            task: None,
            _refresh: refresh,
        };
        card.request(None, cx);
        card
    }

    /// Read the status, or make one change and read the status it returns.
    fn request(
        &mut self,
        change: Option<(&'static str, serde_json::Value)>,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.status = Loadable::Error("This computer isn't connected yet.".into());
            cx.notify();
            return;
        };
        let saving = change.is_some();
        self.saving = saving;
        let (method, params) = change.unwrap_or((methods::GET_AGENT_WAKES, serde_json::json!({})));
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(method, params)
                .await
                .map_err(|error| match error {
                    openagents_rpc::RpcError::UnknownMethod(_) => {
                        "Update OpenAgents on this computer to use agent check-ins.".to_string()
                    }
                    _ => error.to_string(),
                })
                .and_then(|value| {
                    serde_json::from_value::<WakesStatus>(value).map_err(|e| e.to_string())
                });
            this.update(cx, |card, cx| {
                card.saving = false;
                match result {
                    Ok(status) => {
                        card.error = None;
                        card.status = Loadable::Ready(status);
                    }
                    Err(error) if saving => card.error = Some(error),
                    Err(error) => card.status = Loadable::Error(error),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn change(
        &mut self,
        agent: &str,
        edit: impl FnOnce(&mut WakeSettings),
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        let Some(status) = self.status.ready() else {
            return;
        };
        let mut settings = status
            .agents
            .iter()
            .find(|a| a.agent == agent)
            .map(|a| a.settings.clone())
            .unwrap_or_default();
        let before = settings.clone();
        edit(&mut settings);
        if settings != before {
            let set = SetWakes {
                agent: agent.to_owned(),
                settings,
            };
            self.request(
                Some((
                    methods::SET_AGENT_WAKES,
                    serde_json::to_value(set).unwrap_or_default(),
                )),
                cx,
            );
        }
    }

    /// Turn standing authority (every chat scope over the account) on or off.
    fn toggle_standing(&mut self, agent: &AgentWakeStatus, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let change = match agent.grants.iter().find(|g| g.is_standing()) {
            Some(grant) => (
                methods::REVOKE_AGENT_GRANT,
                serde_json::to_value(RevokeGrant {
                    agent: agent.agent.clone(),
                    grant: grant.id.clone(),
                })
                .unwrap_or_default(),
            ),
            None => (
                methods::GRANT_AGENT,
                serde_json::to_value(GrantRequest {
                    agent: agent.agent.clone(),
                    scopes: Scope::CHAT.iter().map(|s| s.word().to_owned()).collect(),
                    over: Over::Account,
                })
                .unwrap_or_default(),
            ),
        };
        self.request(Some(change), cx);
    }

    /// Carry out what the shared card asked for, against the card as it is
    /// drawn now.
    fn act(
        &mut self,
        agent: &AgentWakeStatus,
        activation: &rust_native::Activation,
        cx: &mut Context<Self>,
    ) {
        use agent_lab::autonomy::Intent;
        if self.saving {
            return;
        }
        let card = autonomy_card(agent);
        let Ok(view) = card.view(&card_instance(&agent.agent), 1) else {
            return;
        };
        let Ok(intent) = view.activate(activation) else {
            return;
        };
        let change = match intent {
            Intent::Log { on: true } => (
                methods::GRANT_AGENT,
                serde_json::to_value(GrantRequest {
                    agent: agent.agent.clone(),
                    scopes: vec![Scope::LogWrite.word().to_owned()],
                    over: Over::Account,
                })
                .unwrap_or_default(),
            ),
            Intent::Log { on: false } => {
                let Some(grant) = card.log_grants().into_iter().next() else {
                    return;
                };
                (
                    methods::REVOKE_AGENT_GRANT,
                    serde_json::to_value(RevokeGrant {
                        agent: agent.agent.clone(),
                        grant,
                    })
                    .unwrap_or_default(),
                )
            }
            Intent::Revoke { grant } => (
                methods::REVOKE_AGENT_GRANT,
                serde_json::to_value(RevokeGrant {
                    agent: agent.agent.clone(),
                    grant: grant.clone(),
                })
                .unwrap_or_default(),
            ),
        };
        self.request(Some(change), cx);
    }

    fn render_chat(
        &mut self,
        theme: &Theme,
        status: &WakesStatus,
        agent: &AgentWakeStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut options = vec![widgets::SelectOption::new("None")];
        let mut ids: Vec<Option<String>> = vec![None];
        for candidate in status.candidates.iter().filter(|c| c.agent == agent.agent) {
            options.push(widgets::SelectOption::new(candidate.title.clone()));
            ids.push(Some(candidate.chat_id.clone()));
        }
        if let Some(chat) = &agent.settings.chat_id
            && !ids.contains(&Some(chat.clone()))
        {
            options.push(widgets::SelectOption::new(
                agent
                    .chat_title
                    .clone()
                    .unwrap_or_else(|| "Current chat".into()),
            ));
            ids.push(Some(chat.clone()));
        }
        let selected = ids
            .iter()
            .position(|id| *id == agent.settings.chat_id)
            .unwrap_or(0);
        let key = format!("chat:{}", agent.agent);
        self.selects.entry(key.clone()).or_default();
        let reach_key = key.clone();
        let id = agent.agent.clone();
        widgets::select(
            format!("wakes-chat-{}", agent.agent),
            "Chat",
            theme,
            move |card: &mut Self| card.selects.entry(reach_key.clone()).or_default(),
        )
        .options(options, selected)
        .menu_width(260.0)
        .on_select(move |card, ix, _, cx| {
            let chosen = ids.get(ix).cloned().flatten();
            card.change(&id, |s| s.chat_id = chosen, cx);
        })
        .render(&self.selects[&key], cx)
        .into_any_element()
    }

    fn render_every(
        &mut self,
        theme: &Theme,
        agent: &AgentWakeStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = agent.settings.every_minutes;
        let mut minutes: Vec<u32> = EVERY.to_vec();
        if !minutes.contains(&current) {
            minutes.push(current);
            minutes.sort_unstable();
        }
        let options = minutes
            .iter()
            .map(|m| widgets::SelectOption::new(every_label(*m)));
        let selected = minutes.iter().position(|m| *m == current).unwrap_or(0);
        let key = format!("every:{}", agent.agent);
        self.selects.entry(key.clone()).or_default();
        let reach_key = key.clone();
        let id = agent.agent.clone();
        widgets::select(
            format!("wakes-every-{}", agent.agent),
            "Check in every",
            theme,
            move |card: &mut Self| card.selects.entry(reach_key.clone()).or_default(),
        )
        .options(options, selected)
        .on_select(move |card, ix, _, cx| {
            if let Some(m) = minutes.get(ix).copied() {
                card.change(&id, |s| s.every_minutes = m, cx);
            }
        })
        .render(&self.selects[&key], cx)
        .into_any_element()
    }

    /// A picker for a saved-agent chat whose agent has no check-ins here yet.
    fn render_setup(
        &mut self,
        theme: &Theme,
        status: &WakesStatus,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let fresh: Vec<_> = status
            .candidates
            .iter()
            .filter(|c| !status.agents.iter().any(|a| a.agent == c.agent))
            .cloned()
            .collect();
        if fresh.is_empty() {
            return None;
        }
        let mut options = vec![widgets::SelectOption::new("Choose a chat")];
        options.extend(
            fresh
                .iter()
                .map(|c| widgets::SelectOption::new(c.title.clone())),
        );
        self.selects.entry("setup".into()).or_default();
        Some(
            widgets::select(
                "wakes-setup",
                "Set up check-ins for",
                theme,
                |card: &mut Self| card.selects.entry("setup".into()).or_default(),
            )
            .options(options, 0)
            .menu_width(260.0)
            .on_select(move |card, ix, _, cx| {
                let Some(chosen) = ix.checked_sub(1).and_then(|i| fresh.get(i)) else {
                    return;
                };
                let set = SetWakes {
                    agent: chosen.agent.clone(),
                    settings: WakeSettings {
                        chat_id: Some(chosen.chat_id.clone()),
                        ..WakeSettings::default()
                    },
                };
                card.request(
                    Some((
                        methods::SET_AGENT_WAKES,
                        serde_json::to_value(set).unwrap_or_default(),
                    )),
                    cx,
                );
            })
            .render(&self.selects["setup"], cx)
            .into_any_element(),
        )
    }

    fn render_agent(
        &mut self,
        theme: &Theme,
        status: &WakesStatus,
        agent: &AgentWakeStatus,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let interactive = !self.saving;
        let configured = agent.settings.chat_id.is_some();
        let on = !agent.settings.paused;
        let chat_meta = match (configured, agent.turned_on_at) {
            (true, Some(at)) => turned_on_line(at),
            (true, None) => "Checks in on its own and when the chats it manages need it.".into(),
            (false, _) => "Choose the chat this agent answers in.".to_string(),
        };
        let chat_row = widgets::card_row(theme, true)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(theme, agent.agent_name.clone()))
                    .child(widgets::meta_line(
                        theme,
                        vec![div().child(chat_meta).into_any_element()],
                    )),
            )
            .child(self.render_chat(theme, status, agent, cx));

        let next = match (configured, on, agent.next_check_in) {
            (false, _, _) => "Choose its chat first.".to_string(),
            (true, false, _) => "Check-ins are paused.".to_string(),
            (true, true, Some(at)) => format!("Next check-in at {}.", local_time(at)),
            (true, true, None) => "Checking in soon.".to_string(),
        };
        let every = self.render_every(theme, agent, cx);
        let id = agent.agent.clone();
        let schedule_row = widgets::card_row(theme, false)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(theme, "Check-ins"))
                    .child(widgets::meta_line(
                        theme,
                        vec![div().child(next).into_any_element()],
                    )),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(every)
                    .child(
                        widgets::toggle_switch(theme, on, format!("wakes-on-{id}"))
                            .id(gpui::SharedString::from(format!("wakes-on-toggle-{id}")))
                            .flex_none()
                            .role(gpui::Role::Switch)
                            .aria_label("Check-ins")
                            .aria_toggled(if on {
                                gpui::Toggled::True
                            } else {
                                gpui::Toggled::False
                            })
                            .when(interactive && configured, |el| {
                                let id = id.clone();
                                el.cursor_pointer()
                                    .tab_index(0)
                                    .focus_visible(|s| s.border_2().border_color(theme.accent))
                                    .on_click(cx.listener(move |card, _, _, cx| {
                                        card.change(&id, |s| s.paused = !s.paused, cx)
                                    }))
                            })
                            .when(!(interactive && configured), |el| el.opacity(0.5)),
                    ),
            );

        let standing = agent.grants.iter().find(|g| g.is_standing());
        let granted = standing.is_some();
        let authority_meta = match (standing, agent.grants.len()) {
            (Some(g), _) => format!(
                "Since {}, it can message, start, steer and stop chats without asking. Money, keys, secrets, deploys and closing others' issues still need you.",
                local_time(g.granted_at)
            ),
            (None, 0) => "Off: on check-ins it only looks and reports. Turn on to let it message, start, steer and stop chats without asking.".into(),
            (None, n) => format!(
                "It holds {n} narrower grant{} from your account. Turn on to let it message, start, steer and stop any chat without asking.",
                if n == 1 { "" } else { "s" }
            ),
        };
        let toggle_agent = agent.clone();
        let authority_row = widgets::card_row(theme, false)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(theme, "Standing authority"))
                    .child(widgets::meta_line(
                        theme,
                        vec![div().child(authority_meta).into_any_element()],
                    )),
            )
            .child(
                widgets::toggle_switch(theme, granted, format!("wakes-authority-{id}"))
                    .id(gpui::SharedString::from(format!(
                        "wakes-authority-toggle-{id}"
                    )))
                    .flex_none()
                    .role(gpui::Role::Switch)
                    .aria_label("Standing authority")
                    .aria_toggled(if granted {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                    .when(interactive, |el| {
                        el.cursor_pointer()
                            .tab_index(0)
                            .focus_visible(|s| s.border_2().border_color(theme.accent))
                            .on_click(cx.listener(move |card, _, _, cx| {
                                card.toggle_standing(&toggle_agent, cx)
                            }))
                    })
                    .when(!interactive, |el| el.opacity(0.5)),
            );

        // The grants, the Decision log switch and the log: the autonomy
        // card the phone and the website show too (#12045).
        let shared = autonomy_card(agent)
            .view(&card_instance(&agent.agent), 1)
            .map(|view| {
                let agent = agent.clone();
                crate::native_actions::mount(
                    &view,
                    move |card: &mut Self, activation, cx| card.act(&agent, &activation, cx),
                    cx,
                )
            });
        let log_row = widgets::card_row(theme, false).child(
            div()
                .flex_1()
                .min_w(px(160.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(shared.ok()),
        );
        div()
            .flex()
            .flex_col()
            .child(chat_row)
            .child(schedule_row)
            .child(authority_row)
            .child(log_row)
            .when_some(agent.problem.clone(), |el, problem| {
                el.child(
                    div()
                        .px(px(16.0))
                        .pb(px(16.0))
                        .child(widgets::error_strip(theme, problem)),
                )
            })
    }
}

impl Render for AgentWakesCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).for_settings_surface();
        let Some(status) = self.status.ready().cloned() else {
            let text = match &self.status {
                Loadable::Error(error) => error.clone(),
                _ => "Loading…".into(),
            };
            let card = widgets::section_card(&theme).mt_0().child(
                widgets::card_row(&theme, true).child(
                    div()
                        .flex_1()
                        .child(widgets::row_title(&theme, "Check-ins"))
                        .child(widgets::meta_line(
                            &theme,
                            vec![div().child(text).into_any_element()],
                        )),
                ),
            );
            return widgets::section(&theme, "Check-ins", card).into_any_element();
        };
        let mut card = widgets::section_card(&theme).mt_0();
        if status.agents.is_empty() {
            let text = if status.candidates.is_empty() {
                "Start an agent from Agent Lab and use it once. An agent whose definition asks for check-ins turns them on here by itself."
            } else {
                "Choose a saved agent's chat to give it check-ins."
            };
            card = card.child(
                widgets::card_row(&theme, true).child(
                    div()
                        .flex_1()
                        .child(widgets::row_title(&theme, "No agent checks in yet"))
                        .child(widgets::meta_line(
                            &theme,
                            vec![div().child(text).into_any_element()],
                        )),
                ),
            );
        }
        for agent in &status.agents {
            card = card.child(self.render_agent(&theme, &status, agent, cx));
        }
        if let Some(setup) = self.render_setup(&theme, &status, cx) {
            card = card.child(
                widgets::card_row(&theme, false)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.0))
                            .child(widgets::row_title(&theme, "Another agent")),
                    )
                    .child(setup),
            );
        }
        let error = self.error.clone().or(status.problem.clone());
        let card = card.when_some(error, |card, error| {
            card.child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .child(widgets::error_strip(&theme, error)),
            )
        });
        widgets::section(&theme, "Check-ins", card).into_any_element()
    }
}

fn card_instance(agent: &str) -> String {
    format!("agent-card-{}", &agent[..agent.len().min(24)])
}

/// The shared autonomy card for one agent on this computer (#12045).
fn autonomy_card(agent: &AgentWakeStatus) -> agent_lab::autonomy::Card {
    // The app's types and the account's share one JSON shape.
    fn same<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Option<T> {
        serde_json::from_value(v).ok()
    }
    let configured = agent.settings.chat_id.is_some();
    let next = match (configured, !agent.settings.paused, agent.next_check_in) {
        (false, _, _) => None,
        (true, false, _) => Some("Check-ins are paused.".to_owned()),
        (true, true, Some(at)) => Some(format!("Next check-in at {}.", local_time(at))),
        (true, true, None) => Some("Checking in soon.".to_owned()),
    };
    agent_lab::autonomy::Card {
        agent: agent.agent.clone(),
        name: agent.agent_name.clone(),
        autonomy: Some(agent_contract::Autonomy {
            check_ins: configured && !agent.settings.paused,
            every_minutes: Some(agent.settings.every_minutes),
            wakes: agent
                .wakes
                .iter()
                .filter_map(|w| same(serde_json::to_value(w).ok()?))
                .collect(),
            ..agent_contract::Autonomy::default()
        }),
        grants: agent
            .grants
            .iter()
            .filter_map(|g| same(serde_json::to_value(g).ok()?))
            .collect(),
        here: Some(agent_lab::autonomy::Here {
            chat: agent.chat_title.clone(),
            next_check_in: next,
            log: agent
                .decisions
                .iter()
                .rev()
                .take(SHOWN)
                .rev()
                .map(|e| format!("{}  {}", local_time(e.at()), entry_line(e)))
                .collect(),
            log_kept_before_grant: agent.log_kept_before_grant,
        }),
    }
}

/// Says this computer turned check-ins on by itself (#12030).
fn turned_on_line(at: i64) -> String {
    format!(
        "This computer turned on check-ins for this agent at {}, because its definition asks for them. Turn them off below.",
        local_time(at)
    )
}

fn every_label(minutes: u32) -> String {
    match minutes {
        m if m % 60 == 0 && m >= 120 => format!("{} hours", m / 60),
        60 => "1 hour".into(),
        m => format!("{m} minutes"),
    }
}

fn local_time(ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format("%b %-d, %H:%M").to_string())
        .unwrap_or_default()
}

fn entry_line(entry: &DecisionEntry) -> String {
    match entry {
        DecisionEntry::Wake { reason, events, .. } => match (reason, events.len()) {
            (WakeReason::Schedule, 0) => "Scheduled check-in.".into(),
            (WakeReason::Schedule, n) => format!("Scheduled check-in with {n} updates."),
            (WakeReason::Events, 1) => format!("Checked in: {}", events[0]),
            (WakeReason::Events, n) => format!("Checked in on {n} updates."),
            (WakeReason::Stuck, _) => "Checked in after stopping a stuck turn.".into(),
        },
        DecisionEntry::Decision { decided, did, .. } => format!("Decided: {decided} Did: {did}"),
        DecisionEntry::Outcome { status, .. } => match status.as_str() {
            "finished" => "Check-in finished.".into(),
            "errored" => "Check-in stopped with an error.".into(),
            "stuck" => "Stopped a turn that had stopped producing output.".into(),
            _ => "A check-in couldn't start.".into(),
        },
        DecisionEntry::Authority { granted: true, .. } => "Standing authority turned on.".into(),
        DecisionEntry::Authority { granted: false, .. } => "Standing authority turned off.".into(),
        DecisionEntry::Grant {
            granted, scopes, ..
        } => {
            let words: Vec<&str> = scopes.iter().map(|s| s.word()).collect();
            if *granted {
                format!("Granted {}.", words.join(", "))
            } else {
                format!("Revoked {}.", words.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #12045: any saved agent gets the shared autonomy card here, with
    /// its grants, Decision log switch and log, in plain words.
    #[test]
    fn any_agent_gets_the_shared_autonomy_card() {
        use openagents_proto::agent_wakes::{Grant, Wake};
        let agent = AgentWakeStatus {
            agent: "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3".into(),
            agent_name: "Release reviewer".into(),
            settings: WakeSettings {
                chat_id: Some("chat-1".into()),
                every_minutes: 30,
                ..WakeSettings::default()
            },
            chat_title: Some("Release review".into()),
            wakes: vec![Wake::ChatFinished],
            grants: vec![Grant {
                id: "g1".into(),
                scopes: vec![Scope::LogWrite],
                over: Over::Account,
                granted_at: 0,
            }],
            next_check_in: None,
            problem: None,
            turned_on_at: None,
            decisions: vec![DecisionEntry::Decision {
                at: 0,
                saw: "the build finished".into(),
                decided: "to merge".into(),
                did: "nothing".into(),
                judgments: vec![],
            }],
            log_kept_before_grant: false,
        };
        let card = autonomy_card(&agent);
        assert!(card.log_on());
        assert_eq!(card.log_grants(), ["g1"]);
        let view = card.view(&card_instance(&agent.agent), 1).unwrap();
        let texts = agent_lab::autonomy::texts(&view);
        assert!(texts.contains(&"Decision log: on".to_owned()), "{texts:?}");
        assert!(
            texts.contains(&"Runs in “Release review”".to_owned()),
            "{texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.ends_with("Decided: to merge Did: nothing")),
            "{texts:?}"
        );
        for text in &texts {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }

    #[test]
    fn card_text_is_plain() {
        let entries = [
            DecisionEntry::Wake {
                at: 0,
                id: "w".into(),
                reason: WakeReason::Events,
                events: vec!["Issue #1 opened: x".into()],
            },
            DecisionEntry::Decision {
                at: 0,
                saw: "a".into(),
                decided: "to wait".into(),
                did: "nothing".into(),
                judgments: vec![],
            },
            DecisionEntry::Outcome {
                at: 0,
                id: "w".into(),
                status: "not started".into(),
            },
            DecisionEntry::Outcome {
                at: 0,
                id: "w".into(),
                status: "stuck".into(),
            },
            DecisionEntry::Wake {
                at: 0,
                id: "s".into(),
                reason: WakeReason::Stuck,
                events: vec![],
            },
            DecisionEntry::Grant {
                at: 0,
                granted: true,
                id: "g".into(),
                scopes: Scope::CHAT.to_vec(),
                over: Over::Account,
            },
        ];
        for entry in &entries {
            let line = entry_line(entry);
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
        let line = turned_on_line(0);
        assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        assert_eq!(every_label(60), "1 hour");
        assert_eq!(every_label(240), "4 hours");
        assert_eq!(every_label(20), "20 minutes");
    }
}
