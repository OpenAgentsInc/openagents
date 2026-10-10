//! `/schedule` (#11177): a prompt that runs on this computer on a
//! schedule, even while Coder is closed.
//!
//! A scheduled prompt is a host background rule (`openagents background`,
//! docs/background): a daily or interval trigger, a day-of-week condition
//! for `weekdays` and `weekends`, and one `StartCoderRun` action that
//! starts a Coder run with the prompt in this folder. The host's runner
//! runs it; `openagents background list` and the apps' Background pages
//! show it like any other rule.
//!
//! - `/schedule weekdays 9am triage the new issues`
//! - `/schedule daily 18:30 summarize today's commits`
//! - `/schedule every 2h check the deploy`
//! - `/schedule` lists them; `/schedule remove ID` removes one.

use crate::{App, Mode};

/// How to make one.
pub const USAGE: &str = "Use /schedule WHEN PROMPT, where WHEN is daily TIME, weekdays TIME, weekends TIME, or every INTERVAL (at least 1m). For example /schedule weekdays 9am triage the new issues. /schedule lists them; /schedule remove ID removes one.";

/// When a scheduled prompt runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum When {
    /// At `HH:MM` local time on `days` (0 Sunday to 6 Saturday); every day
    /// when empty.
    At { time: String, days: Vec<u8> },
    /// Every `seconds`.
    Every { seconds: u64 },
}

impl When {
    pub(crate) fn words(&self) -> String {
        match self {
            When::At { time, days } if days.is_empty() => format!("every day at {time}"),
            When::At { time, days } if *days == WEEKDAYS => format!("weekdays at {time}"),
            When::At { time, days } if *days == WEEKENDS => format!("weekends at {time}"),
            When::At { time, .. } => format!("at {time}"),
            When::Every { seconds } => format!("every {}", crate::loops::every(*seconds)),
        }
    }
}

const WEEKDAYS: [u8; 5] = [1, 2, 3, 4, 5];
const WEEKENDS: [u8; 2] = [0, 6];

/// `WHEN PROMPT`: the schedule and the prompt.
pub(crate) fn parse(argument: &str) -> Result<(When, String), String> {
    let mut words = argument.split_whitespace();
    let first = words.next().unwrap_or_default().to_ascii_lowercase();
    let days: Option<Vec<u8>> = match first.as_str() {
        "daily" | "everyday" => Some(Vec::new()),
        "weekdays" | "weekday" => Some(WEEKDAYS.to_vec()),
        "weekends" | "weekend" => Some(WEEKENDS.to_vec()),
        "every" => None,
        _ => return Err(USAGE.into()),
    };
    let Some(days) = days else {
        let amount = words.next().unwrap_or_default();
        let seconds = crate::loops::interval(amount)
            .ok_or_else(|| format!("`{amount}` is not an interval. {USAGE}"))?;
        if seconds < 60 {
            return Err("A scheduled prompt runs at most once a minute.".into());
        }
        let prompt = words.collect::<Vec<_>>().join(" ");
        return if prompt.is_empty() {
            Err(USAGE.into())
        } else {
            Ok((When::Every { seconds }, prompt))
        };
    };
    // A time is one word (`9am`, `09:30`) or two (`9 am`).
    let rest: Vec<&str> = words.collect();
    let (time, used) = (1..=2)
        .filter(|count| *count <= rest.len())
        .find_map(|count| {
            // Two words only for `9 am`, so a prompt's own time is kept.
            let meridiem = || {
                matches!(
                    rest[1].to_ascii_lowercase().as_str(),
                    "am" | "pm" | "a.m." | "p.m."
                )
            };
            background_time(&rest[..count].join(" "))
                .filter(|_| count == 1 || meridiem())
                .map(|time| (time, count))
        })
        .ok_or_else(|| {
            format!("Name a time of day after `{first}`, such as 9am or 18:30. {USAGE}")
        })?;
    let prompt = rest[used..].join(" ");
    if prompt.is_empty() {
        return Err(USAGE.into());
    }
    Ok((When::At { time, days }, prompt))
}

#[cfg(unix)]
fn background_time(text: &str) -> Option<String> {
    background::compile::fields::time_of_day(text)
}

#[cfg(not(unix))]
fn background_time(_text: &str) -> Option<String> {
    None
}

#[cfg(unix)]
mod rules {
    use background::Layout;
    use background::rule::{Action, Condition, Origin, Rule, Trigger};
    use background::store;

    use super::When;

    /// Rules made by `/schedule` have ids that start with this.
    pub const PREFIX: &str = "prompt-";

