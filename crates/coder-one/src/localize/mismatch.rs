//! `evidence.mismatch_trace`: the first case an acceptance check fails,
//! its input, what was observed against what was expected, and a diff.
//! When the check exposes intermediate stages, they're compared in order
//! and the first stage that differs is named.
//!
//! A case comes from a check's structured result when it has one (an
//! oracle's JSON lines, or [`Case`] built by the caller), and otherwise
//! from the assertion formats test runners print ([`FORMATS`]). A format
//! that doesn't say which side is expected reports the sides as left and
//! right, not as observed and expected.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One named intermediate value, as the check computed it on both sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub expected: String,
    pub observed: String,
}

/// One failing case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    /// The case's name, when the check gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The input, when the check shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    pub expected: String,
    pub observed: String,
    /// Whether the format says which side is expected; `false` means
    /// `expected` is the right side and `observed` the left.
    pub sided: bool,
    /// The intermediate stages, in pipeline order, when exposed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<Stage>,
    /// The format it was read from.
    pub format: String,
}

/// The assertion formats read from plain output, in order.
pub const FORMATS: &[&str] = &[
    "oracle JSON line: {expected, observed|actual|got, input?, name?, stages?}",
    "pytest: assert <left> == <right>",
    "unittest: AssertionError: <left> != <right>",
    "Rust assert_eq: left: <l> / right: <r>",
    "Go testing: got <o>, want <e>",
    "Jest: Expected: <e> / Received: <o>",
    "labeled lines: expected: <e> / got|actual|observed|but was: <o>",
];

fn regex(slot: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    slot.get_or_init(|| Regex::new(pattern).expect("a mismatch pattern compiles"))
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A case from one oracle JSON line, when it holds a failing case.
fn from_json(line: &str) -> Option<Case> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let object = value.as_object()?;
    if object.get("pass").and_then(Value::as_bool) == Some(true)
        || object.get("ok").and_then(Value::as_bool) == Some(true)
    {
        return None;
    }
    let expected = object.get("expected")?;
    let observed = ["observed", "actual", "got"]
        .iter()
        .find_map(|key| object.get(*key))?;
    let mut stages = Vec::new();
    match object.get("stages") {
        Some(Value::Array(items)) => {
            for item in items {
                let (Some(name), Some(e), Some(o)) = (
                    item.get("name").or_else(|| item.get("stage")),
                    item.get("expected"),
                    ["observed", "actual", "got"]
                        .iter()
                        .find_map(|key| item.get(*key)),
                ) else {
                    continue;
                };
                stages.push(Stage {
                    name: text_of(name),
                    expected: text_of(e),
                    observed: text_of(o),
                });
            }
        }
        Some(Value::Object(map)) => {
            for (name, item) in map {
                if let (Some(e), Some(o)) = (
                    item.get("expected"),
                    ["observed", "actual", "got"]
                        .iter()
                        .find_map(|key| item.get(*key)),
                ) {
                    stages.push(Stage {
                        name: name.clone(),
                        expected: text_of(e),
                        observed: text_of(o),
                    });
                }
            }
        }
        _ => {}
    }
    if text_of(expected) == text_of(observed) && stages.iter().all(|s| s.expected == s.observed) {
        return None;
    }
    Some(Case {
        name: object
            .get("name")
            .or_else(|| object.get("case"))
            .map(text_of),
        input: object.get("input").map(text_of),
        expected: text_of(expected),
        observed: text_of(observed),
        sided: true,
        stages,
        format: "oracle-json".to_string(),
    })
}

