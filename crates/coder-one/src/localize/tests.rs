use std::collections::BTreeSet;

use serde_json::json;

use super::context::{self, Bounds, Failure};
use super::mismatch;
use super::offline::{self, Touch};
use super::parse::{self, Location};
use super::timing::{self, Profiler};
use super::trace::{self, Call};
use super::*;

fn refs(output: &str) -> Vec<(String, u64, String)> {
    parse::parse(output)
        .into_iter()
        .map(|l| (l.path, l.line, l.rule))
        .collect()
}

fn one(path: &str, line: u64, rule: &str) -> (String, u64, String) {
    (path.to_string(), line, rule.to_string())
}

#[test]
fn a_python_traceback_reads_innermost_frame_first() {
    let output = "Traceback (most recent call last):\n  File \"/app/run.py\", line 9, in <module>\n    main()\n  File \"/app/pkg/core.py\", line 41, in main\n    total = score(rows)\n  File \"/usr/lib/python3.12/statistics.py\", line 300, in mean\n    raise StatisticsError\nstatistics.StatisticsError: mean requires at least one data point";
    assert_eq!(
        refs(output),
        vec![
            one("/usr/lib/python3.12/statistics.py", 300, "file-line"),
            one("/app/pkg/core.py", 41, "file-line"),
            one("/app/run.py", 9, "file-line"),
        ]
    );
}

#[test]
fn pytest_short_and_long_forms_parse() {
    let output = "tests/test_core.py:17: in test_mean\n    assert mean([1, 2]) == 2\nsrc/core.py:5: AssertionError\nFAILED tests/test_core.py::test_mean - assert 1.5 == 2";
    assert_eq!(
        refs(output),
        vec![
            one("tests/test_core.py", 17, "colon"),
            one("src/core.py", 5, "colon"),
        ]
    );
}

#[test]
fn rust_diagnostics_and_panics_parse() {
    let output = "error[E0308]: mismatched types\n  --> src/lib.rs:12:5\n   |\n12 |     x\n   |     ^ expected `u32`, found `i32`\nthread 'main' panicked at src/main.rs:30:9:\nindex out of bounds";
    assert_eq!(
        refs(output),
        vec![
            one("src/lib.rs", 12, "rust-arrow"),
            one("src/main.rs", 30, "rust-panic"),
        ]
    );
}

#[test]
fn gcc_clang_lean_and_javac_diagnostics_parse() {
    let output = "src/kernel.cu:88:14: error: identifier \"half2\" is undefined\ninclude/a.h:3:1: note: declared here\nProofs/Bound.lean:12:5: error: unsolved goals\nsrc/Main.java:7: error: cannot find symbol";
    assert_eq!(
        refs(output),
        vec![
            one("src/kernel.cu", 88, "colon"),
            one("include/a.h", 3, "colon"),
            one("Proofs/Bound.lean", 12, "colon"),
            one("src/Main.java", 7, "colon"),
        ]
    );
}

#[test]
fn go_compile_errors_and_goroutine_stacks_parse() {
    let output = "./main.go:14:2: undefined: frobnicate\npanic: runtime error: index out of range\n\ngoroutine 1 [running]:\nmain.work(...)\n\t/app/cmd/work.go:23 +0x1d\nmain.main()\n\t/app/cmd/main.go:9 +0x25";
    assert_eq!(
        refs(output),
        vec![
            one("./main.go", 14, "colon"),
            one("/app/cmd/work.go", 23, "go-frame"),
            one("/app/cmd/main.go", 9, "go-frame"),
        ]
    );
}

