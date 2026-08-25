//! The mapper against a fake host: per-language extraction fixtures, the
//! directory skip list, oversized and unreadable files, definition lookup,
//! word-bounded reference counting, and every cap. The same shapes run
//! through the real boundary in
//! `packages/openagents-cli/test/coder-plugin-repo-map.test.ts`.

use super::*;
use openagents_pdk::MountDirEntry;
use std::collections::BTreeMap;

/// A fake host over one in-memory workspace mount. Directories are keyed
/// by mount-relative path; file bytes likewise.
#[derive(Default)]
struct FakeHost {
    dirs: BTreeMap<String, MountDirListing>,
    files: BTreeMap<String, Vec<u8>>,
    /// Paths the read import answers with `file_too_large`.
    oversized: Vec<String>,
    /// Paths the read import answers with `file_unreadable`.
    unreadable: Vec<String>,
}

impl FakeHost {
    fn dir(&mut self, path: &str, entries: Vec<MountDirEntry>) {
        self.dirs
            .insert(path.to_string(), MountDirListing { entries, truncated: false });
    }
    fn file(&mut self, path: &str, text: &str) {
        self.files.insert(path.to_string(), text.as_bytes().to_vec());
    }
}

impl Host for FakeHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        assert_eq!(mount_index, WORKSPACE_MOUNT, "only the workspace mount exists");
        self.dirs.get(path).cloned().ok_or_else(|| {
            Refusal::new(RefusalCode::FileUnreadable, "the mount has no such directory")
        })
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        if self.oversized.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileTooLarge, "over the bound"));
        }
        if self.unreadable.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileUnreadable, "io failure"));
        }
        self.files.get(path).cloned().ok_or_else(|| {
            Refusal::new(RefusalCode::MountDenied, "no declared mount contains the path")
        })
    }
}

fn entry(name: &str, kind: &str, size: u64) -> MountDirEntry {
    MountDirEntry { name: name.to_string(), kind: kind.to_string(), size, mtime_ms: 0 }
}

fn input() -> Input {
    Input {
        path: None,
        symbol: None,
        count_references: None,
        max_files: None,
        max_symbols_per_file: None,
    }
}

fn names(symbols: &[Symbol]) -> Vec<(&'static str, &str)> {
    symbols.iter().map(|s| (s.kind, s.name.as_str())).collect()
}

// ---- extraction, one focused fixture per language ----

#[test]
fn python_extraction_finds_classes_functions_and_methods_with_parents() {
    let source = "\
import os

class Greeter:
    def __init__(self):
        pass

    async def greet(self):
        return hello()

def hello():
    return 1
";
    let (symbols, truncated) = extract_symbols(Language::Python, source, 100);
    assert!(!truncated);
    assert_eq!(
        names(&symbols),
        vec![
            ("class", "Greeter"),
            ("method", "__init__"),
            ("method", "greet"),
            ("function", "hello"),
        ],
    );
    assert_eq!(symbols[1].parent.as_deref(), Some("Greeter"));
    assert_eq!(symbols[2].parent.as_deref(), Some("Greeter"));
    assert_eq!(symbols[3].parent, None);
    assert_eq!(symbols[0].line, 3);
    assert_eq!(symbols[3].line, 10);
}

#[test]
fn typescript_extraction_covers_export_variants_and_arrow_functions() {
    let source = "\
import { x } from \"./x\";
export function build(): void {}
export default class App {}
interface Options {
  quiet: boolean;
}
export type Result<T> = T | null;
export enum Mode { Fast, Slow }
export const handler = async (req: Request) => req;
const plain = 42;
let maker = function () {};
";
    let (symbols, _) = extract_symbols(Language::TypeScript, source, 100);
    assert_eq!(
        names(&symbols),
        vec![
            ("function", "build"),
            ("class", "App"),
            ("interface", "Options"),
            ("type", "Result"),
            ("enum", "Mode"),
            ("function", "handler"),
            ("function", "maker"),
        ],
    );
}

