//! What the CLI, the terminal, and the host's `background.*` methods show
//! and change, so every surface says the same thing.

use serde::{Deserialize, Serialize};

use crate::paths::{Layout, bytes, now};
use crate::rule::Rule;
use crate::run::Record;
use crate::store::{self, RuleState, State};

/// One rule as `list` shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub digest: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    pub state: RuleState,
    /// The plugin that brings the rule (`KEY:SLUG`), when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    /// Why the rule could not be read, when it could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Row {
    /// One plain line: on or paused, free space, and the last result.
    #[must_use]
    pub fn line(&self) -> String {
        let status = match (&self.error, self.enabled, self.paused_until) {
            (Some(error), ..) => format!("broken: {error}"),
            // One word for a rule not running: `pause` and `resume` are the
            // commands that change it.
            (None, false, _) => "paused".to_owned(),
            (None, true, Some(until)) if until > now() => format!("paused until {}", date(until)),
            (None, true, _) => "on".to_owned(),
        };
        let mut line = format!("{} · {status}", self.id);
        if let Some(free) = self.state.free {
            line.push_str(&format!(" · {} free", bytes(free)));
        }
        match (&self.state.last_result, self.state.last_run) {
            (Some(result), Some(at)) => line.push_str(&format!(" · {} {result}", ago(at, now()))),
            (Some(result), None) => line.push_str(&format!(" · {result}")),
            (None, _) if self.error.is_none() => line.push_str(" · not run yet"),
            (None, _) => {}
        }
        line
    }
}

/// When `at` was, from `now`, for a list line: "just now", "5 min ago",
/// "3 h ago", "2 days ago", or the date.
#[must_use]
pub fn ago(at: u64, now: u64) -> String {
    let secs = now.saturating_sub(at);
    match secs {
        0..60 => "just now:".to_owned(),
        60..3600 => format!("{} min ago:", secs / 60),
        3600..86_400 => format!("{} h ago:", secs / 3600),
        86_400..1_209_600 => format!("{} days ago:", secs / 86_400),
        _ => format!("on {}:", date(at)),
    }
}

/// Keep a finished run's result in the rule's state, for `list`.
pub fn remember(layout: &Layout, id: &str, report: &crate::run::Report) {
    let Some(record) = &report.record else {
        return;
    };
    let at = now();
    State::update(layout, id, |state| {
        state.last_run = Some(at);
        state.last_run_id = Some(record.run.clone());
        state.last_result = Some(
            report
                .notice
                .clone()
                .unwrap_or_else(|| "Nothing to clean.".to_owned()),
        );
        if let Some(free) = record.observation.iter().map(|o| o.free_after).min() {
            state.free = Some(free);
        }
        if let Some(line) = &report.notice {
            state.notice = Some((at, line.clone()));
        }
    });
}

/// Every rule with its state.
#[must_use]
pub fn list(layout: &Layout) -> Vec<Row> {
    let state = State::load(layout);
    store::list(layout)
        .into_iter()
        .map(|rule| match rule {
            Ok(rule) => Row {
                plugin: match &rule.origin {
                    crate::rule::Origin::Plugin { plugin, .. } => Some(plugin.clone()),
                    _ => None,
                },
                state: state.rules.get(&rule.id).cloned().unwrap_or_default(),
                id: rule.id.clone(),
                name: rule.name.clone(),
                version: rule.version,
                digest: rule.digest(),
                enabled: rule.enabled,
                paused_until: rule.paused_until,
                error: None,
            },
            Err((id, error)) => Row {
                name: id.clone(),
                version: 0,
                digest: String::new(),
                enabled: false,
                paused_until: None,
                state: state.rules.get(&id).cloned().unwrap_or_default(),
                plugin: None,
                error: Some(error),
                id,
            },
        })
        .collect()
}

/// The background watchers running on this computer, by name ("disk
/// cleanup"): the rules that are on and not paused, while a host's runner
/// holds `runner.lock`. Without a runner nothing runs them, so none.
#[must_use]
pub fn watchers(layout: &Layout) -> Vec<String> {
    if !runner_running(layout) {
        return Vec::new();
    }
    let at = now();
    store::list(layout)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|rule| rule.active(at))
        .map(|rule| lower_first(&rule.name))
        .collect()
}

