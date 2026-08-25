//! The search against a fake host: literal and regex matching, the
//! gitignore subset, every bound and its truncation record, context
//! windows, and the clean refusal of patterns outside the regex subset —
//! all without a WASM runtime, the same pattern as `repo_tree`'s tests.

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

fn input(pattern: &str) -> Input {
    Input {
        pattern: pattern.to_string(),
        regex: None,
        path: None,
        case_sensitive: None,
        max_files: None,
        max_matches: None,
        max_matches_per_file: None,
        context_lines: None,
    }
}

fn search(host: &FakeHost, input: &Input) -> Output {
    code_search(host, input).unwrap()
}

#[test]
fn a_literal_search_groups_matches_per_file_with_line_numbers() {
    let host = workspace(&[
        (
            "src/auth.ex",
            "defmodule Auth do\n  def login do\n    :ok\n  end\nend\n",
        ),
        (
            "src/user.ex",
            "defmodule User do\n  # login goes through Auth.login\nend\n",
        ),
        ("README.md", "No such word here.\n"),
    ]);
    let out = search(&host, &input("login"));
    assert_eq!(out.files_considered, 3);
    assert_eq!(out.files_scanned, 3);
    assert_eq!(out.files_matched, 2);
    assert_eq!(out.matches_returned, 2);
    assert!(!out.truncated);
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/auth.ex", "src/user.ex"]);
    assert_eq!(out.files[0].matches[0].line, 2);
    assert_eq!(out.files[0].matches[0].text, "  def login do");
    assert_eq!(out.files[1].matches[0].line, 2);
}

#[test]
fn a_matching_line_is_one_match_however_often_the_pattern_occurs_on_it() {
    let host = workspace(&[("a.txt", "one two two two\nclean\ntwo\n")]);
    let out = search(&host, &input("two"));
    assert_eq!(out.matches_returned, 2);
    assert_eq!(out.files[0].matches[0].line, 1);
    assert_eq!(out.files[0].matches[1].line, 3);
}

#[test]
fn context_lines_surround_each_match_and_stop_at_the_file_edges() {
    let host = workspace(&[("a.txt", "first\nsecond\nhit here\nfourth\nfifth\n")]);
    let out = search(
        &host,
        &Input {
            context_lines: Some(2),
            ..input("hit")
        },
    );
    let matched = &out.files[0].matches[0];
    assert_eq!(matched.before, vec!["first", "second"]);
    assert_eq!(matched.after, vec!["fourth", "fifth"]);

    let edge = search(
        &host,
        &Input {
            context_lines: Some(2),
            ..input("first")
        },
    );
    assert_eq!(edge.files[0].matches[0].before, Vec::<String>::new());
    assert_eq!(edge.files[0].matches[0].after, vec!["second", "hit here"]);
}

#[test]
fn a_regex_search_matches_the_documented_subset() {
    let host = workspace(&[(
        "src/handlers.rs",
        "fn handle_login() {}\nfn handle_logout() {}\nfn ignore_me() {}\n",
    )]);
    let out = search(
        &host,
        &Input {
            regex: Some(true),
            ..input(r"fn handle_\w+\(")
        },
    );
    assert_eq!(out.matches_returned, 2);
    assert_eq!(out.files[0].matches[0].line, 1);
    assert_eq!(out.files[0].matches[1].line, 2);
}

#[test]
fn regex_anchors_classes_quantifiers_and_alternation_hold() {
    let host = workspace(&[("a.txt", "abc\nabbbc\nac\nxabc\nabcx\ndone\nfin\n")]);
    let anchored = search(
        &host,
        &Input {
            regex: Some(true),
            ..input("^ab*c$")
        },
    );
    let lines: Vec<usize> = anchored.files[0].matches.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![1, 2, 3]);

    let class = search(
        &host,
        &Input {
            regex: Some(true),
            ..input("^[a-n]+$")
        },
    );
    // `xabc` starts outside the class, `done` holds an `o` past `n`, and
    // the anchors reject both; `fin` sits inside the range.
    let lines: Vec<usize> = class.files[0].matches.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![1, 2, 3, 7]);

    let either = search(
        &host,
        &Input {
            regex: Some(true),
            ..input("^done$|^fin$")
        },
    );
    let lines: Vec<usize> = either.files[0].matches.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![6, 7]);
}

