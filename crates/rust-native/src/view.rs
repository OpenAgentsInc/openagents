//! A small closed semantic tree and revision-bound activation contract.
//!
//! A validated view is data, not a native mount or execution permission. The
//! application supplies its own serializable intent type and handles an intent
//! only after checking current domain authority.

use crate::style::Style;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashSet;
use std::fmt;
use std::io::{self, Write};

pub const SCHEMA: &str = "rust-native.view.v2";
pub const MAX_VIEW_BYTES: usize = 512 * 1024;
pub const MAX_NODES: usize = 1_024;
pub const MAX_DEPTH: usize = 16;
/// Bound all JSON containers, including the application's intent payload,
/// below the decoder's recursion limit. Tree depth alone cannot do this.
pub const MAX_JSON_DEPTH: usize = 96;
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Body,
    Heading,
    Code,
    Status,
    /// Selectable Markdown. Links and embedded content remain inert unless
    /// the application separately admits an interaction.
    Markdown,
}

/// The initial primitive set. Other native controls need their own contracts
/// before they can appear in a validated tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "props",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Element<I> {
    /// A locally registered native drawing surface. The resource is an opaque
    /// identifier, never a URL, library path, shader, or executable payload.
    /// Platform adapters must explicitly register a renderer for it.
    Surface {
        resource: String,
        label: String,
    },
    Stack {
        axis: Axis,
        children: Vec<Node<I>>,
    },
    /// A bounded window of stable rows. Paging belongs to the application;
    /// recycling must preserve the row keys and access to original content.
    List {
        label: String,
        children: Vec<Node<I>>,
    },
    Text {
        value: String,
        role: TextRole,
    },
    Button {
        label: String,
        enabled: bool,
        intent: I,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node<I> {
    pub key: String,
    pub style: Style,
    pub element: Element<I>,
}

/// Each new surface lifetime needs a fresh instance ID. A revision identifies
/// one immutable tree in that lifetime; do not reuse it for different content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View<I> {
    pub schema: String,
    pub instance: String,
    pub revision: u64,
    pub root: Node<I>,
}

/// Native callbacks carry identity only. The current tree supplies the typed
/// application intent, so the event cannot substitute a new intent payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activation {
    pub instance: String,
    pub revision: u64,
    pub node: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    Schema,
    Identity,
    DuplicateNode(String),
    NodeLimit,
    DepthLimit,
    JsonDepthLimit,
    TextLimit,
    ViewLimit,
    MissingLabel,
    Encoding(String),
    StaleActivation,
    NotInteractive,
    Disabled,
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema => f.write_str("unsupported Rust Native view schema"),
            Self::Identity => f.write_str(
                "view and node identities must be bounded and revision must be positive",
            ),
            Self::DuplicateNode(key) => write!(f, "duplicate node key: {key}"),
            Self::NodeLimit => f.write_str("view exceeds its node bound"),
            Self::DepthLimit => f.write_str("view exceeds its depth bound"),
            Self::JsonDepthLimit => f.write_str("view exceeds its JSON nesting bound"),
            Self::TextLimit => f.write_str("view text exceeds its byte bound"),
            Self::ViewLimit => f.write_str("view exceeds its encoded byte bound"),
            Self::MissingLabel => f.write_str("button, list, or surface requires a nonempty label"),
            Self::Encoding(error) => write!(f, "invalid Rust Native view encoding: {error}"),
            Self::StaleActivation => f.write_str("activation does not name the current view"),
            Self::NotInteractive => f.write_str("activation does not name a button"),
            Self::Disabled => f.write_str("button is disabled"),
        }
    }
}

impl std::error::Error for ViewError {}

/// Structural validation is retained behind an immutable interface. It does
/// not establish platform support, authenticity, or application authorization.
#[derive(Clone, Debug)]
pub struct ValidatedView<I>(View<I>);

impl<I: Serialize> View<I> {
    pub fn new(instance: impl Into<String>, revision: u64, root: Node<I>) -> Self {
        Self {
            schema: SCHEMA.into(),
            instance: instance.into(),
            revision,
            root,
        }
    }

