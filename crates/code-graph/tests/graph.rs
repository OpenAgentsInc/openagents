//! The code graph on a small two-crate repository: the same commit gives
//! the same bytes, the queries answer from the graph, an ambiguous name
//! returns candidates, and a published graph reaches a clone.

use std::path::Path;
use std::process::Command;

use code_graph::build::{Source, build};
use code_graph::model::{EdgeKind, Graph, Kind};
use code_graph::query::{Found, Index};
use code_graph::store::{self, Origin};
use code_graph::tools;
use serde_json::json;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, text).unwrap();
}

/// Two crates: `core` defines a store and a parser; `app` calls into it.
fn fixture(root: &Path) {
    git(root, &["init", "-q"]);
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n",
    );
    write(
        root,
        "crates/core/Cargo.toml",
        "[package]\nname = \"demo-core\"\nversion = \"0.1.0\"\n",
    );
    write(
        root,
        "crates/core/src/lib.rs",
        "pub mod store;\npub mod parse;\n\npub trait Named {\n    fn name(&self) -> String;\n}\n",
    );
    write(
        root,
        "crates/core/src/store.rs",
        "use crate::parse::parse_line;\nuse crate::Named;\n\npub struct Store {\n    rows: Vec<Row>,\n}\n\npub struct Row(pub String);\n\nimpl Store {\n    pub fn new() -> Self {\n        Store { rows: Vec::new() }\n    }\n\n    pub fn load(&mut self, text: &str) {\n        for line in text.lines() {\n            self.push(parse_line(line));\n        }\n    }\n\n    fn push(&mut self, row: Row) {\n        self.rows.push(row);\n    }\n}\n\nimpl Named for Store {\n    fn name(&self) -> String {\n        format!(\"store of {}\", count(self))\n    }\n}\n\nfn count(store: &Store) -> usize {\n    store.rows.len()\n}\n",
    );
    write(
        root,
        "crates/core/src/parse.rs",
        "use crate::store::Row;\n\n/// Parse one line.\npub fn parse_line(line: &str) -> Row {\n    Row(clean(line))\n}\n\nfn clean(line: &str) -> String {\n    line.trim().to_owned()\n}\n\npub fn new() {}\n",
    );
    write(
        root,
        "crates/app/Cargo.toml",
        "[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\n",
    );
    write(
        root,
        "crates/app/src/main.rs",
        "use demo_core::store::Store;\n\nfn main() {\n    let mut store = Store::new();\n    store.load(\"a\\nb\");\n    report(&store);\n}\n\nfn report(store: &Store) {\n    let _ = store;\n}\n",
    );
    write(root, "docs/notes.rs", "fn not_code() {}\n");
    write(root, "target/debug/build.rs", "fn not_code_either() {}\n");
    write(
        root,
        "crates/core/src/generated.rs",
        "// @generated\nfn generated_code() {}\n",
    );
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "fixture"]);
}

fn built(root: &Path) -> Graph {
    build(&Source::Commit {
        root: root.to_path_buf(),
        rev: "HEAD".into(),
    })
    .unwrap()
    .graph
}

#[test]
fn the_same_commit_gives_the_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let first = built(dir.path()).to_bytes();
    let second = built(dir.path()).to_bytes();
    assert_eq!(first, second);
    // A clean working tree builds to the same bytes as its commit.
    let worktree = build(&Source::Worktree {
        root: dir.path().to_path_buf(),
    })
    .unwrap()
    .graph
    .to_bytes();
    assert_eq!(first, worktree);
    // Round trip.
    let read = Graph::read(first.as_slice()).unwrap();
    assert_eq!(read.to_bytes(), first);
    let header = &read.header;
    assert_eq!(header.parser, "tree-sitter 0.27.0");
    assert_eq!(header.grammar, "tree-sitter-rust 0.24.2");
    assert_eq!(header.commit, git(dir.path(), &["rev-parse", "HEAD"]));
    assert!(!header.dirty);
}

#[test]
fn code_only_and_every_node_has_a_place() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let graph = built(dir.path());
    assert_eq!(
        graph.files,
        vec![
            "crates/app/src/main.rs",
            "crates/core/src/lib.rs",
            "crates/core/src/parse.rs",
            "crates/core/src/store.rs",
        ]
    );
    let index = Index::new(graph);
    let found = |symbol: &str| match index.find(symbol, None) {
        Found::One(id) => index.graph.nodes[id as usize].clone(),
        other => panic!("{symbol}: {other:?}"),
    };
    let load = found("demo_core::store::Store::load");
    assert_eq!(load.kind, Kind::Method);
    assert_eq!(
        index.graph.files[load.file as usize],
        "crates/core/src/store.rs"
    );
    assert_eq!((load.line, load.end_line), (15, 19));
    assert_eq!(load.signature, "pub fn load(&mut self, text: &str)");
    assert_eq!(found("Named").kind, Kind::Trait);
    assert_eq!(found("demo_core::store").kind, Kind::Module);
    assert_eq!(found("crates/core/src/parse.rs:5").name, "parse_line");
}

