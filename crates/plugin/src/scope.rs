//! What a host hands a `snapshot-read` guest from the run's own request.
//!
//! A program's `module` step binding fixes its guest's input and read
//! scope. Two optional binding fields let a guest work on what the person
//! asked about, without widening anything the step could already reach:
//!
//! - `request` names an input field that receives the run's request text,
//!   held to [`REQUEST_BYTES`] by [`request_text`].
//! - `read_named` adds to the read scope each workspace file the request
//!   names that exists inside the workspace ([`named_paths`], then
//!   [`resolve`]), and each file a named log names in turn, at most
//!   [`MAX_NAMED`] in all. `read_present` adds each listed path that
//!   exists, where `read` refuses a missing one.
//!
//! Both the program runtime that runs a step and the replayer that reruns
//! it compute the scope and the input here, so a replay rebuilds the same
//! invocation from the same request and workspace.
//!
//! Finding paths in the request is deterministic parsing of a bounded
//! field after the program was selected: tokens shaped like a file path,
//! each kept only when that file is in the workspace. Nothing here reads
//! the request for intent.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

/// The most request bytes a guest's input carries.
pub const REQUEST_BYTES: usize = 32 * 1024;

/// The most paths a request adds to a read scope.
pub const MAX_NAMED: usize = 16;

/// The most candidate paths [`named_paths`] returns: more than
/// [`MAX_NAMED`], because most tokens shaped like a path in compiler output
/// (`self.total`, `e.g.`) name no file.
pub const MAX_CANDIDATES: usize = 128;

/// The longest token [`named_paths`] considers.
const MAX_TOKEN: usize = 512;

/// How many leading components an absolute path may lose while
/// [`resolve`] looks for it under the workspace: a path printed on another
/// machine, such as a CI runner's checkout, still names a file here.
const MAX_STRIP: usize = 12;

/// The request as a guest's input carries it: whole when it fits in
/// [`REQUEST_BYTES`], else its first and last halves around a marker line,
/// and whether it was cut. Both ends matter for a failing command's
/// output: the first error is near the top and the exception near the
/// bottom.
#[must_use]
pub fn request_text(request: &str) -> (String, bool) {
    if request.len() <= REQUEST_BYTES {
        return (request.to_string(), false);
    }
    let half = REQUEST_BYTES / 2;
    let mut head = half;
    while !request.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = request.len() - half;
    while !request.is_char_boundary(tail) {
        tail += 1;
    }
    (
        format!(
            "{}\n[... {} bytes left out ...]\n{}",
            &request[..head],
            tail - head,
            &request[tail..]
        ),
        true,
    )
}

/// The guest input a step's binding fixes, with the request under `key`
/// and `<key>_truncated` saying whether [`request_text`] cut it.
///
/// # Errors
///
/// A sentence when the fixed input is neither absent nor an object, so
/// there is nowhere to put the request.
pub fn with_request(input: &Value, key: &str, request: &str) -> Result<Value, String> {
    let mut object = match input {
        Value::Null => Map::new(),
        Value::Object(object) => object.clone(),
        _ => return Err("a module step that takes the request needs an object input".into()),
    };
    let (text, truncated) = request_text(request);
    object.insert(key.to_string(), Value::String(text));
    object.insert(format!("{key}_truncated"), Value::Bool(truncated));
    Ok(Value::Object(object))
}

