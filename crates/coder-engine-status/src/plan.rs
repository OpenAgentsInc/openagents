//! Where a Claude plan stands, as Claude Code itself reported it (#11292).
//!
//! Claude Code prints a `rate_limit_event` for each plan window during a
//! run, and the Agent SDK answers `get_usage` the same way. The computer
//! that ran it keeps those readings (`microcoder_loop::usage`, `usage.json`)
//! and answers with [`PlanUsage`]: closed window kinds, whole percents, and
//! Unix seconds, never text, so it can ride in a [`crate::Status`] without
//! opening a path for anything a login holds. Every client renders it with
//! [`PlanUsage::text`], the same line Coder's status bar shows.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The feature an engine status read names in `requires` to have the
/// plan's windows in its answer. A host from before them refuses it as
/// unsupported, and the client asks again without it.
pub const FEATURE: &str = "openagents.terminal-engine-plan-usage.v1";

/// The `event` of a run event carrying the plan's windows.
pub const EVENT: &str = "plan_usage";

/// The usage book's file name in the task store
/// (`microcoder_loop::usage::FILE`).
pub const BOOK: &str = "usage.json";

/// The task store's override (`background::task_paths::STORE_VAR`).
const STORE_VAR: &str = "OPENAGENTS_TASKS";

/// A plan window Claude Code names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// The rolling five-hour window.
    FiveHour,
    /// The seven-day window.
    SevenDay,
    /// The separate seven-day window for the top model (Max plans).
    SevenDayTopModel,
}

impl WindowKind {
    /// The short word the plan line uses.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            WindowKind::FiveHour => "5h",
            WindowKind::SevenDay => "week",
            WindowKind::SevenDayTopModel => "top model week",
        }
    }
}

/// One window's use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanWindow {
    pub kind: WindowKind,
    /// The share of the window used, 0 to 100.
    pub used_percent: u8,
    /// When the window resets, in Unix seconds, if Claude Code said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
}

/// The plan's windows, in [`WindowKind`] order, and whether the last report
/// was Claude Code's `allowed_warning` (a quiet note, not a stop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<PlanWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<PlanWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day_top_model: Option<PlanWindow>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub warning: bool,
}

/// A fraction from 0 to 1 as a whole percent; anything else is 0 or 100.
#[must_use]
pub fn percent(fraction: f64) -> u8 {
    if !fraction.is_finite() || fraction <= 0.0 {
        return 0;
    }
    // Bounded to 0..=100 above and below, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = (fraction.min(1.0) * 100.0).round() as u8;
    whole
}

impl PlanUsage {
    /// Keeps `window` in its kind's place, replacing an older reading.
    pub fn record(&mut self, window: PlanWindow) {
        *self.slot(window.kind) = Some(window);
    }

    fn slot(&mut self, kind: WindowKind) -> &mut Option<PlanWindow> {
        match kind {
            WindowKind::FiveHour => &mut self.five_hour,
            WindowKind::SevenDay => &mut self.seven_day,
            WindowKind::SevenDayTopModel => &mut self.seven_day_top_model,
        }
    }

    /// Every window held, in [`WindowKind`] order.
    pub fn windows(&self) -> impl Iterator<Item = PlanWindow> {
        [self.five_hour, self.seven_day, self.seven_day_top_model]
            .into_iter()
            .flatten()
    }

    /// The same windows less each one that has reset by `now`; `None` when
    /// none is left.
    #[must_use]
    pub fn current(&self, now: u64) -> Option<PlanUsage> {
        let mut current = PlanUsage {
            warning: self.warning,
            ..PlanUsage::default()
        };
        for window in self.windows() {
            if window.resets_at.is_none_or(|at| at > now) {
                current.record(window);
            }
        }
        current.windows().next().is_some().then_some(current)
    }

    /// The run event a Coder run emits when Claude Code reports a window:
    /// `{"event":"plan_usage","usage":{…}}`.
    #[must_use]
    pub fn event(&self) -> Value {
        json!({"event": EVENT, "usage": self})
    }

