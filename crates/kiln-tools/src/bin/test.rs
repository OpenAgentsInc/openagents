//! `test [DIR]`: run every `*_test.rhai` file under DIR (default `.`).
//! A file's top level runs first; then each function named `test_*` with
//! no parameters runs as its own test. Prints `ok` or `FAIL` per test and
//! a summary, and exits 1 when a test fails or none exist.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rhai::{CallFnOptions, Dynamic, Scope};

fn find(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            find(&path, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with("_test.rhai"))
        {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut files = Vec::new();
    find(Path::new(&root), &mut files);
    if files.is_empty() {
        println!("no tests: name a test file NAME_test.rhai");
        return ExitCode::FAILURE;
    }
    let (mut passed, mut failed) = (0, 0);
    for file in &files {
        let shown = file.display().to_string();
        let shown = shown.strip_prefix("./").unwrap_or(&shown).to_string();
        let engine = kiln_tools::engine(&kiln_tools::script_dir(file));
        let source = match std::fs::read_to_string(file) {
            Ok(source) => source,
            Err(error) => {
                println!("FAIL {shown}: {error}");
                failed += 1;
                continue;
            }
        };
        let ast = match engine.compile(&source) {
            Ok(ast) => ast,
            Err(error) => {
                println!("FAIL {shown}: {error}");
                failed += 1;
                continue;
            }
        };
        let mut scope = Scope::new();
        if let Err(error) = engine.run_ast_with_scope(&mut scope, &ast) {
            println!("FAIL {shown}: {}", kiln_tools::describe(&error));
            failed += 1;
            continue;
        }
        let tests: Vec<String> = ast
            .iter_functions()
            .filter(|f| f.name.starts_with("test_") && f.params.is_empty())
            .map(|f| f.name.to_string())
            .collect();
        if tests.is_empty() {
            println!("ok   {shown}");
            passed += 1;
            continue;
        }
        for name in tests {
            // Evaluating the file again first brings its imports into
            // scope; functions don't see a previous run's imports.
            let options = CallFnOptions::new().eval_ast(true);
            let mut scope = Scope::new();
            match engine.call_fn_with_options::<Dynamic>(options, &mut scope, &ast, &name, ()) {
                Ok(_) => {
                    println!("ok   {shown}::{name}");
                    passed += 1;
                }
                Err(error) => {
                    println!("FAIL {shown}::{name}: {}", kiln_tools::describe(&error));
                    failed += 1;
                }
            }
        }
    }
    println!("{passed} passed, {failed} failed");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
