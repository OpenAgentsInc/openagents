//! The operations the SDK docs describe, read from the committed contracts.
//!
//! Three documents are composed, in this order, and the first to describe
//! a method and path keeps it:
//!
//! 1. the app API ([`crate::app_openapi::document`], committed as
//!    `docs/sdk/openapi.app.json`), which also holds the Computers and Labs
//!    operations;
//! 2. the System One slice (`docs/sdk/openapi.systemone.json`);
//! 3. the decision API (`docs/agents/openapi.yaml`), limited to the routes
//!    the public API forwards ([`crate::api_alias::forwards`]).
//!
//! Each operation keeps a pointer to its own document, so a `$ref` resolves
//! where it was written.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use axum::http::Method;
use serde_json::Value;

/// One composed document.
pub(crate) struct Source {
    /// What the reference calls it.
    pub title: &'static str,
    /// The contract file in the repository.
    pub file: &'static str,
    pub document: Value,
}

/// One operation of a contract.
pub(crate) struct Operation {
    pub source: usize,
    pub method: String,
    pub path: String,
    pub id: String,
    pub tag: String,
    pub area: &'static Area,
    pub value: Value,
}

impl Operation {
    /// Its anchor on the area page: the operation ID, which the contracts
    /// keep unique and stable.
    pub fn anchor(&self) -> &str {
        &self.id
    }

    /// Its address on the site.
    pub fn href(&self) -> String {
        format!("/docs/sdk/api/{}#{}", self.area.slug, self.anchor())
    }

    /// Its heading: the summary, or the operation ID in words.
    pub fn title(&self) -> String {
        match self.value["summary"].as_str().map(str::trim) {
            Some(summary) if !summary.is_empty() => summary.trim_end_matches('.').to_owned(),
            _ => words(&self.id),
        }
    }

    /// Its description, or an empty string.
    pub fn description(&self) -> &str {
        self.value["description"]
            .as_str()
            .unwrap_or_default()
            .trim()
    }

    /// The document it was written in.
    pub fn document(&self) -> &'static Value {
        &sources()[self.source].document
    }
}

/// `listLabDocuments` as "List lab documents".
pub(crate) fn words(id: &str) -> String {
    let mut out = String::new();
    for (index, c) in id.chars().enumerate() {
        if c.is_ascii_uppercase() && index > 0 {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else if index == 0 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A page of the API reference: the contract tags it holds.
pub(crate) struct Area {
    pub slug: &'static str,
    pub title: &'static str,
    pub lead: &'static str,
    pub tags: &'static [&'static str],
}

/// The API reference's pages, in reading order. A tag no area names fails
/// the module's tests, so a new tag gets a page on purpose.
pub(crate) const AREAS: &[Area] = &[
    Area {
        slug: "sign-in",
        title: "Sign-in and account",
        lead: "Sign an app in, read the signed-in account, and sign out.",
        tags: &["App authentication", "Account"],
    },
    Area {
        slug: "keys",
        title: "Keys",
        lead: "List, create, and revoke the keys that act for your account.",
        tags: &["Account keys"],
    },
    Area {
        slug: "chats",
        title: "Chats",
        lead: "Start chats, send messages, follow answers as they arrive, share chats, and sync \
chats from your computers.",
        tags: &["Threads", "Cloud chats", "Chat homes"],
    },
    Area {
        slug: "history",
        title: "Chat history",
        lead: "Read saved chats, their files and captures, and the sessions your computers \
publish.",
        tags: &["Saved history"],
    },
    Area {
        slug: "home",
        title: "Home",
        lead: "The projects, work, and dashboards on your Home, and shared dashboard templates.",
        tags: &["Home"],
    },
    Area {
        slug: "agents",
        title: "Agents",
        lead: "Define your own agents and assign them work.",
        tags: &["Custom agents"],
    },
    Area {
        slug: "runs",
        title: "Runs",
        lead: "Report agent and harness activity and progress, and see which runs are still active.",
        tags: &["Runs"],
    },
    Area {
        slug: "computers",
        title: "Computers",
        lead: "Create cloud computers, run commands on them, move files, and read usage.",
        tags: &["Computers"],
    },
    Area {
        slug: "projects",
        title: "Projects",
        lead: "Create projects, add repositories and members, and share them with invitations.",
        tags: &["Projects"],
    },
    Area {
        slug: "files",
        title: "Files",
        lead: "Upload files to a project, attach documents, and download them again.",
        tags: &["Project files"],
    },
    Area {
        slug: "connections",
        title: "Connected sources",
        lead: "Connect Google Drive and attach its files and folders to a project.",
        tags: &["Connected project sources"],
    },
    Area {
        slug: "labs",
        title: "Labs",
        lead: "Lab documents, versions, notes, jobs, artifacts, and the workers that run jobs.",
        tags: &["Labs"],
    },
    Area {
        slug: "decisions",
        title: "Decisions",
        lead: "Ask a decision model typed questions about a state and get typed answers.",
        tags: &["System One", "Decisions"],
    },
    Area {
        slug: "models",
        title: "Models and balance",
        lead: "The models you can call and the balance your calls draw on.",
        tags: &["Models", "Balance"],
    },
    Area {
        slug: "workspaces",
        title: "Workspaces",
        lead: "Workspaces, their members and invitations, keys, billing, and usage.",
        tags: &["Workspaces"],
    },
];

/// The tag of an operation its contract leaves untagged, from its path.
fn tag_for(path: &str) -> Option<&'static str> {
    let first = path.strip_prefix("/v1/")?.split('/').next()?;
    Some(match first {
        "computers" => "Computers",
        "systemone" => "System One",
        "decisions" => "Decisions",
        "models" => "Models",
        "balance" => "Balance",
        "account" => "Account",
        "workspaces" | "invitations" => "Workspaces",
        _ => return None,
    })
}

const METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];

