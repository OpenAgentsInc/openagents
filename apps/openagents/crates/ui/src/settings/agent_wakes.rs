//! Settings → General → Check-ins: every saved agent's check-ins, grants
//! and decision log on this computer (#11965, #12044). Any agent whose
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

impl AgentWakesCard {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
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

        let mut log = div().flex().flex_col().gap(px(6.0));
        let entries: Vec<&DecisionEntry> = agent.decisions.iter().rev().take(SHOWN).collect();
        if entries.is_empty() {
            log = log.child(description(
                theme,
                "Nothing yet. Each check-in adds what it saw, decided and did.",
            ));
        }
        for entry in entries {
            log = log.child(description(
                theme,
                &format!("{}  {}", local_time(entry.at()), entry_line(entry)),
            ));
        }
        let log_row = widgets::card_row(theme, false).child(
            div()
                .flex_1()
                .min_w(px(160.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(widgets::row_title(theme, "Decision log"))
                .child(log),
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

fn description(theme: &Theme, text: &str) -> gpui::Div {
    div()
        .text_size(crate::typography::ui_rems(widgets::ROW_DESCRIPTION_SIZE))
        .line_height(crate::typography::ui_rems(16.0))
        .text_color(theme.text_muted)
        .child(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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
