//! From typed proposals to the exact case files a runner reads.
//!
//! A draft test holds the whole `prompt.md` and each whole
//! `graders/<name>.md`; this module writes them from typed records, with
//! TOML the `toml` crate escapes, and reads them back through the engine's
//! own parser ([`Case::parse`]), so a draft holds only files a run accepts.

use nostr::cj_conversation::DraftCase;
use nostr::eval_ext::CaseKind;
use toml::{Table, Value as Toml};

use crate::case::{CASE_SCHEMA, Case, CaseError, CaseFiles, Kind};
use crate::grader::{Check, Focus};

use super::proposal::{FocusProposal, GraderProposal};

/// The starting check every test gets until its checks are proposed, and
/// the check the floor adds to a test with no check of its outcome.
pub const OUTCOME: &str = "outcome";
/// The starting question.
pub const OUTCOME_QUESTION: &str = "Did the run do what the task asked?";
/// The threshold the starting check uses.
pub const OUTCOME_THRESHOLD: f64 = 0.7;

/// The case kind in the wire's words.
#[must_use]
pub fn wire_kind(kind: Kind) -> CaseKind {
    match kind {
        Kind::ShouldFire => CaseKind::ShouldFire,
        Kind::ShouldNotFire => CaseKind::ShouldNotFire,
    }
}

/// The engine's kind for the wire's.
#[must_use]
pub fn engine_kind(kind: CaseKind) -> Kind {
    match kind {
        CaseKind::ShouldFire => Kind::ShouldFire,
        CaseKind::ShouldNotFire => Kind::ShouldNotFire,
    }
}

/// The kind a proposal names; anything but `should-not-fire` is
/// `should-fire`, the case format's default.
#[must_use]
pub fn proposed_kind(word: &str) -> Kind {
    match word.trim() {
        "should-not-fire" | "should_not_fire" => Kind::ShouldNotFire,
        _ => Kind::ShouldFire,
    }
}

/// A TOML string, quoted and escaped.
fn quoted(text: &str) -> String {
    Toml::String(text.to_string()).to_string()
}

/// `prompt.md`: the schema, the kind, the `write` grant, and the task as
/// the body. Every run starts in an empty workspace, so a task makes the
/// files it needs, and a check may read them: the grant is read and write
/// in the sandbox, what the hosted runner allows. `runs` is never written,
/// so every test runs the default three times per arm.
#[must_use]
pub fn prompt_md(kind: Kind, task: &str) -> String {
    let mut text = format!(
        "+++\nv = {}\nkind = {}\n",
        quoted(CASE_SCHEMA),
        quoted(kind.word())
    );
    text.push_str("\n[run]\nallowed_operations = [\"read\", \"write\"]\n");
    text.push_str("+++\n\n");
    text.push_str(task.trim());
    text.push('\n');
    text
}

/// A grader file: TOML frontmatter and an optional body.
fn grader_file(table: &Table, body: Option<&str>) -> String {
    let front = toml::to_string(table).unwrap_or_default();
    let mut text = format!("+++\n{front}+++\n");
    if let Some(body) = body.map(str::trim).filter(|b| !b.is_empty()) {
        text.push('\n');
        text.push_str(body);
        text.push('\n');
    }
    text
}

/// The starting outcome check: a `decision` over Coder's last message.
#[must_use]
pub fn outcome_grader(rubric: Option<&str>) -> (String, String) {
    let mut table = Table::new();
    table.insert("type".into(), Toml::String("decision".into()));
    table.insert("question".into(), Toml::String(OUTCOME_QUESTION.into()));
    table.insert("threshold".into(), Toml::Float(OUTCOME_THRESHOLD));
    (OUTCOME.to_string(), grader_file(&table, rubric))
}

/// A `max = 0` check that `operation` never ran, for a test where the tool
/// should stay out of the way.
#[must_use]
pub fn unused_grader(operation: &str) -> (String, String) {
    let mut table = Table::new();
    table.insert("type".into(), Toml::String("operation_used".into()));
    table.insert("operation".into(), Toml::String(operation.into()));
    table.insert("min".into(), Toml::Integer(0));
    table.insert("max".into(), Toml::Integer(0));
    (
        grader_name(&format!("no-{operation}")),
        grader_file(&table, None),
    )
}

