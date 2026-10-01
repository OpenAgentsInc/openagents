//! Release-notes guest.
//!
//! The `notes` operation turns a list of commits into grouped, user-facing
//! release notes (breaking changes, features, fixes, and the rest), each
//! line citing its commit. It is the Wasm in the *Release notes* plugin
//! (`docs/plugins/examples/release-notes.md`).
//!
//! A guest can't run `git`, so the commits come from text: the `text`
//! input, which the program's binding fills with the person's request
//! (`"request": "text"`), and any granted text file the request names,
//! such as a saved `git log`. It reads `git log`'s default format (with or
//! without `--decorate`), `--oneline`, `--graph`, and `--format="%h %s"`.
//!
//! Grouping is deterministic. A commit written as a Conventional Commit
//! (`feat(api)!: …`) is grouped by its type, and `!` or a `BREAKING
//! CHANGE:` footer makes it a breaking change. Any other commit is grouped
//! by its first word (`Add …` is a feature, `Fix …` a fix), and a commit
//! whose first word says nothing lands in *Other changes*, never in a
//! guessed group. Merge commits are counted and left out. Each entry says
//! which rule placed it.
//!
//! The range: a `FROM..TO` token in the request names it. Without one, a
//! decorated log that shows an older tag stops there, so a whole pasted
//! log yields the notes since the last release.

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Map, Value, json};

plugin_pdk::export_guest!(handle);

/// The most commits one run groups.
const MAX_COMMITS: usize = 500;
/// The most bytes of one granted file the guest reads.
const FILE_BYTES: usize = 60 * 1024;
/// The most granted files the guest reads.
const MAX_FILES: usize = 4;
const MAX_LISTED: usize = 2_048;
/// The most characters of a subject the result keeps.
const SUBJECT_CHARS: usize = 200;

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "notes" => notes(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// One commit as the log shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Commit {
    sha: String,
    subject: String,
    body: Vec<String>,
    tags: Vec<String>,
    merge: bool,
}

/// Where a commit lands in the notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Breaking,
    Features,
    Fixes,
    Performance,
    Removed,
    Reverts,
    Documentation,
    Other,
    Internal,
}

impl Group {
    const ALL: [Group; 9] = [
        Group::Breaking,
        Group::Features,
        Group::Fixes,
        Group::Performance,
        Group::Removed,
        Group::Reverts,
        Group::Documentation,
        Group::Other,
        Group::Internal,
    ];

    fn title(self) -> &'static str {
        match self {
            Group::Breaking => "Breaking changes",
            Group::Features => "Features",
            Group::Fixes => "Fixes",
            Group::Performance => "Performance",
            Group::Removed => "Removed",
            Group::Reverts => "Reverts",
            Group::Documentation => "Documentation",
            Group::Other => "Other changes",
            Group::Internal => "Internal",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Group::Breaking => "breaking",
            Group::Features => "features",
            Group::Fixes => "fixes",
            Group::Performance => "performance",
            Group::Removed => "removed",
            Group::Reverts => "reverts",
            Group::Documentation => "documentation",
            Group::Other => "other",
            Group::Internal => "internal",
        }
    }
}

/// One line of the notes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    group: Group,
    sha: String,
    text: String,
    scope: Option<String>,
    pull: Option<String>,
    breaking: Option<String>,
    /// `type` (Conventional Commits), `verb` (the first word), or `none`.
    rule: &'static str,
}

