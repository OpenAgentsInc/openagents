//! Repository map guest.
//!
//! The `map` operation lists a granted snapshot and returns its shape: how
//! many files and bytes it holds, which languages they are in, the
//! top-level entries and the directories one level below them, the largest
//! files, the build manifests, and the test files. It reads no file
//! contents, only the listing and each file's size.
//!
//! A size is what the snapshot holds, which the host may cap per file, so a
//! file the host cut reports its captured size. The guest says when its own
//! bounds stopped it: `complete` is false when the listing hit `max_files`
//! or a size couldn't be read.
//!
//! This reimplements the pre-reset `repo-map` plugin's purpose on the
//! `openagents.plugin-packet.v1` ABI. It keeps the shape and drops the
//! symbol outline, which read every file.

use std::collections::BTreeMap;

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Value, json};

plugin_pdk::export_guest!(handle);

/// The most files one map lists by default.
const DEFAULT_MAX_FILES: usize = 5_000;
/// The most files one map may be asked to list.
const MAX_FILES_CAP: usize = 20_000;
/// Rows each ranked list keeps.
const LANGUAGES: usize = 12;
const TOP: usize = 40;
const DIRS: usize = 30;
const LARGEST: usize = 10;
const MANIFESTS: usize = 30;
const TEST_DIRS: usize = 10;

/// Files that say how a project builds, by name.
const MANIFEST_NAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "go.mod",
    "Makefile",
    "CMakeLists.txt",
    "build.gradle",
    "pom.xml",
    "Gemfile",
    "mix.exs",
    "composer.json",
    "Dockerfile",
    "flake.nix",
    "tox.ini",
    "pytest.ini",
];

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "map" => map(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// One file in the map.
struct File {
    path: String,
    bytes: u64,
}

fn map(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let root = guest::root(request).ok_or_else(|| Refusal::refused("no granted snapshot"))?;
    let max_files = request.input["max_files"]
        .as_u64()
        .map_or(DEFAULT_MAX_FILES, |n| n as usize)
        .clamp(1, MAX_FILES_CAP);
    let listing = guest::list(host, &root, max_files)
        .map_err(|code| Refusal::refused(format!("list {code}")))?;
    let mut complete = listing.complete;
    let mut files = Vec::new();
    for entry in listing.entries.iter().filter(|entry| entry.kind == "file") {
        let bytes = match guest::size(host, &entry.handle) {
            Ok(bytes) => bytes,
            Err(_) => {
                complete = false;
                0
            }
        };
        files.push(File {
            path: guest::relative(&entry.name).to_string(),
            bytes,
        });
    }
    Ok(summarize(&files, complete))
}

/// Totals for a group of files.
#[derive(Default, Clone, Copy)]
struct Tally {
    files: u64,
    bytes: u64,
}

impl Tally {
    fn add(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes += bytes;
    }
}

fn summarize(files: &[File], complete: bool) -> Value {
    let mut total = Tally::default();
    let mut languages: BTreeMap<&'static str, Tally> = BTreeMap::new();
    // Top-level entry to (is a directory, tally).
    let mut top: BTreeMap<&str, (bool, Tally)> = BTreeMap::new();
    let mut dirs: BTreeMap<String, Tally> = BTreeMap::new();
    let mut manifests = Vec::new();
    let mut tests = Tally::default();
    let mut test_dirs: BTreeMap<String, u64> = BTreeMap::new();
    for file in files {
        total.add(file.bytes);
        languages
            .entry(language(&file.path))
            .or_default()
            .add(file.bytes);
        let segments: Vec<&str> = file.path.split('/').collect();
        top.entry(segments[0])
            .or_insert((segments.len() > 1, Tally::default()))
            .1
            .add(file.bytes);
        if segments.len() > 2 {
            dirs.entry(format!("{}/{}", segments[0], segments[1]))
                .or_default()
                .add(file.bytes);
        }
        let name = segments.last().copied().unwrap_or_default();
        if MANIFEST_NAMES.contains(&name) && manifests.len() < MANIFESTS {
            manifests.push(file.path.clone());
        }
        if is_test(&segments) {
            tests.add(file.bytes);
            let dir = segments[..segments.len() - 1].join("/");
            *test_dirs
                .entry(if dir.is_empty() { ".".into() } else { dir })
                .or_default() += 1;
        }
    }

    let mut languages: Vec<(&str, Tally)> = languages.into_iter().collect();
    languages.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes).then(a.0.cmp(b.0)));
    let mut dirs: Vec<(String, Tally)> = dirs.into_iter().collect();
    dirs.sort_by(|a, b| b.1.files.cmp(&a.1.files).then(a.0.cmp(&b.0)));
    let mut largest: Vec<&File> = files.iter().collect();
    largest.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.path.cmp(&b.path)));
    let mut test_dirs: Vec<(String, u64)> = test_dirs.into_iter().collect();
    test_dirs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let omitted_top = top.len().saturating_sub(TOP);
    json!({
        "kind": "repo-map",
        "files": total.files,
        "bytes": total.bytes,
        "complete": complete,
        "languages": languages.iter().take(LANGUAGES).map(|(language, tally)| json!({
            "language": language,
            "files": tally.files,
            "bytes": tally.bytes,
        })).collect::<Vec<_>>(),
        "top": top.iter().take(TOP).map(|(path, (dir, tally))| json!({
            "path": path,
            "kind": if *dir { "dir" } else { "file" },
            "files": tally.files,
            "bytes": tally.bytes,
        })).collect::<Vec<_>>(),
        "top_omitted": omitted_top,
        "dirs": dirs.iter().take(DIRS).map(|(path, tally)| json!({
            "path": path,
            "files": tally.files,
            "bytes": tally.bytes,
        })).collect::<Vec<_>>(),
        "largest": largest.iter().take(LARGEST).map(|file| json!({
            "path": file.path,
            "bytes": file.bytes,
        })).collect::<Vec<_>>(),
        "manifests": manifests,
        "tests": {
            "files": tests.files,
            "dirs": test_dirs.iter().take(TEST_DIRS).map(|(dir, files)| json!({
                "path": dir,
                "files": files,
            })).collect::<Vec<_>>(),
        },
    })
}

