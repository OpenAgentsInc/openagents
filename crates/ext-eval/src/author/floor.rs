//! The floor: what every draft the interview returns holds, whatever the
//! model proposed.
//!
//! - Every test parses as a case the runner accepts, with no `TODO` line.
//! - No test prompt names the tool or one of its operations. The check is
//!   against the tool's declared names, a bounded field, and a test that
//!   fails it is left out, never rewritten.
//! - Every test has at least one scored check of its outcome; the floor adds
//!   the starting outcome check to a test without one.
//! - Every test runs at least three times per arm (`runs` is never below
//!   the default).
//! - At least one test is one where the tool should stay out of the way;
//!   the floor adds one when none is left.
//! - At most `max_cases` tests, and a draft within the wire's 64 KiB.
//! - Tests in the order the runner runs them: by id.
//!
//! The baseline arm is not a file: every run the interview offers compares
//! with and without the plugin (`arms = 2`), which [`super::machine`] checks.

use std::collections::BTreeSet;

use nostr::cj_conversation::{Draft, DraftCase, draft_value};
use nostr::eval_ext::CaseKind;

use crate::case::{DEFAULT_RUNS, Kind};

use super::catalog::{Catalog, Tool};
use super::render::{self, OUTCOME, is_outcome, outcome_grader, prompt_md, unused_grader};

/// The id of the test the floor adds when no test is one where the tool
/// should stay out of the way.
pub const ADDED_ID: &str = "stays-out-of-the-way";
/// Its task: one no tool here should help with.
pub const ADDED_TASK: &str = "In one or two sentences, what does the word \"idempotent\" mean in programming? Answer from what you know; there is no need to look at any files.";
/// Its outcome check's rubric.
pub const ADDED_RUBRIC: &str = "The answer says an idempotent operation has the same effect whether it runs once or many times.";

/// A way a draft breaks the floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A test the engine refuses.
    Invalid { id: String, reason: String },
    /// A test with a `TODO` line.
    Todo { id: String },
    /// A test prompt that names the tool or an operation.
    NamesTool { id: String, name: String },
    /// A test with no scored check of its outcome.
    NoOutcome { id: String },
    /// A test that runs fewer than three times per arm.
    FewRuns { id: String, runs: u32 },
    /// No test where the tool should stay out of the way.
    NoShouldNotFire,
    /// More tests than allowed.
    TooMany { count: usize, max: usize },
    /// A draft over 64 KiB.
    TooLarge,
}