fn notes(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let input = &request.input;
    let request_text = match &input["text"] {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        _ => return Err(Refusal::unsupported("text is a string")),
    };
    let mut truncated = input["text_truncated"].as_bool().unwrap_or(false);
    let mut text = request_text.clone();
    let mut read = Vec::new();
    let mut unread = Vec::new();
    if let Some(root) = guest::root(request) {
        let listing = guest::list(host, &root, MAX_LISTED)
            .map_err(|code| Refusal::refused(format!("list {code}")))?;
        for entry in listing
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .take(MAX_FILES)
        {
            let path = guest::relative(&entry.name).to_string();
            match read_some(host, &entry.handle, FILE_BYTES) {
                Ok(bytes) => {
                    if !bytes.complete {
                        truncated = true;
                    }
                    text.push('\n');
                    text.push_str(&String::from_utf8_lossy(&bytes.bytes));
                    read.push(path);
                }
                Err(_) => unread.push(path),
            }
        }
    }

    let mut commits = commits(&text);
    if commits.len() > MAX_COMMITS {
        commits.truncate(MAX_COMMITS);
        truncated = true;
    }
    let range = range(&request_text);
    let title_input = input["title"].as_str().map(str::to_string);

    // Without a named range, a decorated log stops at the first older tag.
    let mut left_out = 0;
    let mut since: Option<String> = None;
    if range.is_none()
        && let Some(cut) = commits
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, commit)| !commit.tags.is_empty())
            .map(|(index, _)| index)
    {
        since = commits[cut].tags.first().cloned();
        left_out = commits.len() - cut;
        commits.truncate(cut);
    }
    let release = commits
        .first()
        .and_then(|commit| commit.tags.first().cloned());

    if commits.is_empty() {
        return Ok(json!({
            "kind": "release-notes",
            "found": false,
            "read": read,
            "unread": unread,
            "truncated": truncated,
            "markdown": "I didn't find a list of commits in the request or the files it names. \
                         Paste the output of `git log --oneline FROM..TO` (or save it to a file \
                         and name the file), and I'll group it into release notes.",
        }));
    }

    let merges = commits.iter().filter(|commit| commit.merge).count();
    let entries: Vec<Entry> = commits
        .iter()
        .filter(|commit| !commit.merge)
        .map(classify)
        .collect();
    let title = title_input.unwrap_or_else(|| match (&range, &release, &since) {
        (Some((from, to)), _, _) => format!("{from}..{to}"),
        (None, Some(release), Some(since)) => format!("{release} (since {since})"),
        (None, None, Some(since)) => format!("Changes since {since}"),
        (None, Some(release), None) => release.clone(),
        (None, None, None) => "Unreleased changes".to_string(),
    });

    let groups: Vec<Value> = Group::ALL
        .iter()
        .filter_map(|group| {
            let members: Vec<Value> = entries
                .iter()
                .filter(|entry| entry.group == *group)
                .map(entry_json)
                .collect();
            (!members.is_empty())
                .then(|| json!({"group": group.slug(), "title": group.title(), "entries": members}))
        })
        .collect();
    let markdown = render(&title, &entries, merges, left_out, since.as_deref());
    Ok(json!({
        "kind": "release-notes",
        "found": true,
        "title": title,
        "range": range.map(|(from, to)| json!({"from": from, "to": to})),
        "since_tag": since,
        "commits": commits.len(),
        "entries": entries.len(),
        "merges_left_out": merges,
        "older_left_out": left_out,
        "groups": groups,
        "read": read,
        "unread": unread,
        "truncated": truncated,
        "markdown": markdown,
    }))
}

/// The bytes one read call asks for: small, so a nearly spent read budget
/// still yields the part of a file that fits.
const CHUNK: usize = 4 * 1024;

/// Read at most `max_bytes` of a file from offset zero in [`CHUNK`]-sized
/// calls. A budget that runs out after some bytes were read returns those
/// bytes, marked incomplete.
fn read_some(host: &mut dyn Host, handle: &str, max_bytes: usize) -> Result<guest::Read, i32> {
    let total = guest::size(host, handle).map_or(usize::MAX, |size| {
        usize::try_from(size).unwrap_or(usize::MAX)
    });
    let mut read = guest::Read::default();
    if total == 0 {
        read.complete = true;
        return Ok(read);
    }
    loop {
        let chunk = CHUNK
            .min(max_bytes.saturating_sub(read.bytes.len()))
            .min(total.saturating_sub(read.bytes.len()));
        if chunk == 0 {
            read.complete = read.bytes.len() >= total;
            return Ok(read);
        }
        let answer = host.call(&json!({
            "v": 1,
            "handle": handle,
            "operation": "read",
            "args": {"offset": read.bytes.len(), "max_bytes": chunk}
        }));
        let answer = match answer {
            Ok(answer) => answer,
            Err(code) if read.bytes.is_empty() => return Err(code),
            Err(_) => return Ok(read),
        };
        let bytes = answer["bytes_base64"]
            .as_str()
            .map_or(Some(Vec::new()), guest::decode_base64)
            .ok_or(guest::MALFORMED)?;
        let eof = answer["eof"].as_bool().unwrap_or(true);
        let empty = bytes.is_empty();
        read.bytes.extend_from_slice(&bytes);
        if eof {
            read.complete = true;
            return Ok(read);
        }
        if empty {
            return Ok(read);
        }
    }
}

