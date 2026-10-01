//! Explain-this-error guest.
//!
//! The `explain` operation reads a failing command's output, finds the
//! file and line in the repository it points at, reads that code, and says
//! the likely cause and a likely fix. It is the Wasm in the *Explain this
//! error* plugin (`docs/plugins/examples/explain-this-error.md`).
//!
//! Where the output comes from: the `text` input, which the program's
//! binding fills with the person's request (`"request": "text"`), and any
//! granted file that isn't source code, such as a saved `build.log` the
//! request names. Which files the guest can read is the host's grant: the
//! workflow asks for the files the request names (`"read_named": true`),
//! so a frame's file is readable when the output names it and it exists in
//! the workspace.
//!
//! What it recognizes, from the text itself and never from a file name:
//! `rustc` errors and Rust panics, Python tracebacks, Node.js and browser
//! stack traces, TypeScript (`tsc`) errors, Go compiler errors and panics,
//! Java exceptions, and the `path:line:col: error: message` shape most
//! other compilers print. The first error in the output is the one it
//! explains; the rest are counted.
//!
//! The explanation is deterministic: a table of error kinds per language,
//! with what the code at the line adds (the names defined nearby that are
//! close to a missing one, the dictionary keys a file uses, the expression
//! that was `undefined`, a loop bound that runs one past the end). It
//! says what it read and what it left out, and it never runs anything.

use plugin_pdk::Request;
use plugin_pdk::guest::{self, Host, Refusal};
use serde_json::{Map, Value, json};

plugin_pdk::export_guest!(handle);

const DEFAULT_MAX_FRAMES: usize = 8;
const MAX_FRAMES_CAP: usize = 32;
const DEFAULT_CONTEXT: usize = 4;
const CONTEXT_CAP: usize = 20;
/// The most bytes of one source file the guest reads.
const SOURCE_BYTES: usize = 24 * 1024;
/// The most bytes of saved output the guest reads from one file.
const LOG_BYTES: usize = 40 * 1024;
/// The most granted files the guest reads.
const MAX_READS: usize = 6;
const MAX_LISTED: usize = 2_048;
/// The most characters of an error message the result keeps.
const MESSAGE_CHARS: usize = 400;
/// The most characters of one excerpt line the result keeps.
const LINE_CHARS: usize = 200;

fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    match request.operation.as_str() {
        "explain" => explain(request, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

/// One stack frame or error location.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Frame {
    file: String,
    line: Option<u64>,
    column: Option<u64>,
    function: Option<String>,
}

/// The error the output shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Diagnostic {
    /// `rust`, `python`, `javascript`, `typescript`, `go`, `java`, `c`,
    /// or `unknown`.
    language: &'static str,
    /// The error's kind: a code (`E0308`, `TS2322`), an exception class
    /// (`KeyError`), `panic`, or `error`.
    kind: String,
    message: String,
    /// The frames in the order the output prints them.
    frames: Vec<Frame>,
    /// Lines the output adds under the error: rustc's `expected …, found
    /// …`, notes, and help.
    notes: Vec<String>,
    /// Whether frames run outermost first (Python), so the innermost is
    /// last.
    outermost_first: bool,
    /// The line of the output the error starts on, from 1.
    at: usize,
}

/// A granted file the guest may read.
#[derive(Debug, Clone)]
struct Granted {
    path: String,
    handle: String,
}

