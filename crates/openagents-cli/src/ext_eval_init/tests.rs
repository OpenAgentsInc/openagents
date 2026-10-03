//! Scripted terminal interviews over a fixture extension, with a fake
//! model and a fake runner.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;

use coder::eval_author::fake::StepModel;
use ext_eval::author::runner::fake::FakeRunner;
use ext_eval::{LoadOptions, Suite};

use super::*;

/// A resolvable extension: a package record pinning a one-step program
/// over `repo_map`, and a README.
fn extension(dir: &Path) {
    std::fs::create_dir_all(dir.join("programs")).unwrap();
    let program = serde_json::to_string_pretty(&json!({
        "slug": "project-map",
        "definition": {
            "id": format!("{LOCAL_KEY}:project-map/project-map"),
            "summary": "Maps the workspace before Coder starts.",
            "steps": [{"name": "repo_map", "kind": "module"}],
        },
    }))
    .unwrap();
    std::fs::write(dir.join("programs/project-map.json"), &program).unwrap();
    let package = serde_json::to_string_pretty(&json!({
        "v": 1,
        "slug": "project-map",
        "name": "Project map",
        "summary": "Shows Coder how the project is laid out before it starts.",
        "program": {"name": "project-map", "digest": coder::package::digest(&program)},
    }))
    .unwrap();
    std::fs::write(dir.join("package.json"), package).unwrap();
    std::fs::write(
        dir.join("README.md"),
        "# Project map\n\nIt reads sizes, not contents.\n",
    )
    .unwrap();
}

/// Every file under `dir` and its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    found
}

fn read_only(dir: &Path, on: bool) {
    let mut stack = vec![dir.to_path_buf()];
    let mut dirs = Vec::new();
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let mode = if on { 0o444 } else { 0o644 };
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            }
        }
        dirs.push(at);
    }
    for at in dirs {
        let mode = if on { 0o555 } else { 0o755 };
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}

fn author() -> Author<StepModel> {
    Author::new(StepModel::default(), "fake-model", None, Catalog::starter())
}

const WALK: &str = "y
A good run names the right file; a failed one guesses.
Can you add a test about a workspace?
y
y
y
y
y
";

/// The whole interview in a terminal against a read-only extension: every
/// gate asked in order, a try from a temporary copy, and the test set
/// written to `--out`, never into the extension.
#[tokio::test]
async fn the_terminal_walks_every_gate_and_never_writes_into_the_extension() {
    let ext = tempfile::tempdir().unwrap();
    extension(ext.path());
    let before = snapshot(ext.path());
    read_only(ext.path(), true);
    let out_dir = tempfile::tempdir().unwrap();
    let target = out_dir.path().join("evals");
    let runner = FakeRunner::helpful(vec!["repo_map".into()]);
    let mut screen = Vec::new();
    let input = format!("{WALK}\n");
    let written = interview(
        &author(),
        ext.path(),
        &target,
        &runner,
        Cursor::new(input),
        &mut screen,
    )
    .await;
    read_only(ext.path(), false);
    let screen = String::from_utf8(screen).unwrap();
    let written = written.unwrap_or_else(|e| panic!("{e:?}\n{screen}"));
    assert_eq!(snapshot(ext.path()), before, "the extension is unchanged");
    assert!(
        screen.contains(&format!(
            "We read Project map at {}. We only read it",
            ext.path().canonicalize().unwrap().display()
        )),
        "{screen}"
    );

    // Each gate's line, in order, with the tests gate asked twice.
    let order = [
        Stage::Tool.line(Surface::Terminal).unwrap(),
        Stage::Quality.line(Surface::Terminal).unwrap(),
        Stage::Tests.line(Surface::Terminal).unwrap(),
        Stage::Tests.line(Surface::Terminal).unwrap(),
        Stage::Checks.line(Surface::Terminal).unwrap(),
        "Try it once now?",
        Stage::Pilot.line(Surface::Terminal).unwrap(),
        Stage::Size.line(Surface::Terminal).unwrap(),
        Stage::Done.line(Surface::Terminal).unwrap(),
    ];
    let mut at = 0;
    for line in order {
        let found = screen[at..]
            .find(line)
            .unwrap_or_else(|| panic!("`{line}` after byte {at}:\n{screen}"));
        at += found + line.len();
    }
    assert!(
        screen.contains("With the plugin, Coder passed 5 of 5 tests; without it, 1 of 5."),
        "{screen}"
    );
    assert!(screen.contains("we can't price that from here"), "{screen}");
    assert_eq!(runner.requests().len(), 1);
    assert_eq!(runner.requests()[0].1, 1, "the try is one run per arm");
    assert!(
        !runner.requests()[0]
            .0
            .starts_with(&ext.path().display().to_string()),
        "the try runs from a temporary copy"
    );

    // The written test set is a suite the runner loads, and reads back as
    // the draft it was written from.
    assert!(!written.is_empty());
    let suite = Suite::load(&target, LoadOptions::default()).unwrap();
    assert_eq!(suite.cases.len(), 5);
    assert!(suite.cases.iter().all(|c| c.runs == 3));
    let cases = files::read(&target).unwrap();
    for (path, bytes) in files::case_files(&cases) {
        assert_eq!(std::fs::read(target.join(path)).unwrap(), bytes);
    }
}