#[test]
fn jvm_stacks_and_scala_compiler_errors_parse() {
    let output = "Exception in thread \"main\" java.lang.NullPointerException\n\tat com.acme.dedup.Shingle.hash(Shingle.java:42)\n\tat com.acme.dedup.Main.main(Main.scala:10)\n[error] /app/src/main/scala/Job.scala:55:7: type mismatch;";
    let found = parse::parse(output);
    assert_eq!(found[0].path, "Shingle.java");
    assert_eq!(found[0].line, 42);
    assert_eq!(found[0].rule, "jvm-frame");
    assert_eq!(
        found[0].qualified.as_deref(),
        Some("com.acme.dedup.Shingle.hash")
    );
    assert_eq!(found[1].path, "Main.scala");
    assert_eq!(found[2].path, "/app/src/main/scala/Job.scala");
    assert_eq!(found[2].rule, "colon");
}

#[test]
fn node_stacks_and_typescript_diagnostics_parse() {
    let output = "TypeError: Cannot read properties of undefined\n    at parse (/app/src/filter.js:12:17)\n    at /app/src/index.mjs:4:3\n    at node:internal/main:5:1\nsrc/types.ts(8,3): error TS2322: Type 'string' is not assignable";
    assert_eq!(
        refs(output),
        vec![
            one("/app/src/filter.js", 12, "node-frame"),
            one("/app/src/index.mjs", 4, "node-frame"),
            one("src/types.ts", 8, "paren"),
        ]
    );
}

#[test]
fn coq_and_ocaml_errors_parse_like_a_traceback() {
    let output = "File \"./theories/Bound.v\", line 27, characters 2-14:\nError: In environment\nn : nat\nUnable to unify";
    assert_eq!(
        refs(output),
        vec![one("./theories/Bound.v", 27, "file-line")]
    );
}

#[test]
fn urls_times_versions_and_counts_are_not_locations() {
    let output = "GET http://localhost:8080/api.py:3 failed\nat 12:30:45 the job ran\nversion 1.2.3:4\nresult 3.5:1 ratio\nratio: 12:3";
    assert!(
        parse::parse(output).is_empty(),
        "{:?}",
        parse::parse(output)
    );
}

