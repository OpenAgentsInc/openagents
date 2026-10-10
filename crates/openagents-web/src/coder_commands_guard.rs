//! Every Coder command the site tells people to run exists in the release
//! they install. The site, its docs, the chat's knowledge, and the chat
//! answer bank name commands such as `coder login` and `/sync on`; the
//! one-line installer installs the published release `CODER_VERSION` names.
//! `coder_release_commands.txt` lists that release's commands, generated
//! from its commit by `scripts/release/coder-commands.sh`. 1.0.0-rc.5 was
//! published without `coder login` while every page said to run it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::pages::CODER_VERSION;

const PUBLISHED: &str = include_str!("coder_release_commands.txt");

/// The published release's version and its commands (`coder login`,
/// `/sync`, ...).
fn published() -> (String, BTreeSet<String>) {
    let mut version = String::new();
    let mut commands = BTreeSet::new();
    for line in PUBLISHED.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with("commit ") {
            continue;
        }
        if let Some(v) = line.strip_prefix("version ") {
            version = v.to_owned();
        } else {
            commands.insert(line.to_owned());
        }
    }
    (version, commands)
}

/// The slash commands the Coder in this checkout has, from its `/help`.
fn current_slash_commands(root: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(root.join("crates/coder-new/src/slash.rs"))
        .expect("coder-new's slash commands");
    let start = text.find("pub fn help()").expect("slash::help");
    let body = &text[start..];
    let body = &body[..body.find("\n}\n").unwrap_or(body.len())];
    let mut words = BTreeSet::new();
    for (at, _) in body.match_indices('/') {
        let before = &body[..at];
        if !(before.ends_with('"') || before.ends_with("\\n")) {
            continue;
        }
        let word = command_word(&body[at + 1..]);
        if !word.is_empty() && word != "demo" {
            words.insert(word.to_owned());
        }
    }
    words
}

/// The leading `[a-z-]` word of `text`.
fn command_word(text: &str) -> &str {
    let end = text
        .find(|c: char| !(c.is_ascii_lowercase() || c == '-'))
        .unwrap_or(text.len());
    &text[..end]
}

fn files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, extensions, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
        {
            out.push(path);
        }
    }
}

/// The Coder commands one line tells people to run: `coder WORD` after a
/// backtick or a quote, and a backticked `/WORD` that is one of Coder's
/// slash commands, on a line about Coder.
fn named_commands(line: &str, slash: &BTreeSet<String>) -> Vec<String> {
    let mut named = Vec::new();
    for quote in ["`coder ", "\"coder "] {
        for (at, _) in line.match_indices(quote) {
            let word = command_word(&line[at + quote.len()..]);
            if !word.is_empty() {
                named.push(format!("coder {word}"));
            }
        }
    }
    let about_coder = line.contains("Coder") || line.contains("coder") || line.contains("type `/");
    if about_coder {
        for (at, _) in line.match_indices("`/") {
            let word = command_word(&line[at + 2..]);
            let rest = &line[at + 2 + word.len()..];
            // `/sync on` and `/memory` are commands; `/download/x` is a path.
            if slash.contains(word) && (rest.starts_with('`') || rest.starts_with(' ')) {
                named.push(format!("/{word}"));
            }
        }
    }
    named
}

#[test]
fn every_coder_command_the_site_names_is_in_the_published_release() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the crate directory");
    let crate_dir = crate_dir.as_path();
    let root = crate_dir
        .join("../..")
        .canonicalize()
        .expect("the checkout");
    let (version, commands) = published();
    assert_eq!(
        version, CODER_VERSION,
        "coder_release_commands.txt is for {version} but CODER_VERSION is {CODER_VERSION}; \
         regenerate it: scripts/release/coder-commands.sh {CODER_VERSION} COMMIT"
    );
    let slash = current_slash_commands(&root);
    assert!(
        slash.contains("sync"),
        "could not read Coder's slash commands"
    );

    let mut paths = Vec::new();
    files(&crate_dir.join("content"), &["md", "toml"], &mut paths);
    files(&crate_dir.join("src"), &["rs"], &mut paths);
    files(&crate_dir.join("static"), &["js", "html"], &mut paths);
    files(&root.join("crates/coder/answers"), &["toml"], &mut paths);
    files(&root.join("knowledge/openagents"), &["md"], &mut paths);
    assert!(paths.len() > 50, "found only {} files to read", paths.len());

    let this_file = crate_dir.join("src/coder_commands_guard.rs");
    let mut missing = Vec::new();
    for path in paths {
        if path == this_file {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (index, line) in text.lines().enumerate() {
            // Code comments describe the code, not what to type.
            if line.trim_start().starts_with("//") {
                continue;
            }
            for command in named_commands(line, &slash) {
                if !commands.contains(&command) {
                    missing.push(format!(
                        "{}:{}: {command}",
                        path.strip_prefix(&root).unwrap_or(&path).display(),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these lines tell people to run Coder commands that the published release \
         {CODER_VERSION} doesn't have. Publish a release that has them \
         (scripts/release/coder.sh), then set CODER_VERSION and regenerate \
         coder_release_commands.txt; or fix the copy:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_guard_reads_commands_from_copy() {
    let slash: BTreeSet<String> = ["sync", "memory"].map(str::to_owned).into();
    assert_eq!(
        named_commands(
            "run `coder login`, type `/sync on`, and use `coder`.",
            &slash
        ),
        ["coder login", "/sync"]
    );
    assert_eq!(
        named_commands("Coder: open `/download/coder` or `/settings`.", &slash),
        Vec::<String>::new()
    );
    assert_eq!(
        named_commands("\"coder trace upload\"", &slash),
        ["coder trace"]
    );
}
