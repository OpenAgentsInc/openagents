//! Guard: no crate reads `HOME` from the environment itself.
//!
//! Windows does not set `HOME`, so a direct `std::env::var_os("HOME")` or
//! `std::env::var("HOME")` finds no home there (Coder 1.0.0-rc.7 failed on
//! Windows this way). Ask `user_home` instead. Setting `HOME` for a child
//! process (`.env("HOME", …)`, `set_var`, `remove_var`) is fine and is not
//! matched here; give the child `user_home::VARS` so Windows children get
//! `USERPROFILE` too.
//!
//! `crates/psionic` is its own Cargo workspace for Linux and macOS GPU
//! training and serving, never built into Coder, and is not scanned.

use std::fs;
use std::path::{Path, PathBuf};

const PATTERNS: &[&str] = &["var_os(\"HOME\")", "var(\"HOME\")"];
const SKIPPED: &[&str] = &["psionic", "user-home", "target"];

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                sources(&path, found);
            }
        } else if name.ends_with(".rs") {
            found.push(path);
        }
    }
}

/// Every place `line` reads `HOME` directly: a pattern not preceded by an
/// identifier character, so `set_var("HOME")` and friends do not count.
fn reads_home(line: &str) -> bool {
    PATTERNS.iter().any(|pattern| {
        line.match_indices(pattern).any(|(at, _)| {
            !line[..at]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_alphanumeric() || before == '_')
        })
    })
}

#[test]
fn the_matcher_tells_reads_from_setters() {
    assert!(reads_home(r#"let h = std::env::var_os("HOME");"#));
    assert!(reads_home(r#"env::var("HOME").ok()"#));
    assert!(reads_home(r#"var_os("HOME")"#));
    assert!(!reads_home(r#"command.env("HOME", home);"#));
    assert!(!reads_home(r#"unsafe { std::env::remove_var("HOME") }"#));
    assert!(!reads_home(r#"lookup.my_var("HOME")"#));
}

#[test]
fn no_crate_reads_home_directly() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .to_path_buf();
    let mut files = Vec::new();
    for entry in fs::read_dir(&crates).expect("read crates").flatten() {
        let name = entry.file_name();
        if !SKIPPED.contains(&name.to_string_lossy().as_ref()) {
            sources(&entry.path(), &mut files);
        }
    }
    assert!(files.len() > 100, "the scan found the workspace's sources");
    let mut offenders = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if reads_home(line) {
                let shown = file.strip_prefix(&crates).unwrap_or(&file);
                offenders.push(format!(
                    "crates/{}:{}: {}",
                    shown.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "read the home folder through `user_home` (home(), home_dir(), home_os(), \
         home_string(), or home_in(lookup)), not `HOME` directly; it is unset on \
         Windows:\n{}",
        offenders.join("\n")
    );
}