fn files(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

fn location(path: &str, line: u64) -> Location {
    Location {
        path: path.to_string(),
        line,
        column: None,
        qualified: None,
        rule: "test".to_string(),
    }
}

#[test]
fn resolution_takes_workspace_files_and_leaves_out_libraries() {
    let known = files(&[
        "pkg/core.py",
        "run.py",
        "src/a/Main.java",
        "src/b/Main.java",
    ]);
    assert_eq!(
        parse::resolve(&location("/app/pkg/core.py", 3), &known, "/app"),
        Some("pkg/core.py".to_string())
    );
    assert_eq!(
        parse::resolve(&location("/work/copy/pkg/core.py", 3), &known, "/app"),
        Some("pkg/core.py".to_string())
    );
    assert_eq!(
        parse::resolve(&location("./run.py", 1), &known, ""),
        Some("run.py".to_string())
    );
    assert_eq!(
        parse::resolve(
            &location("/usr/lib/python3.12/site-packages/pkg/core.py", 3),
            &known,
            "/app"
        ),
        None
    );
    // A bare name that matches two files resolves only with a package.
    assert_eq!(
        parse::resolve(&location("Main.java", 3), &known, "/app"),
        None
    );
    let mut frame = location("Main.java", 3);
    frame.qualified = Some("b.Main.run".to_string());
    assert_eq!(
        parse::resolve(&frame, &known, "/app"),
        Some("src/b/Main.java".to_string())
    );
}

fn source(lines: usize) -> String {
    (1..=lines).map(|n| format!("line {n}\n")).collect()
}

#[test]
fn regions_merge_in_one_file_and_render_the_named_lines() {
    let known = files(&["a.py", "b.py"]);
    let failures = vec![
        Failure {
            command: "python3 first.py".to_string(),
            output: "  File \"a.py\", line 40, in f".to_string(),
            exit: Some(1),
            ..Failure::default()
        },
        Failure {
            command: "python3 second.py".to_string(),
            output: "  File \"a.py\", line 10, in g\n  File \"a.py\", line 14, in h\n  File \"b.py\", line 2, in k".to_string(),
            exit: Some(1),
            ..Failure::default()
        },
    ];
    let located = context::located(&failures, &known, "/app");
    // The most recent failure first, innermost frame first.
    assert_eq!(
        located
            .iter()
            .map(|(f, l, _)| (f.as_str(), l.line))
            .collect::<Vec<_>>(),
        vec![("b.py", 2), ("a.py", 14), ("a.py", 10), ("a.py", 40)]
    );
    let read = |f: &str| Some(source(if f == "a.py" { 50 } else { 3 }));
    let bounds = Bounds {
        window: 3,
        ..Bounds::default()
    };
    let regions = context::regions(&located, bounds, read);
    assert_eq!(regions.len(), 3);
    assert_eq!((regions[0].first, regions[0].last), (1, 3));
    // Lines 10 and 14 share one region: 7-17.
    assert_eq!((regions[1].first, regions[1].last), (7, 17));
    assert_eq!(regions[1].named, vec![14, 10]);
    assert_eq!((regions[2].first, regions[2].last), (37, 43));
    let text = context::render(&regions, bounds, read).unwrap();
    assert!(text.contains("`a.py` lines 7-17, named by `python3 second.py`"));
    assert!(text.contains(">    14  line 14"));
    assert!(text.contains("     13  line 13"));
    // A character bound keeps whole regions only.
    let tight = Bounds {
        chars: 120,
        ..bounds
    };
    let short = context::render(&regions, tight, read).unwrap();
    assert!(short.contains("`b.py`") && !short.contains("`a.py`"));
}

#[test]
fn a_line_past_the_end_of_its_file_is_skipped() {
    let known = files(&["a.py"]);
    let failures = vec![Failure {
        command: "x".to_string(),
        output: "a.py:99: error".to_string(),
        exit: Some(1),
        ..Failure::default()
    }];
    let located = context::located(&failures, &known, "");
    let regions = context::regions(&located, Bounds::default(), |_| Some(source(5)));
    assert!(regions.is_empty());
}

#[test]
fn error_context_reads_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("pkg")).unwrap();
    std::fs::write(dir.path().join("pkg/core.py"), source(20)).unwrap();
    let policy = Localize {
        error_context: true,
        in_session: false,
        mismatch_trace: false,
        phase_timing: false,
        window: 2,
        timing_sec: 30,
    };
    let failure = Failure {
        command: "python3 -m pkg".to_string(),
        output: format!(
            "  File \"{}/pkg/core.py\", line 8, in f\nValueError",
            dir.path().display()
        ),
        exit: Some(1),
        ..Failure::default()
    };
    let (text, record) = error_context(&policy, dir.path(), &[failure], &BTreeSet::new());
    let text = text.unwrap();
    assert!(text.contains("`pkg/core.py` lines 6-10"), "{text}");
    assert!(text.contains(">     8  line 8"), "{text}");
    assert_eq!(record["resolved"], 1);
    // A line already told isn't told again.
    let told = BTreeSet::from([("pkg/core.py".to_string(), 8)]);
    let again = Failure {
        command: "again".to_string(),
        output: "pkg/core.py:8: boom".to_string(),
        exit: Some(1),
        ..Failure::default()
    };
    assert!(
        error_context(&policy, dir.path(), &[again], &told)
            .0
            .is_none()
    );
}

