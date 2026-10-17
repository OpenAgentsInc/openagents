//! The Rust SDK reference, read from `docs/sdk/sdk-reference.json`.
//!
//! The SDK crate's `tests/reference.rs` writes that file from the crate's
//! source and fails when it is stale, so these pages follow the code.

use std::sync::OnceLock;

use serde::Deserialize;

const REFERENCE: &str = include_str!("../../../../../docs/sdk/sdk-reference.json");

#[derive(Debug, Deserialize)]
pub(crate) struct Reference {
    pub description: String,
    pub modules: Vec<Module>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Module {
    pub path: String,
    pub doc: String,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub reexports: Vec<Reexport>,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Reexport {
    pub from: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Item {
    pub kind: String,
    pub name: String,
    pub doc: String,
    pub declaration: String,
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub methods: Vec<Method>,
    #[serde(default)]
    pub feature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Method {
    pub name: String,
    pub doc: String,
    pub declaration: String,
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub feature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Call {
    pub method: String,
    pub path: String,
}

/// The parsed reference.
pub(crate) fn reference() -> &'static Reference {
    static PARSED: OnceLock<Reference> = OnceLock::new();
    PARSED.get_or_init(|| {
        serde_json::from_str(REFERENCE).unwrap_or(Reference {
            description: String::new(),
            modules: Vec::new(),
        })
    })
}

impl Module {
    /// Its page's slug: `crate` for the root, else the path with `-`.
    pub fn slug(&self) -> String {
        if self.path.is_empty() {
            "crate".to_owned()
        } else {
            self.path.replace("::", "-")
        }
    }

    /// Its Rust path, such as `openagents_sdk::chats`.
    pub fn rust_path(&self) -> String {
        if self.path.is_empty() {
            "openagents_sdk".to_owned()
        } else {
            format!("openagents_sdk::{}", self.path)
        }
    }

    /// The first sentence of its doc comment.
    pub fn summary(&self) -> String {
        first_sentence(&self.doc)
    }

    /// Its page.
    pub fn href(&self) -> String {
        format!("/docs/sdk/rust/{}", self.slug())
    }
}

/// The first sentence of `doc`, on one line.
pub(crate) fn first_sentence(doc: &str) -> String {
    let paragraph = doc
        .split("\n\n")
        .next()
        .unwrap_or_default()
        .replace('\n', " ");
    match paragraph.find(". ") {
        Some(end) => paragraph[..=end].trim().to_owned(),
        None => paragraph.trim().to_owned(),
    }
}

/// Every SDK function or method that calls `method path` (a contract
/// operation), as (label, address) pairs.
pub(crate) fn callers(operation: &super::contract::Operation) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for module in &reference().modules {
        for item in &module.items {
            if item.calls.iter().any(|c| names(c, operation)) {
                out.push((
                    format!("{}::{}", module.rust_path(), item.name),
                    format!("{}#{}", module.href(), item.name),
                ));
            }
            for method in &item.methods {
                if method.calls.iter().any(|c| names(c, operation)) {
                    out.push((
                        format!("{}::{}", item.name, method.name),
                        format!("{}#{}.{}", module.href(), item.name, method.name),
                    ));
                }
            }
        }
    }
    out
}

fn names(call: &Call, operation: &super::contract::Operation) -> bool {
    super::contract::operation_for(&call.method, &call.path).is_some_and(|op| op.id == operation.id)
}