fn entry_json(entry: &Entry) -> Value {
    let mut object = Map::new();
    object.insert("sha".into(), json!(entry.sha));
    object.insert("text".into(), json!(entry.text));
    if let Some(scope) = &entry.scope {
        object.insert("scope".into(), json!(scope));
    }
    if let Some(pull) = &entry.pull {
        object.insert("pull".into(), json!(pull));
    }
    if let Some(note) = &entry.breaking {
        object.insert("breaking".into(), json!(note));
    }
    object.insert("rule".into(), json!(entry.rule));
    Value::Object(object)
}

// ---------------------------------------------------------------------------
// Reading a log
// ---------------------------------------------------------------------------

fn is_hex(text: &str) -> bool {
    (7..=40).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// The tags in a `--decorate` list: `(HEAD -> main, tag: v1.3.0)`.
fn tags(decorations: &str) -> Vec<String> {
    decorations
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(", ")
        .filter_map(|part| part.trim().strip_prefix("tag: "))
        .map(str::to_string)
        .collect()
}

/// A line with `--graph`'s drawing removed.
fn ungraphed(line: &str) -> &str {
    line.trim_start_matches(['*', '|', '/', '\\', ' ', '_'])
}

/// The commits a log shows, newest first as `git log` prints them.
fn commits(text: &str) -> Vec<Commit> {
    let mut commits: Vec<Commit> = Vec::new();
    let mut current: Option<Commit> = None;
    let mut in_message = false;
    for raw in text.lines() {
        let line = ungraphed(raw);
        // `git log`'s default format: a `commit` line, headers, a blank
        // line, then the message indented by four spaces.
        if let Some(rest) = line.strip_prefix("commit ") {
            let (sha, decorations) = rest.split_once(' ').unwrap_or((rest, ""));
            if is_hex(sha) {
                if let Some(done) = current.take() {
                    commits.push(done);
                }
                current = Some(Commit {
                    sha: sha.to_string(),
                    tags: tags(decorations),
                    ..Commit::default()
                });
                in_message = false;
                continue;
            }
        }
        if let Some(commit) = current.as_mut() {
            if !in_message {
                if line.starts_with("Merge: ") {
                    commit.merge = true;
                    continue;
                }
                if line.trim().is_empty() {
                    in_message = true;
                    continue;
                }
                if line.contains(": ") && !raw.starts_with("    ") {
                    continue;
                }
            }
            // The message: every non-empty line until the next `commit`
            // line, with `--graph`'s drawing and the indent removed.
            if in_message {
                let message = line.trim();
                if message.is_empty() {
                    continue;
                }
                if commit.subject.is_empty() {
                    commit.subject = message.to_string();
                } else {
                    commit.body.push(message.to_string());
                }
                continue;
            }
        }
        // `--oneline` and `--format="%h %s"`: a short hash, optional
        // decorations, and the subject.
        if raw.starts_with(' ') && !raw.trim_start().starts_with(['*', '|']) {
            continue;
        }
        let Some((sha, rest)) = line.split_once(' ') else {
            continue;
        };
        if !is_hex(sha) || rest.trim().is_empty() {
            continue;
        }
        let (decorations, subject) = if rest.starts_with('(') {
            match rest.split_once(") ") {
                Some((decorations, subject)) => (decorations, subject),
                None => ("", rest),
            }
        } else {
            ("", rest)
        };
        let subject = subject.trim_start_matches("- ").trim().to_string();
        let merge = subject.starts_with("Merge pull request ")
            || subject.starts_with("Merge branch ")
            || subject.starts_with("Merge remote-tracking branch ");
        commits.push(Commit {
            sha: sha.to_string(),
            subject,
            tags: tags(decorations),
            merge,
            body: Vec::new(),
        });
    }
    if let Some(done) = current.take() {
        commits.push(done);
    }
    for commit in &mut commits {
        if commit.subject.starts_with("Merge pull request ")
            || commit.subject.starts_with("Merge branch ")
            || commit.subject.starts_with("Merge remote-tracking branch ")
        {
            commit.merge = true;
        }
    }
    let mut seen: Vec<String> = Vec::new();
    commits.retain(|commit| {
        let short = commit.sha.chars().take(7).collect::<String>();
        if commit.subject.is_empty() || seen.contains(&short) {
            return false;
        }
        seen.push(short);
        true
    });
    commits
}

/// A `FROM..TO` (or `FROM...TO`) token: the range the request names.
fn range(text: &str) -> Option<(String, String)> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '`' | '"' | '\'' | ',' | '(' | ')'))
        .find_map(|token| {
            let token = token.trim_end_matches(['.', ':', '?', '!']);
            let (from, to) = token.split_once("...").or_else(|| token.split_once(".."))?;
            let plain = |part: &str| {
                !part.is_empty()
                    && part.len() <= 100
                    && part.chars().all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '~' | '^')
                    })
                    && !part.starts_with('.')
                    && !part.ends_with('.')
            };
            (plain(from) && plain(to)).then(|| (from.to_string(), to.to_string()))
        })
}

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