/// The composed documents.
pub(crate) fn sources() -> &'static [Source] {
    static SOURCES: OnceLock<Vec<Source>> = OnceLock::new();
    SOURCES.get_or_init(|| {
        vec![
            Source {
                title: "App API",
                file: "docs/sdk/openapi.app.json",
                document: crate::app_openapi::document(),
            },
            Source {
                title: "System One",
                file: "docs/sdk/openapi.systemone.json",
                document: serde_json::from_str(crate::systemone_openapi::CONTRACT)
                    .unwrap_or(Value::Null),
            },
            Source {
                title: "Decision API",
                file: "docs/agents/openapi.yaml",
                document: serde_yaml_ng::from_str(discovery::site::OPENAPI_YAML)
                    .unwrap_or(Value::Null),
            },
        ]
    })
}

/// Every operation, grouped by area in [`AREAS`] order, then by path.
pub(crate) fn operations() -> &'static [Operation] {
    static OPERATIONS: OnceLock<Vec<Operation>> = OnceLock::new();
    OPERATIONS.get_or_init(|| {
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for (index, source) in sources().iter().enumerate() {
            let Some(paths) = source.document["paths"].as_object() else {
                continue;
            };
            for (path, item) in paths {
                for method in METHODS {
                    let Some(value) = item.get(method) else {
                        continue;
                    };
                    // The decision API lists gateway routes; keep only the
                    // ones the public API forwards.
                    if index == 2 {
                        let Ok(verb) = Method::from_bytes(method.to_uppercase().as_bytes()) else {
                            continue;
                        };
                        if !crate::api_alias::forwards(&verb, path, false) {
                            continue;
                        }
                    }
                    if !seen.insert((method, path.clone())) {
                        continue;
                    }
                    let tag = value["tags"][0]
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| tag_for(path).map(str::to_owned))
                        .unwrap_or_default();
                    let Some(area) = AREAS.iter().find(|a| a.tags.contains(&tag.as_str())) else {
                        // The module's tests name it.
                        continue;
                    };
                    let id = value["operationId"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            format!("{method}{}", path.replace(['/', '{', '}'], "-"))
                        });
                    out.push(Operation {
                        source: index,
                        method: method.to_uppercase(),
                        path: path.clone(),
                        id,
                        tag,
                        area,
                        value: value.clone(),
                    });
                }
            }
        }
        let order: BTreeMap<&str, usize> =
            AREAS.iter().enumerate().map(|(i, a)| (a.slug, i)).collect();
        let rank = |m: &str| METHODS.iter().position(|x| x.eq_ignore_ascii_case(m));
        out.sort_by(|a, b| {
            (order[a.area.slug], &a.path, rank(&a.method)).cmp(&(
                order[b.area.slug],
                &b.path,
                rank(&b.method),
            ))
        });
        out
    })
}

