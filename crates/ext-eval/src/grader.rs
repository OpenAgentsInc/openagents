//! Grader specifications: what a case checks, parsed and validated.
//!
//! A grader comes from a `graders/<name>.md` file (TOML frontmatter and a
//! body) or from an entry of `case.toml`'s `graders` list. Each type has a
//! closed key set, and an unknown key is an error that names the allowed
//! ones. Running a grader is [`crate::grade`]'s job; this module only says
//! what it is.

use indexmap::IndexMap;
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use toml::{Table, Value as Toml};

use crate::case::CaseError;

/// The keys every grader may carry.
pub const COMMON_KEYS: [&str; 4] = ["type", "name", "weight", "arm"];

/// The grader types v1 knows, in the order the spec lists them.
pub const TYPES: [&str; 8] = [
    "regex",
    "operation_used",
    "operation_order",
    "file_exists",
    "decision",
    "judge",
    "receipt",
    "command",
];

/// A `command` check's deadline when it names none, in seconds.
pub const DEFAULT_COMMAND_SECONDS: u32 = 600;
/// The longest deadline a `command` check may ask for, in seconds.
pub const MAX_COMMAND_SECONDS: u32 = 1800;

/// The largest regular expression a grader may compile, in bytes of the
/// compiled program. The default of the `regex` crate, stated.
const REGEX_SIZE_LIMIT: usize = 10 * (1 << 20);

/// Which arms a grader is scored in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmRule {
    /// Only meaningful with the extension present: dropped from the
    /// baseline arm and from the score in both arms.
    SubjectOnly,
    /// Scored in both arms.
    Both,
}

impl ArmRule {
    /// The word a case file writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::SubjectOnly => "subject-only",
            Self::Both => "both",
        }
    }
}

/// What a `regex`, `decision`, or `judge` grader reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Focus {
    /// Coder's final assistant text.
    LastMessage,
    /// The run's rendered ATIF document. A door sees the first and last
    /// twelve steps and the final message.
    Trajectory,
    /// The paths of files the run created, one per line.
    Files,
    /// The contents of one file in the run's workspace after the run.
    File(String),
    /// What a files test changed: one `added|modified|deleted <path>` line
    /// per path.
    Changed,
    /// The unified diff of what a files test changed.
    Diff,
}

impl Focus {
    /// How a report names the focus.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::LastMessage => "last_message".into(),
            Self::Trajectory => "trajectory".into(),
            Self::Files => "files".into(),
            Self::File(path) => format!("file {path}"),
            Self::Changed => "changed".into(),
            Self::Diff => "diff".into(),
        }
    }
}

/// How a `regex` grader counts matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    /// At least one match.
    Contains,
    /// No match.
    NotContains,
    /// Exactly this many matches.
    Count(usize),
}

/// The typed question a `decision` grader asks Jev.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionQuestion {
    /// A Noul: the probability that the answer is yes.
    Noul {
        /// The judgment.
        instructions: String,
    },
    /// A Score over ordered levels; the probability is the answer's
    /// position divided by the top level.
    Score {
        /// The judgment.
        instructions: String,
        /// The levels, lowest first, 2 to 10 of them.
        levels: Vec<String>,
    },
    /// A Choice; the probability is the mass on the passing options.
    Choice {
        /// The judgment.
        instructions: String,
        /// The options and what each means.
        options: IndexMap<String, String>,
        /// The options that count as a pass.
        pass: Vec<String>,
    },
}