#[test]
fn mismatch_formats_each_give_a_first_case() {
    let pytest = mismatch::first_case(
        "E       input: [3, 1, 2]\nE       assert [1, 2, 3, 4] == [1, 2, 3]\n",
    )
    .unwrap();
    assert_eq!(pytest.format, "pytest");
    assert_eq!(pytest.observed, "[1, 2, 3, 4]");
    assert_eq!(pytest.expected, "[1, 2, 3]");
    assert_eq!(pytest.input.as_deref(), Some("[3, 1, 2]"));
    assert!(!pytest.sided);

    let unittest = mismatch::first_case("AssertionError: 0.5 != 0.25").unwrap();
    assert_eq!(unittest.format, "unittest");
    assert_eq!(
        (unittest.observed.as_str(), unittest.expected.as_str()),
        ("0.5", "0.25")
    );

    let rust =
        mismatch::first_case("assertion `left == right` failed\n  left: 3\n right: 4").unwrap();
    assert_eq!(rust.format, "rust-assert");
    assert_eq!((rust.observed.as_str(), rust.expected.as_str()), ("3", "4"));

    let go = mismatch::first_case("    dedup_test.go:31: got 12, want 11").unwrap();
    assert_eq!(go.format, "go-testing");
    assert!(go.sided);
    assert_eq!((go.observed.as_str(), go.expected.as_str()), ("12", "11"));

    let jest = mismatch::first_case("    Expected: \"a,b\"\n    Received: \"a;b\"").unwrap();
    assert_eq!(jest.format, "jest");
    assert_eq!(jest.observed, "\"a;b\"");

    let labeled = mismatch::first_case("case 7\nexpected: 0.125\nactual: 0.130\n").unwrap();
    assert_eq!(labeled.format, "labeled");
    assert_eq!(labeled.observed, "0.130");

    assert!(mismatch::first_case("all 12 tests passed").is_none());
}

#[test]
fn an_oracle_with_stages_names_the_first_stage_that_differs() {
    let line = json!({
        "name": "row 17",
        "input": {"text": "Straße 5"},
        "expected": "strasse 5",
        "observed": "straße 5",
        "stages": [
            {"name": "lowercase", "expected": "straße 5", "observed": "straße 5"},
            {"name": "fold", "expected": "strasse 5", "observed": "straße 5"},
            {"name": "trim", "expected": "strasse 5", "observed": "straße 5"},
        ],
    })
    .to_string();
    let output = format!(
        "{}\n{line}\n",
        json!({"name": "row 1", "expected": 1, "observed": 1})
    );
    let case = mismatch::first_case(&output).unwrap();
    assert_eq!(case.name.as_deref(), Some("row 17"));
    let (index, stage) = mismatch::first_differing_stage(&case).unwrap();
    assert_eq!((index, stage.name.as_str()), (1, "fold"));
    let text = mismatch::render(&case, "python3 oracle.py");
    assert!(text.contains("the first that differs is `fold`"), "{text}");
    assert!(
        text.contains("The stage before it, `lowercase`, agrees"),
        "{text}"
    );
    assert!(text.contains("first difference at character 5"), "{text}");
    let (note, record) = mismatch_trace(&[("python3 oracle.py".to_string(), output)]);
    assert!(note.is_some());
    assert_eq!(record["first_differing_stage"]["name"], "fold");
}

#[test]
fn diffs_name_the_first_difference() {
    assert_eq!(
        mismatch::diff("abcdef", "abcxef"),
        "first difference at character 4: expected `abcdef`, observed `abcxef`"
    );
    let multi = mismatch::diff("a\nb\nc", "a\nB\nc");
    assert!(multi.starts_with("first difference at line 2"), "{multi}");
    assert!(multi.contains("- expected: b") && multi.contains("+ observed: B"));
    let sides = mismatch::render(
        &mismatch::first_case("AssertionError: 1 != 2").unwrap(),
        "t",
    );
    assert!(
        sides.contains("Left side: 1") && sides.contains("right `2`, left `1`"),
        "{sides}"
    );
}