/// The first failing case `output` shows, or `None`.
#[must_use]
pub fn first_case(output: &str) -> Option<Case> {
    static PYTEST: OnceLock<Regex> = OnceLock::new();
    static UNITTEST: OnceLock<Regex> = OnceLock::new();
    static GO: OnceLock<Regex> = OnceLock::new();
    static LABELED_E: OnceLock<Regex> = OnceLock::new();
    static LABELED_O: OnceLock<Regex> = OnceLock::new();
    static INPUT: OnceLock<Regex> = OnceLock::new();
    let lines: Vec<&str> = output.lines().collect();
    let input_near = |at: usize| {
        let pattern = regex(
            &INPUT,
            r"(?i)^\s*(?:E\s+)?(?:input|given|args?|case)\s*[:=]\s*(.+)$",
        );
        lines[at.saturating_sub(4)..(at + 4).min(lines.len())]
            .iter()
            .find_map(|l| pattern.captures(l).map(|c| c[1].trim().to_string()))
    };
    for (at, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with('{')
            && let Some(case) = from_json(line)
        {
            return Some(case);
        }
        let pytest = regex(
            &PYTEST,
            r"^\s*(?:E\s+)?(?:AssertionError: )?assert (.+?) == (.+?)\s*$",
        );
        if let Some(c) = pytest.captures(line) {
            return Some(Case {
                name: None,
                input: input_near(at),
                expected: c[2].to_string(),
                observed: c[1].to_string(),
                sided: false,
                stages: Vec::new(),
                format: "pytest".to_string(),
            });
        }
        let unittest = regex(
            &UNITTEST,
            r"^\s*(?:E\s+)?AssertionError: (.+?) != (.+?)\s*$",
        );
        if let Some(c) = unittest.captures(line) {
            return Some(Case {
                name: None,
                input: input_near(at),
                expected: c[2].to_string(),
                observed: c[1].to_string(),
                sided: false,
                stages: Vec::new(),
                format: "unittest".to_string(),
            });
        }
        if line.trim_start().starts_with("left:")
            && let Some(right) = lines.get(at + 1).map(|l| l.trim_start())
            && let Some(right) = right.strip_prefix("right:")
        {
            return Some(Case {
                name: None,
                input: input_near(at),
                expected: right.trim().to_string(),
                observed: line.trim_start()["left:".len()..].trim().to_string(),
                sided: false,
                stages: Vec::new(),
                format: "rust-assert".to_string(),
            });
        }
        let go = regex(&GO, r"(?i)\bgot:?\s+(.+?),\s*want:?\s+(.+?)\s*$");
        if let Some(c) = go.captures(line) {
            return Some(Case {
                name: None,
                input: input_near(at),
                expected: c[2].to_string(),
                observed: c[1].to_string(),
                sided: true,
                stages: Vec::new(),
                format: "go-testing".to_string(),
            });
        }
        let expected = regex(
            &LABELED_E,
            r"(?i)^\s*(?:E\s+)?(?:-\s*)?expected(?: value| output)?\s*[:=]\s*(.+?)\s*$",
        );
        if let Some(e) = expected.captures(line) {
            let observed = regex(
                &LABELED_O,
                r"(?i)^\s*(?:E\s+)?(?:\+\s*)?(?:received|got|actual|observed|but was|output)(?: value| output)?\s*[:=]\s*(.+?)\s*$",
            );
            let near = lines[at + 1..(at + 4).min(lines.len())]
                .iter()
                .find_map(|l| observed.captures(l));
            if let Some(o) = near {
                return Some(Case {
                    name: None,
                    input: input_near(at),
                    expected: e[1].to_string(),
                    observed: o[1].to_string(),
                    sided: true,
                    stages: Vec::new(),
                    format: if line.contains("Expected")
                        && lines.iter().any(|l| l.contains("Received"))
                    {
                        "jest".to_string()
                    } else {
                        "labeled".to_string()
                    },
                });
            }
        }
    }
    None
}

/// The first stage whose values differ, by position.
#[must_use]
pub fn first_differing_stage(case: &Case) -> Option<(usize, &Stage)> {
    case.stages
        .iter()
        .enumerate()
        .find(|(_, s)| s.expected != s.observed)
}