/// The operations on one area's page.
pub(crate) fn in_area(slug: &str) -> impl Iterator<Item = &'static Operation> {
    let slug = slug.to_owned();
    operations().iter().filter(move |op| op.area.slug == slug)
}

/// Every method and path the contracts describe but no area holds: a tag
/// missing from [`AREAS`]. Empty when the reference is complete.
#[cfg(test)]
pub(crate) fn unplaced() -> Vec<String> {
    let mut out = Vec::new();
    for (index, source) in sources().iter().enumerate() {
        for (path, item) in source.document["paths"].as_object().into_iter().flatten() {
            for method in METHODS {
                let Some(value) = item.get(method) else {
                    continue;
                };
                if index == 2 {
                    let verb = Method::from_bytes(method.to_uppercase().as_bytes()).unwrap();
                    if !crate::api_alias::forwards(&verb, path, false) {
                        continue;
                    }
                }
                let tag = value["tags"][0]
                    .as_str()
                    .or_else(|| tag_for(path))
                    .unwrap_or_default();
                if !AREAS.iter().any(|a| a.tags.contains(&tag)) {
                    out.push(format!("{method} {path} (tag {tag:?})"));
                }
            }
        }
    }
    out
}

/// Follows `value`'s `$ref`, if any, within `document`. An address in
/// another file stays as it is; [`external`] names it.
pub(crate) fn deref<'a>(document: &'a Value, value: &'a Value) -> &'a Value {
    let mut current = value;
    for _ in 0..16 {
        let Some(reference) = current["$ref"].as_str() else {
            return current;
        };
        let Some(pointer) = reference.strip_prefix('#') else {
            return current;
        };
        match document.pointer(pointer) {
            Some(next) => current = next,
            None => return current,
        }
    }
    current
}

/// The name a local `$ref` points at, such as `AppComputer`.
pub(crate) fn ref_name(value: &Value) -> Option<&str> {
    value["$ref"]
        .as_str()
        .filter(|r| r.starts_with('#'))
        .and_then(|r| r.rsplit('/').next())
}

/// A `$ref` into another file, as its repository path.
pub(crate) fn external(source: &Source, value: &Value) -> Option<String> {
    let reference = value["$ref"].as_str().filter(|r| !r.starts_with('#'))?;
    let mut base: Vec<&str> = source.file.split('/').collect();
    base.pop();
    for part in reference.split('#').next()?.split('/') {
        match part {
            ".." => {
                base.pop();
            }
            "." | "" => {}
            other => base.push(other),
        }
    }
    Some(base.join("/"))
}

/// Whether `pattern` (an SDK call, `*` for a value it fills in) names
/// `path` (a contract path, `{name}` for a parameter).
pub(crate) fn path_matches(pattern: &str, path: &str) -> bool {
    let want: Vec<&str> = pattern.split('/').collect();
    let have: Vec<&str> = path.split('/').collect();
    want.len() == have.len()
        && want.iter().zip(&have).all(|(w, h)| {
            w == h || (*w == "*" && h.starts_with('{')) || (h.starts_with('{') && *w != "")
        })
}

/// The operation an SDK call names, if exactly one does. A literal segment
/// beats a parameter, so `/v1/threads/synced` is not `/v1/threads/{id}`.
pub(crate) fn operation_for(method: &str, pattern: &str) -> Option<&'static Operation> {
    let candidates: Vec<&Operation> = operations()
        .iter()
        .filter(|op| op.method == method && path_matches(pattern, &op.path))
        .collect();
    let literal = |op: &&Operation| {
        pattern
            .split('/')
            .zip(op.path.split('/'))
            .filter(|(w, h)| w == h)
            .count()
    };
    let best = candidates.iter().map(literal).max()?;
    let mut top = candidates.into_iter().filter(|op| literal(op) == best);
    let first = top.next()?;
    top.next().is_none().then_some(first)
}