/// What a grader checks.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Check {
    /// A pattern over the focus.
    Regex {
        /// The pattern as written.
        pattern: String,
        /// `i`, `m`, `s`, and `x`, as written.
        flags: String,
        /// How matches are counted.
        #[serde(rename = "match")]
        matching: Match,
        /// What the pattern reads.
        target: Focus,
        /// The compiled pattern.
        #[serde(skip)]
        regex: Regex,
    },
    /// How often an operation ran.
    OperationUsed {
        /// The operation's name as the trajectory records it.
        operation: String,
        /// A pattern the call's input must match, when set.
        input_match: Option<String>,
        /// Fewest matching calls.
        min: usize,
        /// Most matching calls, when bounded.
        max: Option<usize>,
        /// The compiled input pattern.
        #[serde(skip)]
        input_regex: Option<Regex>,
    },
    /// Two operations ran, the first one first.
    OperationOrder {
        /// The operation that has to come first.
        before: String,
        /// The operation that has to come after it.
        after: String,
    },
    /// A created file matches, or none does.
    FileExists {
        /// A glob over created paths.
        path: String,
        /// Whether a match is required (`true`) or forbidden.
        exists: bool,
    },
    /// Jev answers a typed question about the focus.
    Decision {
        /// The question.
        question: DecisionQuestion,
        /// The probability a vote has to reach.
        threshold: f64,
        /// What the question reads.
        focus: Focus,
        /// What a successful run looks like, in the author's words.
        rubric: Option<String>,
    },
    /// The chat model door answers PASS or FAIL against prose criteria.
    Judge {
        /// The criteria.
        criteria: String,
        /// What the door reads.
        focus: Focus,
    },
    /// A Wasm guest's invocation receipts replay exactly.
    Receipt {
        /// The guest operation.
        operation: String,
    },
    /// A shell command, run in a files test's workspace after the turn and
    /// inside the run's boundary, exits with the expected code.
    Command {
        /// The command, run with `/bin/sh -c`.
        command: String,
        /// The exit code that passes.
        exit_code: i32,
        /// How long it may run, 1 to 1800 seconds.
        deadline_seconds: u32,
    },
}

impl Check {
    /// The type word.
    #[must_use]
    pub const fn type_word(&self) -> &'static str {
        match self {
            Self::Regex { .. } => "regex",
            Self::OperationUsed { .. } => "operation_used",
            Self::OperationOrder { .. } => "operation_order",
            Self::FileExists { .. } => "file_exists",
            Self::Decision { .. } => "decision",
            Self::Judge { .. } => "judge",
            Self::Receipt { .. } => "receipt",
            Self::Command { .. } => "command",
        }
    }

    /// Whether the check reads a files test's workspace: a `command`, or a
    /// `changed` or `diff` focus.
    #[must_use]
    pub const fn needs_workspace(&self) -> bool {
        match self {
            Self::Command { .. } => true,
            Self::Regex { target, .. } => matches!(target, Focus::Changed | Focus::Diff),
            Self::Decision { focus, .. } | Self::Judge { focus, .. } => {
                matches!(focus, Focus::Changed | Focus::Diff)
            }
            _ => false,
        }
    }

    /// Whether the check calls a door and spends quota.
    #[must_use]
    pub const fn calls_a_door(&self) -> bool {
        matches!(self, Self::Decision { .. } | Self::Judge { .. })
    }

    /// The type-specific keys, which with [`COMMON_KEYS`] are the whole
    /// allowed set.
    #[must_use]
    pub const fn keys(type_word: &str) -> Option<&'static [&'static str]> {
        Some(match type_word.as_bytes() {
            b"regex" => &["pattern", "flags", "match", "target"],
            b"operation_used" => &["operation", "input_match", "min", "max"],
            b"operation_order" => &["before", "after"],
            b"file_exists" => &["path", "exists"],
            b"decision" => &["question", "threshold", "focus", "rubric"],
            b"judge" => &["criteria", "focus"],
            b"receipt" => &["operation"],
            b"command" => &["command", "exit_code", "deadline_seconds"],
            _ => return None,
        })
    }
}

/// One grader of a case.
#[derive(Clone, Debug, Serialize)]
pub struct Grader {
    /// Unique within the case.
    pub name: String,
    /// Greater than zero; the run score is the weighted mean.
    pub weight: f64,
    /// The arm rule the case wrote, when it wrote one.
    pub arm: Option<ArmRule>,
    /// What it checks.
    #[serde(flatten)]
    pub check: Check,
    /// Where it was written: `graders/<file>.md` or `case.toml#graders[i]`.
    pub origin: String,
}

impl Grader {
    /// Whether this grader is dropped from the baseline arm and from the
    /// score: `arm = "subject-only"`, or an `operation_used` on an
    /// operation the extension supplies with no explicit arm.
    #[must_use]
    pub fn subject_only(&self, extension_operations: &std::collections::BTreeSet<String>) -> bool {
        match self.arm {
            Some(ArmRule::SubjectOnly) => true,
            Some(ArmRule::Both) => false,
            None => matches!(
                &self.check,
                Check::OperationUsed { operation, .. } if extension_operations.contains(operation)
            ),
        }
    }

