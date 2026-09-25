//! Wide entry-point discovery (issue #9654): the kinds
//! [`super::Discovery::Wide`] adds to #9633's four.
//!
//! - **Packages** anywhere a session would run one with `python3 -m`: a
//!   top-level package, a package under `src/` (run with
//!   `PYTHONPATH=src`), and a subpackage one level down, each with a
//!   `__main__.py`.
//! - **Console scripts** a `pyproject.toml` (`[project.scripts]`,
//!   `[tool.poetry.scripts]`), `setup.cfg` (`console_scripts`), or
//!   `setup.py` declares, run as `python3 -c` on the declared function, so
//!   the package needn't be installed.
//! - **Makefile targets** from a fixed list of run and test names
//!   ([`MAKE_TARGETS`]), at most two.
//! - **`package.json` scripts** from the same kind of list
//!   ([`NPM_SCRIPTS`]), at most two, run with the package manager the
//!   lockfile names.
//! - **Cargo binaries** of a package's `Cargo.toml`, run with
//!   `cargo run --offline`, at most two.
//!
//! Arguments come from the files the instruction names, as before. When it
//! names none, they come from the input files the task ships
//! ([`shipped_inputs`]): the files in conventional input directories such
//! as `data/` or `scenarios/`, and data files at the top level. A program
//! whose usage those files don't fit, and that takes a fixed number of
//! them, runs once per file for its last argument, at most [`SWEEP`] times
//! ([`argument_sets`]).
//!
//! The rules for what may run are #9633's: a command that looks like it
//! needs the network, or a recipe or script body that does, is refused.

use std::collections::BTreeSet;
use std::path::Path;

use regex::Regex;
use serde_json::Value;

use super::{Entry, EntryKind, extract};

/// Directories whose files are the task's shipped inputs.
pub const INPUT_DIRS: &[&str] = &[
    "data",
    "input",
    "inputs",
    "scenarios",
    "scenario",
    "examples",
    "example",
    "samples",
    "sample",
    "fixtures",
    "cases",
];

/// Extensions of a data file at the workspace's top level.
pub const DATA_EXTENSIONS: &[&str] = &[
    "npy", "npz", "csv", "tsv", "json", "jsonl", "ndjson", "parquet", "txt", "log", "dat",
];

/// Names that are a project's configuration or documentation, never its
/// input.
pub const NOT_INPUT: &[&str] = &[
    "package.json",
    "package-lock.json",
    "composer.json",
    "tsconfig.json",
    "jsconfig.json",
    "requirements.txt",
    "constraints.txt",
    "cmakelists.txt",
    "robots.txt",
];

/// Shipped input files used, at most.
pub const MAX_SHIPPED: usize = 12;

/// Runs of one program over the shipped files, at most.
pub const SWEEP: usize = 3;

/// Makefile targets that run or test the program, in order of preference.
pub const MAKE_TARGETS: &[&str] = &[
    "test", "check", "run", "demo", "example", "examples", "eval", "evaluate", "all",
];

/// `package.json` scripts that run or test the program, in order of
/// preference. `start`, `dev`, and `serve` usually start a server that
/// doesn't exit, so they aren't here.
pub const NPM_SCRIPTS: &[&str] = &[
    "test", "check", "eval", "evaluate", "demo", "example", "build",
];

/// Entries of one kind, at most, for make, npm, console, and Cargo.
const PER_KIND: usize = 2;

/// The depth a nested package search reaches below its root.
const PACKAGE_DEPTH: usize = 2;

/// Whether a file name can be a task input: not hidden, not code or
/// documentation, and not a project's configuration or lockfile.
#[must_use]
pub fn is_input_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    !name.starts_with('.')
        && !NOT_INPUT.contains(&lower.as_str())
        && !lower.starts_with("requirements")
        && !lower.starts_with("readme")
        && !lower.starts_with("license")
        && !lower.starts_with("changelog")
        && !lower.ends_with(".lock")
        && !lower.starts_with("tsconfig")
        && lower != "makefile"
        && !super::extension(name).is_some_and(|e| super::CODE.contains(&e.as_str()))
}

fn sorted_entries(dir: &Path) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let kind = e.file_type().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            (kind.is_dir() || kind.is_file()).then_some((name, kind.is_dir()))
        })
        .collect();
    out.sort();
    out
}

