//! Check-ins use the paired computer's schedule, not a browser timer.
use openagents_proto::agent_wakes::{AgentWakeStatus, SetWakes};

pub fn pause_change(agent: &AgentWakeStatus) -> SetWakes {
    let mut settings = agent.settings.clone();
    settings.paused = !settings.paused;
    SetWakes {
        agent: agent.agent.clone(),
        settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::agent_wakes::{WakeSettings, WakesStatus};

    #[test]
    fn web_pause_resume_preserves_host_settings() {
        let mut status: WakesStatus = serde_json::from_value(serde_json::json!({
            "agents": [{ "agent": "reviewer", "agentName": "Reviewer",
                "settings": { "chatId": "chat", "everyMinutes": 20, "paused": false,
                    "stuckMinutes": 17, "stallMinutes": 91 },
                "chatTitle": "Review", "wakes": [], "grants": [],
                "nextCheckIn": 600000, "problem": null, "decisions": [] }],
            "candidates": [], "problem": null
        }))
        .unwrap();
        let before = status.agents[0].settings.clone();
        let paused = pause_change(&status.agents[0]);
        assert!(paused.settings.paused);
        assert_eq!(
            paused.settings,
            WakeSettings {
                paused: true,
                ..before.clone()
            }
        );
        status.agents[0].settings = paused.settings;
        assert_eq!(pause_change(&status.agents[0]).settings, before);
        assert_eq!(pause_change(&status.agents[0]).agent, "reviewer");
    }
}

#[cfg(target_family = "wasm")]
mod view {
    use super::*;
    use gpui::{Context, Entity, Render, Task, Window, div, prelude::*, px};
    use openagents_proto::agent_wakes::{
        DecisionEntry, GrantRequest, Over, RevokeGrant, Scope, WakesStatus,
    };
    use openagents_web_chat_view::{
        composer::{Submitted, TextInput},
        theme,
    };
    use openagents_web_client::{computer, web::Client};
    use std::time::Duration;

    pub struct CheckIns {
        client: Client,
        device: String,
        input: Entity<TextInput>,
        status: WakesStatus,
        busy: bool,
        error: Option<String>,
        details: bool,
        task: Option<Task<()>>,
        _refresh: Task<()>,
    }

    impl CheckIns {
        pub fn new(cx: &mut Context<Self>) -> Self {
            let input = cx.new(|cx| TextInput::new("Computer device ID", cx));
            cx.subscribe(&input, |this, _, event: &Submitted, cx| {
                if this.busy {
                    return;
                }
                this.device = event.0.trim().to_owned();
                this.status = WakesStatus::default();
                this.request("GetAgentWakes", serde_json::json!({}), cx);
            })
            .detach();
            let refresh = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_secs(30))
                        .await;
                    if this
                        .update(cx, |this, cx| {
                            if !this.busy && !this.device.is_empty() {
                                this.request("GetAgentWakes", serde_json::json!({}), cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Self {
                client: Client::from_page(),
                device: String::new(),
                input,
                status: WakesStatus::default(),
                busy: false,
                error: None,
                details: false,
                task: None,
                _refresh: refresh,
            }
        }

        fn request(
            &mut self,
            method: &'static str,
            params: serde_json::Value,
            cx: &mut Context<Self>,
        ) {
            if self.busy || self.device.is_empty() {
                return;
            }
            self.busy = true;
            self.error = None;
            let client = self.client.clone();
            let device = self.device.clone();
            self.task = Some(cx.spawn(async move |this, cx| {
                let result = async {
                    let host = computer::connect(&client, &device, || {}).await?;
                    let value = host.call(method, params).await?;
                    Ok::<WakesStatus, anyhow::Error>(serde_json::from_value(value)?)
                }.await;
                this.update(cx, |this, cx| {
                    this.busy = false;
                    match result {
                        Ok(status) => this.status = status,
                        Err(_) => this.error = Some("Couldn't load or save check-ins. Allow this browser in Settings → Devices on your computer, then try again.".into()),
                    }
                    cx.notify();
                }).ok();
            }));
            cx.notify();
        }

        fn action(
            &self,
            id: String,
            label: impl Into<String>,
            method: &'static str,
            params: serde_json::Value,
            cx: &mut Context<Self>,
        ) -> impl IntoElement {
            div()
                .id(id)
                .cursor_pointer()
                .text_color(theme::text_muted())
                .child(label.into())
                .on_click(
                    cx.listener(move |this, _, _, cx| this.request(method, params.clone(), cx)),
                )
        }
    }

    fn time(at: i64) -> String {
        js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(at as f64))
            .to_locale_string("default", &wasm_bindgen::JsValue::UNDEFINED)
            .into()
    }

    impl Render for CheckIns {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .text_size(px(12.))
                .text_color(theme::text())
                .child(
                    div()
                        .id("check-ins-details")
                        .cursor_pointer()
                        .child("Check-ins")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.details = !this.details;
                            cx.notify();
                        })),
                )
                .child(self.input.clone())
                .child(self.action(
                    "refresh-check-ins".into(),
                    if self.busy { "Working…" } else { "Refresh" },
                    "GetAgentWakes",
                    serde_json::json!({}),
                    cx,
                ))
                .children(self.error.clone().map(|e| div().child(e)))
                .children(self.status.problem.clone().map(|e| div().child(e)))
                .children(
                    self.status
                        .agents
                        .iter()
                        .filter(|a| self.details || a.settings.chat_id.is_some())
                        .map(|a| {
                            let set = pause_change(a);
                            let mut row = div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(a.agent_name.clone())
                                .child(format!("Every {} minutes", a.settings.every_minutes))
                                .children(a.wakes.iter().map(|w| {
                                    div().child(match w {
                                        openagents_proto::agent_wakes::Wake::ChatFinished => {
                                            "Chat finished"
                                        }
                                        openagents_proto::agent_wakes::Wake::ChatErrored => {
                                            "Chat failed"
                                        }
                                        openagents_proto::agent_wakes::Wake::ChatNeedsAnswer => {
                                            "Chat needs an answer"
                                        }
                                        openagents_proto::agent_wakes::Wake::IssueOpened => {
                                            "Issue opened"
                                        }
                                        openagents_proto::agent_wakes::Wake::IssueClosed => {
                                            "Issue closed"
                                        }
                                        openagents_proto::agent_wakes::Wake::ClaimStale => {
                                            "Claim is stale"
                                        }
                                        openagents_proto::agent_wakes::Wake::ChatStalled => {
                                            "Chat stopped making progress"
                                        }
                                        openagents_proto::agent_wakes::Wake::ClaimNoProgress => {
                                            "Claim stopped making progress"
                                        }
                                    })
                                }))
                                .child(if a.settings.paused {
                                    "Paused".into()
                                } else {
                                    a.next_check_in
                                        .map(|at| format!("Next: {}", time(at)))
                                        .unwrap_or_else(|| "Checking in soon".into())
                                })
                                .children(a.decisions.iter().rev().find_map(|d| match d {
                                    DecisionEntry::Wake { at, .. } => {
                                        Some(div().child(format!("Last: {}", time(*at))))
                                    }
                                    _ => None,
                                }))
                                .child(self.action(
                                    format!("pause-{}", a.agent),
                                    if a.settings.paused { "Resume" } else { "Pause" },
                                    "SetAgentWakes",
                                    serde_json::to_value(set).unwrap(),
                                    cx,
                                ));
                            if self.details {
                                row = row
                                    .child(format!(
                                        "Chat: {}",
                                        a.chat_title.as_deref().unwrap_or("None")
                                    ))
                                    .child(if a.standing() {
                                        "Standing authority is on"
                                    } else {
                                        "Standing authority is off"
                                    });
                                for minutes in [10, 20, 30, 60, 120, 240] {
                                    let mut settings = a.settings.clone();
                                    settings.every_minutes = minutes;
                                    row = row.child(
                                        self.action(
                                            format!("interval-{}-{minutes}", a.agent),
                                            format!("{minutes} minutes"),
                                            "SetAgentWakes",
                                            serde_json::to_value(SetWakes {
                                                agent: a.agent.clone(),
                                                settings,
                                            })
                                            .unwrap(),
                                            cx,
                                        ),
                                    );
                                }
                                for candidate in
                                    self.status.candidates.iter().filter(|c| c.agent == a.agent)
                                {
                                    let mut settings = a.settings.clone();
                                    settings.chat_id = Some(candidate.chat_id.clone());
                                    row = row.child(
                                        self.action(
                                            format!("chat-{}", candidate.chat_id),
                                            candidate.title.clone(),
                                            "SetAgentWakes",
                                            serde_json::to_value(SetWakes {
                                                agent: a.agent.clone(),
                                                settings,
                                            })
                                            .unwrap(),
                                            cx,
                                        ),
                                    );
                                }
                                for (label, scopes) in [
                                    (
                                        "Allow standing authority across all chats",
                                        Scope::CHAT.to_vec(),
                                    ),
                                    ("Allow decision log", vec![Scope::LogWrite]),
                                ] {
                                    row = row.child(
                                        self.action(
                                            format!("grant-{}-{label}", a.agent),
                                            label,
                                            "GrantAgent",
                                            serde_json::to_value(GrantRequest {
                                                agent: a.agent.clone(),
                                                scopes: scopes
                                                    .iter()
                                                    .map(|s| s.word().to_owned())
                                                    .collect(),
                                                over: Over::Account,
                                            })
                                            .unwrap(),
                                            cx,
                                        ),
                                    );
                                }
                                for grant in &a.grants {
                                    row = row.child(
                                        self.action(
                                            format!("revoke-{}", grant.id),
                                            format!(
                                                "Revoke {}",
                                                grant
                                                    .scopes
                                                    .iter()
                                                    .map(|s| s.word())
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            ),
                                            "RevokeAgentGrant",
                                            serde_json::to_value(RevokeGrant {
                                                agent: a.agent.clone(),
                                                grant: grant.id.clone(),
                                            })
                                            .unwrap(),
                                            cx,
                                        ),
                                    );
                                }
                                for decision in a.decisions.iter().rev().take(8) {
                                    let line = match decision {
                                        DecisionEntry::Decision { decided, did, .. } => {
                                            format!("{decided} — {did}")
                                        }
                                        DecisionEntry::Wake { .. } => "Checked in".into(),
                                        DecisionEntry::Outcome { status, .. } => status.clone(),
                                        DecisionEntry::Authority { granted, .. }
                                        | DecisionEntry::Grant { granted, .. } => if *granted {
                                            "Permission granted"
                                        } else {
                                            "Permission revoked"
                                        }
                                        .into(),
                                    };
                                    row = row.child(format!("{}: {line}", time(decision.at())));
                                }
                            }
                            row
                        }),
                )
        }
    }
}
#[cfg(target_family = "wasm")]
pub use view::CheckIns;
