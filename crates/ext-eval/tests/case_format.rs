//! The case format: discovery, merging, and one test per parse error and
//! limit in `docs/extensions/evaluation.md`, *Case format*.

mod common;

use std::path::Path;

use ext_eval::case::{CASE_TOML_KEYS, CaseFiles, PROMPT_KEYS, RUN_KEYS};
use ext_eval::{Case, CaseError, Filter, Grant, Kind, LoadOptions, RunFailure, Suite, eval_dir};

const V: &str = "v = \"openagents.eval-case.v1\"";
const GRADER: &str = "+++\ntype = \"regex\"\n+++\n\ndone\n";

/// Writes `files` (relative path, contents) under `root`.
fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, contents) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, contents).unwrap();
    }
}

fn prompt(front: &str, body: &str) -> String {
    format!("+++\n{front}\n+++\n\n{body}\n")
}

/// Parses a case from in-memory files, with one grader file unless given.
fn parse(
    prompt_md: &str,
    case_toml: Option<&str>,
    graders: &[(&str, &str)],
) -> Result<Case, CaseError> {
    let graders = if graders.is_empty() && case_toml.is_none_or(|toml| !toml.contains("graders")) {
        vec![("done.md".to_string(), GRADER.as_bytes().to_vec())]
    } else {
        graders
            .iter()
            .map(|(name, text)| ((*name).to_string(), text.as_bytes().to_vec()))
            .collect()
    };
    Case::parse(
        "my-case",
        "evals/my-case",
        CaseFiles {
            prompt: prompt_md.as_bytes().to_vec(),
            case_toml: case_toml.map(|toml| toml.as_bytes().to_vec()),
            graders,
            fixtures: Vec::new(),
        },
    )
}

fn error(result: Result<Case, CaseError>) -> String {
    result.expect_err("the case is refused").to_string()
}

#[test]
fn the_fixture_suite_discovers_in_order() {
    let suite = common::suite();
    let names: Vec<&str> = suite.cases.iter().map(|case| case.name.as_str()).collect();
    assert_eq!(
        names,
        ["find-callers", "map-only", "typo-fix", "write-summary"]
    );
    let typo = suite.case("typo-fix").unwrap();
    assert_eq!(typo.kind, Kind::ShouldNotFire);
    assert_eq!(typo.runs, 3);
    assert_eq!(typo.run.deadline_seconds, 300);
    assert_eq!(typo.run.allowed_operations, [Grant::Read].into());
}

#[test]
fn case_toml_is_the_base_and_prompt_md_overrides_it() {
    let suite = common::suite();
    let case = suite.case("write-summary").unwrap();
    // case.toml says 120; the prompt.md frontmatter's [run] overrides the
    // one key and keeps the rest of case.toml's [run].
    assert_eq!(case.run.deadline_seconds, 90);
    assert_eq!(
        case.run.allowed_operations,
        [Grant::Read, Grant::Write].into()
    );
    assert_eq!(case.tags, ["writing"]);
    assert_eq!(
        case.prompt,
        "Read README.md and write a two-line summary of it to SUMMARY.md."
    );
    // case.toml's graders first, then graders/*.md in name order.
    let graders: Vec<&str> = case
        .graders
        .iter()
        .map(|grader| grader.name.as_str())
        .collect();
    assert_eq!(graders, ["wrote-summary", "summary-mentions"]);
    assert_eq!(case.files.fixtures.len(), 1);
    assert_eq!(case.files.fixtures[0].0, "README.md");
}

#[test]
fn a_grader_is_named_by_its_file_unless_it_names_itself() {
    let case = parse(
        &prompt(V, "task"),
        None,
        &[
            ("b.md", GRADER),
            ("a.md", "+++\ntype = \"regex\"\nname = \"custom\"\n+++\nx\n"),
        ],
    )
    .unwrap();
    let names: Vec<&str> = case
        .graders
        .iter()
        .map(|grader| grader.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["custom", "b"],
        "files in name order: a.md, then b.md"
    );
}

