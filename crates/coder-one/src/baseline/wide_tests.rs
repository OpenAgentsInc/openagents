//! Wide discovery (issue #9654).

use super::*;
use crate::checks::contract::entry;

fn workspace(files: &[(&str, &str)]) -> (PathBuf, Cleanup) {
    let dir = scratch("baseline-wide-test");
    for (path, text) in files {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, text).unwrap();
    }
    let cleanup = Cleanup(dir.clone());
    (dir, cleanup)
}

/// A workspace with one of each wide kind and nothing the named
/// discovery finds.
fn wide_workspace() -> (PathBuf, Cleanup) {
    workspace(&[
        ("src/tool/__init__.py", ""),
        (
            "src/tool/__main__.py",
            "\"\"\"usage: python -m tool <reference.npy> <current.npy>\"\"\"\n",
        ),
        ("app/__init__.py", ""),
        ("app/cli/__init__.py", ""),
        ("app/cli/__main__.py", "print('cli')\n"),
        ("data/reference_window.npy", "x"),
        ("data/current_a.npy", "x"),
        ("data/current_b.npy", "x"),
        ("data/scenarios/current_c.npy", "x"),
        ("notes.md", "not an input"),
        (
            "pyproject.toml",
            "[project]\nname = \"tool\"\n\n[project.scripts]\ntool-run = \"tool.cli:main\"\n",
        ),
        ("src/tool/cli.py", "def main():\n    return 0\n"),
        (
            "Makefile",
            "run:\n\tpython3 -m tool\ntest:\n\tpip install -r requirements.txt\n\tpytest\n",
        ),
        (
            "package.json",
            "{\"scripts\": {\"start\": \"node server.js\", \"test\": \"npm install && jest\"}}",
        ),
        ("bun.lock", ""),
    ])
}

#[tokio::test]
async fn wide_discovery_finds_each_kind_named_discovery_does_not() {
    let (dir, _cleanup) = wide_workspace();
    let instruction = "The monitor is broken. Fix it.";
    let named: Vec<String> = entry::find(instruction, &dir, "/app")
        .await
        .into_iter()
        .map(|e| e.command)
        .collect();
    assert_eq!(named, vec!["make test"]);
    let found = entry::find_with(instruction, &dir, "/app", Discovery::Wide).await;
    let console = format!(
        "PYTHONPATH=src python3 -c {}",
        crate::accept::runner::sh_quote(
            "import sys, tool.cli as m; sys.argv[0] = 'tool-run'; sys.exit(m.main())"
        )
    );
    let shown: Vec<(&str, &str, bool)> = found
        .iter()
        .map(|e| (e.kind.word(), e.command.as_str(), e.refused.is_some()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("module", "python3 -m app.cli", false),
            (
                "module",
                "PYTHONPATH=src python3 -m tool data/reference_window.npy data/current_a.npy",
                false
            ),
            (
                "module",
                "PYTHONPATH=src python3 -m tool data/reference_window.npy data/current_b.npy",
                false
            ),
            (
                "module",
                "PYTHONPATH=src python3 -m tool data/reference_window.npy \
                 data/scenarios/current_c.npy",
                false
            ),
            ("make", "make test", true),
            ("make", "make run", false),
            ("console", console.as_str(), false),
            ("npm", "bun run test", true),
        ],
        "{found:#?}"
    );
    // The sweep says which run it is.
    assert!(found[1].why.contains("run 1 of 3"), "{}", found[1].why);
    // A recipe that installs packages is refused, as a command that does
    // would be.
    assert!(
        found[4]
            .refused
            .as_deref()
            .is_some_and(|r| r.contains("needs the network")),
        "{:?}",
        found[4]
    );
}