/// Whether a path looks like a test file: under a test directory, or named
/// as one.
fn is_test(segments: &[&str]) -> bool {
    let (name, dirs) = segments.split_last().unwrap_or((&"", &[]));
    let dir_says = dirs
        .iter()
        .any(|dir| matches!(*dir, "tests" | "test" | "__tests__" | "spec" | "testdata"));
    let stem = name.split('.').next().unwrap_or_default();
    let name_says = stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || name.contains(".test.")
        || name.contains(".spec.");
    dir_says || name_says
}

/// The language a path's extension or name says it is in.
fn language(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => return "Make",
        "Dockerfile" => return "Docker",
        "CMakeLists.txt" => return "CMake",
        _ => {}
    }
    let Some((_, extension)) = name.rsplit_once('.') else {
        return "Other";
    };
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "Rust",
        "py" | "pyi" => "Python",
        "js" | "mjs" | "cjs" | "jsx" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "cs" => "C#",
        "rb" => "Ruby",
        "php" => "PHP",
        "swift" => "Swift",
        "scala" => "Scala",
        "sh" | "bash" | "zsh" => "Shell",
        "md" | "markdown" => "Markdown",
        "rst" => "reStructuredText",
        "json" | "jsonl" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "html" | "htm" => "HTML",
        "css" | "scss" => "CSS",
        "sql" => "SQL",
        "lua" => "Lua",
        "ex" | "exs" => "Elixir",
        "hs" => "Haskell",
        "ml" | "mli" => "OCaml",
        "r" => "R",
        "jl" => "Julia",
        "zig" => "Zig",
        "nix" => "Nix",
        "xml" => "XML",
        "txt" => "Text",
        "wasm" => "WebAssembly",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn fixture() -> MemoryHost {
        MemoryHost::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree"))
    }

    #[test]
    fn the_map_counts_languages_structure_and_tests() {
        let mut host = fixture();
        let value = handle(&MemoryHost::request("map", json!({})), &mut host).unwrap();
        assert_eq!(value["kind"], "repo-map");
        assert_eq!(value["files"], 9);
        assert_eq!(value["complete"], true);
        assert_eq!(value["languages"][0]["language"], "Python");
        assert_eq!(value["languages"][0]["files"], 4);
        let top: Vec<&str> = value["top"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            top,
            [
                "README.md",
                "docs",
                "pyproject.toml",
                "scripts",
                "src",
                "tests",
                "web"
            ]
        );
        assert_eq!(value["top"][4]["kind"], "dir");
        assert_eq!(value["top"][4]["files"], 3);
        assert_eq!(value["dirs"][0]["path"], "src/app");
        assert_eq!(value["manifests"], json!(["pyproject.toml"]));
        assert_eq!(value["tests"]["files"], 1);
        assert_eq!(value["tests"]["dirs"][0]["path"], "tests");
        assert_eq!(value["largest"][0]["path"], "src/app/core.py");
    }

    #[test]
    fn a_bounded_listing_says_it_is_incomplete() {
        let mut host = fixture();
        let value = handle(
            &MemoryHost::request("map", json!({"max_files": 3})),
            &mut host,
        )
        .unwrap();
        assert_eq!(value["files"], 3);
        assert_eq!(value["complete"], false);
    }

    #[test]
    fn an_unknown_operation_is_unsupported() {
        let mut host = fixture();
        let refused = handle(&MemoryHost::request("outline", json!({})), &mut host).unwrap_err();
        assert_eq!(refused.status, "unsupported_input");
    }

    #[test]
    fn languages_come_from_extensions_and_names() {
        assert_eq!(language("src/lib.rs"), "Rust");
        assert_eq!(language("a/Makefile"), "Make");
        assert_eq!(language("web/app.TSX"), "TypeScript");
        assert_eq!(language("LICENSE"), "Other");
        assert!(is_test(&["pkg", "foo_test.go"]));
        assert!(is_test(&["tests", "it.rs"]));
        assert!(!is_test(&["src", "contest.rs"]));
    }
}