fn explain(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let input = &request.input;
    let max_frames = bounded(input, "max_frames", DEFAULT_MAX_FRAMES, MAX_FRAMES_CAP);
    let context = bounded(input, "context_lines", DEFAULT_CONTEXT, CONTEXT_CAP);
    let mut text = match &input["text"] {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        _ => return Err(Refusal::unsupported("text is a string")),
    };
    let mut truncated = input["text_truncated"].as_bool().unwrap_or(false);
    let mut notes: Vec<String> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    let mut reads = 0_usize;

    let granted: Vec<Granted> = match guest::root(request) {
        Some(root) => guest::list(host, &root, MAX_LISTED)
            .map_err(|code| Refusal::refused(format!("list {code}")))?
            .entries
            .into_iter()
            .filter(|entry| entry.kind == "file")
            .map(|entry| Granted {
                path: guest::relative(&entry.name).to_string(),
                handle: entry.handle,
            })
            .collect(),
        None => Vec::new(),
    };

    // Saved output the request named: a granted file that isn't source.
    let mut sources_read: Vec<String> = Vec::new();
    for file in granted.iter().filter(|file| is_output_file(&file.path)) {
        if reads >= MAX_READS {
            break;
        }
        reads += 1;
        match read_some(host, &file.handle, LOG_BYTES) {
            Ok(read) => {
                if !read.complete {
                    truncated = true;
                }
                text.push('\n');
                text.push_str(&String::from_utf8_lossy(&read.bytes));
                sources_read.push(file.path.clone());
            }
            Err(_) => unread.push(file.path.clone()),
        }
    }

    let diagnostics = diagnostics(&text);
    let Some(first) = diagnostics.first().cloned() else {
        let markdown = "I didn't find an error message I recognize in the output. Paste the \
                        command's full output, from the first `error` line to the end of the \
                        stack trace, and name the command."
            .to_string();
        return Ok(json!({
            "kind": "explain-error",
            "found": false,
            "read": sources_read,
            "unread": unread,
            "truncated": truncated,
            "markdown": markdown,
        }));
    };
    let more = diagnostics.len() - 1;

    // The frames, each matched to a granted file when one is the same
    // file.
    let frames: Vec<(Frame, Option<&Granted>)> = first
        .frames
        .iter()
        .take(max_frames)
        .map(|frame| (frame.clone(), find(&granted, &frame.file)))
        .collect();
    if first.frames.len() > max_frames {
        truncated = true;
    }
    let responsible = responsible(&first, &frames);

    // The responsible frame's code.
    let mut excerpt: Option<Value> = None;
    let mut code_line: Option<String> = None;
    let mut source_text: Option<String> = None;
    let mut function: Option<String> = None;
    if let Some(index) = responsible
        && let (frame, Some(file)) = &frames[index]
    {
        if reads < MAX_READS {
            match read_some(host, &file.handle, SOURCE_BYTES) {
                Ok(read) => {
                    if !read.complete {
                        truncated = true;
                        notes.push(format!(
                            "Read the first {} KiB of {} only.",
                            SOURCE_BYTES / 1024,
                            file.path
                        ));
                    }
                    let text = String::from_utf8_lossy(&read.bytes).into_owned();
                    if let Some(line) = frame.line {
                        let lines: Vec<&str> = text.lines().collect();
                        let at = usize::try_from(line).unwrap_or(usize::MAX);
                        if at == 0 || at > lines.len() {
                            notes.push(format!(
                                "{} has {} lines here and the output names line {line}: the \
                                 file may have changed since the command ran.",
                                file.path,
                                lines.len()
                            ));
                        } else {
                            code_line = Some(lines[at - 1].to_string());
                            function = frame
                                .function
                                .clone()
                                .or_else(|| enclosing_function(&lines, at));
                            excerpt = Some(excerpt_of(&file.path, &lines, at, context));
                        }
                    }
                    source_text = Some(text);
                }
                Err(_) => unread.push(file.path.clone()),
            }
        } else {
            unread.push(file.path.clone());
        }
    }

    let explained = diagnose(
        &first,
        code_line.as_deref(),
        source_text.as_deref(),
        excerpt.as_ref(),
    );
    let location = responsible.map(|index| {
        let (frame, file) = &frames[index];
        let mut object = Map::new();
        object.insert(
            "file".into(),
            json!(file.map_or(frame.file.as_str(), |file| file.path.as_str())),
        );
        if let Some(line) = frame.line {
            object.insert("line".into(), json!(line));
        }
        if let Some(column) = frame.column {
            object.insert("column".into(), json!(column));
        }
        if let Some(function) = &function {
            object.insert("function".into(), json!(function));
        }
        object.insert("in_workspace".into(), json!(file.is_some()));
        Value::Object(object)
    });
    if responsible.is_none() && !first.frames.is_empty() {
        notes.push(
            "None of the files in the output is in what this step could read, so the code \
             isn't shown. Run it in the repository the error came from."
                .to_string(),
        );
    }
    let frames_json: Vec<Value> = frames
        .iter()
        .map(|(frame, file)| {
            let mut object = Map::new();
            object.insert("file".into(), json!(frame.file));
            if let Some(line) = frame.line {
                object.insert("line".into(), json!(line));
            }
            if let Some(function) = &frame.function {
                object.insert("function".into(), json!(function));
            }
            object.insert("in_workspace".into(), json!(file.is_some()));
            Value::Object(object)
        })
        .collect();
    let markdown = render(
        &first,
        location.as_ref(),
        excerpt.as_ref(),
        &explained,
        more,
        &notes,
    );
    Ok(json!({
        "kind": "explain-error",
        "found": true,
        "language": first.language,
        "error": {
            "kind": first.kind,
            "message": first.message,
            "output_line": first.at,
            "notes": first.notes,
        },
        "location": location,
        "frames": frames_json,
        "excerpt": excerpt,
        "cause": explained.cause,
        "fix": explained.fix,
        "suggestions": explained.suggestions,
        "more_errors": more,
        "notes": notes,
        "read": sources_read,
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

fn bounded(input: &Value, key: &str, default: usize, cap: usize) -> usize {
    input[key]
        .as_u64()
        .map_or(default, |n| usize::try_from(n).unwrap_or(cap))
        .clamp(1, cap)
}

/// Whether a granted file holds saved output rather than source: a log,
/// a text file, or an `.out` file.
fn is_output_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    name.ends_with(".log") || name.ends_with(".txt") || name.ends_with(".out")
}

/// The granted file a frame names: the same relative path, or, for an
/// absolute or partial path, the granted file whose path is the longest
/// suffix of it, by whole components.
fn find<'a>(granted: &'a [Granted], file: &str) -> Option<&'a Granted> {
    let wanted = file.trim_start_matches("./");
    if let Some(exact) = granted.iter().find(|g| g.path == wanted) {
        return Some(exact);
    }
    granted
        .iter()
        .filter(|g| {
            wanted.ends_with(&format!("/{}", g.path)) || g.path.ends_with(&format!("/{wanted}"))
        })
        .max_by_key(|g| g.path.len())
}

/// The frame the error is in: the location of a compile error, or the
/// innermost frame in the workspace for a stack trace.
fn responsible(diagnostic: &Diagnostic, frames: &[(Frame, Option<&Granted>)]) -> Option<usize> {
    let in_workspace: Vec<usize> = frames
        .iter()
        .enumerate()
        .filter(|(_, (frame, file))| file.is_some() && frame.line.is_some())
        .map(|(index, _)| index)
        .collect();
    let chosen = if diagnostic.outermost_first {
        in_workspace.last().copied()
    } else {
        in_workspace.first().copied()
    };
    chosen.or_else(|| {
        (!frames.is_empty()).then_some(if diagnostic.outermost_first {
            frames.len() - 1
        } else {
            0
        })
    })
}

// ---------------------------------------------------------------------------
// Reading the output
// ---------------------------------------------------------------------------

/// Every error the output shows, in the order it shows them.
fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let parsed = rustc(&lines, at)
            .or_else(|| rust_panic(&lines, at))
            .or_else(|| python(&lines, at))
            .or_else(|| go_panic(&lines, at))
            .or_else(|| tsc(&lines, at))
            .or_else(|| thrown(&lines, at))
            .or_else(|| located(&lines, at));
        match parsed {
            Some((diagnostic, next)) => {
                found.push(diagnostic);
                at = next.max(at + 1);
            }
            None => at += 1,
        }
    }
    found
}

