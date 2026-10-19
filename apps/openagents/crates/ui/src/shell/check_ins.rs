//! The sidebar's Check-ins list (#12173): each saved agent's scheduled
//! check-ins on this computer, how often they run, what wakes them, when they
//! last ran and run next, with Pause / Resume. It reads and writes the same
//! `GetAgentWakes` / `SetAgentWakes` calls as Settings → General → Check-ins,
//! so the two always agree. Nothing shows while no agent is scheduled.

use super::*;
use crate::settings::agent_wakes;
use openagents_proto::agent_wakes::{
    AgentWakeStatus, DecisionEntry, SetWakes, Wake, WakeSettings, WakesStatus,
};

const REFRESH: Duration = Duration::from_secs(30);

pub(super) struct SidebarCheckIns {
    state: Entity<AppState>,
    status: Option<WakesStatus>,
    open: bool,
    /// The agent whose Pause / Resume is in flight.
    saving: Option<String>,
    error: Option<String>,
    task: Option<Task<()>>,
    _refresh: Task<()>,
    _changed: Subscription,
}

impl SidebarCheckIns {
    pub(super) fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let changed = agent_wakes::observe_changes(cx, |this: &mut Self, cx| {
            if this.saving.is_none() {
                this.request(None, cx);
            }
        });
        if let Some(status) = agent_wakes::fixture() {
            return Self {
                state,
                status: Some(status.clone()),
                open: true,
                saving: None,
                error: None,
                task: None,
                _refresh: Task::ready(()),
                _changed: changed,
            };
        }
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH).await;
                if this
                    .update(cx, |this, cx| {
                        if this.saving.is_none() {
                            this.request(None, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            state,
            status: None,
            open: true,
            saving: None,
            error: None,
            task: None,
            _refresh: refresh,
            _changed: changed,
        };
        this.request(None, cx);
        this
    }

    /// Read the status, or save one agent's settings and read what comes back.
    fn request(&mut self, set: Option<SetWakes>, cx: &mut Context<Self>) {
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            return;
        };
        let saving = set.is_some();
        self.saving = set.as_ref().map(|s| s.agent.clone());
        let (method, params) = match set {
            Some(set) => (
                methods::SET_AGENT_WAKES,
                serde_json::to_value(set).unwrap_or_default(),
            ),
            None => (methods::GET_AGENT_WAKES, serde_json::json!({})),
        };
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = engine
                .client()
                .call(method, params)
                .await
                .map_err(|error| error.to_string())
                .and_then(|value| {
                    serde_json::from_value::<WakesStatus>(value).map_err(|e| e.to_string())
                });
            this.update(cx, |this, cx| {
                this.saving = None;
                match result {
                    Ok(status) => {
                        this.error = None;
                        this.status = Some(status);
                        if saving {
                            agent_wakes::changed(cx);
                        }
                    }
                    Err(_) if saving => {
                        this.error = Some("Couldn't save that. Try again.".into());
                    }
                    // A read that fails keeps the last list; the next one retries.
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn toggle_paused(&mut self, agent: &str, cx: &mut Context<Self>) {
        if self.saving.is_some() {
            return;
        }
        let Some(current) = self
            .status
            .as_ref()
            .and_then(|s| s.agents.iter().find(|a| a.agent == agent))
        else {
            return;
        };
        let settings = WakeSettings {
            paused: !current.settings.paused,
            ..current.settings.clone()
        };
        self.request(
            Some(SetWakes {
                agent: agent.to_owned(),
                settings,
            }),
            cx,
        );
    }

    fn render_agent(
        &self,
        theme: &Theme,
        agent: &AgentWakeStatus,
        now: i64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let paused = agent.settings.paused;
        let busy = self.saving.as_deref() == Some(agent.agent.as_str());
        let enabled = self.saving.is_none();
        let id = agent.agent.clone();
        let small = |text: String| {
            div()
                .text_size(crate::typography::ui_rems(11.5))
                .text_color(theme.text_muted)
                .truncate()
                .child(text)
        };
        let button = div()
            .id(SharedString::from(format!("sidebar-check-ins-toggle-{id}")))
            .flex_none()
            .px(px(8.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .text_size(crate::typography::ui_rems(11.5))
            .text_color(theme.text)
            .role(gpui::Role::Button)
            .aria_label(if paused {
                format!("Resume {}'s check-ins", agent.agent_name)
            } else {
                format!("Pause {}'s check-ins", agent.agent_name)
            })
            .child(match (busy, paused) {
                (true, true) => "Resuming…",
                (true, false) => "Pausing…",
                (false, true) => "Resume",
                (false, false) => "Pause",
            })
            .when(enabled, |el| {
                el.cursor_pointer()
                    .tab_index(0)
                    .hover(|s| s.bg(theme.glass_hover()))
                    .focus_visible(|s| s.border_color(theme.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_paused(&id, cx);
                    }))
            })
            .when(!enabled, |el| el.opacity(0.5));
        let state_word = div()
            .flex_none()
            .text_size(crate::typography::ui_rems(11.5))
            .text_color(if paused {
                theme.text_muted
            } else {
                theme.accent
            })
            .child(if paused { "Paused" } else { "On" });
        div()
            .px(px(Theme::SPACE_SM))
            .py(px(4.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(crate::typography::ui_rems(13.0))
                            .text_color(theme.text)
                            .child(agent.agent_name.clone()),
                    )
                    .child(state_word)
                    .child(button),
            )
            .child(small(every_text(agent.settings.every_minutes)))
            .when_some(wakes_text(&agent.wakes), |el, text| el.child(small(text)))
            .child(small(runs_text(agent, now)))
            .into_any_element()
    }
}

impl Render for SidebarCheckIns {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let agents: Vec<AgentWakeStatus> = self.status.as_ref().map(scheduled).unwrap_or_default();
        if agents.is_empty() {
            return div().into_any_element();
        }
        let open = self.open;
        let paused = agents.iter().filter(|a| a.settings.paused).count();
        let summary = match (paused, agents.len()) {
            (0, _) => None,
            (p, n) if p == n => Some("Paused"),
            _ => Some("Some paused"),
        };
        let chevron = icon(icons::ALT_ARROW_RIGHT)
            .size(px(12.0))
            .text_color(spaces::sidebar_header_glyph(&theme))
            .when(open, |el| {
                el.with_transformation(gpui::Transformation::rotate(gpui::radians(
                    std::f32::consts::FRAC_PI_2,
                )))
            })
            .into_any_element();
        let action = summary.map(|text| {
            div()
                .text_size(crate::typography::ui_rems(11.5))
                .text_color(theme.text_muted.opacity(0.7))
                .child(text)
                .into_any_element()
        });
        let header = spaces::sidebar_disclosure_header(
            &theme,
            None,
            "Check-ins".into(),
            None,
            action,
            chevron,
        )
        .id("sidebar-check-ins-toggle")
        .role(gpui::Role::Button)
        .aria_label("Check-ins")
        .on_click(cx.listener(|this, _, _, cx| {
            this.open = !this.open;
            cx.notify();
        }));
        let now = Utc::now().timestamp_millis();
        let rows: Vec<AnyElement> = if open {
            agents
                .iter()
                .map(|agent| self.render_agent(&theme, agent, now, cx))
                .collect()
        } else {
            Vec::new()
        };
        div()
            .flex_none()
            .flex()
            .flex_col()
            .pt(px(4.0))
            .child(header)
            .children(rows)
            .when_some(self.error.clone().filter(|_| open), |el, error| {
                el.child(
                    div()
                        .px(px(Theme::SPACE_SM))
                        .text_size(crate::typography::ui_rems(11.5))
                        .text_color(theme.danger)
                        .child(error),
                )
            })
            .into_any_element()
    }
}

/// The agents with a schedule: those whose check-ins have a chat to run in.
fn scheduled(status: &WakesStatus) -> Vec<AgentWakeStatus> {
    status
        .agents
        .iter()
        .filter(|a| a.settings.chat_id.is_some())
        .cloned()
        .collect()
}

fn every_text(minutes: u32) -> String {
    match minutes {
        60 => "Every hour".into(),
        m if m % 60 == 0 && m > 60 => format!("Every {} hours", m / 60),
        1 => "Every minute".into(),
        m => format!("Every {m} minutes"),
    }
}

/// What wakes it besides its schedule, or `None` when nothing does.
fn wakes_text(wakes: &[Wake]) -> Option<String> {
    if wakes.is_empty() {
        return None;
    }
    if Wake::ALL.iter().all(|w| wakes.contains(w)) {
        return Some("Also wakes on any chat or issue update".into());
    }
    let words: Vec<&str> = wakes
        .iter()
        .map(|w| match w {
            Wake::ChatFinished => "finished chats",
            Wake::ChatErrored => "chat errors",
            Wake::ChatNeedsAnswer => "chats needing an answer",
            Wake::IssueOpened => "new issues",
            Wake::IssueClosed => "closed issues",
            Wake::ClaimStale => "stale claims",
            Wake::ChatStalled => "stuck chats",
            Wake::ClaimNoProgress => "claims with no progress",
        })
        .collect();
    Some(format!("Also wakes on {}", words.join(", ")))
}

/// When it last checked in, from its log.
fn last_run(agent: &AgentWakeStatus) -> Option<i64> {
    agent
        .decisions
        .iter()
        .filter_map(|e| match e {
            DecisionEntry::Wake { at, .. } => Some(*at),
            _ => None,
        })
        .max()
}

fn runs_text(agent: &AgentWakeStatus, now: i64) -> String {
    let last = last_run(agent).map(|at| format!("Last {}", short_time(at, now)));
    let next = if agent.settings.paused {
        None
    } else {
        Some(match agent.next_check_in {
            Some(at) => format!("next {}", short_time(at, now)),
            None => "next soon".into(),
        })
    };
    match (last, next) {
        (Some(last), Some(next)) => format!("{last} · {next}"),
        (Some(last), None) => last,
        (None, Some(next)) => {
            let mut next = next;
            next[..1].make_ascii_uppercase();
            next
        }
        (None, None) => "Hasn't checked in yet".into(),
    }
}

/// `14:05` today, `Oct 18 14:05` on another day.
fn short_time(ms: i64, now: i64) -> String {
    use chrono::TimeZone;
    let Some(at) = chrono::Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    let today = chrono::Local
        .timestamp_millis_opt(now)
        .single()
        .map(|n| n.date_naive());
    if today == Some(at.date_naive()) {
        at.format("%H:%M").to_string()
    } else {
        at.format("%b %-d %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(paused: bool, chat: bool) -> AgentWakeStatus {
        AgentWakeStatus {
            agent: "a".into(),
            agent_name: "Foreman".into(),
            settings: WakeSettings {
                chat_id: chat.then(|| "chat".into()),
                every_minutes: 10,
                paused,
                ..WakeSettings::default()
            },
            chat_title: None,
            wakes: vec![Wake::IssueOpened, Wake::IssueClosed, Wake::ClaimStale],
            grants: vec![],
            next_check_in: Some(1_000_600_000),
            problem: None,
            turned_on_at: None,
            decisions: vec![DecisionEntry::Wake {
                at: 1_000_000_000,
                id: "w".into(),
                reason: openagents_proto::agent_wakes::WakeReason::Schedule,
                events: vec![],
            }],
            log_kept_before_grant: false,
        }
    }

    #[test]
    fn only_scheduled_agents_show() {
        let status = WakesStatus {
            agents: vec![agent(false, true), agent(false, false)],
            ..WakesStatus::default()
        };
        assert_eq!(scheduled(&status).len(), 1);
        assert!(scheduled(&WakesStatus::default()).is_empty());
    }

    #[test]
    fn lines_are_plain() {
        let on = agent(false, true);
        let off = agent(true, true);
        let lines = [
            every_text(10),
            every_text(60),
            every_text(240),
            wakes_text(&on.wakes).unwrap(),
            wakes_text(&Wake::ALL).unwrap(),
            runs_text(&on, 1_000_000_000),
            runs_text(&off, 1_000_000_000),
            runs_text(
                &AgentWakeStatus {
                    decisions: vec![],
                    ..on.clone()
                },
                1_000_000_000,
            ),
        ];
        assert_eq!(lines[0], "Every 10 minutes");
        assert_eq!(lines[1], "Every hour");
        assert_eq!(lines[2], "Every 4 hours");
        assert_eq!(
            lines[3],
            "Also wakes on new issues, closed issues, stale claims"
        );
        assert!(lines[5].starts_with("Last ") && lines[5].contains(" · next "));
        assert!(!lines[6].contains("next"), "paused shows no next run");
        assert!(lines[7].starts_with("Next "));
        for line in &lines {
            assert!(oa_copy::violations(line, &[]).is_empty(), "{line}");
        }
        assert_eq!(last_run(&on), Some(1_000_000_000));
    }
}
