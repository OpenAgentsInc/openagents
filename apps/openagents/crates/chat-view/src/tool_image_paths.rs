//! The image files a tool call touched, found in its input and output, and
//! where they live on the computer that ran it. Expanded tool chips preview
//! them.

use std::sync::Arc;

use gpui::SharedString;
use openagents_proto::ToolCall;

/// Previews per chip; further paths stay in the text output.
pub const MAX_IMAGES_PER_TOOL: usize = 4;

const IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "tif", "tiff",
];

pub fn is_image_path(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && IMAGE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e))
    })
}

/// UNC (`\\host\share`, `//host/share`), device and verbatim (`\\.\`,
/// `\\?\`, `\??\`) and drive-relative (`C:x.png`) forms. Agent output names
/// them freely, and merely opening one can reach another host (on Windows
/// with the user's NTLM credentials), so they are never previewed. The
/// engine's jail refuses them as well.
pub fn is_remote_or_device_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let separator = |b: Option<&u8>| matches!(b, Some(b'/' | b'\\'));
    (separator(bytes.first()) && separator(bytes.get(1)))
        || path.starts_with(r"\??\")
        || (bytes.len() >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && !separator(bytes.get(2)))
}

pub fn push_path(out: &mut Vec<SharedString>, path: &str) {
    if out.len() < MAX_IMAGES_PER_TOOL
        && is_image_path(path)
        && !is_remote_or_device_path(path)
        && !out.iter().any(|known| known.as_ref() == path)
    {
        out.push(SharedString::from(path.to_owned()));
    }
}

/// Image-looking local paths in free text (commands, output, JSON strings).
/// URLs and glob patterns are skipped; trailing sentence punctuation is not
/// part of a path.
pub fn push_text_paths(out: &mut Vec<SharedString>, text: &str) {
    let separators = |c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\''
                    | '`'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | ';'
                    | '|'
                    | '='
            )
    };
    for token in text.split(separators) {
        if out.len() >= MAX_IMAGES_PER_TOOL {
            return;
        }
        let token = token.trim_end_matches(['.', ':', '!', '?']);
        let token = token.strip_prefix("file://").unwrap_or(token);
        if token.contains("://") || token.starts_with("data:") || token.contains(['*', '?']) {
            continue;
        }
        push_path(out, token);
    }
}

pub fn push_json_paths(out: &mut Vec<SharedString>, value: &serde_json::Value) {
    match value {
        serde_json::Value::String(text) => push_text_paths(out, text),
        serde_json::Value::Array(items) => items.iter().for_each(|v| push_json_paths(out, v)),
        serde_json::Value::Object(map) => map.values().for_each(|v| push_json_paths(out, v)),
        _ => {}
    }
}

/// The image files a tool call touched, as reported (possibly relative to the
/// chat's working directory). Computed once per row build, never per paint.
pub fn image_paths(call: &ToolCall, output: Option<&str>) -> Arc<[SharedString]> {
    let mut out = Vec::new();
    match call {
        ToolCall::ReadFile { path }
        | ToolCall::WriteFile { path, .. }
        | ToolCall::EditFile { path, .. }
        | ToolCall::ApplyPatch { path: Some(path) } => push_path(&mut out, path),
        ToolCall::Exec { command } => push_text_paths(&mut out, command),
        ToolCall::Mcp {
            input: Some(input), ..
        }
        | ToolCall::Unknown {
            input: Some(input), ..
        } => push_json_paths(&mut out, input),
        _ => {}
    }
    if let Some(output) = output {
        push_text_paths(&mut out, output);
    }
    out.into()
}

/// Absolute path on the owning host. Relative paths join the chat's cwd in
/// that host's separator style (the UI may run on another OS).
pub fn resolve_path(path: &str, cwd: Option<&str>) -> String {
    let bytes = path.as_bytes();
    let absolute = path.starts_with(['/', '\\'])
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'));
    match cwd.filter(|_| !absolute) {
        Some(cwd) if cwd.contains('\\') => format!(
            "{}\\{}",
            cwd.trim_end_matches(['/', '\\']),
            path.trim_start_matches("./").replace('/', "\\")
        ),
        Some(cwd) => format!(
            "{}/{}",
            cwd.trim_end_matches('/'),
            path.trim_start_matches("./")
        ),
        None => path.to_owned(),
    }
}

/// One file on one device.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub device: String,
    pub path: String,
}