/// The input files the task ships, as the instruction would name them
/// (under `alias`), in path order, at most [`MAX_SHIPPED`]: the files
/// directly inside each [`INPUT_DIRS`] directory at the top level and in
/// its immediate subdirectories, then the top level's files with a
/// [`DATA_EXTENSIONS`] extension.
#[must_use]
pub fn shipped_inputs(root: &Path, alias: &str) -> Vec<String> {
    let alias = alias.trim_end_matches('/');
    let mut found: Vec<String> = Vec::new();
    for (name, is_dir) in sorted_entries(root) {
        if !is_dir || !INPUT_DIRS.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        for (inner, inner_dir) in sorted_entries(&root.join(&name)) {
            if inner_dir {
                if inner.starts_with('.') || super::SKIPPED.contains(&inner.as_str()) {
                    continue;
                }
                for (leaf, leaf_dir) in sorted_entries(&root.join(&name).join(&inner)) {
                    if !leaf_dir && is_input_name(&leaf) {
                        found.push(format!("{name}/{inner}/{leaf}"));
                    }
                }
            } else if is_input_name(&inner) {
                found.push(format!("{name}/{inner}"));
            }
        }
    }
    found.sort();
    for (name, is_dir) in sorted_entries(root) {
        if !is_dir
            && is_input_name(&name)
            && super::extension(&name).is_some_and(|e| DATA_EXTENSIONS.contains(&e.as_str()))
        {
            found.push(name);
        }
    }
    found.truncate(MAX_SHIPPED);
    found
        .into_iter()
        .map(|path| format!("{alias}/{path}"))
        .collect()
}

/// The files a usage's stated extensions allow, as [`super::fit`] filters
/// them.
fn allowed<'a>(usage: &super::Usage, files: &'a [String]) -> Vec<&'a String> {
    let wanted: BTreeSet<String> = usage
        .required
        .iter()
        .filter_map(|p| super::extension(p))
        .collect();
    files
        .iter()
        .filter(|f| wanted.is_empty() || super::extension(f).is_some_and(|e| wanted.contains(&e)))
        .collect()
}

/// The argument sets for one program: [`super::arguments`]'s one set,
/// or, when the files don't fit a usage that takes a fixed number of
/// them, one set per file for the last placeholder, at most [`SWEEP`].
/// Every placeholder before the last takes the file whose name shares the
/// most words with it, and must share at least one; otherwise there is no
/// sweep.
#[must_use]
pub fn argument_sets(
    text: &str,
    program: &str,
    files: &[String],
    alias: &str,
) -> Vec<(Vec<String>, String)> {
    let one = super::arguments(text, program, files, alias);
    let Some(usage) = super::usage_of(text, program) else {
        return vec![one];
    };
    if usage.variadic || usage.required.is_empty() || super::fit(&usage, files).is_some() {
        return vec![one];
    }
    let mut left = allowed(&usage, files);
    let count = usage.required.len();
    if left.len() <= count {
        return vec![one];
    }
    let mut fixed: Vec<&String> = Vec::new();
    for placeholder in &usage.required[..count - 1] {
        let want = extract::stem_tokens(placeholder);
        let best = left
            .iter()
            .enumerate()
            .map(|(i, f)| (extract::stem_tokens(f).intersection(&want).count(), i))
            .max_by_key(|(n, i)| (*n, std::cmp::Reverse(*i)));
        match best {
            Some((n, i)) if n > 0 => fixed.push(left.remove(i)),
            _ => return vec![one],
        }
    }
    let total = left.len();
    left.into_iter()
        .take(SWEEP)
        .enumerate()
        .map(|(i, last)| {
            let args: Vec<String> = fixed
                .iter()
                .chain(std::iter::once(&last))
                .map(|f| super::shell_word(&super::relative(f, alias)))
                .collect();
            (
                args,
                format!(
                    "run {} of {} over the {total} shipped or named files its {} doesn't take at \
                     once ({} required)",
                    i + 1,
                    total.min(SWEEP),
                    usage.source,
                    count
                ),
            )
        })
        .collect()
}

/// A package's module name, its directory, and the environment prefix
/// that makes it importable.
struct Package {
    module: String,
    dir: std::path::PathBuf,
    prefix: &'static str,
}