    /// Parses one grader.
    ///
    /// `body` is the Markdown body of a grader file: the pattern of a
    /// `regex`, the rubric of a `decision`, and the criteria of a `judge`.
    ///
    /// # Errors
    ///
    /// Returns [`CaseError`] naming the file and the key.
    pub fn parse(
        table: &Table,
        body: Option<&str>,
        default_name: Option<&str>,
        origin: &str,
    ) -> Result<Self, CaseError> {
        let at = |detail: String| CaseError::Invalid {
            file: origin.to_string(),
            detail,
        };
        let type_word = match table.get("type") {
            Some(Toml::String(word)) => word.as_str(),
            Some(_) => return Err(at("`type` must be a string".into())),
            None => {
                return Err(at(format!(
                    "a grader needs a `type`; the types are {}",
                    TYPES.join(", ")
                )));
            }
        };
        let Some(specific) = Check::keys(type_word) else {
            return Err(at(format!(
                "unknown grader type `{type_word}`; the types are {}",
                TYPES.join(", ")
            )));
        };
        let allowed: Vec<&str> = COMMON_KEYS.iter().chain(specific).copied().collect();
        crate::case::check_keys(table, &allowed, origin, "")?;

        let name = match table.get("name") {
            Some(Toml::String(name)) => name.clone(),
            Some(_) => return Err(at("`name` must be a string".into())),
            None => default_name
                .map(str::to_string)
                .ok_or_else(|| at("a grader in case.toml needs a `name`".into()))?,
        };
        crate::case::check_name(&name, origin, "grader name")?;
        let weight = match table.get("weight") {
            None => 1.0,
            Some(value) => number(value).ok_or_else(|| at("`weight` must be a number".into()))?,
        };
        if !weight.is_finite() || weight <= 0.0 {
            return Err(at(format!("`weight` must be greater than 0, got {weight}")));
        }
        let arm = match table.get("arm") {
            None => None,
            Some(Toml::String(word)) if word == "subject-only" => Some(ArmRule::SubjectOnly),
            Some(Toml::String(word)) if word == "both" => Some(ArmRule::Both),
            Some(_) => {
                return Err(at("`arm` must be \"subject-only\" or \"both\"".to_string()));
            }
        };
        let body = body.map(str::trim).filter(|body| !body.is_empty());
        let check = parse_check(type_word, table, body, origin)?;
        Ok(Self {
            name,
            weight,
            arm,
            check,
            origin: origin.to_string(),
        })
    }
}