#[test]
fn the_name_defaults_to_the_directory_and_is_checked() {
    assert_eq!(
        parse(&prompt(V, "task"), None, &[]).unwrap().name,
        "my-case"
    );
    let named = parse(
        &prompt(&format!("{V}\nname = \"other\""), "task"),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(named.name, "other");
    let text = error(parse(
        &prompt(&format!("{V}\nname = \"has space\""), "task"),
        None,
        &[],
    ));
    assert!(text.contains("letters, digits"), "{text}");
    let text = error(parse(
        &prompt(&format!("{V}\nname = \".hidden\""), "task"),
        None,
        &[],
    ));
    assert!(text.contains("not start with"), "{text}");
}

#[test]
fn an_unknown_key_in_prompt_md_names_the_allowed_set() {
    let text = error(parse(
        &prompt(&format!("{V}\nmax_turns = 10"), "task"),
        None,
        &[],
    ));
    assert!(text.contains("prompt.md"), "{text}");
    assert!(text.contains("`max_turns`"), "{text}");
    assert!(text.contains(&PROMPT_KEYS.join(", ")), "{text}");
}

#[test]
fn graders_belong_to_case_toml_not_the_frontmatter() {
    let text = error(parse(
        &prompt(&format!("{V}\ngraders = []"), "task"),
        None,
        &[],
    ));
    assert!(text.contains("`graders`"), "{text}");
    assert!(text.contains(&PROMPT_KEYS.join(", ")), "{text}");
}

#[test]
fn an_unknown_key_in_case_toml_names_the_allowed_set() {
    let text = error(parse(
        &prompt("", "task"),
        Some(&format!("{V}\nschema_version = \"1.0\"")),
        &[],
    ));
    assert!(text.contains("case.toml"), "{text}");
    assert!(text.contains("`schema_version`"), "{text}");
    assert!(text.contains(&CASE_TOML_KEYS.join(", ")), "{text}");
}

#[test]
fn an_unknown_run_key_names_the_run_keys() {
    let text = error(parse(
        &prompt(&format!("{V}\n[run]\nmax_turns = 3"), "task"),
        None,
        &[],
    ));
    assert!(text.contains("`run.max_turns`"), "{text}");
    assert!(text.contains(&RUN_KEYS.join(", ")), "{text}");
}

#[test]
fn the_version_is_required_and_checked() {
    let text = error(parse(&prompt("kind = \"should-fire\"", "task"), None, &[]));
    assert!(text.contains("declares no `v`"), "{text}");
    let text = error(parse(&prompt("v = \"1.0\"", "task"), None, &[]));
    assert!(
        text.contains("must be \"openagents.eval-case.v1\""),
        "{text}"
    );
    let text = error(parse(&prompt("v = 1", "task"), None, &[]));
    assert!(text.contains("must be a string"), "{text}");
}

#[test]
fn a_newer_major_version_refuses_with_the_version_it_needs() {
    let result = parse(
        &prompt("v = \"openagents.eval-case.v2\"", "task"),
        None,
        &[],
    );
    assert!(matches!(result, Err(CaseError::NewerVersion { .. })));
    let text = error(result);
    assert!(text.contains("openagents.eval-case.v2"), "{text}");
    assert!(text.contains("update OpenAgents"), "{text}");
}

#[test]
fn the_version_may_live_in_case_toml() {
    let case = parse(
        "the task\n",
        Some(&format!(
            "{V}\n[[graders]]\ntype = \"regex\"\nname = \"g\"\npattern = \"x\""
        )),
        &[],
    )
    .unwrap();
    assert_eq!(case.prompt, "the task");
}

#[test]
fn kind_is_should_fire_or_should_not_fire() {
    let text = error(parse(
        &prompt(&format!("{V}\nkind = \"maybe\""), "task"),
        None,
        &[],
    ));
    assert!(text.contains("should-fire"), "{text}");
    let case = parse(
        &prompt(&format!("{V}\nkind = \"should-not-fire\""), "task"),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(case.kind, Kind::ShouldNotFire);
}

#[test]
fn runs_are_one_to_ten() {
    for bad in ["0", "11", "\"3\"", "2.5"] {
        let text = error(parse(
            &prompt(&format!("{V}\nruns = {bad}"), "task"),
            None,
            &[],
        ));
        assert!(text.contains("from 1 to 10"), "{bad}: {text}");
    }
    for good in [1, 10] {
        let case = parse(&prompt(&format!("{V}\nruns = {good}"), "task"), None, &[]).unwrap();
        assert_eq!(case.runs, good);
    }
    assert_eq!(parse(&prompt(V, "task"), None, &[]).unwrap().runs, 3);
}

#[test]
fn the_deadline_is_at_most_1800_seconds() {
    for bad in ["0", "1801", "\"60\""] {
        let text = error(parse(
            &prompt(&format!("{V}\n[run]\ndeadline_seconds = {bad}"), "task"),
            None,
            &[],
        ));
        assert!(text.contains("1 to 1800"), "{bad}: {text}");
    }
    let case = parse(
        &prompt(&format!("{V}\n[run]\ndeadline_seconds = 1800"), "task"),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(case.run.deadline_seconds, 1800);
}

#[test]
fn allowed_operations_are_the_four_grants() {
    let text = error(parse(
        &prompt(
            &format!("{V}\n[run]\nallowed_operations = [\"sudo\"]"),
            "task",
        ),
        None,
        &[],
    ));
    assert!(text.contains("read, write, exec, and network"), "{text}");
    let case = parse(
        &prompt(
            &format!("{V}\n[run]\nallowed_operations = [\"exec\"]"),
            "task",
        ),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(
        case.run.allowed_operations,
        [Grant::Read, Grant::Exec].into(),
        "read is always in"
    );
}

#[test]
fn env_keys_outside_oa_eval_fail_the_run() {
    let case = parse(
        &prompt(
            &format!("{V}\n[run.env]\nOA_EVAL_MODE = \"fast\"\nOA_EVAL_2 = \"x\""),
            "task",
        ),
        None,
        &[],
    )
    .unwrap();
    assert_eq!(case.check_env(), Ok(()));
    for key in [
        "HOME",
        "CODER_DOOR_KEY",
        "oa_eval_lower",
        "OA_EVAL_bad",
        "OA_EVALX",
    ] {
        let case = parse(
            &prompt(&format!("{V}\n[run.env]\n{key} = \"x\""), "task"),
            None,
            &[],
        )
        .unwrap();
        assert_eq!(
            case.check_env(),
            Err((RunFailure::EnvVarRejected, key.to_string())),
            "{key}"
        );
    }
    let text = error(parse(
        &prompt(&format!("{V}\n[run.env]\nOA_EVAL_N = 1"), "task"),
        None,
        &[],
    ));
    assert!(text.contains("must be a string"), "{text}");
}

#[test]
fn a_case_file_is_at_most_one_mebibyte() {
    let big = "x".repeat(1024 * 1024);
    let result = parse(&prompt(V, &big), None, &[]);
    assert!(
        matches!(result, Err(CaseError::TooLarge { .. })),
        "prompt.md"
    );
    let result = parse(
        &prompt(V, "task"),
        Some(&format!("{V}\ndescription = \"{big}\"")),
        &[],
    );
    assert!(
        matches!(result, Err(CaseError::TooLarge { .. })),
        "case.toml"
    );
    let grader = format!("+++\ntype = \"regex\"\n+++\n{big}");
    let result = parse(&prompt(V, "task"), None, &[("big.md", &grader)]);
    let text = error(result);
    assert!(text.contains("1 MiB"), "grader: {text}");

    // On disk, the size is checked before the file is read.
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[
            ("c/prompt.md", &prompt(V, &big)),
            ("c/graders/g.md", GRADER),
        ],
    );
    let result = Case::load(&dir.path().join("c"), "c", LoadOptions::default());
    assert!(matches!(result, Err(CaseError::TooLarge { .. })));
}

#[test]
fn a_case_has_at_most_64_grader_files_and_64_graders() {
    let files: Vec<(String, String)> = (0..65)
        .map(|index| (format!("g{index:02}.md"), GRADER.to_string()))
        .collect();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    let text = error(parse(&prompt(V, "task"), None, &borrowed));
    assert!(text.contains("at most 64"), "{text}");

    let mut toml = String::from(V);
    for index in 0..60 {
        toml.push_str(&format!(
            "\n[[graders]]\ntype = \"regex\"\nname = \"t{index}\"\npattern = \"x\""
        ));
    }
    let text = error(parse(&prompt("", "task"), Some(&toml), &borrowed[..5]));
    assert!(text.contains("65 graders"), "{text}");

    let dir = tempfile::tempdir().unwrap();
    let mut on_disk = vec![("c/prompt.md".to_string(), prompt(V, "task"))];
    for (name, text) in &files {
        on_disk.push((format!("c/graders/{name}"), text.clone()));
    }
    let on_disk: Vec<(&str, &str)> = on_disk
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    write(dir.path(), &on_disk);
    let text = Case::load(&dir.path().join("c"), "c", LoadOptions::default())
        .unwrap_err()
        .to_string();
    assert!(text.contains("at most 64"), "{text}");
}

#[test]
fn a_case_needs_a_prompt_and_a_grader() {
    let text = error(parse(&prompt(V, "   "), None, &[]));
    assert!(text.contains("no prompt"), "{text}");
    let text = error(Case::parse(
        "c",
        "c",
        CaseFiles {
            prompt: prompt(V, "task").into_bytes(),
            ..CaseFiles::default()
        },
    ));
    assert!(text.contains("no graders"), "{text}");
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), &[("c/graders/g.md", GRADER)]);
    let text = Case::load(&dir.path().join("c"), "c", LoadOptions::default())
        .unwrap_err()
        .to_string();
    assert!(text.contains("needs a prompt.md"), "{text}");
}

#[test]
fn frontmatter_and_toml_syntax_errors_are_named() {
    let text = error(parse(
        "+++\nv = \"openagents.eval-case.v1\"\n\ntask\n",
        None,
        &[],
    ));
    assert!(text.contains("never closes"), "{text}");
    let text = error(parse(&prompt("v = ", "task"), None, &[]));
    assert!(text.contains("TOML does not parse"), "{text}");
    let text = error(parse(
        &prompt(V, "task"),
        None,
        &[("g.md", "type = \"regex\"\n")],
    ));
    assert!(text.contains("frontmatter"), "{text}");
    let text = error(Case::parse(
        "c",
        "c",
        CaseFiles {
            prompt: vec![0xff, 0xfe],
            ..CaseFiles::default()
        },
    ));
    assert!(text.contains("UTF-8"), "{text}");
}

#[test]
fn duplicate_grader_names_are_refused() {
    let text = error(parse(
        &prompt(V, "task"),
        None,
        &[
            ("a.md", GRADER),
            ("b.md", "+++\ntype = \"regex\"\nname = \"a\"\n+++\nx\n"),
        ],
    ));
    assert!(text.contains("grader names are unique"), "{text}");
}

#[test]
fn a_run_refuses_a_case_with_a_todo_line() {
    let dir = tempfile::tempdir().unwrap();
    // The template `openagents ext eval init <name> --bare` writes.
    write(
        dir.path(),
        &[
            (
                "smoke/prompt.md",
                "+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\n+++\n\nTODO: describe a task someone would give Coder\n",
            ),
            (
                "smoke/graders/criteria.md",
                "+++\ntype = \"decision\"\nquestion = \"Did the run do what the task asked?\"\nthreshold = 0.7\n+++\n\nTODO: describe what a successful run looks like\n",
            ),
        ],
    );
    let result = Case::load(&dir.path().join("smoke"), "smoke", LoadOptions::default());
    let Err(CaseError::Todo { file, line }) = result else {
        panic!("a TODO line refuses the case");
    };
    assert_eq!((file.as_str(), line), ("smoke/prompt.md", 6));
    let draft = Case::load(
        &dir.path().join("smoke"),
        "smoke",
        LoadOptions { allow_todo: true },
    )
    .expect("an author editing a draft may load it");
    let todos = draft.todo_lines();
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[1].file, "graders/criteria.md");
}

