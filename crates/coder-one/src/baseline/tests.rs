use super::*;
use crate::checks::contract::entry::{self, Usage};

fn usage(required: &[&str], variadic: bool) -> Usage {
    Usage {
        required: required.iter().map(|s| (*s).to_string()).collect(),
        variadic,
        source: "usage line".to_string(),
    }
}

fn workspace(files: &[(&str, &str)]) -> (PathBuf, Cleanup) {
    let dir = scratch("baseline-test");
    for (path, text) in files {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, text).unwrap();
    }
    let cleanup = Cleanup(dir.clone());
    (dir, cleanup)
}

fn enforced() -> bool {
    coder_boundary::Boundary::writing(std::env::temp_dir())
        .offline()
        .build()
        .is_ok()
}

#[test]
fn a_usage_line_in_a_docstring_or_a_string_literal_reads() {
    let docstring =
        "\"\"\"CLI: python -m drift_monitor <reference.npy> <current.npy> [more...]\"\"\"";
    assert_eq!(
        entry::usage_of(docstring, "drift_monitor"),
        Some(usage(&["reference.npy", "current.npy"], true))
    );
    let printed =
        "print(\"usage: drift_monitor <reference.npy> <current.npy> [...]\", file=sys.stderr)";
    assert_eq!(
        entry::usage_of(printed, "drift_monitor"),
        Some(usage(&["reference.npy", "current.npy"], true))
    );
    let block = "Usage:\n  python apply.py rules.json ordering.txt input.tsv output.tsv\n  python \
                 apply.py rules.json ordering.txt --word \"proto_form\"\n";
    assert_eq!(
        entry::usage_of(block, "apply.py"),
        Some(usage(
            &["rules.json", "ordering.txt", "input.tsv", "output.tsv"],
            false
        ))
    );
    assert_eq!(entry::usage_of("print('hello')", "tool.py"), None);
}

#[test]
fn argparse_positionals_are_the_usage() {
    let text = "import argparse\np = argparse.ArgumentParser()\np.add_argument(\"config\")\n\
                p.add_argument('inputs', nargs='+')\np.add_argument(\"--out\", default=None)\n";
    let found = entry::usage_of(text, "render.py").unwrap();
    assert_eq!(found.required, vec!["config", "inputs"]);
    assert!(found.variadic);
    assert_eq!(found.source, "argparse");
    let flags_only = "import argparse\np.add_argument(\"--config\", required=True)\n";
    assert!(
        entry::usage_of(flags_only, "solve.py")
            .unwrap()
            .required
            .is_empty()
    );
}

