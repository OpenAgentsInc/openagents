use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use codex_client::{
    SUPPORTED_CLIENT_REQUEST_METHODS, SUPPORTED_SERVER_NOTIFICATION_METHODS,
    SUPPORTED_SERVER_REQUEST_METHODS,
};

#[test]
fn supported_method_lists_are_unique() {
    assert_unique("client requests", SUPPORTED_CLIENT_REQUEST_METHODS);
    assert_unique(
        "server notifications",
        SUPPORTED_SERVER_NOTIFICATION_METHODS,
    );
    assert_unique("server requests", SUPPORTED_SERVER_REQUEST_METHODS);
}

#[test]
fn client_request_methods_match_upstream_when_available() {
    let Some(common_rs) = load_upstream_common_rs() else {
        eprintln!("skipping upstream comparison: protocol/common.rs not available");
        return;
    };

    let Some(mut expected) = extract_wire_methods(
        &common_rs,
        "client_request_definitions!",
        "/// DEPRECATED APIs below",
    ) else {
        panic!("failed to parse client_request_definitions block from upstream");
    };
    expected.insert("initialize".to_string());

    let actual = set_of(SUPPORTED_CLIENT_REQUEST_METHODS);
    assert_eq!(
        actual, expected,
        "client request method set drifted from upstream protocol"
    );
}

#[test]
fn server_notification_methods_match_upstream_when_available() {
    let Some(common_rs) = load_upstream_common_rs() else {
        eprintln!("skipping upstream comparison: protocol/common.rs not available");
        return;
    };

    let Some(expected) = extract_notification_methods(
        &common_rs,
        "server_notification_definitions!",
        "/// DEPRECATED NOTIFICATIONS below",
    ) else {
        panic!("failed to parse server_notification_definitions block from upstream");
    };

    let actual = set_of(SUPPORTED_SERVER_NOTIFICATION_METHODS);
    assert_eq!(
        actual, expected,
        "server notification method set drifted from upstream protocol"
    );
}

#[test]
fn server_request_methods_match_upstream_when_available() {
    let Some(common_rs) = load_upstream_common_rs() else {
        eprintln!("skipping upstream comparison: protocol/common.rs not available");
        return;
    };

    let Some(expected) = extract_wire_methods(
        &common_rs,
        "server_request_definitions!",
        "/// DEPRECATED APIs below",
    ) else {
        panic!("failed to parse server_request_definitions block from upstream");
    };

    let actual = set_of(SUPPORTED_SERVER_REQUEST_METHODS);
    assert_eq!(
        actual, expected,
        "server request method set drifted from upstream protocol"
    );
}

fn load_upstream_common_rs() -> Option<String> {
    let from_env = std::env::var("CODEX_PROTOCOL_COMMON_RS")
        .ok()
        .map(PathBuf::from);
    let default = PathBuf::from(
        "/Users/christopherdavid/code/codex/codex-rs/app-server-protocol/src/protocol/common.rs",
    );
    let candidates = if let Some(path) = from_env {
        vec![path, default]
    } else {
        vec![default]
    };

    for path in candidates {
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                return Some(contents);
            }
        }
    }

    None
}

fn extract_wire_methods(
    source: &str,
    macro_name: &str,
    cutoff_marker: &str,
) -> Option<BTreeSet<String>> {
    let block = extract_macro_block(source, macro_name)?;
    let block = block.split(cutoff_marker).next().unwrap_or(&block);

    let mut methods = BTreeSet::new();
    for line in block.lines() {
        if let Some(method) = method_from_wire_alias(line) {
            methods.insert(method);
        }
    }
    Some(methods)
}

fn extract_notification_methods(
    source: &str,
    macro_name: &str,
    cutoff_marker: &str,
) -> Option<BTreeSet<String>> {
    let block = extract_macro_block(source, macro_name)?;
    let block = block.split(cutoff_marker).next().unwrap_or(&block);

    let mut methods = BTreeSet::new();
    let mut pending_serde_rename: Option<String> = None;

    for raw_line in block.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(method) = method_from_wire_alias(line) {
            methods.insert(method);
            pending_serde_rename = None;
            continue;
        }

        if let Some(renamed) = method_from_serde_rename(line) {
            pending_serde_rename = Some(renamed);
            continue;
        }

        if is_variant_line(line) {
            if let Some(method) = pending_serde_rename.take() {
                methods.insert(method);
            }
        }
    }

    Some(methods)
}

fn extract_macro_block(source: &str, macro_name: &str) -> Option<String> {
    let start = source.find(macro_name)?;
    let after_start = &source[start..];
    let open_offset = after_start.find('{')?;
    let start_index = start + open_offset;

    let mut depth = 0usize;
    let mut end_index = None;
    for (offset, ch) in source[start_index..].char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                end_index = Some(start_index + offset + 1);
                break;
            }
        }
    }

    let end = end_index?;
    Some(source[start_index..end].to_string())
}

fn method_from_wire_alias(line: &str) -> Option<String> {
    let marker = "=> \"";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn method_from_serde_rename(line: &str) -> Option<String> {
    let marker = "#[serde(rename = \"";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_variant_line(line: &str) -> bool {
    if line.starts_with('#') {
        return false;
    }
    if line.starts_with("//") {
        return false;
    }
    line.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && (line.contains('(') || line.contains('{'))
}

fn set_of(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|value| (*value).to_string()).collect()
}

fn assert_unique(label: &str, items: &[&str]) {
    let set = set_of(items);
    assert_eq!(
        set.len(),
        items.len(),
        "duplicate entries found in {label} method list"
    );
}
