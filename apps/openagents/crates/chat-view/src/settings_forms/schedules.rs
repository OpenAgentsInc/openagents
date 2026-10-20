//! Account scheduled prompts. Computers execute the same records through sync.
use super::memory::{field, paging};
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub id: String,
    pub computer: String,
    pub prompt: String,
    pub days: Vec<u8>,
    pub time: Option<String>,
    pub every_secs: Option<u64>,
    pub chat: Option<String>,
    pub chat_title: Option<String>,
    pub paused: bool,
    pub last_ran_at: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub session: String,
    pub computer: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Listed {
    pub schedules: Vec<Schedule>,
    pub computers: Vec<String>,
    pub chats: Vec<Chat>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub schedule: Schedule,
    pub interval: bool,
    pub seconds: String,
}

impl Draft {
    pub fn body(&self) -> Result<serde_json::Value, &'static str> {
        let schedule = &self.schedule;
        if schedule.computer.trim().is_empty() {
            return Err("Choose a computer.");
        }
        if schedule.prompt.trim().is_empty() {
            return Err("Enter a prompt.");
        }
        let seconds = if self.interval {
            Some(
                self.seconds
                    .parse::<u64>()
                    .ok()
                    .filter(|s| (60..=2_592_000).contains(s))
                    .ok_or("Enter an interval from 60 to 2592000 seconds.")?,
            )
        } else {
            None
        };
        Ok(
            serde_json::json!({"computer": schedule.computer, "prompt": schedule.prompt, "days": if self.interval { vec![] } else { schedule.days.clone() }, "time": if self.interval { None } else { schedule.time.clone() }, "every_secs": seconds, "chat": schedule.chat, "paused": schedule.paused}),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub listed: Load<Listed>,
    pub draft: Option<Draft>,
    pub busy: bool,
    pub page: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Reload,
    Add,
    Edit(String),
    Delete(String),
    Pause(String, bool),
    Save,
    Cancel,
    Computer(String),
    Chat(Option<String>),
    Interval(bool),
    Day(u8),
    Field(String),
    Page(usize),
}

impl State {
    pub fn edit(&mut self, id: Option<&str>) {
        let Some(listed) = self.listed.ready() else {
            return;
        };
        let schedule = match id {
            Some(id) => listed.schedules.iter().find(|s| s.id == id).cloned(),
            None => listed.computers.first().map(|computer| Schedule {
                computer: computer.clone(),
                time: Some("09:00".into()),
                ..Schedule::default()
            }),
        };
        self.draft = schedule.map(|s| Draft {
            interval: s.every_secs.is_some(),
            seconds: s.every_secs.unwrap_or(3600).to_string(),
            schedule: s,
        });
    }