fn parse_check(
    type_word: &str,
    table: &Table,
    body: Option<&str>,
    origin: &str,
) -> Result<Check, CaseError> {
    let at = |detail: String| CaseError::Invalid {
        file: origin.to_string(),
        detail,
    };
    let text = |key: &str| -> Result<Option<String>, CaseError> {
        match table.get(key) {
            None => Ok(None),
            Some(Toml::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
            Some(Toml::String(_)) => Err(at(format!("`{key}` is empty"))),
            Some(_) => Err(at(format!("`{key}` must be a string"))),
        }
    };
    let required = |key: &str| -> Result<String, CaseError> {
        text(key)?.ok_or_else(|| at(format!("a {type_word} grader needs `{key}`")))
    };
    let from_body = |key: &str| -> Result<String, CaseError> {
        match (text(key)?, body) {
            (Some(_), Some(_)) => Err(at(format!(
                "`{key}` is given twice: in the frontmatter and as the body"
            ))),
            (Some(value), None) => Ok(value),
            (None, Some(body)) => Ok(body.to_string()),
            (None, None) => Err(at(format!(
                "a {type_word} grader needs `{key}`, in the frontmatter or as the body"
            ))),
        }
    };
    let count = |key: &str| -> Result<Option<usize>, CaseError> {
        match table.get(key) {
            None => Ok(None),
            Some(Toml::Integer(value)) if *value >= 0 => Ok(Some(
                usize::try_from(*value).map_err(|_| at(format!("`{key}` is too large")))?,
            )),
            Some(_) => Err(at(format!("`{key}` must be a whole number, 0 or more"))),
        }
    };
    Ok(match type_word {
        "regex" => {
            let pattern = from_body("pattern")?;
            let flags = text("flags")?.unwrap_or_default();
            let regex = compile(&pattern, &flags).map_err(at)?;
            let matching = match text("match")?.as_deref() {
                None | Some("contains") => Match::Contains,
                Some("not_contains") => Match::NotContains,
                Some(other) => match other.strip_prefix("count:").map(str::parse::<usize>) {
                    Some(Ok(n)) => Match::Count(n),
                    _ => {
                        return Err(at(format!(
                            "`match` is `{other}`; it must be contains, not_contains, or count:N"
                        )));
                    }
                },
            };
            let target = focus(table.get("target"), "target", origin)?;
            Check::Regex {
                pattern,
                flags,
                matching,
                target,
                regex,
            }
        }
        "operation_used" => {
            let operation = required("operation")?;
            let input_match = text("input_match")?;
            let input_regex = input_match
                .as_deref()
                .map(|pattern| compile(pattern, ""))
                .transpose()
                .map_err(at)?;
            let min = count("min")?.unwrap_or(1);
            let max = count("max")?;
            if let Some(max) = max
                && max < min
            {
                return Err(at(format!("`max` ({max}) is below `min` ({min})")));
            }
            Check::OperationUsed {
                operation,
                input_match,
                min,
                max,
                input_regex,
            }
        }
        "operation_order" => Check::OperationOrder {
            before: required("before")?,
            after: required("after")?,
        },
        "file_exists" => {
            let path = required("path")?;
            let exists = match table.get("exists") {
                None => true,
                Some(Toml::Boolean(value)) => *value,
                Some(_) => return Err(at("`exists` must be true or false".into())),
            };
            Check::FileExists { path, exists }
        }
        "decision" => {
            let question = decision_question(table.get("question"), origin)?;
            let threshold = match table.get("threshold") {
                None => 0.5,
                Some(value) => number(value)
                    .filter(|value| (0.0..=1.0).contains(value))
                    .ok_or_else(|| at("`threshold` must be a number from 0 to 1".into()))?,
            };
            let rubric = match (text("rubric")?, body) {
                (Some(_), Some(_)) => {
                    return Err(at(
                        "`rubric` is given twice: in the frontmatter and as the body".into(),
                    ));
                }
                (rubric, body) => rubric.or_else(|| body.map(str::to_string)),
            };
            Check::Decision {
                question,
                threshold,
                focus: focus(table.get("focus"), "focus", origin)?,
                rubric,
            }
        }
        "judge" => Check::Judge {
            criteria: from_body("criteria")?,
            focus: focus(table.get("focus"), "focus", origin)?,
        },
        "receipt" => Check::Receipt {
            operation: required("operation")?,
        },
        "command" => {
            let command = from_body("command")?;
            let exit_code = match table.get("exit_code") {
                None => 0,
                Some(Toml::Integer(code)) => i32::try_from(*code)
                    .map_err(|_| at(format!("`exit_code` {code} is not an exit code")))?,
                Some(_) => return Err(at("`exit_code` must be a whole number".into())),
            };
            let deadline_seconds = match table.get("deadline_seconds") {
                None => DEFAULT_COMMAND_SECONDS,
                Some(Toml::Integer(seconds))
                    if (1..=i64::from(MAX_COMMAND_SECONDS)).contains(seconds) =>
                {
                    u32::try_from(*seconds).unwrap_or(DEFAULT_COMMAND_SECONDS)
                }
                Some(_) => {
                    return Err(at(format!(
                        "`deadline_seconds` must be a whole number from 1 to {MAX_COMMAND_SECONDS}"
                    )));
                }
            };
            Check::Command {
                command,
                exit_code,
                deadline_seconds,
            }
        }
        other => return Err(at(format!("unknown grader type `{other}`"))),
    })
}

fn compile(pattern: &str, flags: &str) -> Result<Regex, String> {
    let mut builder = RegexBuilder::new(pattern);
    builder.size_limit(REGEX_SIZE_LIMIT);
    for flag in flags.chars() {
        match flag {
            'i' => builder.case_insensitive(true),
            'm' => builder.multi_line(true),
            's' => builder.dot_matches_new_line(true),
            'x' => builder.ignore_whitespace(true),
            other => {
                return Err(format!(
                    "unknown regex flag `{other}`; the flags are i, m, s, and x"
                ));
            }
        };
    }
    builder
        .build()
        .map_err(|error| format!("the pattern does not compile: {error}"))
}

fn number(value: &Toml) -> Option<f64> {
    match value {
        #[allow(clippy::cast_precision_loss)]
        Toml::Integer(value) => Some(*value as f64),
        Toml::Float(value) => Some(*value),
        _ => None,
    }
}

fn focus(value: Option<&Toml>, key: &str, origin: &str) -> Result<Focus, CaseError> {
    let at = |detail: String| CaseError::Invalid {
        file: origin.to_string(),
        detail,
    };
    match value {
        None => Ok(Focus::LastMessage),
        Some(Toml::String(word)) => match word.as_str() {
            "last_message" => Ok(Focus::LastMessage),
            "trajectory" => Ok(Focus::Trajectory),
            "files" => Ok(Focus::Files),
            "changed" => Ok(Focus::Changed),
            "diff" => Ok(Focus::Diff),
            other => Err(at(format!(
                "`{key}` is `{other}`; it must be last_message, trajectory, files, changed, \
                 diff, or {{ file = \"<path>\" }}"
            ))),
        },
        Some(Toml::Table(table)) => {
            crate::case::check_keys(table, &["file"], origin, &format!("{key}."))?;
            match table.get("file") {
                Some(Toml::String(path)) => {
                    workspace_path(path).map_err(|detail| at(format!("`{key}.file` {detail}")))?;
                    Ok(Focus::File(path.clone()))
                }
                _ => Err(at(format!("`{key}.file` must be a path string"))),
            }
        }
        Some(_) => Err(at(format!("`{key}` must be a string or a table"))),
    }
}

/// Checks that a path names something inside the run's workspace: relative,
/// with no `..`, no empty segment, and no `.` segment.
///
/// # Errors
///
/// Returns why the path is refused.
pub fn workspace_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return Err(format!(
            "is `{path}`; it must be a relative path inside the workspace"
        ));
    }
    if path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(format!(
            "is `{path}`; it must not contain empty, `.`, or `..` segments"
        ));
    }
    Ok(())
}

