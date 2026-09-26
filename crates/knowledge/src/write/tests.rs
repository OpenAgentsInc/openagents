//! Rendering, the template, in-place edits, and versions.

use std::path::PathBuf;

use crate::lint::{Corpus, lint};
use crate::{Base, Entry, Kind, Status};

use super::*;

fn seed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../knowledge")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("knowledge-write-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn same(a: &Entry, b: &Entry) {
    assert_eq!(
        (&a.id, a.version, a.kind, &a.title, &a.summary, &a.tags),
        (&b.id, b.version, b.kind, &b.title, &b.summary, &b.tags)
    );
    assert_eq!(
        (
            &a.applies_when,
            a.status,
            &a.author,
            &a.written_from,
            &a.cites,
            &a.evidence,
            &a.body
        ),
        (
            &b.applies_when,
            b.status,
            &b.author,
            &b.written_from,
            &b.cites,
            &b.evidence,
            &b.body
        )
    );
}

#[test]
fn every_seed_entry_survives_a_render() {
    let (entries, problems) = Base::read(&seed());
    assert!(problems.is_empty());
    for entry in &entries {
        let again = Entry::parse(&entry.render()).unwrap_or_else(|e| panic!("{}: {e}", entry.id));
        same(entry, &again);
    }
}

#[test]
fn awkward_values_survive_a_render() {
    let (entries, _) = Base::read(&seed());
    let mut entry = entries[0].clone();
    entry.title = "Ratios: when a # sign isn't a comment".to_string();
    entry.author = "microcoder kb harvest (openai/gpt-6-luna)".to_string();
    entry.cites = vec![
        "Knuth, TAOCP vol. 2, \"Seminumerical Algorithms\", 3.2".to_string(),
        "- a dash first".to_string(),
    ];
    entry.tags = vec!["two words".to_string(), "x".to_string()];
    entry.evidence = vec!["admitted 2026-09-25 by review: A. Person".to_string()];
    entry.summary = "First paragraph.\nSecond paragraph after a break.".to_string();
    let again = Entry::parse(&entry.render()).unwrap();
    same(&entry, &again);
}

#[test]
fn the_template_parses_and_the_lint_holds_it_back() {
    let text = template(
        "numerics.kahan-summation",
        Kind::Method,
        "Kahan summation",
        "me",
    )
    .unwrap();
    let entry = Entry::parse(&text).unwrap();
    assert_eq!(entry.status, Status::Candidate);
    assert_eq!(entry.kind, Kind::Method);
    let problems = lint(&[entry], &Corpus::default());
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        text.iter().any(|p| p.contains("cites no source")),
        "{text:?}"
    );
    assert!(text.iter().any(|p| p.contains("template text")), "{text:?}");
    assert!(template("Bad ID", Kind::Slip, "T", "me").is_err());
    assert!(template("a.b", Kind::Slip, " ", "me").is_err());
}

#[test]
fn status_and_evidence_edits_keep_the_rest_of_the_file() {
    let path = seed().join("statistics.mmd-estimators.md");
    let text = std::fs::read_to_string(path).unwrap();
    let before = Entry::parse(&text).unwrap();
    let lines = vec!["admitted 2026-09-25 by review: A. Person".to_string()];
    let edited = set_evidence(&set_status(&text, Status::Candidate).unwrap(), &lines).unwrap();
    let after = Entry::parse(&edited).unwrap();
    assert_eq!(after.status, Status::Candidate);
    assert_eq!(after.evidence, lines);
    assert_eq!(after.body, before.body);
    assert_eq!(after.cites, before.cites);
    assert_eq!(after.summary, before.summary);
    // Setting it back to an empty list works too.
    let cleared = Entry::parse(&set_evidence(&edited, &[]).unwrap()).unwrap();
    assert!(cleared.evidence.is_empty());
}

#[test]
fn dates_are_utc() {
    assert_eq!(date(0), "1970-01-01");
    assert_eq!(date(951_782_400), "2000-02-29");
    assert_eq!(date(1_790_393_791), "2026-09-26");
}

#[test]
fn archived_and_pending_versions_live_in_versions() {
    let dir = scratch("versions");
    let text = std::fs::read_to_string(seed().join("shell.heredoc-quoting.md")).unwrap();
    std::fs::write(dir.join("shell.heredoc-quoting.md"), &text).unwrap();
    assert!(pending(&dir, "shell.heredoc-quoting", 1).is_none());
    let mut next = Entry::parse(&text).unwrap();
    next.version = 2;
    next.status = Status::Candidate;
    let at = version_path(&dir, "shell.heredoc-quoting", 2);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, next.render()).unwrap();
    let (path, found) = pending(&dir, "shell.heredoc-quoting", 1).unwrap();
    assert_eq!((path, found.version), (at, 2));
    assert!(pending(&dir, "shell.heredoc-quoting", 2).is_none());
    let moved = archive(&dir, "shell.heredoc-quoting").unwrap();
    assert!(moved.ends_with("versions/shell.heredoc-quoting.v1.md"));
    assert!(!dir.join("shell.heredoc-quoting.md").exists());
}