/// Whether `text` names `name`: the words of `name`, in order, as whole
/// words of `text`, ignoring case, with `_` and `-` read as spaces. An
/// exact check of a declared name, not a reading of intent.
#[must_use]
pub fn names(text: &str, name: &str) -> bool {
    let words = |s: &str| -> String {
        let spaced: String = s
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect();
        spaced.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let name = words(name);
    if name.is_empty() {
        return false;
    }
    format!(" {} ", words(text)).contains(&format!(" {name} "))
}

/// Every way `draft` breaks the floor for `tool`.
#[must_use]
pub fn check(
    tool: &Tool,
    catalog: &Catalog,
    cases: &[DraftCase],
    max_cases: usize,
) -> Vec<Violation> {
    let forbidden = tool.forbidden_names(catalog);
    let operations: BTreeSet<String> = tool.operations.iter().cloned().collect();
    let mut found = Vec::new();
    for case in cases {
        let parsed = match render::parse(case) {
            Ok(parsed) => parsed,
            Err(error) => {
                found.push(Violation::Invalid {
                    id: case.id.clone(),
                    reason: error.to_string(),
                });
                continue;
            }
        };
        if !parsed.todo_lines().is_empty() {
            found.push(Violation::Todo {
                id: case.id.clone(),
            });
        }
        if let Some(name) = forbidden.iter().find(|name| names(&parsed.prompt, name)) {
            found.push(Violation::NamesTool {
                id: case.id.clone(),
                name: name.clone(),
            });
        }
        let outcome = parsed
            .graders
            .iter()
            .any(|g| is_outcome(&g.check) && !g.subject_only(&operations));
        if !outcome {
            found.push(Violation::NoOutcome {
                id: case.id.clone(),
            });
        }
        if parsed.runs < DEFAULT_RUNS {
            found.push(Violation::FewRuns {
                id: case.id.clone(),
                runs: parsed.runs,
            });
        }
    }
    if !cases.iter().any(|c| c.kind == CaseKind::ShouldNotFire) {
        found.push(Violation::NoShouldNotFire);
    }
    if cases.len() > max_cases {
        found.push(Violation::TooMany {
            count: cases.len(),
            max: max_cases,
        });
    }
    let draft = Draft {
        tool: tool.draft_tool(),
        cases: cases.to_vec(),
    };
    if draft_value(&draft).is_err() {
        found.push(Violation::TooLarge);
    }
    found
}

/// The test the floor adds: a task the tool shouldn't touch, its outcome
/// check, and a check that each of the tool's operations never ran.
#[must_use]
pub fn added_case(tool: &Tool, taken: &BTreeSet<String>) -> DraftCase {
    let mut id = ADDED_ID.to_string();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{ADDED_ID}-{n}");
        n += 1;
    }
    let mut graders = vec![outcome_grader(Some(ADDED_RUBRIC))];
    for operation in &tool.operations {
        graders.push(unused_grader(operation));
    }
    graders.sort();
    graders.dedup_by(|a, b| a.0 == b.0);
    DraftCase {
        id,
        kind: CaseKind::ShouldNotFire,
        prompt: prompt_md(Kind::ShouldNotFire, ADDED_TASK),
        graders,
    }
}

