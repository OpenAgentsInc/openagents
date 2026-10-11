//! Stuck detection for Coder's own tool loop (#11285).
//!
//! A run of identical consecutive tool calls (same name, same arguments) is
//! the clearest sign a model is going in circles. At [`REMIND_AT`] identical
//! calls the model is told once to change approach; at [`STOP_AT`] identical
//! calls that all failed, the turn ends with a plain notice. The thresholds
//! follow the harness design study (arXiv:2609.20804, 5 and 8).

use serde_json::{Value, json};

/// Identical consecutive calls before the one-time reminder.
pub const REMIND_AT: usize = 5;
/// Identical consecutive failing calls that end the turn.
pub const STOP_AT: usize = 8;

/// What the model reads once a streak reaches [`REMIND_AT`].
pub const REMINDER: &str = "You have made the same tool call with the same arguments 5 times in a row. Repeating it will not change the result. Step back, reread the recent tool results, and try a different approach.";

/// What the person sees when the turn ends at [`STOP_AT`].
pub const STOP_NOTICE: &str =
    "Stopped: the same step failed 8 times in a row. Try a different instruction or more detail.";

/// Whether a tool result is a failure: an error, a nonzero exit, or a timeout.
#[must_use]
pub fn failed(output: &Value) -> bool {
    output.get("error").is_some()
        || output
            .get("exit")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0)
        || output.get("timed_out").and_then(Value::as_bool) == Some(true)
}

/// The identity two calls must share to count as the same call: the name
/// and the arguments, with object keys in a fixed order.
#[must_use]
pub fn identity(name: &str, arguments: &Value) -> String {
    // serde_json's default map is ordered, so this string is canonical.
    format!("{name}\u{0}{arguments}")
}

/// What the loop should do after a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Continue,
    Remind,
    Stop,
}

/// The current streak of identical consecutive calls in one turn.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<String>,
    count: usize,
    failing: usize,
    reminded: bool,
}

impl Tracker {
    /// Record one finished call and say what to do next.
    pub fn record(&mut self, name: &str, arguments: &Value, output: &Value) -> Verdict {
        let key = identity(name, arguments);
        if self.last.as_deref() == Some(key.as_str()) {
            self.count += 1;
        } else {
            self.last = Some(key);
            self.count = 1;
            self.failing = 0;
            self.reminded = false;
        }
        if failed(output) {
            self.failing += 1;
        } else {
            self.failing = 0;
        }
        if self.failing >= STOP_AT {
            Verdict::Stop
        } else if self.count >= REMIND_AT && !self.reminded {
            self.reminded = true;
            Verdict::Remind
        } else {
            Verdict::Continue
        }
    }
}

/// Streaks of two or more identical consecutive calls in a finished list of
/// `(name, arguments, output)` calls, for the export: where each starts,
/// how long it ran, how many of its calls failed, and whether it reached
/// the reminder or the stop.
#[must_use]
pub fn streaks<'a>(
    calls: impl IntoIterator<Item = (usize, &'a str, &'a Value, &'a Value)>,
) -> Vec<Value> {
    let mut found = Vec::new();
    let mut current: Option<(String, usize, &str, usize, usize, usize)> = None;
    let mut flush = |current: &mut Option<(String, usize, &str, usize, usize, usize)>| {
        if let Some((_, start, name, length, failing, longest_failing)) = current.take()
            && length >= 2
        {
            found.push(json!({
                "tool": name,
                "first_step": start + 1,
                "length": length,
                "failed": failing,
                "reminded": length >= REMIND_AT,
                "stopped": longest_failing >= STOP_AT,
            }));
        }
    };
    let mut failing_run = 0usize;
    for (index, name, arguments, output) in calls {
        let key = identity(name, arguments);
        let same = current.as_ref().is_some_and(|(last, ..)| *last == key);
        if !same {
            flush(&mut current);
            failing_run = 0;
            current = Some((key, index, name, 0, 0, 0));
        }
        let entry = current.as_mut().expect("a streak was just opened");
        entry.3 += 1;
        if failed(output) {
            entry.4 += 1;
            failing_run += 1;
        } else {
            failing_run = 0;
        }
        entry.5 = entry.5.max(failing_run);
    }
    flush(&mut current);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_identical_calls_remind_once_and_eight_failures_stop() {
        let mut tracker = Tracker::default();
        let arguments = json!({"command":"cargo test"});
        let failure = json!({"exit":101,"output":"failed"});
        let verdicts: Vec<_> = (0..8)
            .map(|_| tracker.record("Run", &arguments, &failure))
            .collect();
        assert_eq!(&verdicts[..4], &[Verdict::Continue; 4]);
        assert_eq!(verdicts[4], Verdict::Remind);
        assert_eq!(&verdicts[5..7], &[Verdict::Continue; 2]);
        assert_eq!(verdicts[7], Verdict::Stop);
    }

    #[test]
    fn a_different_call_or_a_success_resets_the_streak() {
        let mut tracker = Tracker::default();
        let read = json!({"path":"a.rs"});
        let error = json!({"error":"missing"});
        for _ in 0..4 {
            assert_eq!(tracker.record("Read", &read, &error), Verdict::Continue);
        }
        assert_eq!(
            tracker.record("Read", &json!({"path":"b.rs"}), &error),
            Verdict::Continue
        );
        // Identical successful calls remind but never stop.
        let mut tracker = Tracker::default();
        let verdicts: Vec<_> = (0..12)
            .map(|_| tracker.record("Read", &read, &json!({"content":"x"})))
            .collect();
        assert_eq!(
            verdicts.iter().filter(|v| **v == Verdict::Remind).count(),
            1
        );
        assert!(!verdicts.contains(&Verdict::Stop));
    }

    #[test]
    fn streaks_list_repeats_with_their_thresholds() {
        let a = json!({"command":"ls"});
        let b = json!({"path":"x"});
        let ok = json!({"exit":0});
        let bad = json!({"error":"no"});
        let mut calls = vec![(0, "Run", &a, &ok)];
        for index in 1..9 {
            calls.push((index, "Read", &b, &bad));
        }
        calls.push((9, "Run", &a, &ok));
        let found = streaks(calls);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["tool"], "Read");
        assert_eq!(found[0]["first_step"], 2);
        assert_eq!(found[0]["length"], 8);
        assert_eq!(found[0]["failed"], 8);
        assert_eq!(found[0]["reminded"], true);
        assert_eq!(found[0]["stopped"], true);
    }
}