#[test]
fn files_fit_a_usage_by_count_and_by_name() {
    let files: Vec<String> = [
        "/app/data/current_clear_drift.npy",
        "/app/data/current_stable.npy",
        "/app/data/reference_embeddings.npy",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    // The reference placeholder takes the reference file, whatever the
    // order; the rest follow.
    assert_eq!(
        entry::fit(&usage(&["reference.npy", "current.npy"], true), &files),
        Some(vec![
            "/app/data/reference_embeddings.npy".to_string(),
            "/app/data/current_clear_drift.npy".to_string(),
            "/app/data/current_stable.npy".to_string(),
        ])
    );
    // Without "more allowed", three files don't fit two placeholders.
    assert_eq!(
        entry::fit(&usage(&["reference.npy", "current.npy"], false), &files),
        None
    );
    // A stated extension filters the files.
    assert_eq!(
        entry::fit(&usage(&["rules.json"], false), &files),
        None,
        "no JSON file to fit"
    );
}

#[test]
fn network_and_placeholder_and_bare_compiler_commands_are_refused() {
    for command in [
        "pip install requests",
        "npm ci",
        "git clone https://github.com/x/y",
        "python3 fetch.py https://example.com/a.json",
        "curl -s example.com",
        "apt-get install -y jq",
    ] {
        assert!(entry::needs_network(command).is_some(), "{command}");
    }
    for command in [
        "python3 -m drift_monitor data/a.npy",
        "make test",
        "bun run release",
        "pytest -q",
    ] {
        assert!(entry::needs_network(command).is_none(), "{command}");
    }
}

#[tokio::test]
async fn find_reads_each_kind_and_skips_what_it_already_has() {
    let (dir, _cleanup) = workspace(&[
        (
            "pkg/__main__.py",
            "\"\"\"usage: python -m pkg <reference.csv> <current.csv>\"\"\"\n",
        ),
        ("pkg/__init__.py", ""),
        ("data/reference.csv", "1\n"),
        ("data/current.csv", "2\n"),
        ("Makefile", "check:\n\ttrue\n"),
        (
            "tools/run.py",
            "import sys\nif __name__ == \"__main__\":\n    print(sys.argv)\n",
        ),
        ("lib.py", "def f():\n    return 1\n"),
        ("go.sh", "echo go\n"),
    ]);
    let instruction = "The package at `/app/pkg/` reads `/app/data/`. Run `sh go.sh` first. \
                       `/app/tools/run.py` and `/app/lib.py` are there too. Use `python3 \
                       submission/solve.py --config CONFIG_JSON`.";
    let found = entry::find(instruction, &dir, "/app").await;
    let shown: Vec<(&str, &str, bool)> = found
        .iter()
        .map(|e| (e.kind.word(), e.command.as_str(), e.refused.is_some()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("named", "sh go.sh", false),
            (
                "named",
                "python3 submission/solve.py --config CONFIG_JSON",
                true
            ),
            (
                "module",
                "python3 -m pkg data/reference.csv data/current.csv",
                false
            ),
            ("make", "make check", false),
            ("script", "python3 tools/run.py", false),
        ],
        "{found:#?}"
    );
}

#[tokio::test]
async fn a_run_leaves_the_workspace_alone_and_shows_the_paths_the_task_uses() {
    if !enforced() {
        return;
    }
    let (dir, _cleanup) = workspace(&[
        ("data/in.txt", "a b c\n"),
        (
            "count.py",
            "import os, sys\nopen('written.txt', 'w').write('x')\nprint(os.getcwd())\n\
             print(len(open(sys.argv[1]).read().split()))\n",
        ),
    ]);
    let instruction = "Check it with `python3 count.py /app/data/in.txt`.";
    let setup = Setup {
        root: dir.clone(),
        alias: "/app".to_string(),
        wall: Duration::from_secs(20),
        container: false,
        discovery: Discovery::Named,
    };
    let baseline = run(instruction, &setup).await;
    assert_eq!(baseline.runs.len(), 1, "{baseline:#?}");
    let run = &baseline.runs[0];
    assert_eq!(run.exit, Some(0), "{run:#?}");
    assert_eq!(run.confine, Some(Confine::Boundary));
    assert_eq!(run.stdout_head, "/app\n3\n");
    assert_eq!(baseline.untouched, Some(true));
    assert!(!dir.join("written.txt").exists());
    assert_eq!(
        baseline.commands(),
        vec!["python3 count.py /app/data/in.txt"]
    );
    let evidence = baseline.evidence().unwrap();
    assert_eq!(evidence.label, LABEL);
    assert!(
        evidence
            .text
            .contains("$ python3 count.py /app/data/in.txt")
    );
    assert!(evidence.text.contains("(named: exit 0 after"));
}

#[tokio::test]
async fn the_network_is_off_inside_a_run() {
    if !enforced() {
        return;
    }
    let (dir, _cleanup) = workspace(&[(
        "probe.py",
        "import socket\ntry:\n    socket.create_connection(('1.1.1.1', 53), timeout=3)\n    \
         print('reached')\nexcept OSError as e:\n    print('refused', type(e).__name__)\n",
    )]);
    let setup = Setup {
        root: dir.clone(),
        alias: "/app".to_string(),
        wall: Duration::from_secs(20),
        container: false,
        discovery: Discovery::Named,
    };
    let baseline = run("Run `python3 probe.py` to see.", &setup).await;
    assert!(
        baseline.runs[0].stdout_head.starts_with("refused"),
        "{baseline:#?}"
    );
}

#[test]
fn records_have_the_run_card_shape_and_read_back_as_commands() {
    let finished = Run {
        kind: EntryKind::Module,
        command: "python3 -m pkg data/a.npy".to_string(),
        stated: None,
        why: String::new(),
        at: 1,
        exit: Some(1),
        timed_out: false,
        wall_sec: 60,
        ms: 10,
        stdout_head: "out".to_string(),
        stderr_head: "RuntimeWarning: invalid value".to_string(),
        stdout_bytes: 3,
        stderr_bytes: 29,
        failed: None,
        confine: Some(Confine::Boundary),
    };
    let over = Run {
        kind: EntryKind::Named,
        command: "sh slow.sh".to_string(),
        timed_out: true,
        exit: None,
        ..finished.clone()
    };
    let missing = Run {
        command: "coqc Main.v".to_string(),
        exit: Some(127),
        ..over.clone()
    };
    let baseline = Baseline {
        cwd: "/app".to_string(),
        runs: vec![finished, over, missing],
        ..Baseline::default()
    };
    assert_eq!(baseline.commands(), vec!["python3 -m pkg data/a.npy"]);
    let record = &baseline.records()[0];
    for field in [
        "schema",
        "at",
        "stage",
        "session",
        "candidate",
        "kind",
        "command",
        "cwd",
        "exit",
        "timed_out",
        "ms",
        "stdout_digest",
        "stderr_digest",
        "stdout_head",
        "stderr_head",
        "requirements",
        "verdict",
        "rule",
    ] {
        assert!(record.get(field).is_some(), "no {field}");
    }
    assert_eq!(record["schema"], EXECUTED_SCHEMA);
    assert_eq!(record["stage"], "baseline");
    assert_eq!(record["kind"], "module");
    let (dir, _cleanup) = workspace(&[]);
    let file = dir.join("lean-1").join(EXECUTED_FILE);
    baseline.append_records(&file).unwrap();
    assert_eq!(read_commands(&file), baseline.commands());
    let evidence = baseline.evidence().unwrap().text;
    assert!(evidence.contains("stopped at the 60 s bound"));
    assert!(evidence.contains("RuntimeWarning: invalid value"));
}

#[test]
fn a_long_stream_keeps_its_warnings_in_the_briefing() {
    let mut stderr = "x\n".repeat(BRIEF_CHARS);
    stderr.push_str("normalize.py:18: RuntimeWarning: invalid value encountered in divide\n");
    let run = Run {
        kind: EntryKind::Module,
        command: "python3 -m pkg".to_string(),
        stated: None,
        why: String::new(),
        at: 0,
        exit: Some(0),
        timed_out: false,
        wall_sec: 60,
        ms: 5,
        stdout_head: String::new(),
        stderr_head: stderr.clone(),
        stdout_bytes: 0,
        stderr_bytes: stderr.len() as u64,
        failed: None,
        confine: None,
    };
    let text = brief_run(&run);
    assert!(text.contains("Warnings and errors later in stderr:"));
    assert!(text.contains("normalize.py:18: RuntimeWarning"));
    assert!(text.contains("stdout: empty"));
}

#[test]
fn copy_paths_map_back_to_the_task_s_directory() {
    let copy = PathBuf::from("/var/folders/x/baseline-copy-1");
    assert_eq!(
        unmap(
            "/var/folders/x/baseline-copy-1/pkg/a.py:3: warning",
            &copy,
            "/app"
        ),
        "/app/pkg/a.py:3: warning"
    );
}