    pub fn validate(self) -> Result<ValidatedView<I>, ViewError> {
        if self.schema != SCHEMA {
            return Err(ViewError::Schema);
        }
        if !crate::valid_id(&self.instance) || self.revision == 0 {
            return Err(ViewError::Identity);
        }
        let mut keys = HashSet::new();
        let mut pending = vec![(&self.root, 1)];
        while let Some((node, depth)) = pending.pop() {
            if depth > MAX_DEPTH {
                return Err(ViewError::DepthLimit);
            }
            if !crate::valid_id(&node.key) {
                return Err(ViewError::Identity);
            }
            if !keys.insert(node.key.as_str()) {
                return Err(ViewError::DuplicateNode(node.key.clone()));
            }
            if keys.len() > MAX_NODES {
                return Err(ViewError::NodeLimit);
            }
            match &node.element {
                Element::Stack { children, .. } | Element::List { children, .. } => {
                    if let Element::List { label, .. } = &node.element {
                        check_text(label)?;
                        if label.trim().is_empty() {
                            return Err(ViewError::MissingLabel);
                        }
                    }
                    if keys.len() + pending.len() + children.len() > MAX_NODES {
                        return Err(ViewError::NodeLimit);
                    }
                    pending.extend(children.iter().map(|node| (node, depth + 1)));
                }
                Element::Text { value, .. } => check_text(value)?,
                Element::Surface { resource, label } => {
                    if !crate::valid_id(resource) {
                        return Err(ViewError::Identity);
                    }
                    check_text(label)?;
                    if label.trim().is_empty() {
                        return Err(ViewError::MissingLabel);
                    }
                }
                Element::Button { label, .. } => {
                    check_text(label)?;
                    if label.trim().is_empty() {
                        return Err(ViewError::MissingLabel);
                    }
                }
            }
        }
        encode(&self)?;
        Ok(ValidatedView(self))
    }
}

impl<I: Serialize + DeserializeOwned> View<I> {
    /// An interchange helper, not a trusted remote-UI transport. The host must
    /// establish source/disclosure policy and validate its intent type as well.
    pub fn from_json(bytes: &[u8]) -> Result<ValidatedView<I>, ViewError> {
        if bytes.len() > MAX_VIEW_BYTES {
            return Err(ViewError::ViewLimit);
        }
        check_json_depth(bytes)?;
        let view: Self =
            serde_json::from_slice(bytes).map_err(|e| ViewError::Encoding(e.to_string()))?;
        view.validate()
    }
}

impl<I> ValidatedView<I> {
    pub fn view(&self) -> &View<I> {
        &self.0
    }

    /// Resolve only against the application's current view. Repeated valid
    /// activations can return the same intent; domain idempotency is separate.
    pub fn activate(&self, event: &Activation) -> Result<&I, ViewError> {
        if event.instance != self.0.instance || event.revision != self.0.revision {
            return Err(ViewError::StaleActivation);
        }
        let mut pending = vec![&self.0.root];
        while let Some(node) = pending.pop() {
            if node.key == event.node {
                return match &node.element {
                    Element::Button {
                        enabled: true,
                        intent,
                        ..
                    } => Ok(intent),
                    Element::Button { enabled: false, .. } => Err(ViewError::Disabled),
                    _ => Err(ViewError::NotInteractive),
                };
            }
            if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element
            {
                pending.extend(children);
            }
        }
        Err(ViewError::NotInteractive)
    }
}

impl<I: Serialize> ValidatedView<I> {
    pub fn to_json(&self) -> Result<Vec<u8>, ViewError> {
        encode(&self.0)
    }
}

fn check_text(value: &str) -> Result<(), ViewError> {
    if value.len() > MAX_TEXT_BYTES {
        Err(ViewError::TextLimit)
    } else {
        Ok(())
    }
}

fn encode<I: Serialize>(view: &View<I>) -> Result<Vec<u8>, ViewError> {
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_VIEW_BYTES {
                return Err(io::Error::other("view byte bound"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, view).map_err(|error| {
        if error.is_io() {
            ViewError::ViewLimit
        } else {
            ViewError::Encoding(error.to_string())
        }
    })?;
    check_json_depth(&output.0)?;
    Ok(output.0)
}

// This scan bounds nesting before decoding. Serde remains responsible for
// syntax and schema validation; braces inside escaped strings are just text.
fn check_json_depth(bytes: &[u8]) -> Result<(), ViewError> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_JSON_DEPTH {
                        return Err(ViewError::JsonDepthLimit);
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