#[test]
fn duplicate_case_names_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[
            (
                "evals/a/prompt.md",
                &prompt(&format!("{V}\nname = \"same\""), "one"),
            ),
            ("evals/a/graders/g.md", GRADER),
            (
                "evals/b/prompt.md",
                &prompt(&format!("{V}\nname = \"same\""), "two"),
            ),
            ("evals/b/graders/g.md", GRADER),
        ],
    );
    let result = Suite::load(&dir.path().join("evals"), LoadOptions::default());
    let Err(CaseError::DuplicateCase {
        name,
        first,
        second,
    }) = result
    else {
        panic!("duplicate names are refused");
    };
    assert_eq!(
        (name.as_str(), first.as_str(), second.as_str()),
        ("same", "a", "b")
    );
}

#[test]
fn discovery_skips_tool_directories_and_never_recurses_into_a_case() {
    let dir = tempfile::tempdir().unwrap();
    let case = |body: &str| prompt(V, body);
    write(
        dir.path(),
        &[
            ("evals/b-case/prompt.md", &case("b")),
            ("evals/b-case/graders/g.md", GRADER),
            // Inside a case: never a case of its own.
            ("evals/b-case/fixtures/inner/prompt.md", &case("inner")),
            ("evals/group/a-case/prompt.md", &case("a")),
            ("evals/group/a-case/graders/g.md", GRADER),
            ("evals/results/2026/prompt.md", &case("old")),
            ("evals/node_modules/x/prompt.md", &case("dep")),
            ("evals/.git/x/prompt.md", &case("git")),
            ("evals/.openagents/x/prompt.md", &case("state")),
        ],
    );
    let suite = Suite::load(&dir.path().join("evals"), LoadOptions::default()).unwrap();
    let paths: Vec<&str> = suite.cases.iter().map(|case| case.path.as_str()).collect();
    assert_eq!(paths, ["b-case", "group/a-case"]);
    assert_eq!(suite.cases[1].name, "a-case");
    let fixtures = &suite.cases[0].files.fixtures;
    assert_eq!(fixtures[0].0, "inner/prompt.md");
}

