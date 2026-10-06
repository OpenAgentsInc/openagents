//! Action-items guest.
//!
//! The `items` operation finds the action items in meeting notes: who does
//! what, and by when, each citing the exact line it came from. It is the
//! Wasm in the *Action items* plugin, the workbench's noncoding example
//! (#10671): it reads text, not a repository, and writes nothing.
//!
//! The notes come from the `text` input, which the program's binding fills
//! with the person's request, and from any granted text file the request
//! names, such as `notes/standup.md`.
//!
//! Finding items is deterministic. A line is an action item when it is
//! marked as one (`ACTION:`, `Action item:`, `TODO:`, or an open checkbox
//! `- [ ]`), when it sits under an *Action items*, *Next steps*, or *To do*
//! heading, or when it starts with a person's name followed by `will` or
//! `to` (`Ana will send the budget by Friday`). A done checkbox (`- [x]`)
//! is counted and left out. The owner is that name, an `@name`, or a
//! leading `Name:`; the due date is what follows `by` or `due`. A line
//! without an owner is an unassigned item, never a guessed one.

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Value, json};

plugin_pdk::export_guest!(handle);

/// The most items one run lists.
const MAX_ITEMS: usize = 200;
/// The most bytes of one granted file the guest reads.
const FILE_BYTES: usize = 60 * 1024;
/// The most granted files the guest reads.
const MAX_FILES: usize = 4;
const MAX_LISTED: usize = 2_048;
/// The most characters of a line an item keeps.
const LINE_CHARS: usize = 300;
/// The bytes one read call asks for.
const CHUNK: usize = 4 * 1024;

/// Words that start a sentence but never name a person.
const NOT_NAMES: &[&str] = &[
    "We",
    "I",
    "You",
    "They",
    "He",
    "She",
    "It",
    "This",
    "That",
    "These",
    "Those",
    "The",
    "A",
    "An",
    "Everyone",
    "Someone",
    "Nobody",
    "Team",
    "All",
    "Then",
    "Next",
    "Also",
    "And",
    "But",
    "If",
    "When",
    "Who",
    "What",
    "There",
    "Here",
    "Please",
    "Need",
    "Needs",
    "Remember",
    "Note",
    "Decided",
    "Agreed",
    "Discussed",
    "Action",
    "Todo",
    "TODO",
];

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "items" => items(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// One action item.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    owner: Option<String>,
    task: String,
    due: Option<String>,
    from: String,
    line: usize,
    text: String,
}

fn items(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let input = &request.input;
    let request_text = match &input["text"] {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        _ => return Err(Refusal::unsupported("text is a string")),
    };
    let mut truncated = input["text_truncated"].as_bool().unwrap_or(false);
    let mut sources = vec![("request".to_string(), request_text)];
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
                    sources.push((path.clone(), String::from_utf8_lossy(&bytes.bytes).into()));
                    read.push(path);
                }
                Err(_) => unread.push(path),
            }
        }
    }
    let mut found = Vec::new();
    let mut done = 0;
    for (from, text) in &sources {
        let (items, closed) = find(from, text);
        found.extend(items);
        done += closed;
    }
    if found.len() > MAX_ITEMS {
        found.truncate(MAX_ITEMS);
        truncated = true;
    }
    let unassigned = found.iter().filter(|item| item.owner.is_none()).count();
    let rows: Vec<Value> = found
        .iter()
        .map(|item| {
            json!({
                "owner": item.owner,
                "task": item.task,
                "due": item.due,
                "source": {"from": item.from, "line": item.line},
                "text": item.text,
            })
        })
        .collect();
    Ok(json!({
        "kind": "action-items",
        "found": !found.is_empty(),
        "items": rows,
        "unassigned": unassigned,
        "done_left_out": done,
        "read": read,
        "unread": unread,
        "truncated": truncated,
        "markdown": render(&found, done, unassigned),
    }))
}