#[test]
fn edges_follow_calls_imports_impls_and_types() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let index = Index::new(built(dir.path()));
    let id = |symbol: &str| index.resolve(symbol, None).unwrap();
    let has = |from: &str, to: &str, kind: EdgeKind| {
        let (from, to) = (id(from), id(to));
        index
            .graph
            .edges
            .iter()
            .any(|e| e.from == from && e.to == to && e.kind == kind)
    };
    // Across crates, through a `use`.
    assert!(has("demo_app::main", "Store::new", EdgeKind::Calls));
    // A method call on a local: matched by name.
    assert!(has("demo_app::main", "Store::load", EdgeKind::Calls));
    // `self.push` is pinned to the impl's own method.
    assert!(has("Store::load", "Store::push", EdgeKind::Calls));
    // Through an import.
    assert!(has("Store::load", "parse_line", EdgeKind::Calls));
    // Inside a macro's arguments.
    assert!(has(
        "demo_core::store::Store::name",
        "demo_core::store::count",
        EdgeKind::Calls
    ));
    // Type references and a tuple-struct constructor.
    assert!(has("demo_app::report", "Store", EdgeKind::References));
    assert!(has("parse_line", "Row", EdgeKind::References));
    // Imports and impls.
    assert!(has("demo_core::parse", "Row", EdgeKind::Uses));
    let impls: Vec<_> = index
        .graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Implements && e.to == id("Named"))
        .collect();
    assert_eq!(impls.len(), 1);
}

#[test]
fn the_queries_answer_in_file_line_rows() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let index = Index::new(built(dir.path()));
    let callers = index.callers("parse_line", 40).unwrap();
    assert!(
        callers.contains("crates/core/src/store.rs:15  demo_core::store::Store::load [calls]"),
        "{callers}"
    );
    let callees = index.callees("Store::load", 40).unwrap();
    assert!(
        callees.contains("demo_core::parse::parse_line"),
        "{callees}"
    );
    assert!(
        callees.contains("demo_core::store::Store::push"),
        "{callees}"
    );
    // Changing `clean` reaches `parse_line`, then `Store::load`, then
    // `main` in the other crate.
    let impact = index.impact("clean", 3, 40, false).unwrap();
    assert!(
        impact.contains("d1 crates/core/src/parse.rs:4  demo_core::parse::parse_line"),
        "{impact}"
    );
    assert!(
        impact.contains("d2 crates/core/src/store.rs:15  demo_core::store::Store::load"),
        "{impact}"
    );
    assert!(
        impact.contains("d3 crates/app/src/main.rs:3  demo_app::main"),
        "{impact}"
    );
    assert!(!index.impact("clean", 1, 40, false).unwrap().contains("d2 "));
    let path = index.path("demo_app::main", "clean", 8).unwrap();
    assert!(path.starts_with("path, 3 steps:"), "{path}");
    let symbol = index.symbol("Store", None).unwrap();
    assert!(symbol.contains("crates/core/src/store.rs:4-6"), "{symbol}");
    assert!(symbol.contains("impl Named for Store: name"), "{symbol}");
    let module = index.module("demo_core::parse", 40).unwrap();
    assert!(
        module.contains("4  pub fn parse_line(line: &str) -> Row"),
        "{module}"
    );
    assert!(module.contains("(2 public items, 1 private)"), "{module}");
    assert!(!module.contains("clean"), "{module}");
    let root = index.module("crates/core/src/lib.rs", 40).unwrap();
    assert!(
        root.contains(
            "submodules: parse (crates/core/src/parse.rs), store (crates/core/src/store.rs)"
        ),
        "{root}"
    );
}

#[test]
fn an_ambiguous_name_returns_candidates() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let index = Index::new(built(dir.path()));
    let answer = index.callers("new", 40).unwrap_err();
    assert!(
        answer.starts_with("ambiguous: 2 symbols match `new`"),
        "{answer}"
    );
    assert!(
        answer.contains("demo_core::parse::new (fn) crates/core/src/parse.rs:12"),
        "{answer}"
    );
    assert!(
        answer.contains("demo_core::store::Store::new (method) crates/core/src/store.rs:11"),
        "{answer}"
    );
    // A qualified name picks one.
    assert!(index.callers("Store::new", 40).is_ok());
    // A kind narrows too.
    assert!(index.symbol("new", Some(Kind::Function)).is_ok());
    assert!(
        index
            .symbol("nothing_here", None)
            .unwrap_err()
            .starts_with("No symbol")
    );
}

#[test]
fn tools_build_locally_then_read_the_published_graph() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    fixture(&repo);
    // Uncommitted work is built from the working tree.
    write(
        &repo,
        "crates/core/src/parse.rs",
        "use crate::store::Row;\n\npub fn parse_line(line: &str) -> Row {\n    Row(fresh(line))\n}\n\nfn fresh(line: &str) -> String {\n    line.to_owned()\n}\n",
    );
    let answer = tools::execute(tools::CALLERS, &json!({"symbol": "fresh"}), &repo).unwrap();
    assert!(
        answer["graph"].as_str().unwrap().contains("uncommitted"),
        "{answer}"
    );
    assert!(
        answer["answer"].as_str().unwrap().contains("parse_line"),
        "{answer}"
    );
    git(&repo, &["checkout", "--", "."]);

    // Publish to a bare remote; a clone reads the branch instead of building.
    let remote = dir.path().join("remote.git");
    git(
        dir.path(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    let published = store::publish(&repo, "HEAD", Some("origin"), false).unwrap();
    assert!(published.branch_commit.is_some());
    // Nothing changed: the publisher skips.
    let again = store::publish(&repo, "HEAD", Some("origin"), false).unwrap();
    assert!(again.branch_commit.is_none());
    // `main` never holds the graph.
    let main_files = git(&remote, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!main_files.contains("code-graph"), "{main_files}");
    let branch_files = git(&remote, &["ls-tree", "-r", "--name-only", "code-graph"]);
    assert_eq!(branch_files, "COMMIT\nREADME.md\ncode-graph.jsonl.gz");

    let clone = dir.path().join("clone");
    git(
        dir.path(),
        &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let opened = store::open(&clone).unwrap();
    assert_eq!(opened.origin, Origin::Published);
    assert!(!opened.dirty);
    let local = built(&repo).to_bytes();
    assert_eq!(opened.index.graph.to_bytes(), local);
    // The second open comes from memory.
    assert_eq!(store::open(&clone).unwrap().origin, Origin::Memory);
}