fn clip(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// `path:line[:col]`, or `path(line,col)`, split.
fn location(text: &str) -> Option<(String, Option<u64>, Option<u64>)> {
    let text = text.trim().trim_end_matches(':');
    if let Some((path, rest)) = text.split_once('(')
        && let Some(numbers) = rest.strip_suffix(')')
    {
        let mut parts = numbers.split(',');
        let line = parts.next()?.trim().parse().ok()?;
        let column = parts.next().and_then(|c| c.trim().parse().ok());
        return looks_like_path(path).then(|| (path.to_string(), Some(line), column));
    }
    let mut parts: Vec<&str> = text.rsplitn(3, ':').collect();
    parts.reverse();
    match parts.as_slice() {
        [path, line, column] if column.parse::<u64>().is_ok() && line.parse::<u64>().is_ok() => {
            looks_like_path(path)
                .then(|| ((*path).to_string(), line.parse().ok(), column.parse().ok()))
        }
        [path, line, column] if line.parse::<u64>().is_ok() => {
            let path = format!("{path}:{line}");
            let _ = column;
            looks_like_path(&path).then_some((path, None, None))
        }
        [first, second, third] => {
            // `path:line` where the path itself had no colon: rsplitn put
            // part of the path in `first`.
            let path = format!("{first}:{second}");
            third
                .parse::<u64>()
                .ok()
                .filter(|_| looks_like_path(&path))
                .map(|line| (path, Some(line), None))
        }
        [path, line] => line
            .parse::<u64>()
            .ok()
            .filter(|_| looks_like_path(path))
            .map(|line| ((*path).to_string(), Some(line), None)),
        _ => None,
    }
}

/// Whether text is shaped like a source path: a name with an extension,
/// no spaces.
fn looks_like_path(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() || path.contains(' ') || path.contains("://") {
        return false;
    }
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && (1..=6).contains(&extension.len())
            && extension.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

fn language_of(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or_default();
    match extension {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "go" => "go",
        "java" | "kt" => "java",
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => "c",
        _ => "unknown",
    }
}

/// A `rustc` error: `error[E0308]: mismatched types` and its ` --> `
/// location, notes, and help.
fn rustc(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let line = lines[at].trim_start();
    let rest = line.strip_prefix("error")?;
    let (kind, message) = if let Some(coded) = rest.strip_prefix('[') {
        let (code, message) = coded.split_once("]:")?;
        (code.to_string(), message.trim())
    } else {
        ("error".to_string(), rest.strip_prefix(':')?.trim())
    };
    if message.starts_with("aborting due to")
        || message.starts_with("could not compile")
        || message.starts_with("Recipe")
    {
        return None;
    }
    let mut diagnostic = Diagnostic {
        language: "rust",
        kind,
        message: clip(message, MESSAGE_CHARS),
        at: at + 1,
        ..Diagnostic::default()
    };
    let mut next = at + 1;
    while next < lines.len() {
        let current = lines[next];
        let trimmed = current.trim();
        if trimmed.starts_with("error") || trimmed.starts_with("warning") {
            break;
        }
        if let Some(place) = trimmed.strip_prefix("--> ")
            && diagnostic.frames.is_empty()
            && let Some((file, line, column)) = location(place)
        {
            diagnostic.frames.push(Frame {
                file,
                line,
                column,
                function: None,
            });
        } else if let Some(note) = trimmed
            .strip_prefix("= note: ")
            .or_else(|| trimmed.strip_prefix("= help: "))
        {
            diagnostic.notes.push(clip(note, MESSAGE_CHARS));
        } else if let Some(pipe) = trimmed.split_once("| ").map(|(_, rest)| rest)
            && (pipe.contains("expected ") || pipe.contains("help: "))
        {
            let note = pipe.trim_start_matches(['^', '-', ' ']).trim();
            if !note.is_empty() {
                diagnostic.notes.push(clip(note, MESSAGE_CHARS));
            }
        } else if trimmed.starts_with("expected ") || trimmed.starts_with("found ") {
            diagnostic.notes.push(clip(trimmed, MESSAGE_CHARS));
        }
        next += 1;
    }
    (!diagnostic.frames.is_empty() || diagnostic.kind.starts_with('E'))
        .then_some((diagnostic, next))
}

/// A Rust panic: `thread 'main' panicked at src/main.rs:5:10:` and the
/// message on the next line, or the older `panicked at 'msg', src/x.rs:5:10`.
fn rust_panic(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let line = lines[at].trim();
    let (_, rest) = line.split_once("panicked at ")?;
    let (message, place) = if let Some(old) = rest.strip_prefix('\'') {
        let (message, place) = old.rsplit_once("', ")?;
        (message.to_string(), place.to_string())
    } else {
        let place = rest.trim_end_matches(':').to_string();
        let message = lines.get(at + 1).map_or("", |next| next.trim()).to_string();
        (message, place)
    };
    let (file, line_number, column) = location(&place)?;
    Some((
        Diagnostic {
            language: "rust",
            kind: "panic".into(),
            message: clip(&message, MESSAGE_CHARS),
            frames: vec![Frame {
                file,
                line: line_number,
                column,
                function: None,
            }],
            at: at + 1,
            ..Diagnostic::default()
        },
        at + 2,
    ))
}

/// A Python traceback: its `File "...", line N, in f` frames, outermost
/// first, and the exception line that ends it.
fn python(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    if !lines[at]
        .trim()
        .starts_with("Traceback (most recent call last)")
    {
        return None;
    }
    let mut frames = Vec::new();
    let mut next = at + 1;
    while next < lines.len() {
        let current = lines[next];
        let trimmed = current.trim();
        if let Some(rest) = trimmed.strip_prefix("File \"") {
            if let Some((file, rest)) = rest.split_once('"') {
                let line = rest
                    .split_once("line ")
                    .and_then(|(_, n)| n.split(',').next())
                    .and_then(|n| n.trim().parse().ok());
                let function = rest.split_once(", in ").map(|(_, f)| f.trim().to_string());
                frames.push(Frame {
                    file: file.to_string(),
                    line,
                    column: None,
                    function,
                });
            }
            next += 1;
            continue;
        }
        if current.starts_with(' ') || current.starts_with('\t') || trimmed.is_empty() {
            next += 1;
            continue;
        }
        break;
    }
    let exception = lines.get(next)?.trim();
    let (kind, message) = match exception.split_once(": ") {
        Some((kind, message)) => (kind, message),
        None => (exception, ""),
    };
    let kind = kind.rsplit('.').next().unwrap_or(kind);
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((
        Diagnostic {
            language: "python",
            kind: kind.to_string(),
            message: clip(message, MESSAGE_CHARS),
            frames,
            outermost_first: true,
            at: next + 1,
            ..Diagnostic::default()
        },
        next + 1,
    ))
}

/// A Go panic and its goroutine trace: `panic: runtime error: …`, then
/// pairs of a function line and a `\tpath.go:12 +0x1d` line.
fn go_panic(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let message = lines[at].trim().strip_prefix("panic: ")?;
    let mut frames = Vec::new();
    let mut next = at + 1;
    let mut function: Option<String> = None;
    while next < lines.len() && frames.len() < MAX_FRAMES_CAP {
        let current = lines[next];
        let trimmed = current.trim();
        if trimmed.starts_with("goroutine ") || trimmed.is_empty() || trimmed.starts_with("[signal")
        {
            next += 1;
            continue;
        }
        if current.starts_with('\t') {
            let place = trimmed.split(" +0x").next().unwrap_or(trimmed);
            if let Some((file, line, _)) = location(place) {
                frames.push(Frame {
                    file,
                    line,
                    column: None,
                    function: function.take(),
                });
            }
        } else if trimmed.contains('(') {
            function = Some(
                trimmed
                    .split('(')
                    .next()
                    .unwrap_or(trimmed)
                    .rsplit('.')
                    .next()
                    .unwrap_or(trimmed)
                    .to_string(),
            );
        } else {
            break;
        }
        next += 1;
    }
    Some((
        Diagnostic {
            language: "go",
            kind: "panic".into(),
            message: clip(message, MESSAGE_CHARS),
            frames,
            at: at + 1,
            ..Diagnostic::default()
        },
        next,
    ))
}

/// A `tsc` error: `src/a.ts(4,5): error TS2322: …` or
/// `src/a.ts:4:5 - error TS2322: …`.
fn tsc(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let line = lines[at].trim();
    let (place, rest) = line
        .split_once(": error TS")
        .or_else(|| line.split_once(" - error TS"))?;
    let (code, message) = rest.split_once(": ")?;
    let (file, line_number, column) = location(place)?;
    Some((
        Diagnostic {
            language: "typescript",
            kind: format!("TS{code}"),
            message: clip(message, MESSAGE_CHARS),
            frames: vec![Frame {
                file,
                line: line_number,
                column,
                function: None,
            }],
            at: at + 1,
            ..Diagnostic::default()
        },
        at + 1,
    ))
}

/// A thrown exception with a stack below it: JavaScript's `TypeError:
/// …` and `at f (path:1:2)` frames, or Java's `java.lang.X: …` and
/// `at pkg.Class.f(File.java:12)` frames.
fn thrown(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let line = lines[at].trim();
    let line = line
        .strip_prefix("Uncaught ")
        .or_else(|| {
            line.split_once("Exception in thread ")
                .and_then(|(_, rest)| rest.split_once("\" ").map(|(_, rest)| rest))
        })
        .unwrap_or(line);
    let (name, message) = match line.split_once(": ") {
        Some((name, message)) => (name, message),
        None => (line, ""),
    };
    let short = name.rsplit('.').next().unwrap_or(name);
    let is_error = (short.ends_with("Error") || short.ends_with("Exception"))
        && short
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && !name.contains(' ');
    if !is_error {
        return None;
    }
    let mut frames = Vec::new();
    let mut next = at + 1;
    let mut language = "javascript";
    while next < lines.len() {
        let trimmed = lines[next].trim();
        let Some(rest) = trimmed.strip_prefix("at ") else {
            if trimmed.is_empty() && frames.is_empty() {
                next += 1;
                continue;
            }
            break;
        };
        let (function, place) = match rest.rsplit_once(" (") {
            Some((function, place)) => (Some(function.to_string()), place.trim_end_matches(')')),
            None => match rest.split_once('(') {
                // Java: `pkg.Class.method(File.java:12)`.
                Some((function, place)) => {
                    language = "java";
                    (
                        Some(function.rsplit('.').next().unwrap_or(function).to_string()),
                        place.trim_end_matches(')'),
                    )
                }
                None => (None, rest),
            },
        };
        let place = place.strip_prefix("file://").unwrap_or(place);
        if let Some((file, line, column)) = location(place) {
            frames.push(Frame {
                file,
                line,
                column,
                function: function
                    .filter(|f| !f.is_empty() && !f.contains(' ') || f.starts_with("async ")),
            });
        }
        next += 1;
    }
    if frames.is_empty() {
        return None;
    }
    if frames
        .iter()
        .any(|frame| language_of(&frame.file) == "typescript")
    {
        language = "typescript";
    } else if frames
        .iter()
        .any(|frame| language_of(&frame.file) == "java")
    {
        language = "java";
    }
    Some((
        Diagnostic {
            language,
            kind: short.to_string(),
            message: clip(message, MESSAGE_CHARS),
            frames,
            at: at + 1,
            ..Diagnostic::default()
        },
        next,
    ))
}

/// `path:line[:col]: [error:] message`, the shape gcc, clang, Go, and many
/// linters print. Only lines that say error, or come from Go's compiler,
/// count.
fn located(lines: &[&str], at: usize) -> Option<(Diagnostic, usize)> {
    let line = lines[at].trim();
    let (place, message) = line.split_once(": ")?;
    let (file, line_number, column) = location(place)?;
    let language = language_of(&file);
    let (kind, message) = if let Some(rest) = message
        .strip_prefix("error: ")
        .or_else(|| message.strip_prefix("fatal error: "))
    {
        ("error", rest)
    } else if language == "go" && !message.starts_with("warning") {
        ("error", message)
    } else if message.starts_with("Error") || message.ends_with("Error") {
        let (kind, rest) = message.split_once(": ").unwrap_or((message, ""));
        if kind.contains(' ') {
            return None;
        }
        (kind, rest)
    } else {
        return None;
    };
    Some((
        Diagnostic {
            language,
            kind: kind.to_string(),
            message: clip(message, MESSAGE_CHARS),
            frames: vec![Frame {
                file,
                line: line_number,
                column,
                function: None,
            }],
            at: at + 1,
            ..Diagnostic::default()
        },
        at + 1,
    ))
}

// ---------------------------------------------------------------------------
// Reading the code
// ---------------------------------------------------------------------------

fn excerpt_of(path: &str, lines: &[&str], at: usize, context: usize) -> Value {
    let start = at.saturating_sub(context).max(1);
    let end = (at + context).min(lines.len());
    let shown: Vec<Value> = (start..=end)
        .map(|number| {
            json!({
                "line": number,
                "text": clip(lines[number - 1], LINE_CHARS),
            })
        })
        .collect();
    json!({"file": path, "line": at, "lines": shown})
}

/// The function a line is in: the nearest definition above it.
fn enclosing_function(lines: &[&str], at: usize) -> Option<String> {
    for line in lines[..at].iter().rev() {
        let trimmed = line.trim_start();
        for keyword in [
            "def ",
            "async def ",
            "fn ",
            "pub fn ",
            "pub(crate) fn ",
            "async fn ",
            "pub async fn ",
            "func ",
            "function ",
            "async function ",
            "export function ",
            "export async function ",
        ] {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                let rest = rest.trim_start_matches(|c: char| c == '(' || c.is_whitespace());
                // Go methods: `func (r *T) name(`.
                let rest = if keyword == "func " && trimmed.starts_with("func (") {
                    rest.split_once(") ").map_or(rest, |(_, name)| name)
                } else {
                    rest
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// Identifiers in `text` and the line each first appears on.
fn identifiers(text: &str) -> Vec<(String, usize)> {
    let mut seen: Vec<(String, usize)> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let mut word = String::new();
        for c in line.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                word.push(c);
            } else if !word.is_empty() {
                let starts_well = word
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
                if starts_well && !seen.iter().any(|(known, _)| *known == word) {
                    seen.push((word.clone(), number + 1));
                }
                word.clear();
            }
        }
    }
    seen
}

/// String literals in `text` used as keys: `["key"]`, `.get("key")`, and
/// `"key":`, each with the line it first appears on.
fn keys(text: &str) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        for quote in ['"', '\''] {
            let parts: Vec<&str> = line.split(quote).collect();
            let mut index = 1;
            while index + 1 < parts.len() {
                let before = parts[index - 1].trim_end();
                let after = parts[index + 1].trim_start();
                let literal = parts[index];
                let keyed = before.ends_with('[')
                    || before.ends_with(".get(")
                    || before.ends_with("get(")
                    || after.starts_with(':')
                    || after.starts_with(']');
                if keyed
                    && !literal.is_empty()
                    && literal.len() <= 64
                    && literal
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    && !found.iter().any(|(known, _)| known == literal)
                {
                    found.push((literal.to_string(), number + 1));
                }
                index += 2;
            }
        }
    }
    found
}

/// Edit distance between two short names, case-insensitive.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_ascii_lowercase().chars().collect();
    let b: Vec<char> = b.to_ascii_lowercase().chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current.push(
                (previous[j] + cost)
                    .min(previous[j + 1] + 1)
                    .min(current[j] + 1),
            );
        }
        previous = current;
    }
    previous[b.len()]
}