#[test]
fn case_folding_is_off_by_default_and_ascii_when_asked_for() {
    let host = workspace(&[("a.txt", "Login\nlogin\nLOGIN\n")]);
    let exact = search(&host, &input("login"));
    assert_eq!(exact.matches_returned, 1);
    let folded = search(
        &host,
        &Input {
            case_sensitive: Some(false),
            ..input("login")
        },
    );
    assert_eq!(folded.matches_returned, 3);
    let folded_regex = search(
        &host,
        &Input {
            regex: Some(true),
            case_sensitive: Some(false),
            ..input("^log[a-z]n$")
        },
    );
    assert_eq!(folded_regex.matches_returned, 3);
}

#[test]
fn gitignored_files_and_directories_are_never_searched() {
    let host = workspace(&[
        (".gitignore", "node_modules/\n*.log\n!keep.log\n"),
        ("src/app.ts", "needle in app\n"),
        ("node_modules/pkg/index.js", "needle in a dependency\n"),
        ("debug.log", "needle in a log\n"),
    ]);
    let out = search(&host, &input("needle"));
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/app.ts"]);
    // The skipped directory counts once and the log file once; the `!` line
    // is counted as unhonored rather than silently applied.
    assert_eq!(out.skipped_gitignored, 2);
    assert_eq!(out.ignored_negations, 1);
}

#[test]
fn a_subtree_scope_still_honors_ancestor_gitignores() {
    let host = workspace(&[
        (".gitignore", "*.log\n"),
        ("src/app.ts", "needle\n"),
        ("src/trace.log", "needle\n"),
        ("other/away.ts", "needle\n"),
    ]);
    let out = search(
        &host,
        &Input {
            path: Some("src".to_string()),
            ..input("needle")
        },
    );
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/app.ts"]);
    assert_eq!(out.files_considered, 1);
}

#[test]
fn the_total_match_ceiling_stops_the_scan_and_reports_the_remainder() {
    let host = workspace(&[
        ("a.txt", "hit\nhit\nhit\n"),
        ("b.txt", "hit\nhit\n"),
        ("c.txt", "hit\n"),
    ]);
    let out = search(
        &host,
        &Input {
            max_matches: Some(4),
            ..input("hit")
        },
    );
    assert_eq!(out.matches_returned, 4);
    // The ceiling landed inside b.txt: its third… second match was cut, and
    // c.txt was never opened. Both facts are stated, not implied.
    assert_eq!(out.matches_dropped, 1);
    assert_eq!(out.files_scanned, 2);
    assert_eq!(out.files_unscanned, 1);
    assert!(out.truncated);
    assert_eq!(out.files[1].matches.len(), 1);
    assert_eq!(out.files[1].matches_total, 2);
}

#[test]
fn the_per_file_ceiling_returns_the_first_matches_and_counts_the_rest() {
    let host = workspace(&[("a.txt", "hit\nhit\nhit\nhit\n")]);
    let out = search(
        &host,
        &Input {
            max_matches_per_file: Some(2),
            ..input("hit")
        },
    );
    assert_eq!(out.files[0].matches.len(), 2);
    assert_eq!(out.files[0].matches_total, 4);
    assert_eq!(out.matches_dropped, 2);
    assert!(out.truncated);
}

#[test]
fn the_file_ceiling_stops_the_scan_and_counts_the_unscanned() {
    let host = workspace(&[("a.txt", "hit\n"), ("b.txt", "hit\n"), ("c.txt", "hit\n")]);
    let out = search(
        &host,
        &Input {
            max_files: Some(2),
            ..input("hit")
        },
    );
    assert_eq!(out.files_scanned, 2);
    assert_eq!(out.files_unscanned, 1);
    assert_eq!(out.matches_returned, 2);
    assert!(out.truncated);
}