#[test]
fn an_empty_or_missing_eval_directory_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Suite::load(&dir.path().join("evals"), LoadOptions::default()).is_err());
    std::fs::create_dir(dir.path().join("evals")).unwrap();
    let text = Suite::load(&dir.path().join("evals"), LoadOptions::default())
        .unwrap_err()
        .to_string();
    assert!(text.contains("holds no case"), "{text}");
}

#[test]
fn the_eval_dir_follows_flag_record_default() {
    let root = Path::new("/x");
    assert_eq!(eval_dir(root, None, None).unwrap(), root.join("evals"));
    assert_eq!(
        eval_dir(root, None, Some("checks")).unwrap(),
        root.join("checks")
    );
    assert_eq!(
        eval_dir(root, Some("mine"), Some("checks")).unwrap(),
        root.join("mine")
    );
    assert!(eval_dir(root, Some("../elsewhere"), None).is_err());
}

#[test]
fn filters_keep_cases_by_name_glob_and_tag() {
    let suite = common::suite();
    let names = |filter: Filter| -> Vec<String> {
        suite
            .clone()
            .filtered(&filter)
            .cases
            .into_iter()
            .map(|case| case.name)
            .collect()
    };
    assert_eq!(names(Filter::default()).len(), 4);
    assert_eq!(
        names(Filter {
            cases: vec!["*-fix".into(), "find-*".into()],
            tags: Vec::new()
        }),
        ["find-callers", "typo-fix"]
    );
    assert_eq!(
        names(Filter {
            cases: Vec::new(),
            tags: vec!["writing".into(), "editing".into()]
        }),
        ["typo-fix", "write-summary"]
    );
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_not_read_as_a_case_file() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[
            ("outside.md", &prompt(V, "task")),
            ("c/graders/g.md", GRADER),
        ],
    );
    std::os::unix::fs::symlink(
        dir.path().join("outside.md"),
        dir.path().join("c/prompt.md"),
    )
    .unwrap();
    let text = Case::load(&dir.path().join("c"), "c", LoadOptions::default())
        .unwrap_err()
        .to_string();
    assert!(text.contains("not a regular file"), "{text}");
}

#[test]
fn warnings_name_graders_that_need_a_grant_the_case_lacks() {
    let case = parse(
        &prompt(V, "task"),
        None,
        &[
            (
                "made.md",
                "+++\ntype = \"file_exists\"\npath = \"out.md\"\n+++\n",
            ),
            (
                "ran.md",
                "+++\ntype = \"operation_used\"\noperation = \"shell\"\n+++\n",
            ),
        ],
    )
    .unwrap();
    let warnings = case.warnings();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("`write`"));
    assert!(warnings[1].contains("`exec`"));
    assert!(
        common::suite()
            .case("write-summary")
            .unwrap()
            .warnings()
            .is_empty()
    );
}