/// Tokens in `text` shaped like a file path, in the order they first
/// appear, without duplicates, at most [`MAX_CANDIDATES`]: a relative path with
/// a `/` or a file extension, or an absolute one. A line and column
/// suffix (`:12`, `:12:5`, `(12,5)`) and the quotes, brackets, and
/// punctuation that surround a path in compiler and runtime output are
/// dropped. URLs and paths with `..` are not paths a workspace holds.
#[must_use]
pub fn named_paths(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let separators = |c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\''
                    | '`'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '{'
                    | '}'
                    | ','
                    | ';'
                    | '|'
                    | '='
            )
    };
    for token in text.split(separators) {
        if found.len() >= MAX_CANDIDATES {
            break;
        }
        // A `file://` URL is a path; every other URL isn't.
        let token = token.strip_prefix("file://").unwrap_or(token);
        if token.is_empty() || token.len() > MAX_TOKEN || token.contains("://") {
            continue;
        }
        let Some(path) = path_of(token) else {
            continue;
        };
        if !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// The path a token names, or `None` when it names none.
fn path_of(token: &str) -> Option<String> {
    let mut path = token.trim_end_matches(['.', ':', ',', '!', '?']);
    // A line and column suffix: `file.rs:12`, `file.rs:12:5`.
    while let Some((before, after)) = path.rsplit_once(':') {
        if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
            path = before;
        } else {
            break;
        }
    }
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty() || path.contains(':') || path.contains('\\') || path.contains('*') {
        return None;
    }
    if path.split('/').any(|part| part == "..") {
        return None;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, extension)| extension);
    let has_extension = extension.is_some_and(|extension| {
        (1..=10).contains(&extension.len())
            && extension.chars().all(|c| c.is_ascii_alphanumeric())
            && extension.chars().any(|c| c.is_ascii_alphabetic())
    });
    let shaped = path.starts_with('/') || has_extension || path.contains('/');
    let plain = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@'));
    (shaped && plain && name.contains(|c: char| c.is_ascii_alphanumeric()))
        .then(|| path.trim_end_matches('/').to_string())
}

/// The workspace-relative scope a step reads: `read` as the binding wrote
/// it, then each `present` path that exists, then each path the request
/// names that exists ([`named_paths`]), without duplicates and without a
/// path a directory already in the scope covers.
///
/// An absolute path the request names counts when it is inside `root`, or
/// when dropping some of its leading components leaves a path that exists
/// under `root`. A path that leaves the workspace, by `..` or a symlink, is
/// never added here; `read` paths are the binding's and keep their own
/// checks where the host captures them.
#[must_use]
pub fn resolve(
    root: &Path,
    read: &[String],
    present: &[String],
    request: Option<&str>,
) -> Vec<String> {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut scope: Vec<String> = read.to_vec();
    let inside = |relative: &str| -> bool {
        plain(relative)
            && root
                .join(relative)
                .canonicalize()
                .is_ok_and(|real| real.starts_with(&canonical))
    };
    for path in present {
        if inside(path) {
            push(&mut scope, path);
        }
    }
    if let Some(request) = request {
        let mut added: Vec<String> = Vec::new();
        let add = |text: &str, scope: &mut Vec<String>, added: &mut Vec<String>| {
            for named in named_paths(text) {
                if added.len() >= MAX_NAMED {
                    break;
                }
                let Some(relative) = relative_to(&named, root, &canonical) else {
                    continue;
                };
                if inside(&relative) && root.join(&relative).is_file() && push(scope, &relative) {
                    added.push(relative);
                }
            }
        };
        add(request, &mut scope, &mut added);
        // Saved output the request names (`build.log`) names files too: the
        // ones its errors point at. One level, within the same bound.
        let logs: Vec<String> = added
            .iter()
            .filter(|path| is_saved_output(path))
            .cloned()
            .collect();
        for log in logs {
            if let Ok(text) = read_head(&root.join(&log), LOG_BYTES) {
                add(&text, &mut scope, &mut added);
            }
        }
    }
    scope
}

/// The most bytes of a named log [`resolve`] reads for further paths.
pub const LOG_BYTES: usize = 64 * 1024;