fn focus_value(focus: &FocusProposal) -> Option<Toml> {
    match focus {
        FocusProposal::Word(word) => match word.as_str() {
            "last_message" | "trajectory" | "files" => Some(Toml::String(word.clone())),
            _ => None,
        },
        FocusProposal::File { file } => {
            crate::grader::workspace_path(file).ok()?;
            let mut table = Table::new();
            table.insert("file".into(), Toml::String(file.clone()));
            Some(Toml::Table(table))
        }
    }
}

/// A name every grader file may carry: lowercase letters, digits, and
/// dashes, 1 to 40 characters.
#[must_use]
pub fn grader_name(proposed: &str) -> String {
    let mut name = String::new();
    for c in proposed.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') && !name.is_empty() {
            name.push('-');
        }
        if name.len() >= 40 {
            break;
        }
    }
    let name = name.trim_matches('-').to_string();
    if name.is_empty() {
        "check".into()
    } else {
        name
    }
}

/// A test id: the same shape as a grader name, up to 64 characters.
#[must_use]
pub fn case_id(proposed: &str) -> String {
    let mut id = String::new();
    for c in proposed.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') && !id.is_empty() {
            id.push('-');
        }
        if id.len() >= 64 {
            break;
        }
    }
    let id = id.trim_matches('-').to_string();
    if id.is_empty() { "test".into() } else { id }
}

/// A proposed check as a grader file, or `None` when it can't be one: an
/// unknown focus, an empty pattern, or an operation outside `operations`
/// and the shell.
#[must_use]
pub fn grader(proposal: &GraderProposal, operations: &[String]) -> Option<(String, String)> {
    let mut table = Table::new();
    let mut body = None;
    let name = grader_name(proposal.name());
    match proposal {
        GraderProposal::Decision {
            question,
            focus,
            threshold,
            rubric,
            ..
        } => {
            if question.trim().is_empty() {
                return None;
            }
            table.insert("type".into(), Toml::String("decision".into()));
            table.insert("question".into(), Toml::String(question.trim().into()));
            let threshold = threshold
                .filter(|t| t.is_finite() && (0.5..=0.95).contains(t))
                .unwrap_or(OUTCOME_THRESHOLD);
            table.insert("threshold".into(), Toml::Float(threshold));
            if let Some(focus) = focus {
                table.insert("focus".into(), focus_value(focus)?);
            }
            body = rubric.as_deref();
        }
        GraderProposal::Regex {
            pattern,
            matching,
            target,
            flags,
            ..
        } => {
            if pattern.is_empty() {
                return None;
            }
            table.insert("type".into(), Toml::String("regex".into()));
            table.insert("pattern".into(), Toml::String(pattern.clone()));
            match matching.as_deref() {
                None | Some("contains") => {}
                Some("not_contains") => {
                    table.insert("match".into(), Toml::String("not_contains".into()));
                }
                Some(_) => return None,
            }
            if let Some(target) = target {
                table.insert("target".into(), focus_value(target)?);
            }
            if let Some(flags) = flags.as_deref().filter(|f| !f.is_empty()) {
                table.insert("flags".into(), Toml::String(flags.into()));
            }
        }
        GraderProposal::FileExists { path, exists, .. } => {
            crate::grader::workspace_path(path.trim_start_matches("./")).ok()?;
            table.insert("type".into(), Toml::String("file_exists".into()));
            table.insert(
                "path".into(),
                Toml::String(path.trim_start_matches("./").into()),
            );
            if *exists == Some(false) {
                table.insert("exists".into(), Toml::Boolean(false));
            }
        }
        GraderProposal::OperationUsed {
            operation,
            min,
            max,
            ..
        } => {
            if !(operations.iter().any(|o| o == operation) || operation == "shell") {
                return None;
            }
            table.insert("type".into(), Toml::String("operation_used".into()));
            table.insert("operation".into(), Toml::String(operation.clone()));
            let min = min.unwrap_or(1);
            table.insert("min".into(), Toml::Integer(i64::from(min)));
            if let Some(max) = max {
                if *max < min {
                    return None;
                }
                table.insert("max".into(), Toml::Integer(i64::from(*max)));
            }
        }
    }
    Some((name, grader_file(&table, body)))
}

/// The engine's parse of a draft test: exactly the case a runner would
/// load from the files [`super::files::write`] writes.
///
/// # Errors
///
/// The engine's refusal.
pub fn parse(case: &DraftCase) -> Result<Case, CaseError> {
    Case::parse(
        &case.id,
        &format!("evals/{}", case.id),
        CaseFiles {
            prompt: case.prompt.clone().into_bytes(),
            case_toml: None,
            graders: case
                .graders
                .iter()
                .map(|(name, text)| (format!("{name}.md"), text.clone().into_bytes()))
                .collect(),
            fixtures: Vec::new(),
        },
    )
}