fn find_packages(base: &Path, prefix: &'static str, out: &mut Vec<Package>) {
    let mut stack: Vec<(std::path::PathBuf, String, usize)> = sorted_entries(base)
        .into_iter()
        .filter(|(name, is_dir)| {
            *is_dir
                && !name.starts_with('.')
                && !super::SKIPPED.contains(&name.as_str())
                && name != "src"
                && extract::is_identifier(name)
        })
        .map(|(name, _)| (base.join(&name), name, 1))
        .collect();
    stack.reverse();
    while let Some((dir, module, depth)) = stack.pop() {
        if dir.join("__main__.py").is_file() {
            out.push(Package {
                module: module.clone(),
                dir: dir.clone(),
                prefix,
            });
        }
        if depth >= PACKAGE_DEPTH || !dir.join("__init__.py").is_file() {
            continue;
        }
        let mut inner: Vec<(std::path::PathBuf, String, usize)> = sorted_entries(&dir)
            .into_iter()
            .filter(|(name, is_dir)| {
                *is_dir && !name.starts_with('.') && name != "__pycache__" && {
                    extract::is_identifier(name)
                }
            })
            .map(|(name, _)| (dir.join(&name), format!("{module}.{name}"), depth + 1))
            .collect();
        inner.reverse();
        stack.extend(inner);
    }
}

/// Module entries: every package with a `__main__.py` at the top level,
/// under `src/`, or one level down, not already named.
#[must_use]
pub fn modules(root: &Path, alias: &str, files: &[String], named_text: &str) -> Vec<Entry> {
    let mut packages = Vec::new();
    find_packages(root, "", &mut packages);
    if root.join("src").is_dir() {
        find_packages(&root.join("src"), "PYTHONPATH=src ", &mut packages);
    }
    let mut out = Vec::new();
    for package in packages {
        if named_text.contains(&format!("-m {}", package.module)) {
            continue;
        }
        let text = super::package_text(&package.dir);
        let shown = package
            .dir
            .strip_prefix(root)
            .map_or_else(|_| package.module.clone(), |p| p.display().to_string());
        for (args, how) in argument_sets(&text, &package.module, files, alias) {
            let mut command = format!("{}python3 -m {}", package.prefix, package.module);
            for arg in &args {
                command.push(' ');
                command.push_str(arg);
            }
            out.push(Entry {
                kind: EntryKind::Module,
                command,
                why: format!("{shown}/__main__.py; {how}"),
                refused: None,
            });
        }
    }
    out
}

/// The workspace's Makefile targets and each one's recipe, in file order.
fn make_targets(root: &Path) -> Vec<(String, String)> {
    let Some(text) = ["Makefile", "makefile", "GNUmakefile"]
        .iter()
        .find_map(|name| super::read_text(&root.join(name)))
    else {
        return Vec::new();
    };
    let head = Regex::new(r"^([A-Za-z0-9_.-]+)\s*:([^=]|$)").expect("a valid pattern");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        if let Some(captures) = head.captures(line) {
            out.push((captures[1].to_string(), String::new()));
            current = Some(out.len() - 1);
        } else if line.starts_with('\t') {
            if let Some(at) = current {
                out[at].1.push_str(line.trim());
                out[at].1.push('\n');
            }
        } else if !line.trim().is_empty() && !line.trim_start().starts_with('#') {
            current = None;
        }
    }
    out
}

/// A refusal for a command whose own text or body needs the network.
fn refusal(command: &str, body: &str) -> Option<String> {
    super::needs_network(command)
        .or_else(|| super::needs_network(body))
        .map(|why| format!("it needs the network: {why}"))
}

fn make_entries(root: &Path, named_text: &str) -> Vec<Entry> {
    let targets = make_targets(root);
    let mut out = Vec::new();
    for want in MAKE_TARGETS {
        let Some((_, recipe)) = targets.iter().find(|(name, _)| name == want) else {
            continue;
        };
        let command = format!("make {want}");
        if named_text.contains(&command) {
            continue;
        }
        out.push(Entry {
            kind: EntryKind::Make,
            refused: refusal(&command, recipe),
            command,
            why: format!("the Makefile has a {want} target"),
        });
        if out.len() >= PER_KIND {
            break;
        }
    }
    out
}