/// The items in one source and the done checkboxes left out.
fn find(from: &str, text: &str) -> (Vec<Item>, usize) {
    let mut out = Vec::new();
    let mut done = 0;
    let mut under_heading = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(heading) = heading(line) {
            let heading = heading.to_ascii_lowercase();
            under_heading = [
                "action items",
                "action item",
                "next steps",
                "to do",
                "todo",
                "todos",
            ]
            .iter()
            .any(|name| heading.trim_end_matches(':') == *name);
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("- [x]") || lower.starts_with("* [x]") {
            done += 1;
            continue;
        }
        let (marked, body) = marker(line);
        let body = body.trim();
        let listed = body.len() < line.len();
        let named = leading_name(body);
        let is_item = marked
            || (under_heading && listed)
            || named.as_ref().is_some_and(|(_, verb, _)| !verb.is_empty());
        if !is_item || body.is_empty() {
            continue;
        }
        let (owner, rest) = match named {
            Some((name, _, rest)) => (Some(name), rest),
            None => (
                mention(body),
                body.split_whitespace()
                    .filter(|word| !word.starts_with('@'))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        };
        let (task, due) = due(&rest);
        let task = task.trim().trim_end_matches('.').trim().to_string();
        if task.is_empty() {
            continue;
        }
        out.push(Item {
            owner,
            task,
            due,
            from: from.to_string(),
            line: index + 1,
            text: line.chars().take(LINE_CHARS).collect(),
        });
    }
    (out, done)
}

/// A Markdown or plain heading's text: `## Next steps`, or a short line
/// ending in a colon.
fn heading(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix('#') {
        return Some(rest.trim_start_matches('#').trim());
    }
    let words = line.split_whitespace().count();
    (line.ends_with(':') && words <= 3 && !line.starts_with('-') && !line.starts_with('*'))
        .then(|| line.trim_end_matches(':'))
}

/// Whether the line is marked as an item, and the line after its list
/// marker and any item marker.
fn marker(line: &str) -> (bool, &str) {
    let mut body = line;
    let mut marked = false;
    for prefix in ["- [ ]", "* [ ]", "[ ]"] {
        if let Some(rest) = body.strip_prefix(prefix) {
            body = rest;
            marked = true;
        }
    }
    if !marked {
        for prefix in ["- ", "* ", "+ "] {
            if let Some(rest) = body.strip_prefix(prefix) {
                body = rest;
            }
        }
        if let Some((number, rest)) = body.split_once(". ")
            && !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit())
        {
            body = rest;
        }
    }
    let trimmed = body.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    for word in ["action item:", "action:", "todo:", "to do:", "ai:"] {
        if lower.starts_with(word) {
            return (true, &trimmed[word.len()..]);
        }
    }
    (marked, trimmed)
}

/// A leading person: `Ana will …`, `Ana to …`, `Ana Ruiz will …`, or
/// `Ana: …`, with the verb (empty for a colon) and the rest.
fn leading_name(body: &str) -> Option<(String, String, String)> {
    let words: Vec<&str> = body.split_whitespace().collect();
    let first = *words.first()?;
    let capital = |word: &str| {
        word.chars().next().is_some_and(char::is_uppercase)
            && word
                .chars()
                .skip(1)
                .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
            && !NOT_NAMES.contains(&word)
    };
    if let Some(name) = first.strip_suffix(':')
        && capital(name)
    {
        return Some((name.to_string(), String::new(), words[1..].join(" ")));
    }
    let first = first.trim_start_matches('@');
    if !capital(first) && !words[0].starts_with('@') {
        return None;
    }
    for (count, verb_at) in [(1, 1), (2, 2)] {
        let Some(verb) = words.get(verb_at) else {
            continue;
        };
        if count == 2 && !capital(words[1]) {
            continue;
        }
        if matches!(*verb, "will" | "to" | "should" | "owns") {
            let name = words[..count]
                .iter()
                .map(|word| word.trim_start_matches('@'))
                .collect::<Vec<_>>()
                .join(" ");
            return Some((name, (*verb).to_string(), words[verb_at + 1..].join(" ")));
        }
    }
    None
}