/// Where two texts first differ: for one-line values the character
/// position, for several lines the first differing line with a line of
/// each side.
#[must_use]
pub fn diff(expected: &str, observed: &str) -> String {
    diff_labeled(expected, observed, ("expected", "observed"))
}

/// [`diff`], with the two sides named `labels`.
#[must_use]
pub fn diff_labeled(expected: &str, observed: &str, labels: (&str, &str)) -> String {
    let (e_name, o_name) = labels;
    if expected == observed {
        return "no difference".to_string();
    }
    let e_lines: Vec<&str> = expected.lines().collect();
    let o_lines: Vec<&str> = observed.lines().collect();
    if e_lines.len() <= 1 && o_lines.len() <= 1 {
        let at = expected
            .chars()
            .zip(observed.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let window = |s: &str| -> String {
            let chars: Vec<char> = s.chars().collect();
            let from = at.saturating_sub(20);
            let to = (at + 40).min(chars.len());
            let mut out: String = chars[from.min(chars.len())..to].iter().collect();
            if from > 0 {
                out.insert(0, '…');
            }
            if to < chars.len() {
                out.push('…');
            }
            out
        };
        return format!(
            "first difference at character {}: {e_name} `{}`, {o_name} `{}`",
            at + 1,
            window(expected),
            window(observed)
        );
    }
    let at = e_lines
        .iter()
        .zip(&o_lines)
        .take_while(|(a, b)| a == b)
        .count();
    let side = |lines: &[&str]| {
        lines
            .get(at)
            .map_or_else(|| "(no line)".to_string(), |l| crate::judge::clip(l, 200))
    };
    format!(
        "first difference at line {} ({} {e_name} lines, {} {o_name}):\n- {e_name}: {}\n+ {o_name}: {}",
        at + 1,
        e_lines.len(),
        o_lines.len(),
        side(&e_lines),
        side(&o_lines)
    )
}

/// The evidence text for `case`, named by `check`.
#[must_use]
pub fn render(case: &Case, check: &str) -> String {
    let clip = |s: &str| crate::judge::clip(s, 600);
    let (e_label, o_label) = if case.sided {
        ("Expected", "Observed")
    } else {
        ("Right side", "Left side")
    };
    let mut text = format!(
        "The first failing case of `{}`{}:\n",
        crate::judge::clip(check, 120),
        case.name.as_ref().map_or(String::new(), |n| format!(
            ", `{}`",
            crate::judge::clip(n, 80)
        ))
    );
    if let Some(input) = &case.input {
        text.push_str(&format!("- Input: {}\n", clip(input)));
    }
    text.push_str(&format!("- {e_label}: {}\n", clip(&case.expected)));
    text.push_str(&format!("- {o_label}: {}\n", clip(&case.observed)));
    if case.sided {
        text.push_str(&format!(
            "- Diff: {}\n",
            diff(&case.expected, &case.observed)
        ));
    } else {
        text.push_str(&format!(
            "- Diff: {}\n",
            diff_labeled(&case.expected, &case.observed, ("right", "left"))
        ));
    }
    if !case.stages.is_empty() {
        match first_differing_stage(case) {
            Some((index, stage)) => {
                text.push_str(&format!(
                    "- Stages compared in order: {} of {} agree; the first that differs is \
                     `{}`, expected {}, observed {}.\n",
                    index,
                    case.stages.len(),
                    stage.name,
                    crate::judge::clip(&stage.expected, 200),
                    crate::judge::clip(&stage.observed, 200)
                ));
                if index > 0 {
                    text.push_str(&format!(
                        "  The stage before it, `{}`, agrees, so the difference starts in `{}`.\n",
                        case.stages[index - 1].name,
                        stage.name
                    ));
                }
            }
            None => text.push_str(&format!(
                "- Every one of the {} intermediate stages agrees; the difference starts after \
                 the last stage.\n",
                case.stages.len()
            )),
        }
    }
    text.trim_end().to_string()
}
