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
            (None, false, _) => "off".to_owned(),
            (None, true, Some(until)) if until > now() => "paused".to_owned(),
            (None, true, _) => "on".to_owned(),
        };
        let mut line = format!("{} · {status}", self.id);
        if let Some(free) = self.state.free {
            line.push_str(&format!(" · {} free", bytes(free)));
        }
        if let Some(result) = &self.state.last_result {
            line.push_str(&format!(" · {result}"));
        }
        line
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
                state: state.rules.get(&rule.id).cloned().unwrap_or_default(),
                id: rule.id.clone(),
                name: rule.name.clone(),
                version: rule.version,
                digest: rule.digest(),
                enabled: rule.enabled,
                paused_until: rule.paused_until,
                error: None,
            },
            Err(error) => Row {
                id: "disk".into(),
                name: "Disk cleanup".into(),
                version: 0,
                digest: String::new(),
                enabled: false,
                paused_until: None,
                state: state.rules.get("disk").cloned().unwrap_or_default(),
                error: Some(error),
            },
        })
        .collect()
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
    #[test]
    fn dates() {
        assert_eq!(super::date(0), "1970-01-01");
        assert_eq!(super::date(1_790_899_200), "2026-10-02");
    }
}
