//! The tree and the lookup against a fake host: every gitignore-subset
//! rule, the depth and entry ceilings, query matching and ranking, and the
//! unconditional `.git` skip — all without a WASM runtime, the same
//! pattern as the scanner's and reader's tests.

use super::*;
use openagents_pdk::{MountDirEntry, RefusalCode};
use std::collections::BTreeMap;

#[derive(Default)]
struct FakeHost {
    dirs: BTreeMap<(u32, String), MountDirListing>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Host for FakeHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        self.dirs
            .get(&(mount_index, path.to_string()))
            .cloned()
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::FileUnreadable,
                    "the mount has no such directory",
                )
            })
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Refusal::new(RefusalCode::FileUnreadable, "no such file"))
    }
}

/// Build a workspace host from `(path, contents)` pairs; directories are
/// derived from the paths, listings sorted by name like the real host's.
fn workspace(files: &[(&str, &str)]) -> FakeHost {
    let mut children: BTreeMap<String, BTreeMap<String, (String, u64)>> = BTreeMap::new();
    children.insert(String::new(), BTreeMap::new());
    let mut host = FakeHost::default();
    for (path, body) in files {
        host.files
            .insert((*path).to_string(), body.as_bytes().to_vec());
        let parts: Vec<&str> = path.split('/').collect();
        let mut dir = String::new();
        for (at, part) in parts.iter().enumerate() {
            if at == parts.len() - 1 {
                children
                    .entry(dir.clone())
                    .or_default()
                    .insert((*part).to_string(), ("file".to_string(), body.len() as u64));
            } else {
                children
                    .entry(dir.clone())
                    .or_default()
                    .entry((*part).to_string())
                    .or_insert(("dir".to_string(), 0));
                dir = join(&dir, part);
                children.entry(dir.clone()).or_default();
            }
        }
    }
    for (dir, kids) in children {
        let entries = kids
            .into_iter()
            .map(|(name, (kind, size))| MountDirEntry {
                name,
                kind,
                size,
                mtime_ms: 0,
            })
            .collect();
        host.dirs.insert(
            (0, dir),
            MountDirListing {
                entries,
                truncated: false,
            },
        );
    }
    host
}

fn input() -> Input {
    Input::default()
}

fn tree(host: &FakeHost, input: &Input) -> TreeOutput {
    match repo_tree(host, input).unwrap() {
        Output::Tree(out) => out,
        Output::Query(_) => panic!("expected tree output"),
    }
}

fn query(host: &FakeHost, text: &str, extra: Input) -> QueryOutput {
    match repo_tree(
        host,
        &Input {
            query: Some(text.to_string()),
            ..extra
        },
    )
    .unwrap()
    {
        Output::Query(out) => out,
        Output::Tree(_) => panic!("expected query output"),
    }
}

fn paths(out: &TreeOutput) -> Vec<&str> {
    out.entries.iter().map(|e| e.path.as_str()).collect()
}