#[test]
fn profilers_are_chosen_by_the_command() {
    let (p, command) = timing::plan("python3 -u -m drift_monitor --window 50", 60);
    assert_eq!(p, Profiler::Python);
    assert!(
        command.contains("-X faulthandler -u -m cProfile -o \"$P\" -m drift_monitor --window 50"),
        "{command}"
    );
    assert!(command.contains("timeout -s ABRT 60"));
    let (p, command) = timing::plan("FOO=1 python3 run.py data.csv", 60);
    assert_eq!(p, Profiler::Python);
    assert!(
        command.contains("env FOO=1 python3 -X faulthandler"),
        "{command}"
    );
    assert_eq!(
        timing::plan("python3 -c 'print(1)'", 60).0,
        Profiler::Process
    );
    assert_eq!(
        timing::plan("sh tests/run.sh --fast", 60).0,
        Profiler::Shell
    );
    assert_eq!(timing::plan("./build.sh", 60).0, Profiler::Shell);
    assert_eq!(timing::plan("make test", 60).0, Profiler::Process);
    assert_eq!(timing::plan("python3 a.py | head", 60).0, Profiler::Process);
}

#[test]
fn a_shell_trace_becomes_phases() {
    let trace = "+ 100.000000 make build\n++ 100.100000 inner\n+ 102.500000 ./run --all\n+ 110.000000 diff out expected\n";
    let phases = timing::shell_phases(trace, 10.5);
    assert_eq!(phases.len(), 3);
    assert_eq!(phases[0].0, "make build");
    assert!((phases[0].1 - 2.5).abs() < 1e-6);
    assert!((phases[1].1 - 7.5).abs() < 1e-6);
    assert!((phases[2].1 - 0.5).abs() < 1e-6);
}

#[test]
fn a_profile_report_shows_the_top_functions_or_the_stuck_stack() {
    let stderr = format!(
        "{}\n   120 function calls in 3.200 seconds\n\n   Ordered by: cumulative time\n\n   ncalls  tottime  percall  cumtime  percall filename:lineno(function)\n        1    0.000    0.000    3.200    3.200 run.py:1(<module>)\n       10    3.100    0.310    3.100    0.310 run.py:8(slow)\n",
        timing::MARK
    );
    let run = timing::Profiled {
        stderr,
        exit: Some(0),
        milliseconds: 3300,
        ..timing::Profiled::default()
    };
    let text = timing::report(Profiler::Python, "python3 run.py", &run, 60);
    assert!(text.contains("finished in 3.3 s"), "{text}");
    assert!(text.contains("120 function calls in 3.200 seconds"));
    assert!(text.contains("run.py:8(slow)"));
    let stuck = timing::Profiled {
        stderr: "Fatal Python error: Aborted\n\nCurrent thread 0x1 (most recent call first):\n  File \"run.py\", line 9 in slow\n  File \"run.py\", line 2 in <module>\n".to_string(),
        timed_out: true,
        milliseconds: 60_000,
        ..timing::Profiled::default()
    };
    let text = timing::report(Profiler::Python, "python3 run.py", &stuck, 60);
    assert!(text.contains("stopped at the 60 s bound"), "{text}");
    assert!(text.contains("File \"run.py\", line 9 in slow"));
}

#[tokio::test]
async fn phase_timing_times_a_shell_script_on_a_copy() {
    if std::process::Command::new("bash")
        .arg("-c")
        .arg("[ -n \"$EPOCHREALTIME\" ]")
        .status()
        .map_or(true, |s| !s.success())
    {
        eprintln!("skipped: no bash with EPOCHREALTIME");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("check.sh"),
        "echo start\nsleep 0.6\necho done > marker\n",
    )
    .unwrap();
    let policy = Localize {
        error_context: false,
        in_session: false,
        mismatch_trace: false,
        phase_timing: true,
        window: 6,
        timing_sec: 30,
    };
    let target = Slow {
        command: "sh check.sh".to_string(),
        milliseconds: 9_000,
        bound_ms: 10_000,
        timed_out: false,
    };
    assert!(target.triggers());
    let place = crate::checks::contract::executed::Place {
        workdir: dir.path().to_path_buf(),
        contained: true,
        wall: std::time::Duration::from_secs(30),
        budget: std::time::Duration::from_secs(30),
    };
    let (text, record) = phase_timing(&policy, &target, &place).await;
    let text = text.unwrap();
    assert!(
        text.contains("Top-level commands by wall time"),
        "{text}\n{record}"
    );
    let first = text.lines().nth(2).unwrap_or("");
    assert!(first.contains("sleep 0.6"), "{text}");
    // It ran on a copy: the workspace is untouched.
    assert!(!dir.path().join("marker").exists());
}

