//! Settings → General: Foreman's check-ins, standing authority and
//! decision log (#11965). The engine that hosts Foreman's chat keeps all
//! three; this card reads and sets them on this computer.

use std::time::Duration;

use gpui::{AnyElement, Context, Entity, Render, Task, Window, div, prelude::*, px};
use openagents_proto::foreman::{DecisionEntry, ForemanSettings, ForemanStatus, WakeReason};
use openagents_rpc::methods;

use super::widgets;
use crate::popover::Loadable;
use crate::state::AppState;
use crate::theme::Theme;

/// The intervals offered, in minutes.
const EVERY: [u32; 6] = [10, 20, 30, 60, 120, 240];
/// The log lines shown, newest first.
const SHOWN: usize = 8;
const REFRESH: Duration = Duration::from_secs(30);

pub(super) struct ForemanCard {
    state: Entity<AppState>,
    status: Loadable<ForemanStatus>,
    error: Option<String>,
    saving: bool,
    chat_select: widgets::SelectState,
    every_select: widgets::SelectState,
    task: Option<Task<()>>,
    _refresh: Task<()>,
}

impl ForemanCard {
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
            chat_select: widgets::SelectState::default(),
            every_select: widgets::SelectState::default(),
            task: None,
            _refresh: refresh,
        };
        card.request(None, cx);
        card
    }

    fn request(&mut self, save: Option<ForemanSettings>, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.status = Loadable::Error("This computer isn't connected yet.".into());
            cx.notify();
            return;
        };
        let saving = save.is_some();
        self.saving = saving;
        let (method, params) = match save {
            Some(settings) => (
                methods::SET_FOREMAN,
                serde_json::to_value(settings).unwrap_or_default(),
            ),
            None => (methods::GET_FOREMAN, serde_json::json!({})),
        };
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(method, params)
                .await
                .map_err(|error| match error {
                    openagents_rpc::RpcError::UnknownMethod(_) => {
                        "Update OpenAgents on this computer to use Foreman's check-ins.".to_string()
                    }
                    _ => error.to_string(),
                })
                .and_then(|value| {
                    serde_json::from_value::<ForemanStatus>(value).map_err(|e| e.to_string())
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

    fn change(&mut self, edit: impl FnOnce(&mut ForemanSettings), cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(status) = self.status.ready() else {
            return;
        };
        let mut settings = status.settings.clone();
        edit(&mut settings);
        if settings != status.settings {
            self.request(Some(settings), cx);
        }
    }

    fn render_chat(
        &mut self,
        theme: &Theme,
        status: &ForemanStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut options = vec![widgets::SelectOption::new("None")];
        let mut ids: Vec<Option<String>> = vec![None];
        for candidate in &status.candidates {
            options.push(widgets::SelectOption::new(candidate.title.clone()));
            ids.push(Some(candidate.chat_id.clone()));
        }
        if let Some(chat) = &status.settings.chat_id
            && !ids.contains(&Some(chat.clone()))
        {
            options.push(widgets::SelectOption::new(
                status
                    .chat_title
                    .clone()
                    .unwrap_or_else(|| "Current chat".into()),
            ));
            ids.push(Some(chat.clone()));
        }
        let selected = ids
            .iter()
            .position(|id| *id == status.settings.chat_id)
            .unwrap_or(0);
        widgets::select(
            "foreman-chat",
            "Foreman's chat",
            theme,
            |card: &mut Self| &mut card.chat_select,
        )
        .options(options, selected)
        .menu_width(260.0)
        .on_select(move |card, ix, _, cx| {
            let chosen = ids.get(ix).cloned().flatten();
            card.change(|s| s.chat_id = chosen, cx);
        })
        .render(&self.chat_select, cx)
        .into_any_element()
    }

    fn render_every(
        &mut self,
        theme: &Theme,
        status: &ForemanStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = status.settings.every_minutes;
        let mut minutes: Vec<u32> = EVERY.to_vec();
        if !minutes.contains(&current) {
            minutes.push(current);
            minutes.sort_unstable();
        }
        let options = minutes
            .iter()
            .map(|m| widgets::SelectOption::new(every_label(*m)));
        let selected = minutes.iter().position(|m| *m == current).unwrap_or(0);
        widgets::select(
            "foreman-every",
            "Check in every",
            theme,
            |card: &mut Self| &mut card.every_select,
        )
        .options(options, selected)
        .on_select(move |card, ix, _, cx| {
            if let Some(m) = minutes.get(ix).copied() {
                card.change(|s| s.every_minutes = m, cx);
            }
        })
        .render(&self.every_select, cx)
        .into_any_element()
    }
}

impl Render for ForemanCard {
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
                        .child(widgets::row_title(&theme, "Foreman"))
                        .child(widgets::meta_line(
                            &theme,
                            vec![div().child(text).into_any_element()],
                        )),
                ),
            );
            return widgets::section(&theme, "Foreman", card).into_any_element();
        };
        let interactive = !self.saving;
        let configured = status.settings.chat_id.is_some();
        let on = !status.settings.paused;

        let chat_meta = match (configured, status.turned_on_at) {
            (true, Some(at)) => turned_on_line(at, status.authority.is_some()),
            (true, None) => {
                "Foreman checks in on its own and when the chats it manages need it.".into()
            }
            (false, _) if status.candidates.is_empty() => {
                "Start Foreman from Agent Lab and use it once, then choose its chat here.".into()
            }
            (false, _) => "Choose the chat Foreman answers in.".to_string(),
        };
        let chat_row = widgets::card_row(&theme, true)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(&theme, "Foreman's chat"))
                    .child(widgets::meta_line(
                        &theme,
                        vec![div().child(chat_meta).into_any_element()],
                    )),
            )
            .child(self.render_chat(&theme, &status, cx));

        let next = match (configured, on, status.next_check_in) {
            (false, _, _) => "Choose Foreman's chat first.".to_string(),
            (true, false, _) => "Check-ins are paused.".to_string(),
            (true, true, Some(at)) => format!("Next check-in at {}.", local_time(at)),
            (true, true, None) => "Checking in soon.".to_string(),
        };
        let every = self.render_every(&theme, &status, cx);
        let schedule_row = widgets::card_row(&theme, false)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(&theme, "Check-ins"))
                    .child(widgets::meta_line(
                        &theme,
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
                        widgets::toggle_switch(&theme, on, "foreman-on")
                            .id("foreman-on-toggle")
                            .flex_none()
                            .role(gpui::Role::Switch)
                            .aria_label("Foreman check-ins")
                            .aria_toggled(if on {
                                gpui::Toggled::True
                            } else {
                                gpui::Toggled::False
                            })
                            .when(interactive && configured, |el| {
                                el.cursor_pointer()
                                    .tab_index(0)
                                    .focus_visible(|s| s.border_2().border_color(theme.accent))
                                    .on_click(cx.listener(|card, _, _, cx| {
                                        card.change(|s| s.paused = !s.paused, cx)
                                    }))
                            })
                            .when(!(interactive && configured), |el| el.opacity(0.5)),
                    ),
            );

        let granted = status.authority.is_some();
        let authority_meta = match &status.authority {
            Some(a) => format!(
                "Since {}, Foreman can message, start, steer and stop chats without asking. Money, keys, secrets, deploys and closing others' issues still need you.",
                local_time(a.granted_at)
            ),
            None => "Off: on check-ins Foreman only looks and reports. Turn on to let it message, start, steer and stop chats without asking.".into(),
        };
        let authority_row = widgets::card_row(&theme, false)
            .child(
                div()
                    .flex_1()
                    .min_w(px(160.0))
                    .child(widgets::row_title(&theme, "Standing authority"))
                    .child(widgets::meta_line(
                        &theme,
                        vec![div().child(authority_meta).into_any_element()],
                    )),
            )
            .child(
                widgets::toggle_switch(&theme, granted, "foreman-authority")
                    .id("foreman-authority-toggle")
                    .flex_none()
                    .role(gpui::Role::Switch)
                    .aria_label("Foreman's standing authority")
                    .aria_toggled(if granted {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                    .when(interactive && configured, |el| {
                        el.cursor_pointer()
                            .tab_index(0)
                            .focus_visible(|s| s.border_2().border_color(theme.accent))
                            .on_click(cx.listener(|card, _, _, cx| {
                                card.change(|s| s.authority = !s.authority, cx)
                            }))
                    })
                    .when(!(interactive && configured), |el| el.opacity(0.5)),
            );

        let mut log = div().flex().flex_col().gap(px(6.0));
        let entries: Vec<&DecisionEntry> = status.decisions.iter().rev().take(SHOWN).collect();
        if entries.is_empty() {
            log = log.child(description(
                &theme,
                "Nothing yet. Each check-in adds what Foreman saw, decided and did.",
            ));
        }
        for entry in entries {
            log = log.child(description(
                &theme,
                &format!("{}  {}", local_time(entry.at()), entry_line(entry)),
            ));
        }
        let log_row = widgets::card_row(&theme, false).child(
            div()
                .flex_1()
                .min_w(px(160.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(widgets::row_title(&theme, "Decision log"))
                .child(log),
        );

        let error = self.error.clone().or(status.problem.clone());
        let card = widgets::section_card(&theme)
            .mt_0()
            .child(chat_row)
            .child(schedule_row)
            .child(authority_row)
            .child(log_row)
            .when_some(error, |card, error| {
                card.child(
                    div()
                        .px(px(16.0))
                        .pb(px(16.0))
                        .child(widgets::error_strip(&theme, error)),
                )
            });
        widgets::section(&theme, "Foreman", card).into_any_element()
    }
}

/// Says this computer turned check-ins on by itself (#12030), and that
/// either switch below turns them off.
fn turned_on_line(at: i64, authority: bool) -> String {
    format!(
        "This computer turned on {} for this agent at {}, because its definition asks for them. Turn either off below.",
        if authority {
            "check-ins and standing authority"
        } else {
            "check-ins"
        },
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
                judgments: Vec::new(),
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
        ];
        for entry in &entries {
            let line = entry_line(entry);
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
        for authority in [true, false] {
            let line = turned_on_line(0, authority);
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
        assert_eq!(every_label(60), "1 hour");
        assert_eq!(every_label(240), "4 hours");
        assert_eq!(every_label(20), "20 minutes");
    }
}