/// Makes `cases` hold the floor, and says in plain words what changed.
///
/// Refused tests are left out; a test without an outcome check gets the
/// starting one; a test that runs fewer than three times is rewritten to
/// the default; extra tests past `max_cases` are left out, keeping tests
/// where the tool should stay out of the way; and a test where the tool
/// should stay out of the way is added when none is left.
#[must_use]
pub fn enforce(
    tool: &Tool,
    catalog: &Catalog,
    cases: Vec<DraftCase>,
    max_cases: usize,
) -> (Vec<DraftCase>, Vec<String>) {
    let forbidden = tool.forbidden_names(catalog);
    let operations: BTreeSet<String> = tool.operations.iter().cloned().collect();
    let mut notes = Vec::new();
    let mut kept: Vec<DraftCase> = Vec::new();
    let mut seen = BTreeSet::new();
    for mut case in cases {
        if !seen.insert(case.id.clone()) {
            continue;
        }
        // Drop the checks the engine refuses, one at a time, so one bad
        // check doesn't cost the test.
        let shell = DraftCase {
            graders: Vec::new(),
            ..case.clone()
        };
        case.graders.retain(|grader| {
            render::parse(&DraftCase {
                graders: vec![grader.clone()],
                ..shell.clone()
            })
            .is_ok()
        });
        case.graders.sort_by(|a, b| a.0.cmp(&b.0));
        case.graders.dedup_by(|a, b| a.0 == b.0);
        if case.graders.is_empty() {
            case.graders.push(outcome_grader(None));
        }
        let parsed = match render::parse(&case) {
            Ok(parsed) => parsed,
            Err(_) => {
                notes.push(format!(
                    "We left out the test {} because it isn't a test we can run.",
                    case.id
                ));
                continue;
            }
        };
        if !parsed.todo_lines().is_empty() {
            notes.push(format!(
                "We left out the test {} because it still has a TODO line.",
                case.id
            ));
            continue;
        }
        if let Some(name) = forbidden.iter().find(|name| names(&parsed.prompt, name)) {
            notes.push(format!(
                "We left out the test {} because its task names {name}; a test is a task, and whether the plugin helps is what we measure.",
                case.id
            ));
            continue;
        }
        if parsed.runs < DEFAULT_RUNS {
            case.prompt = prompt_md(parsed.kind, &parsed.prompt);
            notes.push(format!(
                "The test {} runs three times with the plugin and three without, so one lucky run doesn't decide it.",
                case.id
            ));
        }
        let outcome = parsed
            .graders
            .iter()
            .any(|g| is_outcome(&g.check) && !g.subject_only(&operations));
        if !outcome {
            let mut name = OUTCOME.to_string();
            let mut n = 2;
            while case.graders.iter().any(|(existing, _)| *existing == name) {
                name = format!("{OUTCOME}-{n}");
                n += 1;
            }
            let (_, text) = outcome_grader(None);
            case.graders.push((name, text));
            case.graders.sort_by(|a, b| a.0.cmp(&b.0));
            notes.push(format!(
                "We added a check of the outcome to the test {}: every test checks what Coder produced.",
                case.id
            ));
        }
        kept.push(case);
    }
    if kept.len() > max_cases {
        let quiet_keep = kept
            .iter()
            .filter(|c| c.kind == CaseKind::ShouldNotFire)
            .count()
            .min(2)
            .min(max_cases);
        let fire_keep = max_cases - quiet_keep;
        let (mut fire, mut quiet) = (0, 0);
        kept.retain(|case| match case.kind {
            CaseKind::ShouldFire => {
                fire += 1;
                fire <= fire_keep
            }
            CaseKind::ShouldNotFire => {
                quiet += 1;
                quiet <= quiet_keep
            }
        });
        notes.push(format!(
            "We kept {max_cases} tests, the most one test set may run here."
        ));
    }
    if !kept.iter().any(|c| c.kind == CaseKind::ShouldNotFire) {
        if kept.len() >= max_cases {
            kept.pop();
        }
        let taken: BTreeSet<String> = kept.iter().map(|c| c.id.clone()).collect();
        kept.push(added_case(tool, &taken));
        notes.push(
            "We added a test where the plugin should stay out of the way: every test set needs at least one."
                .into(),
        );
    }
    while kept
        .iter()
        .filter(|c| c.kind == CaseKind::ShouldFire)
        .count()
        > 1
        && draft_value(&Draft {
            tool: tool.draft_tool(),
            cases: kept.clone(),
        })
        .is_err()
    {
        if let Some(index) = kept.iter().rposition(|c| c.kind == CaseKind::ShouldFire) {
            kept.remove(index);
            notes.push("We left out a test to keep the test set small enough to send.".into());
        }
    }
    // The runner runs cases in directory order; the draft keeps the same
    // order, so its files read back as the same draft.
    kept.sort_by(|a, b| a.id.cmp(&b.id));
    (kept, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::catalog::Catalog;

    #[test]
    fn names_are_whole_words_ignoring_case_and_separators() {
        assert!(names("Use repo_map first", "repo_map"));
        assert!(names("look at the repo map", "repo_map"));
        assert!(names("Open the PROJECT-MAP", "Project map"));
        assert!(!names("a repository map", "repo_map"));
        assert!(!names("remap the keys", "map"));
        assert!(!names("anything", ""));
    }

    #[test]
    fn an_empty_draft_gets_a_quiet_test_and_every_test_an_outcome() {
        let catalog = Catalog::starter();
        let tool = catalog.tools[0].clone();
        let (cases, notes) = enforce(&tool, &catalog, Vec::new(), 8);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].kind, CaseKind::ShouldNotFire);
        assert!(!notes.is_empty());
        assert!(check(&tool, &catalog, &cases, 8).is_empty());
        let parsed = render::parse(&cases[0]).unwrap();
        assert_eq!(parsed.graders.len(), 2, "outcome and no-repo-map");
    }
}