#[test]
fn only_slow_commands_trigger_a_profile() {
    let fast = Slow {
        command: "python3 run.py".to_string(),
        milliseconds: 1_000,
        bound_ms: 120_000,
        timed_out: false,
    };
    let read = Slow {
        command: "cat big.log".to_string(),
        milliseconds: 100_000,
        bound_ms: 120_000,
        timed_out: false,
    };
    let stopped = Slow {
        timed_out: true,
        ..fast.clone()
    };
    assert!(timing_target(std::slice::from_ref(&fast)).is_none());
    assert!(timing_target(&[read]).is_none());
    assert_eq!(timing_target(&[stopped.clone(), fast]), Some(&stopped));
}

#[test]
fn the_switch_validates_and_stays_out_of_existing_manifests() {
    let off: crate::micro::lean::Lean =
        serde_json::from_value(json!({ "sessions": 1, "source_chars": 1000 })).unwrap();
    assert!(off.localize.is_none());
    assert!(!serde_json::to_string(&off).unwrap().contains("localize"));
    let none: Localize = serde_json::from_value(json!({})).unwrap();
    assert!(none.validate()[0].contains("turns on none"));
    let bad: Localize =
        serde_json::from_value(json!({"in_session": true, "mismatch_trace": true})).unwrap();
    assert!(
        bad.validate()
            .iter()
            .any(|p| p.contains("in_session requires error_context"))
    );
    let good: Localize =
        serde_json::from_value(json!({"error_context": true, "in_session": true})).unwrap();
    assert!(good.validate().is_empty());
    assert_eq!(good.window, context::WINDOW);
    assert!(serde_json::from_value::<Localize>(json!({"errors": true})).is_err());
}

fn step(source: &str, message: &str) -> serde_json::Value {
    json!({"record": "step", "step": {"at": 0, "source": source, "message": message}})
}

fn tool(name: &str, arguments: serde_json::Value, output: &str, ok: bool) -> serde_json::Value {
    json!({"record": "step", "step": {"at": 0, "source": "Agent", "message": "", "call": {
        "id": "c", "name": name, "arguments": arguments, "output": output,
        "outcome": if ok { "Completed" } else { "Failed" }, "milliseconds": 5, "extra": {}
    }}})
}