/// The candidates close to `name`, closest first, at most three.
fn close_to(name: &str, candidates: &[(String, usize)]) -> Vec<(String, usize)> {
    let limit = (name.chars().count() / 3).clamp(1, 3);
    let mut close: Vec<(usize, String, usize)> = candidates
        .iter()
        .filter(|(candidate, _)| candidate != name)
        .map(|(candidate, line)| (distance(name, candidate), candidate.clone(), *line))
        .filter(|(d, candidate, _)| {
            *d <= limit
                || abbreviates(name, candidate)
                || candidate.eq_ignore_ascii_case(name)
                || (name.len() >= 4
                    && (candidate.starts_with(name) || name.starts_with(candidate.as_str()))
                    && candidate.len() >= 4)
        })
        .collect();
    close.sort();
    close
        .into_iter()
        .take(3)
        .map(|(_, candidate, line)| (candidate, line))
        .collect()
}

/// Whether `short` abbreviates `long`: the same first letter, and every
/// letter of `short` appears in `long` in order (`qty` and `quantity`,
/// `amt` and `amount`).
fn abbreviates(short: &str, long: &str) -> bool {
    let short = short.to_ascii_lowercase();
    let long = long.to_ascii_lowercase();
    if short.len() < 2 || long.len() <= short.len() || short.chars().next() != long.chars().next() {
        return false;
    }
    let mut rest = long.chars();
    short.chars().all(|c| rest.any(|l| l == c))
}