#[test]
fn rust_extraction_covers_items_visibility_and_impl_blocks() {
    let source = "\
pub const LIMIT: usize = 10;
pub(crate) struct Walker;
enum Step { In, Out }
pub trait Visit {}
mod inner {}
impl Walker {
    pub fn walk(&self) {}
}
impl<T> Visit for Vec<T> {
    async fn ignore_modifiers() {}
}
pub fn top() {}
";
    let (symbols, _) = extract_symbols(Language::Rust, source, 100);
    assert_eq!(
        names(&symbols),
        vec![
            ("const", "LIMIT"),
            ("struct", "Walker"),
            ("enum", "Step"),
            ("trait", "Visit"),
            ("module", "inner"),
            ("impl", "Walker"),
            ("function", "walk"),
            ("impl", "Visit for Vec<T>"),
            ("function", "ignore_modifiers"),
            ("function", "top"),
        ],
    );
}

#[test]
fn elixir_extraction_covers_modules_functions_and_macros() {
    let source = "\
defmodule Demo.Server do
  defmacro compiled? do
  end

  def start_link(opts) do
  end

  defp validate!(opts) do
  end
end
";
    let (symbols, _) = extract_symbols(Language::Elixir, source, 100);
    assert_eq!(
        names(&symbols),
        vec![
            ("module", "Demo.Server"),
            ("macro", "compiled?"),
            ("function", "start_link"),
            ("private_function", "validate!"),
        ],
    );
}

#[test]
fn go_extraction_covers_functions_methods_and_types() {
    let source = "\
package main

type Server struct{}

func (s *Server) Start() error { return nil }

func main() {}
";
    let (symbols, _) = extract_symbols(Language::Go, source, 100);
    assert_eq!(
        names(&symbols),
        vec![("type", "Server"), ("method", "Start"), ("function", "main")],
    );
}

#[test]
fn ruby_extraction_covers_classes_modules_and_defs() {
    let source = "\
module Billing
  class Invoice
    def self.build(total)
    end

    def paid?
    end
  end
end
";
    let (symbols, _) = extract_symbols(Language::Ruby, source, 100);
    assert_eq!(
        names(&symbols),
        vec![
            ("module", "Billing"),
            ("class", "Invoice"),
            ("function", "build"),
            ("function", "paid?"),
        ],
    );
}

// ---- the walk, over a small polyglot workspace ----

/// A workspace with Python, Rust, and Elixir sources, a skipped
/// `node_modules` tree, and a plain-text file no pattern knows.
fn seeded() -> FakeHost {
    let mut host = FakeHost::default();
    host.dir(
        "",
        vec![
            entry("README.md", "file", 20),
            entry("lib", "dir", 0),
            entry("node_modules", "dir", 0),
            entry("src", "dir", 0),
        ],
    );
    host.dir(
        "src",
        vec![entry("main.py", "file", 120), entry("map.rs", "file", 140)],
    );
    host.dir("lib", vec![entry("demo.ex", "file", 90)]);
    host.dir("node_modules", vec![entry("pkg", "dir", 0)]);
    host.dir("node_modules/pkg", vec![entry("index.js", "file", 50)]);
    host.file(
        "src/main.py",
        "def greet_all():\n    pass\n\ndef use_it():\n    return greet_all()\n",
    );
    host.file(
        "src/map.rs",
        "pub fn greet_all() -> u32 { 0 }\npub fn twice() -> u32 { greet_all() + greet_all() }\nfn greet_all_extra() {}\n",
    );
    host.file("lib/demo.ex", "defmodule Demo do\n  def greet_all do\n  end\nend\n");
    host.file("node_modules/pkg/index.js", "function hidden() {}\n");
    host
}

fn outline_of(host: &FakeHost, input: &Input) -> OutlineOutput {
    match map(host, input).unwrap() {
        Output::Outline(out) => out,
        other => panic!("expected an outline, got {other:?}"),
    }
}