    /// The newest windows a run's events carry, if any, including those a
    /// delegated Claude Code run reported inside a `delegation` event.
    #[must_use]
    pub fn from_events(events: &[Value]) -> Option<PlanUsage> {
        fn of(event: &Value, depth: usize) -> Option<PlanUsage> {
            if event["event"] == EVENT {
                return serde_json::from_value(event["usage"].clone()).ok();
            }
            if event["event"] == "delegation" && depth < 4 {
                return of(&event["update"], depth + 1);
            }
            None
        }
        events.iter().rev().find_map(|event| of(event, 0))
    }

    /// The Claude windows in a usage book's bytes (`usage.json`, written
    /// by `microcoder_loop::usage::record_reported` from Claude Code's own
    /// reports), less each one that has reset by `now`. Only the windows'
    /// names, fractions, and resets are read.
    #[must_use]
    pub fn from_book(bytes: &[u8], now: u64) -> Option<PlanUsage> {
        let book: Value = serde_json::from_slice(bytes).ok()?;
        let reading = book["entries"]
            .as_array()?
            .iter()
            .find(|entry| entry["provider"] == "claude")
            .map(|entry| &entry["reading"])?;
        let mut usage = PlanUsage::default();
        for window in reading["windows"].as_array()? {
            let kind = match window["window"].as_str() {
                Some("five_hour") => WindowKind::FiveHour,
                Some("seven_day") => WindowKind::SevenDay,
                Some("seven_day_top_model") => WindowKind::SevenDayTopModel,
                _ => continue,
            };
            let Some(fraction) = window["used_fraction"].as_f64() else {
                continue;
            };
            usage.record(PlanWindow {
                kind,
                used_percent: percent(fraction),
                resets_at: window["resets_at"].as_u64(),
            });
        }
        usage.current(now)
    }

    /// The windows in this computer's usage book under `home`. It opens
    /// the book and nothing else: never a login or credentials file.
    #[must_use]
    pub fn read(home: &Path, now: u64) -> Option<PlanUsage> {
        std::fs::read(book_path(home))
            .ok()
            .and_then(|bytes| PlanUsage::from_book(&bytes, now))
    }

    /// "Claude plan: 5h 62% · week 31% · resets 14:20", with a quiet
    /// "near the limit" after a warning; the reset is the fullest
    /// window's, written by `clock` from Unix milliseconds. `None` once
    /// every window has reset.
    #[must_use]
    pub fn text(&self, now: u64, clock: &dyn Fn(u64) -> String) -> Option<String> {
        let current = self.current(now)?;
        let mut parts: Vec<String> = current
            .windows()
            .map(|window| format!("{} {}%", window.kind.word(), window.used_percent))
            .collect();
        let fullest = current.windows().max_by_key(|window| window.used_percent);
        if let Some(at) = fullest.and_then(|window| window.resets_at) {
            parts.push(format!("resets {}", clock(at.saturating_mul(1000))));
        }
        if current.warning {
            parts.push("near the limit".into());
        }
        Some(format!("Claude plan: {}", parts.join(" · ")))
    }
}