    pub fn field(&mut self, key: &str, value: String) {
        if let Some(draft) = &mut self.draft {
            match key {
                "schedule-prompt" => draft.schedule.prompt = value,
                "schedule-time" => draft.schedule.time = Some(value),
                "schedule-seconds" => draft.seconds = value,
                _ => {}
            }
        }
    }
}

pub fn page(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let action = |a| Intent::Schedules(a);
    let mut out = vec![header(
        "schedules-title",
        "Schedules",
        Some("Prompts that run on your computers."),
        paints,
    )];
    match &state.listed {
        Load::Idle | Load::Loading => out.push(text(
            "schedules-loading",
            "Loading schedules…",
            paints.muted,
        )),
        Load::Failed(message) => {
            out.push(text("schedules-error", message, paints.danger));
            out.push(button(
                "schedules-retry",
                "Try again",
                action(Action::Reload),
            ));
        }
        Load::Ready(listed) => {
            if listed.computers.is_empty() {
                out.push(text(
                    "schedules-no-computer",
                    "Connect a computer to add scheduled prompts.",
                    paints.muted,
                ));
            } else if state.draft.is_none() {
                out.push(enabled_button(
                    "schedules-add",
                    "Add schedule",
                    !state.busy,
                    action(Action::Add),
                ));
            }
            if listed.schedules.is_empty() {
                out.push(text("schedules-empty", "No schedules yet.", paints.muted));
            }
            let start = state
                .page
                .min(listed.schedules.len().saturating_sub(1) / 20)
                * 20;
            for s in listed.schedules.iter().skip(start).take(20) {
                let key = format!("schedule-{}", s.id);
                let when = s
                    .every_secs
                    .map(|n| format!("Every {n} seconds"))
                    .unwrap_or_else(|| {
                        format!(
                            "At {} on {}",
                            s.time.as_deref().unwrap_or("09:00"),
                            if s.days.is_empty() {
                                "every day".into()
                            } else {
                                s.days
                                    .iter()
                                    .filter_map(|d| {
                                        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                                            .get(*d as usize)
                                            .copied()
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        )
                    });
                let detail = format!(
                    "{when} · {} · {}{}",
                    s.computer,
                    s.chat_title.as_deref().unwrap_or("New run"),
                    if s.paused { " · Paused" } else { "" }
                );
                out.push(wide_row(
                    &key,
                    &s.prompt,
                    Some(&detail),
                    vec![
                        enabled_button(
                            &format!("{key}-edit"),
                            "Edit",
                            !state.busy,
                            action(Action::Edit(s.id.clone())),
                        ),
                        enabled_button(
                            &format!("{key}-pause"),
                            if s.paused { "Resume" } else { "Pause" },
                            !state.busy,
                            action(Action::Pause(s.id.clone(), !s.paused)),
                        ),
                        enabled_button(
                            &format!("{key}-delete"),
                            "Delete",
                            !state.busy,
                            action(Action::Delete(s.id.clone())),
                        ),
                    ],
                    paints,
                ));
            }
            paging(&mut out, "schedules", start, listed.schedules.len(), |p| {
                action(Action::Page(p))
            });
            if let Some(draft) = &state.draft {
                let s = &draft.schedule;
                let mut fields = vec![header(
                    "schedule-edit-title",
                    if s.id.is_empty() {
                        "Add schedule"
                    } else {
                        "Edit schedule"
                    },
                    None,
                    paints,
                )];
                fields.push(field(
                    "schedule-prompt",
                    "Prompt",
                    &s.prompt,
                    true,
                    4000,
                    !state.busy,
                    action(Action::Field("schedule-prompt".into())),
                ));
                // The account preserves an existing schedule's computer on edit.
                if s.id.is_empty() {
                    fields.push(wide_row(
                        "schedule-computers",
                        "Computer",
                        None,
                        listed
                            .computers
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                enabled_button(
                                    &format!("schedule-computer-{i}"),
                                    &format!("{}{}", if *c == s.computer { "✓ " } else { "" }, c),
                                    !state.busy,
                                    action(Action::Computer(c.clone())),
                                )
                            })
                            .collect(),
                        paints,
                    ));
                }
                fields.push(wide_row(
                    "schedule-repeat",
                    "Repeat",
                    Some("Times use the computer’s clock."),
                    vec![
                        enabled_button(
                            "schedule-daily",
                            if !draft.interval {
                                "✓ At a time"
                            } else {
                                "At a time"
                            },
                            !state.busy,
                            action(Action::Interval(false)),
                        ),
                        enabled_button(
                            "schedule-every",
                            if draft.interval {
                                "✓ Every…"
                            } else {
                                "Every…"
                            },
                            !state.busy,
                            action(Action::Interval(true)),
                        ),
                    ],
                    paints,
                ));
                if draft.interval {
                    fields.push(field(
                        "schedule-seconds",
                        "Interval in seconds",
                        &draft.seconds,
                        false,
                        8,
                        !state.busy,
                        action(Action::Field("schedule-seconds".into())),
                    ));
                } else {
                    fields.push(field(
                        "schedule-time",
                        "Time (HH:MM)",
                        s.time.as_deref().unwrap_or("09:00"),
                        false,
                        5,
                        !state.busy,
                        action(Action::Field("schedule-time".into())),
                    ));
                    fields.push(wide_row(
                        "schedule-days",
                        "Days",
                        Some("Choose none for every day."),
                        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                            .iter()
                            .enumerate()
                            .map(|(d, label)| {
                                enabled_button(
                                    &format!("schedule-day-{d}"),
                                    &format!(
                                        "{}{}",
                                        if s.days.contains(&(d as u8)) {
                                            "✓ "
                                        } else {
                                            ""
                                        },
                                        label
                                    ),
                                    !state.busy,
                                    action(Action::Day(d as u8)),
                                )
                            })
                            .collect(),
                        paints,
                    ));
                }
                let mut chats = vec![enabled_button(
                    "schedule-new-run",
                    if s.chat.is_none() {
                        "✓ New run"
                    } else {
                        "New run"
                    },
                    !state.busy,
                    action(Action::Chat(None)),
                )];
                chats.extend(
                    listed
                        .chats
                        .iter()
                        .filter(|c| c.computer == s.computer)
                        .take(100)
                        .enumerate()
                        .map(|(i, c)| {
                            enabled_button(
                                &format!("schedule-chat-{i}"),
                                &format!(
                                    "{}{}",
                                    if s.chat.as_deref() == Some(&c.session) {
                                        "✓ "
                                    } else {
                                        ""
                                    },
                                    c.title
                                ),
                                !state.busy,
                                action(Action::Chat(Some(c.session.clone()))),
                            )
                        }),
                );
                fields.push(wide_row("schedule-chats", "Post in", None, chats, paints));
                fields.push(enabled_button(
                    "schedule-save",
                    if state.busy {
                        "Saving…"
                    } else {
                        "Save schedule"
                    },
                    !state.busy && !s.prompt.trim().is_empty(),
                    action(Action::Save),
                ));
                fields.push(enabled_button(
                    "schedule-cancel",
                    "Cancel",
                    !state.busy,
                    action(Action::Cancel),
                ));
                out.push(card("schedule-form", fields, paints));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::View;

    #[test]
    fn editing_preserves_days_computer_chat_and_pause_state() {
        let schedule = Schedule {
            id: "prompt-one".into(),
            computer: "mac".into(),
            prompt: "Check the inbox".into(),
            days: vec![1, 3, 5],
            time: Some("08:30".into()),
            chat: Some("chat-one".into()),
            paused: true,
            ..Schedule::default()
        };
        let mut state = State {
            listed: Load::Ready(Listed {
                schedules: vec![schedule],
                computers: vec!["mac".into()],
                chats: vec![],
            }),
            ..State::default()
        };
        state.edit(Some("prompt-one"));
        state.field("schedule-prompt", "Review the inbox".into());
        let draft = state.draft.as_mut().unwrap();
        let body = draft.body().unwrap();
        assert_eq!(body["days"], serde_json::json!([1, 3, 5]));
        assert_eq!(body["time"], "08:30");
        assert_eq!(body["chat"], "chat-one");
        assert_eq!(body["computer"], "mac");
        assert_eq!(body["paused"], true);
        draft.interval = true;
        draft.seconds = "5".into();
        assert!(draft.body().is_err());
        draft.seconds = "7200".into();
        let body = draft.body().unwrap();
        assert!(body["time"].is_null());
        assert_eq!(body["days"], serde_json::json!([]));
        assert_eq!(body["every_secs"], 7200);
        View::new_v3(
            "settings",
            1,
            column("schedules", page(&state, &Paints::default())),
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn no_computer_means_no_dead_add_control() {
        let mut state = State {
            listed: Load::Ready(Listed::default()),
            ..State::default()
        };
        state.edit(None);
        assert!(state.draft.is_none());
        let root = column("schedules", page(&state, &Paints::default()));
        let json = serde_json::to_string(&root).unwrap();
        assert!(!json.contains("schedules-add"));
        View::new_v3("settings", 1, root).validate().unwrap();
    }
}

/// The complete shared page, mounted by the page host.
pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    column("settings-schedules", page(state, paints))
}