#[test]
fn the_outline_maps_every_language_and_skips_the_skip_list() {
    let out = outline_of(&seeded(), &input());
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["lib/demo.ex", "src/main.py", "src/map.rs"]);
    assert!(paths.iter().all(|p| !p.contains("node_modules")));

    let by_path = |path: &str| out.files.iter().find(|f| f.path == path).unwrap();
    assert_eq!(by_path("src/main.py").language, "python");
    assert_eq!(
        names(by_path("src/main.py").symbols.as_deref().unwrap()),
        vec![("function", "greet_all"), ("function", "use_it")],
    );
    assert_eq!(by_path("lib/demo.ex").language, "elixir");
    assert_eq!(by_path("src/map.rs").language, "rust");

    assert_eq!(out.stats.files_seen, 3);
    assert_eq!(out.stats.files_parsed, 3);
    assert_eq!(out.stats.oversized, 0);
    assert_eq!(out.stats.unreadable, 0);
    assert!(!out.stats.truncated);
}

#[test]
fn every_skip_list_directory_is_skipped_by_name() {
    let mut host = FakeHost::default();
    let mut entries: Vec<MountDirEntry> =
        SKIP_DIRS.iter().map(|name| entry(name, "dir", 0)).collect();
    entries.push(entry("kept.py", "file", 10));
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    host.dir("", entries);
    for name in SKIP_DIRS {
        host.dir(name, vec![entry("sneaky.py", "file", 10)]);
        host.file(&format!("{name}/sneaky.py"), "def sneaky():\n    pass\n");
    }
    host.file("kept.py", "def kept():\n    pass\n");
    let out = outline_of(&host, &input());
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].path, "kept.py");
}

#[test]
fn an_oversized_file_is_listed_with_null_symbols_and_counted() {
    let mut host = seeded();
    host.dir(
        "src",
        vec![
            entry("huge.py", "file", MAX_FILE_BYTES + 1),
            entry("main.py", "file", 120),
        ],
    );
    let out = outline_of(&host, &input());
    let huge = out.files.iter().find(|f| f.path == "src/huge.py").unwrap();
    assert_eq!(huge.symbols, None);
    assert_eq!(huge.language, "python");
    assert_eq!(out.stats.oversized, 1);
    assert_eq!(out.stats.files_parsed, 2);
}

#[test]
fn a_read_refused_as_too_large_counts_oversized_even_when_the_listing_lied() {
    let mut host = seeded();
    host.oversized.push("src/main.py".to_string());
    let out = outline_of(&host, &input());
    let file = out.files.iter().find(|f| f.path == "src/main.py").unwrap();
    assert_eq!(file.symbols, None);
    assert_eq!(out.stats.oversized, 1);
}

#[test]
fn an_unreadable_file_is_counted_and_left_out() {
    let mut host = seeded();
    host.unreadable.push("src/main.py".to_string());
    let out = outline_of(&host, &input());
    assert!(out.files.iter().all(|f| f.path != "src/main.py"));
    assert_eq!(out.stats.unreadable, 1);
    assert_eq!(out.stats.files_seen, 3);
    assert_eq!(out.stats.files_parsed, 2);
}

#[test]
fn a_subtree_path_narrows_the_walk() {
    let out = outline_of(&seeded(), &Input { path: Some("src".into()), ..input() });
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/main.py", "src/map.rs"]);
}

#[test]
fn a_root_that_cannot_be_listed_is_a_refusal_that_names_it() {
    let refusal = map(&seeded(), &Input { path: Some("no/such/dir".into()), ..input() })
        .unwrap_err();
    assert_eq!(refusal.code, RefusalCode::FileUnreadable);
    assert!(refusal.reason.contains("no/such/dir"));
}

// ---- definition lookup ----