fn clip(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// A Conventional Commit header: type, scope, and whether `!` marks it
/// breaking.
fn conventional(subject: &str) -> Option<(String, Option<String>, bool, String)> {
    let (header, description) = subject.split_once(": ")?;
    let (head, bang) = match header.strip_suffix('!') {
        Some(head) => (head, true),
        None => (header, false),
    };
    let (kind, scope) = match head.split_once('(') {
        Some((kind, scope)) => (kind, Some(scope.strip_suffix(')')?.to_string())),
        None => (head, None),
    };
    let valid = !kind.is_empty()
        && kind.len() <= 12
        && kind.chars().all(|c| c.is_ascii_alphabetic())
        && scope
            .as_deref()
            .is_none_or(|scope| !scope.is_empty() && scope.len() <= 40 && !scope.contains(' '));
    valid.then(|| {
        (
            kind.to_ascii_lowercase(),
            scope,
            bang,
            description.trim().to_string(),
        )
    })
}

fn by_type(kind: &str) -> Group {
    match kind {
        "feat" | "feature" | "add" => Group::Features,
        "fix" | "bugfix" | "hotfix" => Group::Fixes,
        "perf" => Group::Performance,
        "revert" => Group::Reverts,
        "docs" | "doc" => Group::Documentation,
        "refactor" | "test" | "tests" | "ci" | "build" | "chore" | "style" | "deps" | "release" => {
            Group::Internal
        }
        "remove" | "removed" => Group::Removed,
        _ => Group::Other,
    }
}

/// The group a commit's first word places it in, or `None` when the word
/// says nothing.
fn by_verb(subject: &str) -> Option<Group> {
    let word: String = subject
        .split_whitespace()
        .next()?
        .trim_end_matches([':', ','])
        .to_ascii_lowercase();
    let group = match word.as_str() {
        "add" | "adds" | "added" | "introduce" | "introduces" | "introduced" | "implement"
        | "implements" | "implemented" | "support" | "supports" | "allow" | "allows" | "enable"
        | "enables" | "new" | "create" | "creates" | "expose" | "exposes" | "provide"
        | "provides" => Group::Features,
        "fix" | "fixes" | "fixed" | "correct" | "corrects" | "repair" | "repairs" | "resolve"
        | "resolves" | "prevent" | "prevents" | "handle" | "handles" | "avoid" | "avoids"
        | "restore" | "restores" | "guard" | "guards" => Group::Fixes,
        "speed" | "optimize" | "optimizes" | "optimise" | "faster" => Group::Performance,
        "remove" | "removes" | "removed" | "drop" | "drops" | "delete" | "deletes"
        | "deprecate" | "deprecates" => Group::Removed,
        "revert" | "reverts" => Group::Reverts,
        "doc" | "docs" | "document" | "documents" | "readme" => Group::Documentation,
        "refactor" | "refactors" | "rename" | "renames" | "move" | "moves" | "clean"
        | "cleanup" | "bump" | "bumps" | "chore" | "test" | "tests" | "ci" | "build" | "format"
        | "lint" | "tidy" | "wip" | "fixup!" | "squash!" => Group::Internal,
        _ => return None,
    };
    Some(group)
}

/// A pull request reference at the end of a subject: `(#123)`.
fn pull(subject: &str) -> (String, Option<String>) {
    if let Some((before, number)) = subject.rsplit_once(" (#")
        && let Some(number) = number.strip_suffix(')')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
    {
        return (before.to_string(), Some(format!("#{number}")));
    }
    (subject.to_string(), None)
}

/// A subject as a note line: the first letter upper case, no trailing
/// period.
fn sentence(text: &str) -> String {
    let text = text.trim().trim_end_matches('.');
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn classify(commit: &Commit) -> Entry {
    let short: String = commit.sha.chars().take(7).collect();
    let footer = commit.body.iter().find_map(|line| {
        line.strip_prefix("BREAKING CHANGE:")
            .or_else(|| line.strip_prefix("BREAKING-CHANGE:"))
            .map(|note| note.trim().to_string())
    });
    let (subject, pull) = pull(&commit.subject);
    if let Some((kind, scope, bang, description)) = conventional(&subject) {
        let breaking = bang || footer.is_some();
        return Entry {
            group: if breaking {
                Group::Breaking
            } else {
                by_type(&kind)
            },
            sha: short,
            text: clip(&sentence(&description), SUBJECT_CHARS),
            scope,
            pull,
            breaking: if breaking {
                Some(footer.unwrap_or_default()).filter(|note| !note.is_empty())
            } else {
                None
            },
            rule: "type",
        };
    }
    let flagged = subject.starts_with("BREAKING") || footer.is_some();
    let text = subject
        .strip_prefix("BREAKING CHANGE: ")
        .or_else(|| subject.strip_prefix("BREAKING: "))
        .unwrap_or(&subject);
    let (group, rule) = if flagged {
        (Group::Breaking, "verb")
    } else if subject.starts_with("Revert \"") {
        (Group::Reverts, "verb")
    } else {
        match by_verb(text) {
            Some(group) => (group, "verb"),
            None => (Group::Other, "none"),
        }
    };
    Entry {
        group,
        sha: short,
        text: clip(&sentence(text), SUBJECT_CHARS),
        scope: None,
        pull,
        breaking: footer.filter(|note| !note.is_empty()),
        rule,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(
    title: &str,
    entries: &[Entry],
    merges: usize,
    left_out: usize,
    since: Option<&str>,
) -> String {
    let mut out = format!("## Release notes: {title}\n");
    for group in Group::ALL {
        let members: Vec<&Entry> = entries
            .iter()
            .filter(|entry| entry.group == group)
            .collect();
        if members.is_empty() {
            continue;
        }
        out.push_str(&format!("\n### {}\n\n", group.title()));
        for entry in members {
            let scope = entry
                .scope
                .as_ref()
                .map(|scope| format!("**{scope}:** "))
                .unwrap_or_default();
            let pull = entry
                .pull
                .as_ref()
                .map(|pull| format!(" ({pull})"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- {scope}{}{pull} (`{}`)\n",
                entry.text, entry.sha
            ));
            if let Some(note) = &entry.breaking {
                out.push_str(&format!("  - {}\n", sentence(note)));
            }
        }
    }
    let mut footer = format!(
        "\n{} commit{} grouped",
        entries.len(),
        if entries.len() == 1 { "" } else { "s" }
    );
    if merges > 0 {
        footer.push_str(&format!(
            ", {merges} merge commit{} left out",
            if merges == 1 { "" } else { "s" }
        ));
    }
    if let Some(since) = since {
        footer.push_str(&format!(
            ", {left_out} older commit{} (from {since} back) left out",
            if left_out == 1 { "" } else { "s" }
        ));
    }
    let by_word = entries.iter().filter(|entry| entry.rule == "verb").count();
    let unplaced = entries.iter().filter(|entry| entry.rule == "none").count();
    footer.push_str(". ");
    footer.push_str(match (by_word, unplaced) {
        (0, 0) => "Every commit was grouped by its Conventional Commits type.",
        (_, 0) => "Commits without a Conventional Commits type were grouped by their first word.",
        _ => "Commits without a Conventional Commits type were grouped by their first word, and those whose first word says nothing are under Other changes.",
    });
    out.push_str(&footer);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/logs")
                .join(name),
        )
        .unwrap()
    }

    fn run(text: &str, files: Vec<(&str, String)>) -> Value {
        let mut host = MemoryHost::new(files);
        notes(
            &MemoryHost::request("notes", json!({"text": text})),
            &mut host,
        )
        .unwrap()
    }

    fn group<'a>(value: &'a Value, slug: &str) -> Vec<&'a str> {
        value["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["group"] == slug)
            .map(|group| {
                group["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry["text"].as_str().unwrap())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_conventional_oneline_log_groups_by_type_and_cites_each_commit() {
        let text = format!(
            "Write the release notes for v1.4.0..v1.5.0:\n{}",
            fixture("conventional-oneline.txt")
        );
        let value = run(&text, Vec::new());
        assert_eq!(value["title"], "v1.4.0..v1.5.0");
        assert_eq!(value["merges_left_out"], 1);
        assert_eq!(group(&value, "breaking"), ["Drop the v1 export endpoint"]);
        assert_eq!(
            group(&value, "features"),
            [
                "Export invoices as CSV",
                "Add a dark theme to the dashboard"
            ]
        );
        assert_eq!(
            group(&value, "fixes"),
            ["Round tax per line, not per invoice"]
        );
        assert_eq!(group(&value, "internal").len(), 2);
        let markdown = value["markdown"].as_str().unwrap();
        assert!(
            markdown.contains("- **export:** Export invoices as CSV (#212) (`4f2a9c1`)"),
            "{markdown}"
        );
        assert!(markdown.contains("### Breaking changes"), "{markdown}");
    }

    #[test]
    fn a_decorated_full_log_stops_at_the_last_release_and_reads_footers() {
        let value = run(
            "Release notes please, the log is in commits.txt",
            vec![("commits.txt", fixture("decorated-medium.txt"))],
        );
        assert_eq!(value["read"], json!(["commits.txt"]));
        assert_eq!(value["title"], "v2.1.0 (since v2.0.0)");
        assert_eq!(value["older_left_out"], 2);
        assert_eq!(group(&value, "breaking"), ["Store sessions in Redis"]);
        assert_eq!(
            group(&value, "fixes"),
            ["Fix the retry loop that never backed off"]
        );
        let breaking = &value["groups"][0]["entries"][0];
        assert_eq!(breaking["breaking"], "SESSION_STORE must now be set.");
        assert_eq!(group(&value, "other"), ["Tune the onboarding copy"]);
    }

    #[test]
    fn a_request_without_commits_says_so() {
        let value = run("can you write release notes?", Vec::new());
        assert_eq!(value["found"], false);
        assert_eq!(
            range("v1.2.0..v1.3.0"),
            Some(("v1.2.0".into(), "v1.3.0".into()))
        );
        assert_eq!(range("wait..."), None);
        assert_eq!(
            range("main...feature/x"),
            Some(("main".into(), "feature/x".into()))
        );
    }
}