/// The package manager a `package.json` project uses, by its lockfile.
fn npm_runner(root: &Path) -> &'static str {
    if root.join("bun.lockb").is_file() || root.join("bun.lock").is_file() {
        "bun run"
    } else if root.join("pnpm-lock.yaml").is_file() {
        "pnpm run"
    } else if root.join("yarn.lock").is_file() {
        "yarn run"
    } else {
        "npm run"
    }
}

fn npm_entries(root: &Path, named_text: &str) -> Vec<Entry> {
    let Some(text) = super::read_text(&root.join("package.json")) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(scripts) = manifest["scripts"].as_object() else {
        return Vec::new();
    };
    let runner = npm_runner(root);
    let mut out = Vec::new();
    for want in NPM_SCRIPTS {
        let Some(body) = scripts.get(*want).and_then(Value::as_str) else {
            continue;
        };
        if body.contains("no test specified") {
            continue;
        }
        let command = format!("{runner} {want}");
        if named_text.contains(&format!("run {want}")) {
            continue;
        }
        out.push(Entry {
            kind: EntryKind::Npm,
            refused: refusal(&command, body),
            command,
            why: format!("package.json's {want} script: {body}"),
        });
        if out.len() >= PER_KIND {
            break;
        }
    }
    out
}

/// The lines of one `[section]` of an INI or TOML file.
fn section<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == format!("[{name}]");
            continue;
        }
        if inside {
            out.push(line);
        }
    }
    out
}