/// The first name a message quotes, in backticks or single or double
/// quotes.
fn quoted(message: &str) -> Option<String> {
    for quote in ['`', '\'', '"'] {
        let mut parts = message.split(quote);
        parts.next();
        if let Some(inner) = parts.next()
            && !inner.is_empty()
            && inner.len() <= 80
        {
            return Some(inner.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Explaining
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Explained {
    cause: String,
    fix: String,
    suggestions: Vec<Value>,
}

/// The line in the excerpt that compares against a length with `<=` or
/// an inclusive range, which runs one past the end.
fn one_past_the_end(excerpt: Option<&Value>) -> Option<(u64, String)> {
    let lines = excerpt?["lines"].as_array()?;
    lines.iter().find_map(|line| {
        let text = line["text"].as_str()?;
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let lengthy = ["len(", ".len()", ".length", ".size()", ".count"]
            .iter()
            .any(|length| compact.contains(length));
        let inclusive = compact.contains("<=") || compact.contains("..=");
        let plus_one = compact.contains("range(len(") && compact.contains(")+1)");
        ((lengthy && inclusive) || plus_one)
            .then(|| (line["line"].as_u64().unwrap_or(0), text.trim().to_string()))
    })
}

/// The expression the code reads `.property` from on this line.
fn read_from(code: &str, property: &str) -> Option<String> {
    for accessor in [format!("?.{property}"), format!(".{property}")] {
        if let Some((before, _)) = code.split_once(accessor.as_str()) {
            let expression: String = before
                .chars()
                .rev()
                .take_while(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | ']' | '[')
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let expression = expression.trim_start_matches('.').to_string();
            if !expression.is_empty() {
                return Some(expression);
            }
        }
    }
    None
}

/// The divisor on a line that divides.
fn divisor(code: &str) -> Option<String> {
    for operator in [" / ", " // ", " % ", "/", "%"] {
        if let Some((_, after)) = code.split_once(operator) {
            let name: String = after
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '(' | ')'))
                .collect();
            let name = name.trim_end_matches(')').to_string();
            if !name.is_empty() && !name.chars().all(|c| c.is_ascii_digit()) {
                return Some(name);
            }
        }
    }
    None
}

fn suggest(names: &[(String, usize)], file: Option<&str>) -> Vec<Value> {
    names
        .iter()
        .map(|(name, line)| match file {
            Some(file) => json!({"name": name, "file": file, "line": line}),
            None => json!({"name": name}),
        })
        .collect()
}

fn did_you_mean(names: &[(String, usize)]) -> String {
    match names {
        [] => String::new(),
        [(one, line)] => format!(" Did you mean `{one}` (line {line})?"),
        many => format!(
            " Close names here: {}.",
            many.iter()
                .map(|(name, line)| format!("`{name}` (line {line})"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The cause and fix for a diagnostic, with what the code adds.
#[allow(clippy::too_many_lines)]
fn diagnose(
    diagnostic: &Diagnostic,
    code: Option<&str>,
    source: Option<&str>,
    excerpt: Option<&Value>,
) -> Explained {
    let message = diagnostic.message.as_str();
    let lower = message.to_ascii_lowercase();
    let file = excerpt.and_then(|e| e["file"].as_str());
    let names = source.map(identifiers).unwrap_or_default();
    let missing_name = |name: &str| -> Explained {
        let close = close_to(name, &names);
        Explained {
            cause: format!("`{name}` isn't defined where this line uses it."),
            fix: format!(
                "Define or import `{name}`, or fix the name.{}",
                did_you_mean(&close)
            ),
            suggestions: suggest(&close, file),
        }
    };
    let off_by_one = one_past_the_end(excerpt);
    let index_fix = |what: &str| -> Explained {
        match &off_by_one {
            Some((line, text)) => Explained {
                cause: format!(
                    "{what} Line {line} (`{text}`) goes up to and including the length, but \
                     the last valid index is the length minus one."
                ),
                fix: "Stop one earlier: use `<` instead of `<=` (or an exclusive range).".into(),
                suggestions: Vec::new(),
            },
            None => Explained {
                cause: format!("{what} The index is past the end, or the collection is empty."),
                fix: "Check the length before indexing, or iterate over the items directly.".into(),
                suggestions: Vec::new(),
            },
        }
    };
    let kind = diagnostic.kind.as_str();
    match (diagnostic.language, kind) {
        // Rust compile errors.
        ("rust", "E0308") => {
            let detail = diagnostic
                .notes
                .iter()
                .find(|note| note.contains("expected") && note.contains("found"))
                .map(|note| format!(" ({note})"))
                .unwrap_or_default();
            Explained {
                cause: format!("The value here has a different type than the code expects{detail}."),
                fix: "Convert the value (for example `.parse()`, `.into()`, `as`, `&`, or `*`), or change the declared type so both sides agree.".into(),
                suggestions: Vec::new(),
            }
        }
        ("rust", "E0425" | "E0412" | "E0422" | "E0423") => {
            quoted(message).map_or_else(|| Explained {
                cause: "A name this line uses isn't defined in scope.".into(),
                fix: "Define it, import it with `use`, or fix the spelling.".into(),
                suggestions: Vec::new(),
            }, |name| missing_name(&name))
        }
        ("rust", "E0432" | "E0433") => Explained {
            cause: format!("A path in this line doesn't resolve{}.", quoted(message).map(|n| format!(": `{n}`")).unwrap_or_default()),
            fix: "Add the missing `use`, declare the module with `mod`, or add the crate to Cargo.toml.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "E0599") => {
            let method = quoted(message).unwrap_or_else(|| "the method".into());
            let close = close_to(&method, &names);
            Explained {
                cause: format!("The type has no method `{method}` in scope."),
                fix: format!("Fix the method name, or bring the trait that provides it into scope with `use`.{}", did_you_mean(&close)),
                suggestions: suggest(&close, file),
            }
        }
        ("rust", "E0382") => Explained {
            cause: format!("{} was moved earlier, then used again here.", quoted(message).map_or("A value".into(), |n| format!("`{n}`"))),
            fix: "Borrow it (`&value`) instead of moving it, clone it before the move, or restructure so the move comes last.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "E0499" | "E0502" | "E0506") => Explained {
            cause: "Two borrows of the same value overlap, and one of them is mutable.".into(),
            fix: "Finish using the first borrow before the second starts (often by copying out the value you need), or split the data so the borrows don't overlap.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "E0384" | "E0596") => Explained {
            cause: "The code changes a binding that isn't mutable.".into(),
            fix: "Declare it with `let mut`, or take a `&mut` reference.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "E0277") => Explained {
            cause: "A type here doesn't implement a trait the code needs.".into(),
            fix: "Implement or derive the trait for the type, or convert the value to a type that has it.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "E0061" | "E0060") => Explained {
            cause: "The call passes a different number of arguments than the function takes.".into(),
            fix: "Match the call to the function's signature.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "panic") if lower.contains("unwrap()") || lower.contains("expect") => Explained {
            cause: format!("An `unwrap` or `expect` here met {}.", if lower.contains("none") { "`None`" } else { "an `Err`" }),
            fix: "Handle the missing case with `match`, `if let`, `?`, or `unwrap_or`, or make sure the value is present before this line.".into(),
            suggestions: Vec::new(),
        },
        ("rust", "panic") if lower.contains("index out of bounds") => index_fix("The code indexes past the end of a collection."),
        ("rust", "panic") if lower.contains("divide by zero") => zero(code),
        ("rust", "panic") if lower.contains("overflow") => Explained {
            cause: "An arithmetic operation overflowed its integer type.".into(),
            fix: "Use `checked_`, `saturating_`, or `wrapping_` arithmetic, or a wider type.".into(),
            suggestions: Vec::new(),
        },
        // Python.
        ("python", "KeyError") => {
            let key = quoted(message).unwrap_or_else(|| message.trim().to_string());
            let known = source.map(keys).unwrap_or_default();
            let close = close_to(&key, &known);
            let nearby = if close.is_empty() {
                known.iter().take(5).map(|(k, _)| format!("`{k}`")).collect::<Vec<_>>().join(", ")
            } else {
                String::new()
            };
            Explained {
                cause: format!(
                    "The dictionary has no key `{key}`.{}",
                    if nearby.is_empty() { String::new() } else { format!(" Keys this file uses: {nearby}.") }
                ),
                fix: format!(
                    "Use the key the data actually has, or read it with `.get(\"{key}\")` and handle a missing key.{}",
                    did_you_mean(&close)
                ),
                suggestions: suggest(&close, file),
            }
        }
        ("python", "NameError" | "UnboundLocalError") => quoted(message)
            .map_or_else(|| Explained {
                cause: "A name this line uses isn't defined.".into(),
                fix: "Define or import it.".into(),
                suggestions: Vec::new(),
            }, |name| missing_name(&name)),
        ("python", "AttributeError") if message.contains("'NoneType'") => Explained {
            cause: format!(
                "A value is `None` where this line expects an object{}.",
                code.and_then(|c| attribute_read(c, message)).map(|e| format!(": `{e}`")).unwrap_or_default()
            ),
            fix: "Find where it was set (often a function that returned nothing or a lookup that found nothing) and handle the `None` case.".into(),
            suggestions: Vec::new(),
        },
        ("python", "AttributeError") => {
            let attribute = message.rsplit('\'').nth(1).unwrap_or_default().to_string();
            let close = close_to(&attribute, &names);
            Explained {
                cause: format!("The object has no attribute `{attribute}`."),
                fix: format!("Fix the attribute name, or check the object's type.{}", did_you_mean(&close)),
                suggestions: suggest(&close, file),
            }
        }
        ("python", "ZeroDivisionError") => zero(code),
        ("python", "IndexError") => index_fix("The code indexes past the end of a list."),
        ("python", "TypeError") if lower.contains("unsupported operand") || lower.contains("can only concatenate") || lower.contains("must be str") => Explained {
            cause: format!("The operation mixes types that don't combine: {message}."),
            fix: "Convert one side first (for example `int(...)` or `str(...)`), or fix where the value got the wrong type.".into(),
            suggestions: Vec::new(),
        },
        ("python", "TypeError") if lower.contains("nonetype") => Explained {
            cause: "A value is `None` where this line needs something else.".into(),
            fix: "Find where it was set to `None` and handle that case before this line.".into(),
            suggestions: Vec::new(),
        },
        ("python", "TypeError") if lower.contains("argument") => Explained {
            cause: format!("The call doesn't match the function's parameters: {message}."),
            fix: "Pass the arguments the function's signature asks for.".into(),
            suggestions: Vec::new(),
        },
        ("python", "ValueError") if lower.contains("invalid literal") => Explained {
            cause: "The code converts text to a number, and the text isn't a number.".into(),
            fix: "Validate or clean the input before converting, and handle the error.".into(),
            suggestions: Vec::new(),
        },
        ("python", "ModuleNotFoundError" | "ImportError") => Explained {
            cause: format!("Python can't import {}.", quoted(message).map_or("the module".into(), |n| format!("`{n}`"))),
            fix: "Install the package into the environment that runs this, or fix the module path.".into(),
            suggestions: Vec::new(),
        },
        ("python", "FileNotFoundError") => Explained {
            cause: "The path the code opens doesn't exist from where the command ran.".into(),
            fix: "Build the path from the script's location (`pathlib.Path(__file__).parent`), or run the command from the expected directory.".into(),
            suggestions: Vec::new(),
        },
        ("python", "AssertionError") => Explained {
            cause: format!("An assertion failed{}.", code.map(|c| format!(": `{}`", c.trim())).unwrap_or_default()),
            fix: "Compare the asserted values: either the code computes the wrong value, or the expectation is out of date.".into(),
            suggestions: Vec::new(),
        },
        ("python", "RecursionError") => Explained {
            cause: "A function calls itself without reaching a base case.".into(),
            fix: "Add or fix the base case so the recursion stops.".into(),
            suggestions: Vec::new(),
        },
        // JavaScript and TypeScript at run time.
        ("javascript" | "typescript", "TypeError") if lower.contains("cannot read propert") || lower.contains("is undefined") || lower.contains("is null") => {
            let property = message
                .split_once("(reading '")
                .and_then(|(_, rest)| rest.split_once('\''))
                .map(|(property, _)| property.to_string())
                .or_else(|| quoted(message));
            let empty = if lower.contains("null") { "null" } else { "undefined" };
            let expression = property
                .as_deref()
                .and_then(|property| code.and_then(|code| read_from(code, property)));
            Explained {
                cause: match (&expression, &property) {
                    (Some(expression), Some(property)) => format!("`{expression}` is `{empty}` when this line reads `{expression}.{property}`."),
                    (None, Some(property)) => format!("The object this line reads `{property}` from is `{empty}`."),
                    _ => format!("A value this line reads from is `{empty}`."),
                },
                fix: match (&expression, &property) {
                    (Some(expression), Some(property)) => format!("Make sure `{expression}` is set before this line (check where it comes from), or guard the read with `{expression}?.{property}`."),
                    _ => "Make sure the value is set before this line, or guard the read with optional chaining (`?.`).".into(),
                },
                suggestions: Vec::new(),
            }
        }
        ("javascript" | "typescript", "TypeError") if lower.contains("is not a function") => {
            let name = message.split(" is not a function").next().unwrap_or_default().rsplit('.').next().unwrap_or_default().to_string();
            let close = close_to(&name, &names);
            Explained {
                cause: format!("`{name}` isn't a function where this line calls it: misspelled, not exported, or called on the wrong object."),
                fix: format!("Check the name and what it's called on.{}", did_you_mean(&close)),
                suggestions: suggest(&close, file),
            }
        }
        ("javascript" | "typescript", "ReferenceError") => {
            let name = message.split(" is not defined").next().unwrap_or_default().trim().to_string();
            missing_name(&name)
        }
        ("javascript" | "typescript", "RangeError") if lower.contains("call stack") => Explained {
            cause: "A function calls itself without reaching a base case.".into(),
            fix: "Add or fix the base case so the recursion stops.".into(),
            suggestions: Vec::new(),
        },
        // TypeScript compile errors.
        ("typescript", "TS2304" | "TS2552") => quoted(message).map_or_else(Explained::default, |name| missing_name(&name)),
        ("typescript", "TS2339") => {
            let property = quoted(message).unwrap_or_default();
            let close = close_to(&property, &names);
            Explained {
                cause: format!("The type has no property `{property}`."),
                fix: format!("Fix the property name, or add it to the type.{}", did_you_mean(&close)),
                suggestions: suggest(&close, file),
            }
        }
        ("typescript", "TS2322" | "TS2345") => Explained {
            cause: format!("The value's type doesn't match what's expected here: {message}"),
            fix: "Convert the value, or change the declared type so both sides agree.".into(),
            suggestions: Vec::new(),
        },
        ("typescript", "TS2531" | "TS2532" | "TS18047" | "TS18048") => Explained {
            cause: "The value may be `null` or `undefined` here.".into(),
            fix: "Check it first (`if (value)`), use optional chaining, or give it a default.".into(),
            suggestions: Vec::new(),
        },
        ("typescript", "TS2307") => Explained {
            cause: format!("TypeScript can't find the module {}.", quoted(message).map(|n| format!("`{n}`")).unwrap_or_default()),
            fix: "Install the package (and its types), or fix the import path.".into(),
            suggestions: Vec::new(),
        },
        // Go.
        ("go", "error") if lower.starts_with("undefined: ") => missing_name(message.trim_start_matches("undefined: ").trim()),
        ("go", "error") if lower.contains("declared and not used") || lower.contains("declared but not used") => Explained {
            cause: "A variable is declared and never used, which Go refuses to compile.".into(),
            fix: "Use it, remove it, or assign it to `_`.".into(),
            suggestions: Vec::new(),
        },
        ("go", "error") if lower.contains("imported and not used") => Explained {
            cause: "An import is never used, which Go refuses to compile.".into(),
            fix: "Remove the import, or use it.".into(),
            suggestions: Vec::new(),
        },
        ("go", "error") if lower.starts_with("cannot use ") => Explained {
            cause: format!("The value's type doesn't match what's expected here: {message}."),
            fix: "Convert the value, or change the declared type so both sides agree.".into(),
            suggestions: Vec::new(),
        },
        ("go", "panic") if lower.contains("index out of range") => index_fix("The code indexes past the end of a slice."),
        ("go", "panic") if lower.contains("nil pointer") || lower.contains("nil map") => Explained {
            cause: "The code uses a pointer or map that is `nil`.".into(),
            fix: "Initialize it before this line, or check for `nil` first.".into(),
            suggestions: Vec::new(),
        },
        ("go", "panic") if lower.contains("divide by zero") => zero(code),
        // Java.
        ("java", "NullPointerException") => Explained {
            cause: "The code uses a reference that is `null`.".into(),
            fix: "Initialize it before this line, or check for `null` first.".into(),
            suggestions: Vec::new(),
        },
        ("java", "ArrayIndexOutOfBoundsException" | "IndexOutOfBoundsException" | "StringIndexOutOfBoundsException") => index_fix("The code indexes past the end of an array or list."),
        ("java", "NumberFormatException") => Explained {
            cause: "The code converts text to a number, and the text isn't a number.".into(),
            fix: "Validate or clean the input before converting, and handle the exception.".into(),
            suggestions: Vec::new(),
        },
        // C and C++.
        ("c", "error") if lower.contains("undeclared") || lower.contains("was not declared") => quoted(message).map_or_else(Explained::default, |name| missing_name(&name)),
        _ => Explained::default(),
    }
    .or_generic(diagnostic)
}

impl Explained {
    fn or_generic(self, diagnostic: &Diagnostic) -> Self {
        if !self.cause.is_empty() {
            return self;
        }
        Explained {
            cause: if diagnostic.message.is_empty() {
                format!("`{}` was raised here.", diagnostic.kind)
            } else {
                format!("{}: {}", diagnostic.kind, diagnostic.message)
            },
            fix: "Read the code at this line against the message above; the notes the tool printed, if any, are below.".into(),
            suggestions: Vec::new(),
        }
    }
}

fn zero(code: Option<&str>) -> Explained {
    let divisor = code.and_then(divisor);
    Explained {
        cause: match &divisor {
            Some(name) => format!("`{name}` is zero when this line divides by it."),
            None => "The divisor on this line is zero.".into(),
        },
        fix: "Check for zero before dividing, or fix where the value comes from (an empty list's length is a common source).".into(),
        suggestions: Vec::new(),
    }
}

/// The expression a `'NoneType' object has no attribute 'x'` error read
/// `.x` from.
fn attribute_read(code: &str, message: &str) -> Option<String> {
    let attribute = message.rsplit('\'').nth(1)?;
    read_from(code, attribute)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn fence(language: &str) -> &str {
    match language {
        "unknown" => "",
        other => other,
    }
}

fn render(
    diagnostic: &Diagnostic,
    location: Option<&Value>,
    excerpt: Option<&Value>,
    explained: &Explained,
    more: usize,
    notes: &[String],
) -> String {
    let mut out = String::new();
    let label = if diagnostic.message.is_empty() {
        diagnostic.kind.clone()
    } else {
        format!("{}: {}", diagnostic.kind, diagnostic.message)
    };
    out.push_str(&format!("**Error:** `{}`\n", label.replace('`', "'")));
    if let Some(location) = location {
        let file = location["file"].as_str().unwrap_or_default();
        let line = location["line"]
            .as_u64()
            .map(|line| format!(":{line}"))
            .unwrap_or_default();
        let function = location["function"]
            .as_str()
            .map(|function| format!(", in `{function}`"))
            .unwrap_or_default();
        let note = if location["in_workspace"].as_bool() == Some(true) {
            ""
        } else {
            " (outside what this step could read)"
        };
        out.push_str(&format!("**Where:** `{file}{line}`{function}{note}\n"));
    }
    if let Some(excerpt) = excerpt {
        let marked = excerpt["line"].as_u64().unwrap_or(0);
        let lines = excerpt["lines"].as_array().cloned().unwrap_or_default();
        let width = lines
            .last()
            .and_then(|line| line["line"].as_u64())
            .map_or(1, |n| n.to_string().len());
        out.push_str(&format!("\n```{}\n", fence(diagnostic.language)));
        for line in &lines {
            let number = line["line"].as_u64().unwrap_or(0);
            let mark = if number == marked { ">" } else { " " };
            out.push_str(&format!(
                "{mark} {number:>width$} | {}\n",
                line["text"].as_str().unwrap_or_default()
            ));
        }
        out.push_str("```\n");
    }
    out.push_str(&format!("\n**Likely cause:** {}\n", explained.cause));
    out.push_str(&format!("**Likely fix:** {}\n", explained.fix));
    for note in diagnostic.notes.iter().take(3) {
        out.push_str(&format!("- compiler note: {note}\n"));
    }
    for note in notes {
        out.push_str(&format!("- {note}\n"));
    }
    if more > 0 {
        out.push_str(&format!(
            "\nThe output shows {more} more error{} after this one; fixing the first often clears the rest.\n",
            if more == 1 { "" } else { "s" }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_pdk::guest::MemoryHost;
    use std::path::Path;

    fn tree() -> MemoryHost {
        MemoryHost::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree"))
    }

    fn run(host: &mut MemoryHost, text: &str) -> Value {
        explain(&MemoryHost::request("explain", json!({"text": text})), host).unwrap()
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/output")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn a_python_key_error_names_the_line_and_the_close_key() {
        let value = run(&mut tree(), &fixture("python-keyerror.txt"));
        assert_eq!(value["found"], true);
        assert_eq!(value["language"], "python");
        assert_eq!(value["error"]["kind"], "KeyError");
        assert_eq!(value["location"]["file"], "shop/billing.py");
        assert_eq!(value["location"]["line"], 9);
        assert_eq!(value["location"]["function"], "line_total");
        assert_eq!(value["suggestions"][0]["name"], "quantity");
        let markdown = value["markdown"].as_str().unwrap();
        assert!(markdown.contains("`shop/billing.py:9`"), "{markdown}");
        assert!(markdown.contains("row[\"qty\"]"), "{markdown}");
        assert!(markdown.contains("Did you mean `quantity`"), "{markdown}");
    }

    #[test]
    fn a_rustc_error_reads_the_location_and_the_expected_type() {
        let value = run(&mut tree(), &fixture("rustc-mismatch.txt"));
        assert_eq!(value["language"], "rust");
        assert_eq!(value["error"]["kind"], "E0308");
        assert_eq!(value["location"]["file"], "src/ledger.rs");
        assert_eq!(value["location"]["line"], 9);
        assert_eq!(value["location"]["function"], "balance");
        assert_eq!(value["more_errors"], 1);
        assert!(
            value["cause"]
                .as_str()
                .unwrap()
                .contains("expected `u64`, found `&str`")
        );
    }

    #[test]
    fn a_rust_unknown_name_suggests_the_close_one() {
        let value = run(&mut tree(), &fixture("rustc-unknown.txt"));
        assert_eq!(value["error"]["kind"], "E0425");
        assert_eq!(value["suggestions"][0]["name"], "total_cents");
    }

    #[test]
    fn a_node_trace_names_the_undefined_expression() {
        let value = run(&mut tree(), &fixture("node-undefined.txt"));
        assert_eq!(value["language"], "javascript");
        assert_eq!(value["error"]["kind"], "TypeError");
        assert_eq!(value["location"]["file"], "web/cart.js");
        assert_eq!(value["location"]["line"], 4);
        assert!(
            value["cause"]
                .as_str()
                .unwrap()
                .contains("`order.customer` is `undefined`"),
            "{}",
            value["cause"]
        );
    }

    #[test]
    fn a_go_panic_finds_the_loop_that_runs_past_the_end() {
        let value = run(&mut tree(), &fixture("go-index.txt"));
        assert_eq!(value["language"], "go");
        assert_eq!(value["location"]["file"], "cmd/report/main.go");
        assert_eq!(value["location"]["line"], 8);
        assert!(
            value["cause"]
                .as_str()
                .unwrap()
                .contains("i <= len(scores)")
        );
        assert!(value["fix"].as_str().unwrap().contains("`<`"));
    }

    #[test]
    fn a_tsc_error_and_a_saved_log_are_read() {
        let value = run(&mut tree(), &fixture("tsc.txt"));
        assert_eq!(value["error"]["kind"], "TS2339");
        assert_eq!(value["location"]["file"], "web/api.ts");
        assert_eq!(value["suggestions"][0]["name"], "userId");
        let mut host = MemoryHost::new([
            ("ci/build.log", fixture("rustc-mismatch.txt").into_bytes()),
            (
                "src/ledger.rs",
                std::fs::read(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tree/src/ledger.rs"),
                )
                .unwrap(),
            ),
        ]);
        let value = run(&mut host, "the build failed, the output is in ci/build.log");
        assert_eq!(value["read"], json!(["ci/build.log"]));
        assert_eq!(value["location"]["file"], "src/ledger.rs");
    }

    #[test]
    fn output_without_an_error_says_so() {
        let value = run(&mut tree(), "hello, can you help me name this function?");
        assert_eq!(value["found"], false);
        assert!(value["markdown"].as_str().unwrap().contains("didn't find"));
    }

    #[test]
    fn locations_parse_in_every_shape() {
        assert_eq!(
            location("src/a.rs:12:5"),
            Some(("src/a.rs".into(), Some(12), Some(5)))
        );
        assert_eq!(location("a.py:3"), Some(("a.py".into(), Some(3), None)));
        assert_eq!(
            location("web/a.ts(4,5)"),
            Some(("web/a.ts".into(), Some(4), Some(5)))
        );
        assert_eq!(location("not a path:3"), None);
        assert_eq!(distance("qty", "quantity"), 5);
        assert_eq!(distance("totl", "total"), 1);
    }
}
