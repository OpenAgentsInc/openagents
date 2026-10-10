//! Settings' Background page (docs/background, phase 3): each background
//! rule on this computer, on or paused, its last result and when, and a
//! button to pause or resume it. It reads and writes the same rules the
//! host runs (`~/.openagents/background`), as `openagents background
//! list|pause|resume` and the host's `background.*` methods do, so every
//! surface shows the same thing. The newest notice is also a desktop
//! notification ([`crate::notices::Notices::observe_background`]).
//!
//! A scheduled prompt (#11177: a rule Coder's `/schedule`, the website, or
//! the phone made, whose id starts with `prompt-` and which starts a Coder
//! run) also says when it runs and has a Delete button ([`delete`]).

/// Whether a rule runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    On,
    Paused,
    Off,
    /// Its file does not read; the line says why.
    Broken,
}

/// One rule as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub status: Status,
    /// The last run's one line, or why the rule is broken.
    pub last: Option<String>,
    /// When it last ran (seconds since the epoch).
    pub when: Option<u64>,
    /// For a scheduled prompt, when it runs ("Weekdays at 09:00"); such a
    /// rule can be deleted here. `None` for every other rule.
    pub scheduled: Option<String>,
}

impl Rule {
    /// The status line: on or paused, the last result, and how long ago.
    #[must_use]
    pub fn line(&self, now: u64) -> String {
        let mut parts = vec![
            match self.status {
                Status::On => "On",
                Status::Paused => "Paused",
                Status::Off => "Off",
                Status::Broken => "Broken",
            }
            .to_owned(),
        ];
        if let Some(scheduled) = &self.scheduled {
            parts.push(scheduled.clone());
        }
        if let Some(last) = &self.last {
            parts.push(last.clone());
        }
        if let Some(when) = self.when {
            parts.push(ago(now.saturating_sub(when)));
        }
        parts.join(" · ")
    }

    /// Whether the button resumes it (it is paused or off).
    #[must_use]
    pub fn resumes(&self) -> bool {
        matches!(self.status, Status::Paused | Status::Off)
    }
}

/// `secs` ago, in a few words.
#[must_use]
pub fn ago(secs: u64) -> String {
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86_400),
    }
}

/// The rules under `layout`, as the page shows them.
#[cfg(unix)]
#[must_use]
pub fn rows(layout: &background::Layout, now: u64) -> Vec<Rule> {
    background::view::list(layout)
        .into_iter()
        .map(|row| {
            let status = match (&row.error, row.enabled, row.paused_until) {
                (Some(_), ..) => Status::Broken,
                (None, false, _) => Status::Off,
                (None, true, Some(until)) if until > now => Status::Paused,
                (None, true, _) => Status::On,
            };
            let scheduled = row
                .error
                .is_none()
                .then(|| background::store::load(layout, &row.id).ok())
                .flatten()
                .and_then(|rule| schedule_words(&rule));
            Rule {
                last: row.error.clone().or(row.state.last_result.clone()),
                when: row.state.last_run,
                scheduled,
                id: row.id,
                name: row.name,
                status,
            }
        })
        .collect()
}

/// Pause or resume rule `id`.
///
/// # Errors
/// No such rule, or it cannot be saved.
#[cfg(unix)]
pub fn set(layout: &background::Layout, id: &str, resume: bool) -> Result<(), String> {
    background::view::pause(layout, id, None, resume).map(drop)
}

/// Rules a scheduled prompt made have ids that start with this (Coder's
/// `/schedule`, and the schedules synced from the account).
pub const SCHEDULED_PREFIX: &str = "prompt-";

/// When a scheduled prompt runs, in plain words; `None` for a rule that
/// isn't one.
#[cfg(unix)]
#[must_use]
pub fn schedule_words(rule: &background::rule::Rule) -> Option<String> {
    use background::rule::{Action, Condition, Trigger};
    if !rule.id.starts_with(SCHEDULED_PREFIX)
        || !rule
            .actions
            .iter()
            .any(|action| matches!(action, Action::StartCoderRun { .. }))
    {
        return None;
    }
    let days: Vec<u8> = rule
        .conditions
        .iter()
        .find_map(|condition| match condition {
            Condition::Weekdays { days } => Some(days.clone()),
            _ => None,
        })
        .unwrap_or_default();
    rule.triggers.iter().find_map(|trigger| match trigger {
        Trigger::Daily { at } => Some(at_words(&days, at)),
        Trigger::Interval { every_secs } => Some(every_words(*every_secs)),
        _ => None,
    })
}

/// "Every day at 09:00", "Weekdays at 09:00", "Weekends at 09:00", or the
/// days named.
#[must_use]
pub fn at_words(days: &[u8], at: &str) -> String {
    let mut days = days.to_vec();
    days.sort_unstable();
    days.dedup();
    if days.is_empty() || days.len() == 7 {
        format!("Every day at {at}")
    } else if days == [1, 2, 3, 4, 5] {
        format!("Weekdays at {at}")
    } else if days == [0, 6] {
        format!("Weekends at {at}")
    } else {
        const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let names: Vec<&str> = days
            .iter()
            .filter_map(|day| NAMES.get(usize::from(*day)).copied())
            .collect();
        format!("{} at {at}", names.join(", "))
    }
}

