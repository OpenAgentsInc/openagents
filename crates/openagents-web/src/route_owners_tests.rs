//! Every route the website mounts is one it owns (#11159), so `guard`
//! never forwards a request for it, with the app's `Bearer sess_...`, to
//! the previous server behind `--upstream`.
//!
//! The router can't list its own routes, so this reads them from the
//! source: the first argument of each `.route(...)` call outside tests, a
//! string, a `const`, or a `format!` of consts. A route built from a local
//! variable (a loop, a closure) isn't read; those paths are held by the
//! explicit lists in `tests.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Mounted here but answered by the upstream on purpose, by prefix, and
/// why. Nothing else may be.
const NOT_OWNED: &[(&str, &str)] = &[
    (
        "/coder/",
        "Coder's older sync paths: in production the previous server answers \
         them until the shipped Coder and phone apps call /v1/threads/synced \
         and /v1/computers/check-in (#11158), which this site owns",
    ),
    ("/u/", "profiles: the previous server still draws them"),
    (
        "/api/v1/",
        "the API alias: owned when the inference gateway is configured (`guard`)",
    ),
];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                sources(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(path);
        }
    }
}

fn ident(text: &str) -> &str {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
        .unwrap_or(text.len());
    &text[..end]
}

/// The string consts a file declares: `const NAME: &str = "...";`.
fn consts(text: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut rest = text;
    while let Some(at) = rest.find("const ") {
        rest = &rest[at + "const ".len()..];
        let name = ident(rest);
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        let after = rest[name.len()..].trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let after = after.trim_start();
        let Some(after) = after
            .strip_prefix("&str")
            .or_else(|| after.strip_prefix("&'static str"))
        else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('"') else {
            continue;
        };
        let Some(end) = after.find('"') else {
            continue;
        };
        if after[end + 1..].trim_start().starts_with(';') {
            found.insert(name.to_owned(), after[..end].to_owned());
        }
    }
    found
}

struct Source {
    path: PathBuf,
    consts: BTreeMap<String, String>,
}

fn module(path: &Path) -> String {
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    if stem == "mod" {
        path.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    } else {
        stem
    }
}

fn resolve(name: &str, here: &Source, all: &[Source]) -> Option<String> {
    if let Some((qualifier, last)) = name.rsplit_once("::") {
        let module_name = qualifier.rsplit("::").next().unwrap_or(qualifier);
        return all
            .iter()
            .filter(|source| module(&source.path) == module_name)
            .find_map(|source| source.consts.get(last).cloned());
    }
    if let Some(value) = here.consts.get(name) {
        return Some(value.clone());
    }
    let mut values: Vec<&String> = all
        .iter()
        .filter_map(|source| source.consts.get(name))
        .collect();
    values.dedup();
    (values.len() == 1).then(|| values[0].clone())
}

/// `{NAME}` from the consts, `{{`/`}}` as braces.
fn format(template: &str, here: &Source, all: &[Source]) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(after) = rest.strip_prefix("{{") {
            let end = after.find("}}")?;
            out.push('{');
            out.push_str(&after[..end]);
            out.push('}');
            rest = &after[end + 2..];
        } else {
            let end = rest.find('}')?;
            out.push_str(&resolve(&rest[1..end], here, all)?);
            rest = &rest[end + 1..];
        }
    }
    out.push_str(rest);
    Some(out)
}

/// Whether a `format!` template names a local variable (`{base}`).
fn local_in_template(template: &str) -> bool {
    let template = template.split('"').next().unwrap_or(template);
    template
        .replace("{{", "")
        .split('{')
        .skip(1)
        .any(|part| part.starts_with(|c: char| c.is_ascii_lowercase()))
}

/// The paths a file's `.route(...)` calls name, and the arguments it
/// couldn't read.
fn routes(text: &str, here: &Source, all: &[Source]) -> (Vec<String>, Vec<String>) {
    let text = text.split("#[cfg(test)]").next().unwrap_or(text);
    let (mut paths, mut unread) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(at) = rest.find(".route(") {
        rest = rest[at + ".route(".len()..].trim_start();
        if let Some(after) = rest.strip_prefix('"') {
            if let Some(end) = after.find('"') {
                paths.push(after[..end].to_owned());
            }
        } else if let Some(after) = rest.strip_prefix("&format!(\"") {
            match after
                .find('"')
                .and_then(|end| format(&after[..end], here, all))
            {
                Some(path) => paths.push(path),
                // Built from a local variable (`{base}`): read elsewhere.
                None if local_in_template(after) => {}
                None => unread.push(after.chars().take(40).collect()),
            }
        } else {
            let name = ident(rest);
            match resolve(name, here, all) {
                Some(path) => paths.push(path),
                // A local variable (`base`): read elsewhere.
                None if name.starts_with(|c: char| c.is_ascii_lowercase()) => {}
                None => unread.push(name.to_owned()),
            }
        }
    }
    (paths, unread)
}

/// A request path the route answers: each `{param}` (and `{*rest}`) as
/// one segment.
fn sample(route: &str) -> String {
    route
        .split('/')
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                "x"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn every_mounted_route_is_owned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let all: Vec<Source> = files
        .iter()
        .map(|path| Source {
            path: path.clone(),
            consts: consts(&std::fs::read_to_string(path).unwrap()),
        })
        .collect();
    let mut checked = 0;
    let mut unowned = Vec::new();
    let mut unread_all = Vec::new();
    for source in &all {
        // The sales service is its own server, not part of the site.
        if module(&source.path) == "sales_remote" {
            continue;
        }
        let text = std::fs::read_to_string(&source.path).unwrap();
        let (paths, unread) = routes(&text, source, &all);
        unread_all.extend(
            unread
                .into_iter()
                .map(|name| format!("{}: {name}", source.path.display())),
        );
        for route in paths {
            checked += 1;
            let path = sample(&route);
            if crate::upstream::owned(&path)
                || NOT_OWNED.iter().any(|(prefix, _)| path.starts_with(prefix))
            {
                continue;
            }
            unowned.push(format!("{route} ({})", source.path.display()));
        }
    }
    assert!(
        unread_all.is_empty(),
        "route paths this test couldn't read: {unread_all:?}"
    );
    assert!(checked > 150, "only {checked} routes found");
    assert!(
        unowned.is_empty(),
        "mounted but not in upstream::OWNED_EXACT / OWNED_PREFIXES, so --upstream would forward them: {unowned:?}"
    );
}

#[test]
fn the_api_paths_are_owned_and_the_older_coder_paths_are_not_yet() {
    for path in [
        "/v1/device/code",
        "/v1/device/token",
        "/v1/device/sign-out",
        "/v1/traces",
        "/v1/traces/t1/agents",
        "/v1/threads/synced",
        "/v1/threads/synced/s1/replies",
        "/v1/computers/check-in",
        "/device/token",
        "/api/traces",
        "/.well-known/security.txt",
        "/security.txt",
    ] {
        assert!(crate::upstream::owned(path), "{path}");
    }
    for path in ["/coder/sessions", "/coder/check-in", "/coder/sync"] {
        assert!(!crate::upstream::owned(path), "{path}");
    }
}