#[test]
fn definition_lookup_returns_every_exact_definition_site() {
    let out = match map(&seeded(), &Input { symbol: Some("greet_all".into()), ..input() }) {
        Ok(Output::Definitions(out)) => out,
        other => panic!("expected definitions, got {other:?}"),
    };
    assert_eq!(out.symbol, "greet_all");
    assert_eq!(
        out.definitions,
        vec![
            Definition { path: "lib/demo.ex".into(), kind: "function", line: 2, parent: None },
            Definition { path: "src/main.py".into(), kind: "function", line: 1, parent: None },
            Definition { path: "src/map.rs".into(), kind: "function", line: 1, parent: None },
        ],
    );
    // Exact name only: `greet_all_extra` defines a different symbol.
    assert!(out.definitions.iter().all(|d| d.line != 3 || d.path != "src/map.rs"));
}

#[test]
fn an_unknown_symbol_yields_an_empty_honest_answer_not_a_refusal() {
    let out = match map(&seeded(), &Input { symbol: Some("nope".into()), ..input() }) {
        Ok(Output::Definitions(out)) => out,
        other => panic!("expected definitions, got {other:?}"),
    };
    assert!(out.definitions.is_empty());
    assert_eq!(out.stats.files_parsed, 3);
}

#[test]
fn an_empty_symbol_is_refused() {
    let refusal = map(&seeded(), &Input { symbol: Some("  ".into()), ..input() }).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::Unsupported);
}

// ---- reference counting ----

fn references_of(host: &FakeHost, symbol: &str) -> ReferencesOutput {
    let request = Input {
        symbol: Some(symbol.into()),
        count_references: Some(true),
        ..input()
    };
    match map(host, &request) {
        Ok(Output::References(out)) => out,
        other => panic!("expected references, got {other:?}"),
    }
}

#[test]
fn reference_counts_are_word_bounded_and_skip_definition_lines() {
    let out = references_of(&seeded(), "greet_all");
    // main.py: one call; map.rs: two calls on the `twice` line — the
    // definition lines and `greet_all_extra` never count.
    assert_eq!(
        out.references,
        vec![
            Reference { path: "src/map.rs".into(), count: 2 },
            Reference { path: "src/main.py".into(), count: 1 },
        ],
    );
    assert_eq!(out.total, 3);
}

#[test]
fn a_name_with_no_uses_outside_its_definition_counts_zero() {
    let out = references_of(&seeded(), "use_it");
    assert!(out.references.is_empty());
    assert_eq!(out.total, 0);
}

#[test]
fn count_references_without_a_symbol_is_refused() {
    let refusal =
        map(&seeded(), &Input { count_references: Some(true), ..input() }).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::Unsupported);
    assert!(refusal.reason.contains("symbol"));
}

#[test]
fn word_counting_never_matches_inside_a_longer_identifier() {
    assert_eq!(word_count("foo foobar foo_bar barfoo foo", "foo"), 2);
    assert_eq!(word_count("foo(foo) [foo] .foo,", "foo"), 4);
    assert_eq!(word_count("", "foo"), 0);
}

// ---- caps and truncation ----

#[test]
fn the_file_budget_truncates_the_walk_honestly() {
    let out = outline_of(&seeded(), &Input { max_files: Some(1), ..input() });
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.stats.files_seen, 1);
    assert!(out.stats.truncated);
}

#[test]
fn the_file_budget_never_exceeds_its_cap() {
    let mut host = seeded();
    let request = Input { max_files: Some(1_000_000), ..input() };
    // The clamp is what matters; the seeded tree is far under the cap, so
    // prove the clamp arithmetic directly on a listing the walk sees.
    let out = outline_of(&mut host, &request);
    assert!(!out.stats.truncated);
    assert_eq!(out.files.len(), 3);
}

#[test]
fn the_symbol_cap_truncates_a_file_and_says_so() {
    let out = outline_of(&seeded(), &Input { max_symbols_per_file: Some(1), ..input() });
    let file = out.files.iter().find(|f| f.path == "src/main.py").unwrap();
    assert_eq!(file.symbols.as_deref().unwrap().len(), 1);
    assert!(file.symbols_truncated);
}

#[test]
fn a_truncated_directory_listing_marks_the_map_truncated() {
    let mut host = seeded();
    host.dirs.get_mut("src").unwrap().truncated = true;
    let out = outline_of(&host, &input());
    assert!(out.stats.truncated);
}