/// "Every hour", "Every 2 hours", "Every 30 minutes".
#[must_use]
pub fn every_words(secs: u64) -> String {
    match secs {
        3600 => "Every hour".into(),
        s if s > 0 && s % 3600 == 0 => format!("Every {} hours", s / 3600),
        60 => "Every minute".into(),
        s if s > 0 && s % 60 == 0 => format!("Every {} minutes", s / 60),
        s => format!("Every {s} seconds"),
    }
}

/// Delete the scheduled prompt `id` from this computer. Only scheduled
/// prompts are deleted here; every other rule is paused instead.
///
/// # Errors
/// It isn't a scheduled prompt, or its file cannot be removed.
#[cfg(unix)]
pub fn delete(layout: &background::Layout, id: &str) -> Result<(), String> {
    let rule = background::store::load(layout, id)?;
    if schedule_words(&rule).is_none() {
        return Err(format!("{id} isn't a scheduled prompt; pause it instead."));
    }
    background::store::remove(layout, id)
}

/// The newest background notice: when and its line.
#[cfg(unix)]
#[must_use]
pub fn latest(layout: &background::Layout) -> Option<(u64, String)> {
    background::store::State::load(layout)
        .rules
        .values()
        .filter_map(|state| state.notice.clone())
        .max_by_key(|(at, _)| *at)
}

/// This user's layout, from `HOME` and the task store Coder uses.
#[cfg(unix)]
#[must_use]
pub fn here() -> Option<background::Layout> {
    background::Layout::from_env().ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn rows_show_status_last_result_and_pause_and_resume_change_it() {
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let now = 2_000_000_000;
        let all = rows(&layout, now);
        let disk = all.iter().find(|r| r.id == "disk").unwrap();
        assert_eq!(disk.status, Status::Off);
        assert!(disk.resumes());
        assert_eq!(disk.line(now), "Off");
        set(&layout, "disk", true).unwrap();
        background::store::State::update(&layout, "disk", |state| {
            state.last_run = Some(now - 7200);
            state.last_result = Some("Freed 4 GB: 2 old build folders.".into());
            state.notice = Some((now - 7200, "Freed 4 GB: 2 old build folders.".into()));
        });
        let disk = rows(&layout, now)
            .into_iter()
            .find(|r| r.id == "disk")
            .unwrap();
        assert_eq!(disk.status, Status::On);
        assert_eq!(
            disk.line(now),
            "On · Freed 4 GB: 2 old build folders. · 2 h ago"
        );
        assert_eq!(latest(&layout).unwrap().0, now - 7200);
        set(&layout, "disk", false).unwrap();
        let disk = rows(&layout, now)
            .into_iter()
            .find(|r| r.id == "disk")
            .unwrap();
        assert_eq!(disk.status, Status::Off);
        assert!(set(&layout, "nope", true).is_err());
    }

    #[test]
    fn a_scheduled_prompt_says_when_it_runs_and_can_be_deleted() {
        use background::rule::{Condition, Origin, Trigger};
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let mut rule = background::rule::disk();
        rule.id = "prompt-triage".into();
        rule.name = "Scheduled prompt: triage the new issues".into();
        rule.version = 1;
        rule.origin = Origin::Conversation {
            thread: "coder".into(),
            message: "triage the new issues".into(),
        };
        rule.enabled = true;
        rule.paused_until = None;
        rule.cooldown_secs = 0;
        rule.escalate = None;
        rule.conditions = vec![Condition::Weekdays {
            days: vec![1, 2, 3, 4, 5],
        }];
        rule.triggers = vec![Trigger::Daily { at: "09:00".into() }];
        // As the rule file has it, whatever else the action carries.
        rule.actions = vec![
            serde_json::from_value(serde_json::json!({
                "kind": "start_coder_run", "prompt": "triage the new issues"
            }))
            .unwrap(),
        ];
        background::store::save(&layout, &rule).unwrap();
        let now = 2_000_000_000;
        let listed = rows(&layout, now);
        let row = listed.iter().find(|r| r.id == "prompt-triage").unwrap();
        assert_eq!(row.scheduled.as_deref(), Some("Weekdays at 09:00"));
        assert_eq!(row.line(now), "On · Weekdays at 09:00");
        // A built-in rule is no scheduled prompt and isn't deleted here.
        let disk = listed.iter().find(|r| r.id == "disk").unwrap();
        assert!(disk.scheduled.is_none());
        assert!(delete(&layout, "disk").is_err());
        delete(&layout, "prompt-triage").unwrap();
        assert!(rows(&layout, now).iter().all(|r| r.id != "prompt-triage"));
        assert!(delete(&layout, "prompt-triage").is_err());
    }

    #[test]
    fn schedules_read_as_plain_words() {
        assert_eq!(at_words(&[], "07:30"), "Every day at 07:30");
        assert_eq!(at_words(&[6, 0], "10:00"), "Weekends at 10:00");
        assert_eq!(at_words(&[1, 3], "08:00"), "Mon, Wed at 08:00");
        assert_eq!(every_words(7200), "Every 2 hours");
        assert_eq!(every_words(3600), "Every hour");
        assert_eq!(every_words(1800), "Every 30 minutes");
    }

    #[test]
    fn ages_read_in_a_few_words() {
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(600), "10 min ago");
        assert_eq!(ago(3 * 86_400), "3 d ago");
    }
}