#[test]
fn lists_the_tree_depth_first_with_paths_kinds_and_sizes() {
    let host = workspace(&[
        ("README.md", "hello"),
        ("src/lib.rs", "fn main() {}"),
        ("src/sub/deep.rs", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(
        out.entries
            .iter()
            .map(|e| (e.path.as_str(), e.kind.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("README.md", "file"),
            ("src", "dir"),
            ("src/lib.rs", "file"),
            ("src/sub", "dir"),
            ("src/sub/deep.rs", "file"),
        ],
    );
    assert_eq!(out.entries[0].size, 5);
    assert_eq!(out.total_seen, 5);
    assert!(!out.truncated);
    assert_eq!(out.skipped_gitignored, 0);
    assert_eq!(out.ignored_negations, 0);
}

#[test]
fn dot_git_is_always_skipped_even_without_a_gitignore() {
    let host = workspace(&[
        (".git/config", "[core]"),
        (".git/HEAD", "ref"),
        ("a.txt", "a"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(paths(&out), vec!["a.txt"]);
    // Not counted as seen and not counted as gitignored: it never exists.
    assert_eq!(out.total_seen, 1);
    assert_eq!(out.skipped_gitignored, 0);
}

#[test]
fn blank_lines_and_comments_are_skipped() {
    let host = workspace(&[
        (".gitignore", "# logs\n\n*.log\n"),
        ("a.log", "x"),
        ("a.txt", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(paths(&out), vec![".gitignore", "a.txt"]);
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn an_unanchored_name_matches_at_any_depth_and_prunes_the_subtree() {
    let host = workspace(&[
        (".gitignore", "target\n"),
        ("kept.rs", "x"),
        ("sub/target/out.o", "x"),
        ("target/out.o", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(paths(&out), vec![".gitignore", "kept.rs", "sub"]);
    // Each pruned directory counts once; what is under it is never seen.
    assert_eq!(out.skipped_gitignored, 2);
}

#[test]
fn a_trailing_slash_matches_directories_only() {
    let host = workspace(&[
        (".gitignore", "build/\n"),
        ("build/out.o", "x"),
        ("docs/build", "a file named build"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(paths(&out), vec![".gitignore", "docs", "docs/build"]);
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn a_leading_slash_anchors_to_the_gitignore_directory() {
    let host = workspace(&[
        (".gitignore", "/vendor\n"),
        ("pkg/vendor/b.js", "x"),
        ("vendor/a.js", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(
        paths(&out),
        vec![".gitignore", "pkg", "pkg/vendor", "pkg/vendor/b.js"]
    );
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn star_matches_within_a_segment_but_not_across() {
    let host = workspace(&[
        (".gitignore", "/src/*.gen\n"),
        ("src/a.gen", "x"),
        ("src/keep.rs", "x"),
        ("src/sub/b.gen", "x"),
    ]);
    let out = tree(&host, &input());
    // `*` stays inside one segment: `src/sub/b.gen` is not `/src/*.gen`.
    assert_eq!(
        paths(&out),
        vec![
            ".gitignore",
            "src",
            "src/keep.rs",
            "src/sub",
            "src/sub/b.gen"
        ],
    );
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn double_star_matches_zero_or_more_segments() {
    let host = workspace(&[
        (".gitignore", "src/**/gen\n"),
        ("other/gen/kept.txt", "x"),
        ("src/a/b/gen/deep.txt", "x"),
        ("src/gen/direct.txt", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(
        paths(&out),
        vec![
            ".gitignore",
            "other",
            "other/gen",
            "other/gen/kept.txt",
            "src",
            "src/a",
            "src/a/b",
        ],
    );
    // `src/gen` (zero segments) and `src/a/b/gen` (two) both matched.
    assert_eq!(out.skipped_gitignored, 2);
}

#[test]
fn negations_are_ignored_and_counted() {
    let host = workspace(&[
        (".gitignore", "*.log\n!keep.log\n"),
        ("drop.log", "x"),
        ("keep.log", "x"),
    ]);
    let out = tree(&host, &input());
    // The subset cannot un-ignore, and it says so.
    assert_eq!(paths(&out), vec![".gitignore"]);
    assert_eq!(out.skipped_gitignored, 2);
    assert_eq!(out.ignored_negations, 1);
}

#[test]
fn a_nested_gitignore_applies_to_its_subtree_only() {
    let host = workspace(&[
        ("b.tmp", "kept at the root"),
        ("sub/.gitignore", "*.tmp\n"),
        ("sub/a.tmp", "x"),
        ("sub/kept.rs", "x"),
    ]);
    let out = tree(&host, &input());
    assert_eq!(
        paths(&out),
        vec!["b.tmp", "sub", "sub/.gitignore", "sub/kept.rs"]
    );
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn the_depth_default_is_four_and_the_cap_is_eight() {
    let host = workspace(&[("d1/d2/d3/d4/d5/d6/d7/d8/d9/deep.txt", "x")]);
    let shallow = tree(&host, &input());
    assert_eq!(shallow.entries.last().unwrap().path, "d1/d2/d3/d4");

    let deep = tree(
        &host,
        &Input {
            max_depth: Some(99),
            ..input()
        },
    );
    assert_eq!(deep.entries.last().unwrap().path, "d1/d2/d3/d4/d5/d6/d7/d8");
    assert!(!paths(&deep).contains(&"d1/d2/d3/d4/d5/d6/d7/d8/d9"));

    let one = tree(
        &host,
        &Input {
            max_depth: Some(1),
            ..input()
        },
    );
    assert_eq!(paths(&one), vec!["d1"]);
}

#[test]
fn the_entry_ceiling_stops_the_walk_and_says_so() {
    let files: Vec<(String, &str)> = (0..10)
        .map(|at| (format!("file{at:02}.txt"), "x"))
        .collect();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
    let host = workspace(&borrowed);
    let out = tree(
        &host,
        &Input {
            max_entries: Some(3),
            ..input()
        },
    );
    assert_eq!(out.entries.len(), 3);
    assert!(out.truncated);
}

#[test]
fn a_subtree_walk_returns_root_relative_paths_and_honors_ancestor_ignores() {
    let host = workspace(&[
        (".gitignore", "*.log\n"),
        ("other/b.rs", "x"),
        ("src/a.rs", "x"),
        ("src/x.log", "x"),
    ]);
    let out = tree(
        &host,
        &Input {
            path: Some("src".to_string()),
            ..input()
        },
    );
    assert_eq!(paths(&out), vec!["src/a.rs"]);
    assert_eq!(out.skipped_gitignored, 1);
}

#[test]
fn a_missing_subtree_is_a_refusal_not_a_trap() {
    let host = workspace(&[("a.txt", "x")]);
    let refusal = repo_tree(
        &host,
        &Input {
            path: Some("nope".to_string()),
            ..input()
        },
    )
    .unwrap_err();
    assert_eq!(refusal.code, RefusalCode::FileUnreadable);
}

#[test]
fn a_query_matches_a_case_insensitive_subsequence_of_the_path() {
    let host = workspace(&[("lib/auth_controller.ex", "x"), ("lib/other.ex", "x")]);
    let out = query(&host, "AuthController", input());
    assert_eq!(
        out.matches
            .iter()
            .map(|m| m.path.as_str())
            .collect::<Vec<_>>(),
        vec!["lib/auth_controller.ex"],
    );
    assert_eq!(out.searched, 2);
    assert!(!out.truncated);
}

#[test]
fn query_ranking_prefers_tight_spans_then_segment_boundaries() {
    let host = workspace(&[("a_u_t_h.txt", "x"), ("auth.ex", "x"), ("zauthz.md", "x")]);
    let out = query(&host, "auth", input());
    assert_eq!(
        out.matches
            .iter()
            .map(|m| m.path.as_str())
            .collect::<Vec<_>>(),
        // Span 4 beats span 7; on the span tie, `auth.ex` starts on a
        // boundary and `zauthz.md` does not.
        vec!["auth.ex", "zauthz.md", "a_u_t_h.txt"],
    );
}

#[test]
fn the_query_limit_truncates_and_says_so() {
    let files: Vec<(String, &str)> = (0..5).map(|at| (format!("match{at}.txt"), "x")).collect();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
    let host = workspace(&borrowed);
    let out = query(
        &host,
        "match",
        Input {
            limit: Some(2),
            ..input()
        },
    );
    assert_eq!(out.matches.len(), 2);
    assert!(out.truncated);
    assert_eq!(out.searched, 5);
}

#[test]
fn the_query_never_sees_git_or_gitignored_files() {
    let host = workspace(&[
        (".git/carbon.txt", "x"),
        (".gitignore", "*.log\n"),
        ("app.rs", "x"),
        ("carbon.log", "x"),
    ]);
    let out = query(&host, "car", input());
    assert!(out.matches.is_empty());
    // `.gitignore` and `app.rs` were searched; the ignored and `.git`
    // files never entered the candidate set.
    assert_eq!(out.searched, 2);
}

#[test]
fn a_deep_file_is_found_by_query_where_the_tree_default_would_hide_it() {
    let host = workspace(&[("d1/d2/d3/d4/d5/d6/needle.rs", "x")]);
    let out = query(&host, "needle", input());
    assert_eq!(out.matches[0].path, "d1/d2/d3/d4/d5/d6/needle.rs");
}

#[test]
fn scoring_is_what_the_doc_says() {
    let q: Vec<char> = "ac".chars().collect();
    // Tightest start wins: `a` recurs, and the later start is tighter.
    let best = score(&q, "a__x/ac.rs").unwrap();
    assert_eq!(best.span, 2);
    assert_eq!(best.boundaries, 1);
    assert!(score(&q, "ca.rs").is_none());
    assert!(score(&[], "anything").is_none());
}