/// Whether a host's runner holds `runner.lock`. Never creates the file.
#[must_use]
pub fn runner_running(layout: &Layout) -> bool {
    let Ok(file) = std::fs::File::options()
        .read(true)
        .write(true)
        .open(layout.runner_lock())
    else {
        return false;
    };
    // Taken here means no runner holds it; dropping the file frees it.
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// Pause `id` until `until` (or indefinitely), or resume it.
///
/// # Errors
/// No such rule, or it cannot be saved.
pub fn pause(layout: &Layout, id: &str, until: Option<u64>, resume: bool) -> Result<Rule, String> {
    let mut rule = store::load(layout, id)?;
    if resume {
        rule.enabled = true;
        rule.paused_until = None;
    } else if let Some(until) = until {
        rule.enabled = true;
        rule.paused_until = Some(until);
    } else {
        rule.enabled = false;
        rule.paused_until = None;
    }
    store::save(layout, &rule)
}

/// Recorded runs of `rule` (all rules when `None`) since `since`, oldest
/// first, at most the newest `limit`.
#[must_use]
pub fn log(layout: &Layout, rule: Option<&str>, since: Option<u64>, limit: usize) -> Vec<Record> {
    let mut records: Vec<Record> = store::read_log(layout)
        .into_iter()
        .filter(|record| rule.is_none_or(|rule| record.rule == rule))
        .filter(|record| since.is_none_or(|since| record.started >= since))
        .collect();
    let skip = records.len().saturating_sub(limit);
    records.drain(..skip);
    records
}

/// One line for a recorded run.
#[must_use]
pub fn log_line(record: &Record) -> String {
    let done = record
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action.outcome,
                crate::run::Outcome::Deleted | crate::run::Outcome::Removed
            )
        })
        .count();
    format!(
        "{} {} {} {}: {} removed, freed {}{}",
        date(record.started),
        record.run,
        record.rule,
        serde_json::to_value(record.trigger)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default(),
        done,
        bytes(record.freed_sum),
        record
            .notified
            .as_ref()
            .map_or_else(String::new, |line| format!(" · {line}"))
    )
}

/// Bytes freed per class and per week, from the log.
#[must_use]
pub fn stats(records: &[Record]) -> Vec<(String, u64)> {
    let mut totals: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for record in records {
        // Weeks start on Monday; day 0 (1970-01-01) was a Thursday.
        let day = record.started / 86_400;
        let monday = day - (day + 3) % 7;
        for action in &record.actions {
            if matches!(
                action.outcome,
                crate::run::Outcome::Deleted | crate::run::Outcome::Removed
            ) {
                *totals
                    .entry(format!(
                        "week of {} · class {}",
                        date(monday * 86_400),
                        action.class.number()
                    ))
                    .or_default() += action.bytes;
            }
        }
    }
    totals.into_iter().collect()
}

/// `YYYY-MM-DD` (UTC) for seconds since the epoch.
#[must_use]
pub fn date(secs: u64) -> String {
    // Howard Hinnant's civil-from-days.
    let z = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use crate::paths::Layout;

    #[test]
    fn a_list_line_says_paused_and_when_the_last_result_was() {
        let row = |enabled: bool, last: Option<(u64, &str)>| super::Row {
            id: "worktrees".into(),
            name: "Worktrees".into(),
            version: 1,
            digest: String::new(),
            enabled,
            paused_until: None,
            state: crate::store::RuleState {
                last_run: last.map(|(at, _)| at),
                last_result: last.map(|(_, result)| result.to_owned()),
                ..Default::default()
            },
            plugin: None,
            error: None,
        };
        assert_eq!(row(false, None).line(), "worktrees · paused · not run yet");
        let at = super::now() - 2 * 3600;
        assert_eq!(
            row(true, Some((at, "Nothing to clean."))).line(),
            "worktrees · on · 2 h ago: Nothing to clean."
        );
        assert_eq!(super::ago(100, 100 + 3 * 86_400), "3 days ago:");
        assert_eq!(super::ago(100, 130), "just now:");
    }

    #[test]
    fn watchers_are_the_rules_on_while_a_runner_holds_its_lock() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        // No runner: nothing runs the rules.
        assert!(super::watchers(&layout).is_empty());
        std::fs::create_dir_all(layout.background()).unwrap();
        let lock = std::fs::File::create(layout.runner_lock()).unwrap();
        assert!(
            super::watchers(&layout).is_empty(),
            "the file alone is no runner"
        );
        lock.lock().unwrap();
        assert!(super::watchers(&layout).is_empty());
        let mut rule = crate::store::load(&layout, "disk").unwrap();
        rule.enabled = true;
        crate::store::save(&layout, &rule).unwrap();
        assert_eq!(super::watchers(&layout), vec!["disk cleanup".to_owned()]);
        let mut rule = crate::store::load(&layout, "disk").unwrap();
        rule.paused_until = Some(super::now() + 3600);
        crate::store::save(&layout, &rule).unwrap();
        assert!(
            super::watchers(&layout).is_empty(),
            "a paused rule is not running"
        );
    }

    #[test]
    fn dates() {
        assert_eq!(super::date(0), "1970-01-01");
        assert_eq!(super::date(1_790_899_200), "2026-10-02");
    }
}