#[test]
fn binary_and_oversized_files_are_counted_not_searched() {
    let mut host = workspace(&[("a.txt", "hit\n"), ("blob.bin", "placeholder")]);
    host.files
        .insert("blob.bin".to_string(), vec![b'h', b'i', b't', 0, 1, 2]);
    let out = search(&host, &input("hit"));
    assert_eq!(out.skipped_binary, 1);
    assert_eq!(out.files_matched, 1);

    let mut big = workspace(&[("a.txt", "hit\n"), ("huge.txt", "hit\n")]);
    let listing = big.dirs.get_mut(&(0, String::new())).unwrap();
    for entry in &mut listing.entries {
        if entry.name == "huge.txt" {
            entry.size = MAX_FILE_BYTES + 1;
        }
    }
    let out = search(&big, &input("hit"));
    assert_eq!(out.skipped_oversized, 1);
    assert_eq!(out.files_matched, 1);
}

#[test]
fn an_empty_result_is_clean_and_untruncated() {
    let host = workspace(&[("a.txt", "nothing to see\n")]);
    let out = search(&host, &input("absent"));
    assert!(out.files.is_empty());
    assert_eq!(out.files_scanned, 1);
    assert_eq!(out.matches_returned, 0);
    assert!(!out.truncated);
}

#[test]
fn an_empty_pattern_is_refused() {
    let host = workspace(&[("a.txt", "text\n")]);
    let refusal = code_search(&host, &input("   ")).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::Unsupported);
}

#[test]
fn patterns_outside_the_regex_subset_are_refused_with_a_reason() {
    let host = workspace(&[("a.txt", "text\n")]);
    for (pattern, expect) in [
        ("(group)", "groups"),
        ("a{2,3}", "counted repetition"),
        ("*dangling", "nothing to repeat"),
        ("[unclosed", "never closed"),
        ("[]", "empty"),
        ("[z-a]", "backwards"),
        ("trailing\\", "escapes nothing"),
        ("mid^anchor", "start"),
        ("a$b", "end"),
        ("a|", "empty branch"),
    ] {
        let refusal = code_search(
            &host,
            &Input {
                regex: Some(true),
                ..input(pattern)
            },
        )
        .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::Unsupported, "{pattern}");
        assert!(
            refusal.reason.contains(expect),
            "{pattern}: {}",
            refusal.reason
        );
    }
}

#[test]
fn regex_metacharacters_are_plain_text_in_literal_mode() {
    let host = workspace(&[("a.txt", "value[0].method()\nvalue\n")]);
    let out = search(&host, &input("value[0].method()"));
    assert_eq!(out.matches_returned, 1);
    assert_eq!(out.files[0].matches[0].line, 1);
}

#[test]
fn long_lines_are_bounded_in_the_output() {
    let long = format!("{}needle{}\n", "x".repeat(300), "y".repeat(300));
    let host = workspace(&[("a.txt", long.as_str())]);
    let out = search(&host, &input("x"));
    assert_eq!(out.files[0].matches[0].text.chars().count(), 200);
}

#[test]
fn crlf_line_endings_match_and_render_without_the_carriage_return() {
    let host = workspace(&[("a.txt", "one\r\nhit here\r\nthree\r\n")]);
    let out = search(
        &host,
        &Input {
            regex: Some(true),
            ..input("here$")
        },
    );
    assert_eq!(out.matches_returned, 1);
    assert_eq!(out.files[0].matches[0].text, "hit here");
}

#[test]
fn the_same_tree_and_query_always_produce_the_same_output() {
    let host = workspace(&[
        ("src/a.rs", "needle one\n"),
        ("src/b.rs", "needle two\nneedle three\n"),
        ("lib/c.rs", "needle four\n"),
    ]);
    let first = serde_json::to_string(&search(&host, &input("needle"))).unwrap();
    let second = serde_json::to_string(&search(&host, &input("needle"))).unwrap();
    assert_eq!(first, second);
}