#[tokio::test]
async fn a_read_only_extension_without_out_writes_nothing_and_says_why() {
    let ext = tempfile::tempdir().unwrap();
    extension(ext.path());
    let before = snapshot(ext.path());
    read_only(ext.path(), true);
    let runner = FakeRunner::helpful(vec!["repo_map".into()]);
    let result = interview(
        &author(),
        ext.path(),
        &ext.path().join("evals"),
        &runner,
        Cursor::new(format!("{WALK}\n")),
        Vec::new(),
    )
    .await;
    read_only(ext.path(), false);
    let Err(Stop::Failed(message)) = result else {
        panic!("the write is refused: {result:?}")
    };
    assert!(
        message.contains("nothing was written into the plugin"),
        "{message}"
    );
    assert_eq!(snapshot(ext.path()), before);
}

#[tokio::test]
async fn a_directory_that_isnt_an_extension_stops_at_step_zero() {
    let dir = tempfile::tempdir().unwrap();
    let result = interview(
        &author(),
        dir.path(),
        &dir.path().join("evals"),
        &FakeRunner::helpful(Vec::new()),
        Cursor::new("y\n"),
        Vec::new(),
    )
    .await;
    assert!(matches!(result, Err(Stop::Usage(message)) if message.contains("is not a plugin")));
    // A record whose pinned program bytes changed doesn't resolve.
    extension(dir.path());
    std::fs::write(
        dir.path().join("programs/project-map.json"),
        "{\"slug\": \"project-map\"}",
    )
    .unwrap();
    let result = interview(
        &author(),
        dir.path(),
        &dir.path().join("evals"),
        &FakeRunner::helpful(Vec::new()),
        Cursor::new("y\n"),
        Vec::new(),
    )
    .await;
    assert!(matches!(result, Err(Stop::Usage(message)) if message.contains("does not resolve")));
    assert!(!dir.path().join("evals").exists());
}

#[tokio::test]
async fn answers_that_end_early_write_nothing() {
    let ext = tempfile::tempdir().unwrap();
    extension(ext.path());
    let target = ext.path().join("evals");
    let result = interview(
        &author(),
        ext.path(),
        &target,
        &FakeRunner::helpful(vec!["repo_map".into()]),
        Cursor::new("y\nGood runs name the file.\ny\n"),
        Vec::new(),
    )
    .await;
    assert!(
        matches!(result, Err(Stop::Failed(message)) if message.contains("nothing was written"))
    );
    assert!(!target.exists());
}

#[test]
fn reading_the_extension_finds_its_operations_and_words() {
    let dir = tempfile::tempdir().unwrap();
    extension(dir.path());
    let tool = read_extension(dir.path()).unwrap();
    assert_eq!(tool.name, "Project map");
    assert_eq!(tool.operations, ["repo_map"]);
    assert!(tool.words.contains("It reads sizes, not contents."));
    assert!(tool.words.contains("Its steps: repo_map."));
    let Source::Existing(definition) = &tool.source else {
        panic!("a local extension is an existing tool")
    };
    assert_eq!(
        definition.id,
        format!("{LOCAL_KEY}:project-map/project-map")
    );
}