#[tokio::test]
async fn cargo_binaries_and_npm_scripts_are_found() {
    let (dir, _cleanup) = workspace(&[
        (
            "package.json",
            "{\"scripts\": {\"test\": \"echo \\\"Error: no test specified\\\" && exit 1\", \
             \"build\": \"tsc -p .\"}}",
        ),
        (
            "Cargo.toml",
            "[package]\nname = \"sim\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"replay\"\npath \
             = \"src/replay.rs\"\n",
        ),
        ("src/main.rs", "fn main() {}\n"),
        ("src/replay.rs", "fn main() {}\n"),
    ]);
    let found = entry::find_with("Fix it.", &dir, "/app", Discovery::Wide).await;
    let shown: Vec<(&str, &str)> = found
        .iter()
        .map(|e| (e.kind.word(), e.command.as_str()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("npm", "npm run build"),
            ("cargo", "cargo run --offline -q --bin replay"),
            ("cargo", "cargo run --offline -q --bin sim"),
        ],
        "{found:#?}"
    );
}

#[tokio::test]
async fn named_files_still_come_before_shipped_ones() {
    let (dir, _cleanup) = workspace(&[
        (
            "pkg/__main__.py",
            "\"\"\"usage: python -m pkg <reference.csv> <current.csv>\"\"\"\n",
        ),
        ("pkg/__init__.py", ""),
        ("data/reference.csv", "1\n"),
        ("data/current.csv", "2\n"),
        ("data/current_other.csv", "3\n"),
        ("extra/reference.csv", "1\n"),
        ("extra/current.csv", "2\n"),
    ]);
    let instruction = "The package reads `/app/extra/`.";
    let found = entry::find_with(instruction, &dir, "/app", Discovery::Wide).await;
    assert_eq!(
        found.iter().map(|e| e.command.as_str()).collect::<Vec<_>>(),
        vec!["python3 -m pkg extra/reference.csv extra/current.csv"],
        "{found:#?}"
    );
}

#[test]
fn shipped_inputs_skip_code_configuration_and_documentation() {
    let (dir, _cleanup) = workspace(&[
        ("data/a.csv", "1\n"),
        ("data/readme.md", "x"),
        ("data/loader.py", "x"),
        ("inputs/cases/b.json", "{}"),
        ("examples/.hidden", "x"),
        ("top.jsonl", "{}\n"),
        ("package.json", "{}"),
        ("requirements.txt", "numpy\n"),
        ("other/c.csv", "1\n"),
    ]);
    assert_eq!(
        entry::wide::shipped_inputs(&dir, "/app"),
        vec![
            "/app/data/a.csv".to_string(),
            "/app/inputs/cases/b.json".to_string(),
            "/app/top.jsonl".to_string(),
        ]
    );
}

fn enforced() -> bool {
    coder_boundary::Boundary::writing(std::env::temp_dir())
        .offline()
        .build()
        .is_ok()
}

/// Needs `make`: skipped, and says so, on a host without it.
#[tokio::test]
async fn a_wide_make_target_runs_in_the_boundary() {
    if !enforced() {
        eprintln!("skipped: this host can't enforce the boundary");
        return;
    }
    if std::process::Command::new("make")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipped: this host has no make");
        return;
    }
    let (dir, _cleanup) = workspace(&[("Makefile", "run:\n\t@echo ran > out.txt; cat out.txt\n")]);
    let setup = Setup {
        root: dir.clone(),
        alias: "/app".to_string(),
        wall: Duration::from_secs(20),
        container: false,
        discovery: Discovery::Wide,
    };
    let baseline = run("Fix it.", &setup).await;
    assert_eq!(baseline.runs.len(), 1, "{baseline:#?}");
    assert_eq!(baseline.runs[0].kind, EntryKind::Make);
    assert_eq!(baseline.runs[0].stdout_head, "ran\n");
    assert_eq!(baseline.untouched, Some(true));
    assert!(!dir.join("out.txt").exists());
}

#[test]
fn the_wide_implementation_differs_and_the_named_one_is_unchanged() {
    assert_eq!(implementation(), implementation_for(Discovery::Named));
    assert_ne!(
        implementation_for(Discovery::Wide).digest,
        implementation().digest
    );
}