/// Console scripts a project declares: each name and its `module:function`.
fn console_scripts(root: &Path) -> Vec<(String, String, String)> {
    let pair = Regex::new(r#"^\s*["']?([\w.-]+)["']?\s*=\s*["']?([\w.]+)\s*:\s*([\w.]+)["']?\s*$"#)
        .expect("a valid pattern");
    let mut out: Vec<(String, String, String)> = Vec::new();
    let add = |line: &str, out: &mut Vec<(String, String, String)>| {
        if let Some(c) = pair.captures(line)
            && !out.iter().any(|(name, _, _)| name == &c[1])
        {
            out.push((c[1].to_string(), c[2].to_string(), c[3].to_string()));
        }
    };
    if let Some(text) = super::read_text(&root.join("pyproject.toml")) {
        for name in ["project.scripts", "tool.poetry.scripts"] {
            for line in section(&text, name) {
                add(line, &mut out);
            }
        }
    }
    if let Some(text) = super::read_text(&root.join("setup.cfg")) {
        let mut in_console = false;
        for line in section(&text, "options.entry_points") {
            if !line.starts_with([' ', '\t']) {
                in_console = line.trim_start().starts_with("console_scripts");
                if let Some((_, rest)) = line.split_once('=')
                    && in_console
                    && !rest.trim().is_empty()
                {
                    add(rest, &mut out);
                }
                continue;
            }
            if in_console {
                add(line, &mut out);
            }
        }
    }
    if let Some(text) = super::read_text(&root.join("setup.py"))
        && text.contains("console_scripts")
    {
        let quoted =
            Regex::new(r#"["']([\w.-]+\s*=\s*[\w.]+\s*:\s*[\w.]+)["']"#).expect("a valid pattern");
        for c in quoted.captures_iter(&text) {
            add(&c[1], &mut out);
        }
    }
    out
}

/// Where a module's source is: its file and the prefix that makes it
/// importable.
fn module_file(root: &Path, module: &str) -> Option<(std::path::PathBuf, &'static str)> {
    let relative = module.replace('.', "/");
    for (base, prefix) in [
        (root.to_path_buf(), ""),
        (root.join("src"), "PYTHONPATH=src "),
    ] {
        for candidate in [
            base.join(format!("{relative}.py")),
            base.join(&relative).join("__init__.py"),
        ] {
            if candidate.is_file() {
                return Some((candidate, prefix));
            }
        }
    }
    None
}

fn console_entries(root: &Path, alias: &str, files: &[String], named_text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for (name, module, function) in console_scripts(root) {
        if out.len() >= PER_KIND {
            break;
        }
        if named_text
            .split_whitespace()
            .any(|w| w == name || w.ends_with(&format!("/{name}")))
        {
            continue;
        }
        let Some((file, prefix)) = module_file(root, &module) else {
            continue;
        };
        let text = super::read_text(&file).unwrap_or_default();
        let (args, how) = super::arguments(&text, &name, files, alias);
        let mut command = format!(
            "{prefix}python3 -c {}",
            crate::accept::runner::sh_quote(&format!(
                "import sys, {module} as m; sys.argv[0] = '{name}'; sys.exit(m.{function}())"
            ))
        );
        for arg in &args {
            command.push(' ');
            command.push_str(arg);
        }
        out.push(Entry {
            kind: EntryKind::Console,
            command,
            why: format!("the console script {name} = {module}:{function}; {how}"),
            refused: None,
        });
    }
    out
}

/// A Cargo package's binaries: each name and its source file.
fn cargo_binaries(root: &Path) -> Vec<(String, std::path::PathBuf)> {
    let Some(text) = super::read_text(&root.join("Cargo.toml")) else {
        return Vec::new();
    };
    let name = Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#).expect("a valid pattern");
    let path = Regex::new(r#"^\s*path\s*=\s*"([^"]+)""#).expect("a valid pattern");
    let package = section(&text, "package")
        .iter()
        .find_map(|line| name.captures(line).map(|c| c[1].to_string()));
    let Some(package) = package else {
        return Vec::new();
    };
    let mut out: Vec<(String, std::path::PathBuf)> = Vec::new();
    // `[[bin]]` tables.
    let mut inside = false;
    let mut current: Option<(Option<String>, Option<String>)> = None;
    let flush = |current: &mut Option<(Option<String>, Option<String>)>,
                 out: &mut Vec<(String, std::path::PathBuf)>| {
        if let Some((Some(bin), file)) = current.take() {
            let file = file.map_or_else(
                || root.join("src/bin").join(format!("{bin}.rs")),
                |f| root.join(f),
            );
            out.push((bin, file));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            flush(&mut current, &mut out);
            inside = trimmed == "[[bin]]";
            if inside {
                current = Some((None, None));
            }
            continue;
        }
        if let Some(entry) = current.as_mut().filter(|_| inside) {
            if let Some(c) = name.captures(line) {
                entry.0 = Some(c[1].to_string());
            } else if let Some(c) = path.captures(line) {
                entry.1 = Some(c[1].to_string());
            }
        }
    }
    flush(&mut current, &mut out);
    if root.join("src/main.rs").is_file() && !out.iter().any(|(n, _)| n == &package) {
        out.push((package, root.join("src/main.rs")));
    }
    for (file, is_dir) in sorted_entries(&root.join("src/bin")) {
        if let Some(stem) = file.strip_suffix(".rs").filter(|_| !is_dir)
            && !out.iter().any(|(n, _)| n == stem)
        {
            out.push((stem.to_string(), root.join("src/bin").join(&file)));
        }
    }
    out
}

fn cargo_entries(root: &Path, alias: &str, files: &[String], named_text: &str) -> Vec<Entry> {
    let binaries = cargo_binaries(root);
    let mut out = Vec::new();
    for (bin, file) in &binaries {
        if out.len() >= PER_KIND || named_text.contains("cargo run") {
            break;
        }
        let text = super::read_text(file).unwrap_or_default();
        let (args, how) = super::arguments(&text, bin, files, alias);
        let mut command = "cargo run --offline -q".to_string();
        if binaries.len() > 1 {
            command.push_str(&format!(" --bin {}", super::shell_word(bin)));
        }
        if !args.is_empty() {
            command.push_str(" --");
            for arg in &args {
                command.push(' ');
                command.push_str(arg);
            }
        }
        out.push(Entry {
            kind: EntryKind::Cargo,
            command,
            why: format!("Cargo.toml's binary {bin}; {how}"),
            refused: None,
        });
    }
    out
}

/// Make, console, npm, and Cargo entries, in that order.
#[must_use]
pub fn others(root: &Path, alias: &str, files: &[String], named_text: &str) -> Vec<Entry> {
    let mut out = make_entries(root, named_text);
    out.extend(console_entries(root, alias, files, named_text));
    out.extend(npm_entries(root, named_text));
    out.extend(cargo_entries(root, alias, files, named_text));
    out
}