fn decision_question(value: Option<&Toml>, origin: &str) -> Result<DecisionQuestion, CaseError> {
    let at = |detail: String| CaseError::Invalid {
        file: origin.to_string(),
        detail,
    };
    match value {
        None => Err(at("a decision grader needs `question`".into())),
        Some(Toml::String(text)) if !text.trim().is_empty() => Ok(DecisionQuestion::Noul {
            instructions: text.clone(),
        }),
        Some(Toml::Table(table)) => {
            crate::case::check_keys(
                table,
                &["instructions", "levels", "options", "pass"],
                origin,
                "question.",
            )?;
            let instructions = match table.get("instructions") {
                Some(Toml::String(text)) if !text.trim().is_empty() => text.clone(),
                _ => {
                    return Err(at(
                        "`question.instructions` must be a non-empty string".into()
                    ));
                }
            };
            let strings = |key: &str| -> Result<Option<Vec<String>>, CaseError> {
                match table.get(key) {
                    None => Ok(None),
                    Some(Toml::Array(items)) => items
                        .iter()
                        .map(|item| match item {
                            Toml::String(text) if !text.trim().is_empty() => Ok(text.clone()),
                            _ => Err(at(format!("`question.{key}` holds non-empty strings"))),
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(Some),
                    Some(_) => Err(at(format!("`question.{key}` must be an array"))),
                }
            };
            match (table.get("levels"), table.get("options")) {
                (Some(_), Some(_)) => Err(at(
                    "`question` has both `levels` (a Score) and `options` (a Choice); keep one"
                        .into(),
                )),
                (Some(_), None) => {
                    if table.contains_key("pass") {
                        return Err(at(
                            "`question.pass` belongs to a Choice with `options`".into()
                        ));
                    }
                    let levels = strings("levels")?.unwrap_or_default();
                    if !(2..=10).contains(&levels.len()) {
                        return Err(at(format!(
                            "`question.levels` has {} levels; a Score has 2 to 10",
                            levels.len()
                        )));
                    }
                    Ok(DecisionQuestion::Score {
                        instructions,
                        levels,
                    })
                }
                (None, Some(Toml::Table(options))) => {
                    let mut named = IndexMap::new();
                    for (name, meaning) in options {
                        match meaning {
                            Toml::String(text) if !text.trim().is_empty() => {
                                named.insert(name.clone(), text.clone());
                            }
                            _ => {
                                return Err(at(format!(
                                    "`question.options.{name}` must describe the option"
                                )));
                            }
                        }
                    }
                    if named.len() < 2 {
                        return Err(at("`question.options` needs at least two options".into()));
                    }
                    let pass = strings("pass")?.unwrap_or_default();
                    if pass.is_empty() {
                        return Err(at(
                            "`question.pass` names the options that count as a pass".into()
                        ));
                    }
                    if let Some(unknown) = pass.iter().find(|name| !named.contains_key(*name)) {
                        return Err(at(format!(
                            "`question.pass` names `{unknown}`, which is not an option"
                        )));
                    }
                    Ok(DecisionQuestion::Choice {
                        instructions,
                        options: named,
                        pass,
                    })
                }
                (None, Some(_)) => Err(at("`question.options` must be a table".into())),
                (None, None) => {
                    if table.contains_key("pass") {
                        return Err(at(
                            "`question.pass` belongs to a Choice with `options`".into()
                        ));
                    }
                    Ok(DecisionQuestion::Noul { instructions })
                }
            }
        }
        Some(_) => Err(at(
            "`question` must be a non-empty string or a table".to_string()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str, body: Option<&str>) -> Result<Grader, CaseError> {
        let table: Table = toml::from_str(source).expect("toml");
        Grader::parse(&table, body, Some("g"), "graders/g.md")
    }

    #[test]
    fn a_regex_takes_its_pattern_from_the_body() {
        let grader = parse("type = \"regex\"\nflags = \"i\"", Some("hello\n")).expect("valid");
        let Check::Regex { regex, pattern, .. } = &grader.check else {
            panic!("regex");
        };
        assert_eq!(pattern, "hello");
        assert!(regex.is_match("HeLLo"));
        assert_eq!(grader.name, "g");
        assert!((grader.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_pattern_given_twice_is_refused() {
        let error = parse("type = \"regex\"\npattern = \"a\"", Some("b")).unwrap_err();
        assert!(error.to_string().contains("given twice"), "{error}");
    }

    #[test]
    fn an_unknown_key_names_the_allowed_set() {
        let error = parse("type = \"file_exists\"\npath = \"a\"\nglob = 1", None).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("`glob`"), "{text}");
        assert!(
            text.contains("type, name, weight, arm, path, exists"),
            "{text}"
        );
    }

    #[test]
    fn an_unknown_type_names_the_types() {
        let error = parse("type = \"baseline\"", None).unwrap_err();
        assert!(
            error.to_string().contains("regex, operation_used"),
            "{error}"
        );
    }

    #[test]
    fn weights_must_be_positive_and_arms_known() {
        assert!(parse("type = \"receipt\"\noperation = \"x\"\nweight = 0", None).is_err());
        assert!(parse("type = \"receipt\"\noperation = \"x\"\nweight = -1.5", None).is_err());
        assert!(
            parse(
                "type = \"receipt\"\noperation = \"x\"\narm = \"baseline\"",
                None
            )
            .is_err()
        );
        let grader = parse(
            "type = \"receipt\"\noperation = \"x\"\narm = \"subject-only\"",
            None,
        )
        .unwrap();
        assert_eq!(grader.arm, Some(ArmRule::SubjectOnly));
    }

    #[test]
    fn match_modes_parse_and_refuse() {
        let grader = parse("type = \"regex\"\nmatch = \"count:2\"", Some("x")).unwrap();
        assert!(matches!(
            grader.check,
            Check::Regex {
                matching: Match::Count(2),
                ..
            }
        ));
        assert!(parse("type = \"regex\"\nmatch = \"count:two\"", Some("x")).is_err());
        assert!(parse("type = \"regex\"\nflags = \"q\"", Some("x")).is_err());
        assert!(parse("type = \"regex\"", Some("(")).is_err());
    }

    #[test]
    fn operation_used_bounds() {
        let grader = parse("type = \"operation_used\"\noperation = \"shell\"", None).unwrap();
        assert!(matches!(
            grader.check,
            Check::OperationUsed {
                min: 1,
                max: None,
                ..
            }
        ));
        let error = parse(
            "type = \"operation_used\"\noperation = \"shell\"\nmin = 2\nmax = 1",
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("below"), "{error}");
    }

    #[test]
    fn focus_paths_stay_inside_the_workspace() {
        assert!(parse("type = \"judge\"\nfocus = { file = \"../x\" }", Some("c")).is_err());
        assert!(
            parse(
                "type = \"judge\"\nfocus = { file = \"/etc/passwd\" }",
                Some("c")
            )
            .is_err()
        );
        assert!(parse("type = \"judge\"\nfocus = { path = \"x\" }", Some("c")).is_err());
        let grader = parse(
            "type = \"judge\"\nfocus = { file = \"out/a.md\" }",
            Some("c"),
        )
        .unwrap();
        assert!(matches!(
            grader.check,
            Check::Judge { focus: Focus::File(ref path), .. } if path == "out/a.md"
        ));
    }

    #[test]
    fn decision_questions_are_typed() {
        let noul = parse(
            "type = \"decision\"\nquestion = \"Did it work?\"\nthreshold = 0.7",
            Some("rubric"),
        )
        .unwrap();
        assert!(matches!(
            noul.check,
            Check::Decision { question: DecisionQuestion::Noul { .. }, threshold, rubric: Some(_), .. }
                if (threshold - 0.7).abs() < 1e-12
        ));
        let score = parse(
            "type = \"decision\"\nquestion = { instructions = \"How well?\", levels = [\"no\", \"partly\", \"fully\"] }",
            None,
        )
        .unwrap();
        assert!(matches!(
            score.check,
            Check::Decision {
                question: DecisionQuestion::Score { .. },
                ..
            }
        ));
        let choice = parse(
            "type = \"decision\"\n[question]\ninstructions = \"Which?\"\npass = [\"fixed\"]\n[question.options]\nfixed = \"It fixed it\"\nbroke = \"It broke it\"",
            None,
        )
        .unwrap();
        assert!(matches!(
            choice.check,
            Check::Decision {
                question: DecisionQuestion::Choice { .. },
                ..
            }
        ));
        assert!(parse("type = \"decision\"\nthreshold = 0.5", None).is_err());
        assert!(
            parse(
                "type = \"decision\"\nquestion = \"q\"\nthreshold = 1.5",
                None
            )
            .is_err()
        );
        assert!(
            parse(
                "type = \"decision\"\nquestion = { instructions = \"q\", levels = [\"one\"] }",
                None
            )
            .is_err()
        );
        assert!(
            parse(
                "type = \"decision\"\n[question]\ninstructions = \"q\"\npass = [\"c\"]\n[question.options]\na = \"x\"\nb = \"y\"",
                None
            )
            .is_err()
        );
    }

    #[test]
    fn subject_only_follows_the_arm_or_the_extension_operation() {
        let operations = std::collections::BTreeSet::from(["repo-map.map".to_string()]);
        let supplied = parse(
            "type = \"operation_used\"\noperation = \"repo-map.map\"",
            None,
        )
        .unwrap();
        assert!(supplied.subject_only(&operations));
        let both = parse(
            "type = \"operation_used\"\noperation = \"repo-map.map\"\narm = \"both\"",
            None,
        )
        .unwrap();
        assert!(!both.subject_only(&operations));
        let shell = parse("type = \"operation_used\"\noperation = \"shell\"", None).unwrap();
        assert!(!shell.subject_only(&operations));
        let marked = parse("type = \"regex\"\narm = \"subject-only\"", Some("x")).unwrap();
        assert!(marked.subject_only(&operations));
    }
}