fn session_log() -> String {
    let brief = format!(
        "# Task\n\nFix it.\n\n# Evidence\n\n## The current pkg/core.py\n\n{}\n## The current README.md\n\nhello\n",
        source(40)
    );
    let patch =
        "*** Begin Patch\n*** Update File: pkg/core.py\n@@\n-line 21\n+LINE 21\n*** End Patch";
    [
        json!({"record": "session", "at": 0, "session": {"repository": "/app", "id": "s"}}),
        step("User", &brief),
        step("Agent", "run it"),
        tool(
            "run_command",
            json!({"command": "python3 -m pkg"}),
            "[exit 1]\n[stderr]\nTraceback (most recent call last):\n  File \"/app/pkg/core.py\", line 20, in f\nValueError",
            false,
        ),
        step("Agent", "look"),
        tool(
            "read_file",
            json!({"path": "pkg/core.py", "start_line": 15}),
            "    15\tline 15\n    16\tline 16\n    17\tline 17\n    18\tline 18\n    19\tline 19\n    20\tline 20\n[lines 15-20 of 40]",
            true,
        ),
        step("Agent", "look again"),
        tool("run_command", json!({"command": "grep -n f pkg/core.py"}), "[exit 0]\n3:def f", true),
        step("Agent", "fix"),
        tool("apply_patch", json!({"patch": patch}), "Patched pkg/core.py.", true),
        step("Agent", "rerun"),
        tool(
            "run_command",
            json!({"command": "python3 -m pkg", "timeout_seconds": 30}),
            "[timed out after 30 s]\n",
            false,
        ),
    ]
    .iter()
    .map(serde_json::Value::to_string)
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn a_session_log_reads_as_calls_with_the_brief() {
    let log = trace::parse(&session_log());
    assert_eq!(log.root, "/app");
    assert_eq!(log.turns, 5);
    assert_eq!(log.brief.len(), 2);
    assert_eq!(log.brief[0].0, "pkg/core.py");
    assert_eq!(log.brief[0].1.lines().count(), 40);
    assert_eq!(log.calls.len(), 5);
    let failures: Vec<_> = log.calls.iter().filter_map(|(_, c)| c.failure()).collect();
    assert_eq!(failures.len(), 2);
    assert!(failures[1].timed_out);
    assert!(matches!(&log.calls[1].1, Call::Read { lines, .. } if lines.len() == 6));
    assert!(matches!(
        &log.calls[2].1,
        Call::Command {
            reads_only: true,
            ..
        }
    ));
    assert!(matches!(
        &log.calls[4].1,
        Call::Command { bound_sec: 30, .. }
    ));
}

#[test]
fn changed_spans_mark_removed_lines_and_insertion_points() {
    let old = "a\nb\nc\nd\ne\n";
    assert_eq!(offline::changed_spans(old, "a\nB\nc\nd\ne\n"), vec![(2, 2)]);
    assert_eq!(
        offline::changed_spans(old, "a\nb\nc\nX\nd\ne\n"),
        vec![(3, 4)]
    );
    assert_eq!(
        offline::changed_spans(old, "a\nB\nc\nD\ne\n"),
        vec![(2, 2), (4, 4)]
    );
    assert!(offline::changed_spans(old, old).is_empty());
}

#[test]
fn the_offline_replay_finds_the_region_and_the_rereads() {
    let dir = tempfile::tempdir().unwrap();
    let artifacts = dir.path().join("job/demo-task__abc.episode/artifacts");
    std::fs::create_dir_all(&artifacts).unwrap();
    std::fs::write(artifacts.join("microluna-1-1.atif.jsonl"), session_log()).unwrap();
    let skipped = dir
        .path()
        .join("job/embedding-drift-monitor__x.episode/artifacts");
    std::fs::create_dir_all(&skipped).unwrap();
    std::fs::write(skipped.join("microluna-1-1.atif.jsonl"), session_log()).unwrap();
    let (rows, totals, sources) = offline::replay(
        &[dir.path().to_path_buf()],
        &["embedding-drift-monitor".to_string()],
    );
    assert_eq!(totals.trials, 1);
    assert_eq!(totals.skipped_excluded, 1);
    assert_eq!(rows.len(), 2);
    let first = &rows[0];
    assert_eq!(first.file.as_deref(), Some("pkg/core.py"));
    assert_eq!(first.line, Some(20));
    assert_eq!(first.touch, Some(Touch::Region));
    assert!(first.chance.unwrap() > 0.0 && first.chance.unwrap() < 1.0);
    // Two read-only turns re-read the file; the read_file showed line 20.
    assert_eq!(first.reread_turns, 2);
    assert_eq!(first.reread_line_turns, 1);
    assert!(first.last_before_edit);
    let second = &rows[1];
    assert!(second.timed_out);
    assert_eq!(second.profiler, Some(Profiler::Python));
    assert_eq!(second.touch, None);
    let summary = offline::summary(&rows, &totals, &sources);
    assert_eq!(summary["resolved"]["k"], 1);
    assert_eq!(summary["next_edit_every_failure"]["region"], 1);
    assert_eq!(summary["rereads"]["turns"], 2);
}