/// The usage book under `home`: `$OPENAGENTS_TASKS/usage.json`, else
/// `~/.openagents/tasks/usage.json`.
#[must_use]
pub fn book_path(home: &Path) -> PathBuf {
    std::env::var_os(STORE_VAR)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home.join(".openagents/tasks"), PathBuf::from)
        .join(BOOK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(ms: u64) -> String {
        crate::utc(ms / 1000)
    }

    #[test]
    fn the_line_names_each_current_window_and_the_fullest_reset() {
        let mut usage = PlanUsage::default();
        usage.record(PlanWindow {
            kind: WindowKind::SevenDay,
            used_percent: 31,
            resets_at: Some(900_000),
        });
        usage.record(PlanWindow {
            kind: WindowKind::FiveHour,
            used_percent: 62,
            resets_at: Some(51_600),
        });
        assert_eq!(
            usage.text(1, &clock).unwrap(),
            "Claude plan: 5h 62% · week 31% · resets 1970-01-01 14:20 UTC"
        );
        usage.warning = true;
        assert!(usage.text(1, &clock).unwrap().ends_with("near the limit"));
        // The five-hour window reset: only the week is left.
        let later = usage.text(60_000, &clock).unwrap();
        assert!(later.starts_with("Claude plan: week 31%"), "{later}");
        assert!(usage.text(1_000_000, &clock).is_none());
        assert_eq!(percent(0.615), 62);
        assert_eq!(percent(f64::NAN), 0);
        assert_eq!(percent(1.2), 100);
    }

    #[test]
    fn a_book_and_run_events_give_the_same_windows() {
        let book = serde_json::json!({
            "schema": "openagents.coder.provider-usage.v1",
            "entries": [
                {"provider": "codex", "attempted_at": 1, "next_probe_at": 1,
                 "reading": {"provider": "codex", "observed_at": 1, "limit_reached": false,
                             "windows": [{"window": "primary", "used_fraction": 0.9}]}},
                {"provider": "claude", "attempted_at": 1, "next_probe_at": 1,
                 "reading": {"provider": "claude", "observed_at": 1, "limit_reached": false,
                             "account": "a1b2",
                             "windows": [
                                 {"window": "five_hour", "used_fraction": 0.62, "resets_at": 51_600},
                                 {"window": "seven_day", "used_fraction": 0.31, "resets_at": 900_000}
                             ]}}
            ]
        });
        let usage = PlanUsage::from_book(book.to_string().as_bytes(), 2).unwrap();
        assert_eq!(usage.five_hour.unwrap().used_percent, 62);
        assert_eq!(usage.seven_day.unwrap().used_percent, 31);
        assert!(usage.seven_day_top_model.is_none());
        let events = vec![
            serde_json::json!({"event": "delta", "text": "Working."}),
            PlanUsage::default().event(),
            usage.event(),
            serde_json::json!({"event": "delta", "text": "Done."}),
        ];
        assert_eq!(PlanUsage::from_events(&events), Some(usage));
        let delegated =
            [serde_json::json!({"event": "delegation", "id": "d1", "update": usage.event()})];
        assert_eq!(PlanUsage::from_events(&delegated), Some(usage));
        assert_eq!(PlanUsage::from_events(&events[..1]), None);
        assert!(PlanUsage::from_book(b"{}", 2).is_none());
        assert!(PlanUsage::from_book(book.to_string().as_bytes(), 1_000_000).is_none());
    }

    #[test]
    fn reading_the_book_opens_only_the_book() {
        let home = tempfile::tempdir().unwrap();
        // A login beside it is never read: make it unreadable to prove it.
        let claude = home.path().join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join(".credentials.json"), "{not json").unwrap();
        assert_eq!(PlanUsage::read(home.path(), 2), None);
        let path = book_path(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema":"openagents.coder.provider-usage.v1","entries":[{"provider":"claude","attempted_at":1,"next_probe_at":1,"reading":{"provider":"claude","observed_at":1,"limit_reached":false,"windows":[{"window":"seven_day_top_model","used_fraction":0.5}]}}]}"#,
        )
        .unwrap();
        let usage = PlanUsage::read(home.path(), 2).unwrap();
        assert_eq!(usage.seven_day_top_model.unwrap().used_percent, 50);
    }

    #[test]
    fn the_projection_holds_no_text() {
        let usage = PlanUsage {
            five_hour: Some(PlanWindow {
                kind: WindowKind::FiveHour,
                used_percent: 5,
                resets_at: None,
            }),
            warning: true,
            ..PlanUsage::default()
        };
        let value = serde_json::to_value(usage).unwrap();
        assert_eq!(serde_json::from_value::<PlanUsage>(value).unwrap(), usage);
        let forged =
            serde_json::json!({"five_hour":{"kind":"five_hour","used_percent":5,"note":"sk-ant"}});
        assert!(serde_json::from_value::<PlanUsage>(forged).is_err());
        let forged = serde_json::json!({"account":"someone@example.com"});
        assert!(serde_json::from_value::<PlanUsage>(forged).is_err());
    }
}