/// Whether a parsed check reads what the run made, which needs `write`.
#[must_use]
pub fn reads_files(check: &Check) -> bool {
    match check {
        Check::FileExists { exists, .. } => *exists,
        Check::Regex { target, .. } => matches!(target, Focus::File(_) | Focus::Files),
        Check::Decision { focus, .. } | Check::Judge { focus, .. } => {
            matches!(focus, Focus::File(_) | Focus::Files)
        }
        _ => false,
    }
}

/// Whether a parsed check grades the outcome: Coder's last message, the
/// files it made, or a file's contents, rather than the steps it took.
#[must_use]
pub fn is_outcome(check: &Check) -> bool {
    match check {
        Check::Regex { target, .. } => !matches!(target, Focus::Trajectory),
        Check::Decision { focus, .. } | Check::Judge { focus, .. } => {
            !matches!(focus, Focus::Trajectory)
        }
        Check::FileExists { .. } => true,
        Check::OperationUsed { .. } | Check::OperationOrder { .. } | Check::Receipt { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(prompt: String, graders: Vec<(String, String)>) -> DraftCase {
        DraftCase {
            id: "t".into(),
            kind: CaseKind::ShouldFire,
            prompt,
            graders,
        }
    }

    #[test]
    fn rendered_files_parse_as_cases_with_quotes_and_newlines_escaped() {
        let task = "Write the entry for \"--dry-run\".\nKeep it to \\ one line.";
        let prompt = prompt_md(Kind::ShouldNotFire, task);
        let decision = grader(
            &GraderProposal::Decision {
                name: "Entry is short!".into(),
                question: "Is the entry one line with a \"quoted\" flag?".into(),
                focus: Some(FocusProposal::File {
                    file: "CHANGELOG.md".into(),
                }),
                threshold: Some(0.8),
                rubric: Some("One line.\n+++\nStill the body.".into()),
            },
            &[],
        )
        .unwrap();
        assert_eq!(decision.0, "entry-is-short");
        let parsed = parse(&case(prompt, vec![decision, outcome_grader(None)])).unwrap();
        assert_eq!(parsed.kind, Kind::ShouldNotFire);
        assert_eq!(parsed.prompt, task);
        assert_eq!(parsed.runs, 3);
        assert!(
            parsed
                .run
                .allowed_operations
                .contains(&crate::case::Grant::Write)
        );
        assert!(parsed.graders.iter().any(|g| reads_files(&g.check)));
        assert!(parsed.graders.iter().all(|g| is_outcome(&g.check)));
    }

    #[test]
    fn checks_the_machine_cant_hold_are_refused() {
        let ops = vec!["repo_map".to_string()];
        let used = |operation: &str, min, max| GraderProposal::OperationUsed {
            name: String::new(),
            operation: operation.into(),
            min,
            max,
        };
        assert!(grader(&used("repo_map", Some(0), Some(0)), &ops).is_some());
        assert!(grader(&used("shell", None, None), &ops).is_some());
        assert!(grader(&used("rm_rf", None, None), &ops).is_none());
        assert!(grader(&used("repo_map", Some(2), Some(1)), &ops).is_none());
        let outside = GraderProposal::FileExists {
            name: "x".into(),
            path: "../etc/passwd".into(),
            exists: None,
        };
        assert!(grader(&outside, &ops).is_none());
        let unknown_focus = GraderProposal::Decision {
            name: "x".into(),
            question: "q".into(),
            focus: Some(FocusProposal::Word("vibes".into())),
            threshold: None,
            rubric: None,
        };
        assert!(grader(&unknown_focus, &ops).is_none());
        let (name, text) = unused_grader("repo_map");
        assert_eq!(name, "no-repo-map");
        let parsed = parse(&case(
            prompt_md(Kind::ShouldFire, "Say hi."),
            vec![(name, text)],
        ))
        .unwrap();
        assert!(!is_outcome(&parsed.graders[0].check));
    }

    #[test]
    fn names_are_plain() {
        assert_eq!(grader_name("  Uses the MAP  "), "uses-the-map");
        assert_eq!(grader_name("!!!"), "check");
        assert_eq!(case_id("Summarize a merged fix"), "summarize-a-merged-fix");
        assert_eq!(case_id(""), "test");
    }
}