    /// A rule id from the prompt's first words, not taken by another rule.
    fn id_for(prompt: &str, layout: &Layout) -> String {
        let mut slug = String::new();
        for c in prompt.chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
            if slug.len() >= 24 {
                break;
            }
        }
        let slug = slug.trim_end_matches('-');
        let base = if slug.is_empty() {
            format!("{PREFIX}{}", atif::now_ms() % 100_000)
        } else {
            format!("{PREFIX}{slug}")
        };
        let taken = |id: &str| store::load(layout, id).is_ok();
        if !taken(&base) {
            return base;
        }
        (2..1000)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(base)
    }

    /// The rule for `prompt` at `when`, run in `workspace`.
    pub fn rule(
        layout: &Layout,
        when: &When,
        prompt: &str,
        workspace: Option<String>,
        thread: &str,
    ) -> Rule {
        let mut rule = background::rule::disk();
        rule.id = id_for(prompt, layout);
        rule.name = format!(
            "Scheduled prompt: {}",
            crate::long_session::clip(prompt, 60)
        );
        rule.version = 1;
        rule.origin = Origin::Conversation {
            thread: thread.into(),
            message: prompt.chars().take(280).collect(),
        };
        rule.enabled = true;
        rule.paused_until = None;
        rule.cooldown_secs = 0;
        rule.escalate = None;
        rule.conditions = Vec::new();
        rule.triggers = match when {
            When::At { time, days } => {
                if !days.is_empty() {
                    rule.conditions
                        .push(Condition::Weekdays { days: days.clone() });
                }
                vec![Trigger::Daily { at: time.clone() }]
            }
            When::Every { seconds } => vec![Trigger::Interval {
                every_secs: *seconds,
            }],
        };
        rule.actions = vec![Action::StartCoderRun {
            prompt: prompt.into(),
            workspace,
        }];
        rule
    }

    /// Saves a new scheduled prompt.
    pub fn add(
        layout: &Layout,
        when: &When,
        prompt: &str,
        workspace: Option<String>,
        thread: &str,
    ) -> Result<Rule, String> {
        store::save(layout, &rule(layout, when, prompt, workspace, thread))
    }

    /// The scheduled prompts: rules made here that start a Coder run.
    pub fn list(layout: &Layout) -> Vec<Rule> {
        store::list(layout)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|rule| rule.id.starts_with(PREFIX) && prompt_of(rule).is_some())
            .collect()
    }

    /// Removes one.
    pub fn remove(layout: &Layout, id: &str) -> Result<(), String> {
        if !id.starts_with(PREFIX) {
            return Err(format!(
                "{id} is not a scheduled prompt; openagents background manages other rules."
            ));
        }
        store::remove(layout, id)
    }

    /// What a rule's first `StartCoderRun` runs.
    fn prompt_of(rule: &Rule) -> Option<&str> {
        rule.actions.iter().find_map(|action| match action {
            Action::StartCoderRun { prompt, .. } => Some(prompt.as_str()),
            _ => None,
        })
    }

    /// One line for `/schedule`.
    pub fn line(rule: &Rule) -> String {
        let when = rule
            .triggers
            .iter()
            .find_map(|trigger| match trigger {
                Trigger::Daily { at } => {
                    let days = rule
                        .conditions
                        .iter()
                        .find_map(|condition| match condition {
                            Condition::Weekdays { days } => Some(days.clone()),
                            _ => None,
                        });
                    Some(
                        When::At {
                            time: at.clone(),
                            days: days.unwrap_or_default(),
                        }
                        .words(),
                    )
                }
                Trigger::Interval { every_secs } => Some(
                    When::Every {
                        seconds: *every_secs,
                    }
                    .words(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        format!(
            "{}  {}{}  {}",
            rule.id,
            when,
            if rule.enabled { "" } else { " (off)" },
            crate::long_session::clip(prompt_of(rule).unwrap_or_default(), 80)
        )
    }
}

impl App {
    /// `/schedule`, `/schedule WHEN PROMPT`, `/schedule remove ID`.
    #[cfg(unix)]
    pub(crate) fn schedule_command(&mut self, argument: &str) {
        if self.mode != Mode::Live {
            self.notice = Some("Scheduled prompts are made in live mode.".into());
            return;
        }
        let layout = match background::Layout::from_env() {
            Ok(layout) => layout,
            Err(error) => {
                self.notice = Some(format!("Background rules are unavailable here: {error}"));
                return;
            }
        };
        self.notice = Some(self.schedule_with(&layout, argument));
    }

    #[cfg(not(unix))]
    pub(crate) fn schedule_command(&mut self, _argument: &str) {
        self.notice = Some("Scheduled prompts run on macOS and Linux only.".into());
    }

    #[cfg(unix)]
    fn schedule_with(&mut self, layout: &background::Layout, argument: &str) -> String {
        let argument = argument.trim();
        if argument.is_empty() || argument == "list" {
            let rules = rules::list(layout);
            if rules.is_empty() {
                return format!("No scheduled prompts. {USAGE}");
            }
            let mut text = String::from("Scheduled prompts on this computer:\n");
            for rule in &rules {
                text.push_str(&rules::line(rule));
                text.push('\n');
            }
            text.push_str(
                "/schedule remove ID removes one; openagents background list shows every rule.",
            );
            return text;
        }
        if let Some(id) = argument
            .strip_prefix("remove")
            .or_else(|| argument.strip_prefix("delete"))
            .map(str::trim)
        {
            if id.is_empty() {
                return "Use /schedule remove ID; /schedule lists them.".into();
            }
            return match rules::remove(layout, id) {
                Ok(()) => format!("Removed {id}."),
                Err(error) => error,
            };
        }
        let (when, prompt) = match parse(argument) {
            Ok(parsed) => parsed,
            Err(error) => return error,
        };
        let workspace = self
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .and_then(|path| path.canonicalize().ok())
            .map(|path| path.to_string_lossy().into_owned());
        let thread = self.session_id().unwrap_or("coder").to_owned();
        match rules::add(layout, &when, &prompt, workspace, &thread) {
            Ok(rule) => format!(
                "Scheduled {}: {} on this computer, as a Coder run in this folder. The background runner starts it (openagents background status shows whether it is on). /schedule remove {} removes it.",
                rule.id,
                when.words(),
                rule.id
            ),
            Err(error) => format!("The schedule was not saved: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_read_their_words() {
        assert_eq!(
            parse("weekdays 9am triage the new issues").unwrap(),
            (
                When::At {
                    time: "09:00".into(),
                    days: WEEKDAYS.to_vec()
                },
                "triage the new issues".into()
            )
        );
        assert_eq!(
            parse("daily 18:30 summarize today").unwrap(),
            (
                When::At {
                    time: "18:30".into(),
                    days: Vec::new()
                },
                "summarize today".into()
            )
        );
        assert_eq!(
            parse("weekends 7 pm water the plants").unwrap().0,
            When::At {
                time: "19:00".into(),
                days: WEEKENDS.to_vec()
            }
        );
        assert_eq!(
            parse("every 2h check the deploy").unwrap(),
            (When::Every { seconds: 7200 }, "check the deploy".into())
        );
        assert!(parse("every 30s ping").is_err());
        assert!(parse("weekdays triage").is_err());
        assert!(parse("daily 9am").is_err());
        assert!(parse("sometime soon").is_err());
        assert_eq!(
            When::At {
                time: "09:00".into(),
                days: WEEKDAYS.to_vec()
            }
            .words(),
            "weekdays at 09:00"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_scheduled_prompt_is_a_background_rule() {
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.cwd = Some(home.path().to_owned());
        let said = app.schedule_with(&layout, "weekdays 9am triage the new issues");
        assert!(
            said.starts_with("Scheduled prompt-triage-the-new-issues: weekdays at 09:00"),
            "{said}"
        );
        let saved = rules::list(&layout);
        assert_eq!(saved.len(), 1);
        let rule = &saved[0];
        assert!(rule.validate().is_ok());
        assert_eq!(
            rule.triggers,
            vec![background::rule::Trigger::Daily { at: "09:00".into() }]
        );
        assert_eq!(
            rule.conditions,
            vec![background::rule::Condition::Weekdays {
                days: WEEKDAYS.to_vec()
            }]
        );
        assert!(matches!(
            &rule.actions[..],
            [background::rule::Action::StartCoderRun { prompt, workspace: Some(_) }]
                if prompt == "triage the new issues"
        ));
        // A second one with the same words gets its own id.
        app.schedule_with(&layout, "every 2h triage the new issues");
        assert_eq!(rules::list(&layout).len(), 2);
        assert!(app.schedule_with(&layout, "").contains("weekdays at 09:00"));
        assert_eq!(
            app.schedule_with(&layout, "remove prompt-triage-the-new-issues"),
            "Removed prompt-triage-the-new-issues."
        );
        assert_eq!(rules::list(&layout).len(), 1);
        assert!(
            app.schedule_with(&layout, "remove disk")
                .contains("not a scheduled prompt")
        );
    }
}