/// An `@name` anywhere in the line.
fn mention(body: &str) -> Option<String> {
    body.split_whitespace()
        .find_map(|word| word.strip_prefix('@'))
        .map(|name| {
            name.trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|name| !name.is_empty())
}

/// The task and its due date: what follows the last ` by ` or ` due `, up
/// to the end of its clause.
fn due(rest: &str) -> (String, Option<String>) {
    let lower = rest.to_ascii_lowercase();
    let at = ["(due ", " due ", " by "]
        .iter()
        .filter_map(|word| lower.rfind(word).map(|at| (at, word.len())))
        .max_by_key(|(at, _)| *at);
    let Some((at, len)) = at else {
        return (rest.to_string(), None);
    };
    let tail = &rest[at + len..];
    let end = tail.find(['.', ',', ';', ')']).unwrap_or(tail.len());
    let when = tail[..end].trim();
    if when.is_empty() {
        return (rest.to_string(), None);
    }
    let task = format!("{}{}", &rest[..at], &tail[end..])
        .trim()
        .trim_end_matches(')')
        .trim()
        .to_string();
    (task, Some(when.to_string()))
}

fn render(items: &[Item], done: usize, unassigned: usize) -> String {
    if items.is_empty() {
        return "I didn't find any action items in the notes. Paste the notes, or name the \
                file they're in, and mark items with `ACTION:`, a checkbox, or a line like \
                `Ana will send the budget by Friday`."
            .to_string();
    }
    let mut lines = vec![format!("Action items ({})", items.len()), String::new()];
    for (index, item) in items.iter().enumerate() {
        let owner = item.owner.as_deref().unwrap_or("Unassigned");
        let due = item
            .due
            .as_deref()
            .map(|due| format!(", by {due}"))
            .unwrap_or_default();
        lines.push(format!(
            "{}. {owner}: {}{due} ({} line {})",
            index + 1,
            item.task,
            item.from,
            item.line
        ));
    }
    let mut tail = Vec::new();
    if unassigned > 0 {
        tail.push(format!("{unassigned} without an owner"));
    }
    if done > 0 {
        tail.push(format!("{done} already done and left out"));
    }
    if !tail.is_empty() {
        lines.push(String::new());
        lines.push(format!("{}.", tail.join("; ")));
    }
    lines.join("\n")
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join(name),
        )
        .unwrap()
    }

    fn run(text: &str, files: Vec<(&str, String)>) -> Value {
        let mut host = MemoryHost::new(files);
        items(
            &MemoryHost::request("items", json!({"text": text})),
            &mut host,
        )
        .unwrap()
    }

    fn rows(value: &Value) -> Vec<(Option<&str>, &str, Option<&str>, u64)> {
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["owner"].as_str(),
                    item["task"].as_str().unwrap(),
                    item["due"].as_str(),
                    item["source"]["line"].as_u64().unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn the_synthetic_standup_yields_each_item_with_its_owner_due_date_and_line() {
        let value = run("", vec![("standup.md", fixture("standup.md"))]);
        assert_eq!(value["kind"], "action-items");
        assert_eq!(value["read"], json!(["standup.md"]));
        assert_eq!(
            rows(&value),
            vec![
                (
                    Some("Ana"),
                    "send the revised budget to finance",
                    Some("Friday"),
                    6
                ),
                (Some("Ben Okafor"), "book the room for the offsite", None, 7),
                (Some("Chen"), "draft the agenda", Some("Wednesday"), 10),
                (None, "Update the onboarding checklist", None, 11),
                (
                    Some("dana"),
                    "Collect the survey results",
                    Some("the 14th"),
                    12
                ),
            ]
        );
        assert_eq!(value["unassigned"], 1);
        assert_eq!(value["done_left_out"], 1);
        let markdown = value["markdown"].as_str().unwrap();
        assert!(markdown.starts_with("Action items (5)"), "{markdown}");
        assert!(
            markdown.contains(
                "1. Ana: send the revised budget to finance, by Friday (standup.md line 6)"
            )
        );
        assert!(markdown.contains("1 without an owner; 1 already done and left out."));
    }

    #[test]
    fn each_item_cites_its_exact_line() {
        let notes = fixture("standup.md");
        let lines: Vec<&str> = notes.lines().collect();
        let value = run("", vec![("standup.md", notes.clone())]);
        for item in value["items"].as_array().unwrap() {
            let line = usize::try_from(item["source"]["line"].as_u64().unwrap()).unwrap();
            assert_eq!(lines[line - 1].trim(), item["text"].as_str().unwrap());
        }
    }

    #[test]
    fn pasted_notes_and_plain_sentences_are_not_guessed() {
        let value = run(
            "We discussed the roadmap.\nThe team agreed to ship in May.\nACTION: @lee to file the ticket",
            vec![],
        );
        assert_eq!(
            rows(&value),
            vec![(Some("lee"), "file the ticket", None, 3)]
        );
        let none = run("We talked about lunch.", vec![]);
        assert_eq!(none["found"], false);
        assert!(none["markdown"].as_str().unwrap().contains("didn't find"));
    }
}