/// Whether a path names saved command output: a `.log`, `.txt`, or
/// `.out` file.
#[must_use]
pub fn is_saved_output(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    [".log", ".txt", ".out"]
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// The first `limit` bytes of a file, as text.
fn read_head(path: &Path, limit: usize) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Adds `path` unless the scope already covers it; whether it was added.
fn push(scope: &mut Vec<String>, path: &str) -> bool {
    let covered = scope.iter().any(|held| {
        held == "." || held == path || path.starts_with(&format!("{}/", held.trim_end_matches('/')))
    });
    if covered {
        return false;
    }
    scope.push(path.to_string());
    true
}

/// Whether `path` is plain and relative: no root, no `.` or `..` part.
fn plain(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

/// A named path as a workspace-relative one, when it can be one.
fn relative_to(named: &str, root: &Path, canonical: &Path) -> Option<String> {
    if !named.starts_with('/') {
        return Some(named.to_string());
    }
    let absolute = PathBuf::from(named);
    for base in [root, canonical] {
        if let Ok(relative) = absolute.strip_prefix(base) {
            return relative.to_str().map(str::to_string);
        }
    }
    let parts: Vec<&str> = named.split('/').filter(|part| !part.is_empty()).collect();
    for drop in 1..parts.len().min(MAX_STRIP + 1) {
        let candidate = parts[drop..].join("/");
        if root.join(&candidate).is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_come_out_of_compiler_and_runtime_output() {
        let text = "error[E0308]: mismatched types\n --> src/billing.rs:14:22\n\
                    Traceback (most recent call last):\n  File \"/home/ci/work/app/handlers.py\", line 9, in main\n\
                    at total (file:///srv/app/lib/cart.js:31:7)\n\
                    web/app.ts(4,5): error TS2322\n\
                    ./cmd/main.go:12:3: undefined: Foo\n\
                    see https://example.com/a.rs and e.g. version 1.2.3 and ../secret.txt";
        assert_eq!(
            named_paths(text),
            [
                "src/billing.rs",
                "/home/ci/work/app/handlers.py",
                "/srv/app/lib/cart.js",
                "web/app.ts",
                "cmd/main.go",
                "e.g",
            ]
        );
    }

    #[test]
    fn a_long_request_keeps_both_ends() {
        let request = format!("first error{}last line", "x".repeat(REQUEST_BYTES * 2));
        let (text, truncated) = request_text(&request);
        assert!(truncated);
        assert!(text.starts_with("first error"));
        assert!(text.ends_with("last line"));
        assert!(text.len() < REQUEST_BYTES + 100);
        assert_eq!(request_text("short"), ("short".to_string(), false));
        let input = with_request(&serde_json::json!({"max": 2}), "text", "short").unwrap();
        assert_eq!(input["text"], "short");
        assert_eq!(input["text_truncated"], false);
        assert_eq!(input["max"], 2);
        assert!(with_request(&serde_json::json!([1]), "text", "x").is_err());
    }

    #[test]
    fn the_scope_keeps_only_files_inside_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("app")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("app/handlers.py"), "x").unwrap();
        std::fs::write(root.join("src/billing.rs"), "x").unwrap();
        std::fs::write(root.join("Cargo.lock"), "x").unwrap();
        let request = "--> src/billing.rs:14:22\nFile \"/home/ci/work/app/handlers.py\", line 9\n\
                       missing.rs:1 and /etc/passwd";
        let scope = resolve(
            root,
            &[],
            &["Cargo.lock".into(), "package-lock.json".into()],
            Some(request),
        );
        assert_eq!(scope, ["Cargo.lock", "src/billing.rs", "app/handlers.py"]);
        let covered = resolve(root, &["src".into()], &[], Some(request));
        assert_eq!(covered, ["src", "app/handlers.py"]);
        assert_eq!(resolve(root, &[".".into()], &[], Some(request)), ["."]);
        // A saved log the request names adds the files its errors name.
        std::fs::create_dir_all(root.join("ci")).unwrap();
        std::fs::write(
            root.join("ci/build.log"),
            "error\n --> src/billing.rs:3:1\n",
        )
        .unwrap();
        assert_eq!(
            resolve(root, &[], &[], Some("the output is in ci/build.log")),
            ["ci/build.log", "src/billing.rs"]
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/hosts", root.join("hosts.txt")).unwrap();
            assert!(resolve(root, &[], &["hosts.txt".into()], Some("hosts.txt")).is_empty());
        }
    }
}
