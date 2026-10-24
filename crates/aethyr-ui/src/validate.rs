use crate::view::{Element, Intent, IntentKind, Node, View};
use crate::{
    MAX_DEPTH, MAX_FIELD_BYTES, MAX_ID_BYTES, MAX_NODES, MAX_TEXT_BYTES, MAX_VIEW_BYTES, SCHEMA,
};
use std::collections::HashSet;
use std::fmt;

/// Why a view was refused. `code()` is shared with the TypeScript validator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    Schema,
    Identity,
    BadId(String),
    DuplicateId(String),
    NodeLimit,
    DepthLimit,
    TextLimit(String),
    ViewLimit,
    MissingLabel(String),
    BadReference(String),
    Encoding(String),
}

impl ViewError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::Identity => "identity",
            Self::BadId(_) => "bad_id",
            Self::DuplicateId(_) => "duplicate_id",
            Self::NodeLimit => "node_limit",
            Self::DepthLimit => "depth_limit",
            Self::TextLimit(_) => "text_limit",
            Self::ViewLimit => "view_limit",
            Self::MissingLabel(_) => "missing_label",
            Self::BadReference(_) => "bad_reference",
            Self::Encoding(_) => "encoding",
        }
    }
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema => write!(f, "view schema is not {SCHEMA}"),
            Self::Identity => f.write_str("view needs a valid instance and a positive revision"),
            Self::BadId(id) => write!(f, "invalid node id {id:?}"),
            Self::DuplicateId(id) => write!(f, "duplicate node id {id:?}"),
            Self::NodeLimit => write!(f, "view has more than {MAX_NODES} nodes"),
            Self::DepthLimit => write!(f, "view nests deeper than {MAX_DEPTH}"),
            Self::TextLimit(id) => write!(f, "node {id:?} has text over its byte bound"),
            Self::ViewLimit => write!(f, "encoded view is over {MAX_VIEW_BYTES} bytes"),
            Self::MissingLabel(id) => write!(f, "node {id:?} needs a nonempty label"),
            Self::BadReference(id) => write!(f, "node {id:?} refers to something invalid"),
            Self::Encoding(error) => write!(f, "invalid view encoding: {error}"),
        }
    }
}

impl std::error::Error for ViewError {}

/// Why an intent was refused. `code()` is shared with TypeScript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntentError {
    /// Another instance or an older revision: the renderer drew something
    /// that is no longer current.
    Stale,
    UnknownNode,
    /// This node does not take this kind of intent.
    NotAccepted,
    Disabled,
    /// A value is missing, unexpected, or out of bounds.
    BadValue,
}

impl IntentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::UnknownNode => "unknown_node",
            Self::NotAccepted => "not_accepted",
            Self::Disabled => "disabled",
            Self::BadValue => "bad_value",
        }
    }
}

impl fmt::Display for IntentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stale => "intent names a view that is no longer current",
            Self::UnknownNode => "intent names no node in the view",
            Self::NotAccepted => "node does not take this intent",
            Self::Disabled => "node is disabled",
            Self::BadValue => "intent value is missing, unexpected, or too long",
        })
    }
}

impl std::error::Error for IntentError {}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

/// A same-site path: starts with one `/`, no scheme, no backslash, no
/// control characters or spaces.
pub(crate) fn valid_href(href: &str) -> bool {
    href.starts_with('/')
        && !href.starts_with("//")
        && href.len() <= 2048
        && !href
            .chars()
            .any(|c| c == '\\' || c.is_control() || c.is_whitespace())
}

fn label_ok(text: &str) -> bool {
    !text.trim().is_empty()
}

/// A view that passed structural validation. It says nothing about whether
/// the controller is allowed to do what an intent asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedView(View);

impl ValidatedView {
    pub fn view(&self) -> &View {
        &self.0
    }

    pub fn into_inner(self) -> View {
        self.0
    }

    pub fn find(&self, id: &str) -> Option<&Node> {
        let mut pending = vec![&self.0.root];
        while let Some(node) = pending.pop() {
            if node.id == id {
                return Some(node);
            }
            pending.extend(node.element.children());
        }
        None
    }

    /// Check an intent against this revision. On success the controller
    /// handles it; the node returned is the one it names.
    pub fn accept(&self, intent: &Intent) -> Result<&Node, IntentError> {
        if intent.instance != self.0.instance || intent.revision != self.0.revision {
            return Err(IntentError::Stale);
        }
        let node = self.find(&intent.node).ok_or(IntentError::UnknownNode)?;
        let value = intent.value.as_deref();
        let no_value = |result: Result<(), IntentError>| {
            result.and(value.is_none().then_some(()).ok_or(IntentError::BadValue))
        };
        let enabled = |enabled: bool| {
            if enabled {
                Ok(())
            } else {
                Err(IntentError::Disabled)
            }
        };
        match (&node.element, intent.kind) {
            (
                Element::Button { enabled: on, .. } | Element::Chip { enabled: on, .. },
                IntentKind::Activate,
            ) => no_value(enabled(*on))?,
            (
                Element::List {
                    selectable: true,
                    children,
                    ..
                },
                IntentKind::Select,
            ) => {
                let value = value.ok_or(IntentError::BadValue)?;
                if !children.iter().any(|child| child.id == value) {
                    return Err(IntentError::BadValue);
                }
            }
            (
                Element::Field {
                    enabled: on,
                    max_bytes,
                    ..
                },
                IntentKind::Input,
            ) => {
                enabled(*on)?;
                let value = value.ok_or(IntentError::BadValue)?;
                if value.len() > *max_bytes as usize {
                    return Err(IntentError::BadValue);
                }
            }
            (Element::Field { enabled: on, .. }, IntentKind::Submit) => no_value(enabled(*on))?,
            (
                Element::Sheet {
                    open: true,
                    dismissible: true,
                    ..
                }
                | Element::Dialog {
                    open: true,
                    dismissible: true,
                    ..
                }
                | Element::Toast {
                    dismissible: true, ..
                },
                IntentKind::Dismiss,
            ) => no_value(Ok(()))?,
            _ => return Err(IntentError::NotAccepted),
        }
        Ok(node)
    }
}

impl View {
    pub fn validate(self) -> Result<ValidatedView, ViewError> {
        if self.schema != SCHEMA {
            return Err(ViewError::Schema);
        }
        if !valid_id(&self.instance) || self.revision == 0 {
            return Err(ViewError::Identity);
        }
        let mut ids = HashSet::new();
        let mut count = 0usize;
        let mut pending = vec![(&self.root, 1usize)];
        while let Some((node, depth)) = pending.pop() {
            count += 1;
            if count > MAX_NODES {
                return Err(ViewError::NodeLimit);
            }
            if depth > MAX_DEPTH {
                return Err(ViewError::DepthLimit);
            }
            if !valid_id(&node.id) {
                return Err(ViewError::BadId(node.id.clone()));
            }
            if !ids.insert(node.id.as_str()) {
                return Err(ViewError::DuplicateId(node.id.clone()));
            }
            check_node(node)?;
            for child in node.element.children() {
                pending.push((child, depth + 1));
            }
        }
        Ok(ValidatedView(self))
    }

    /// Decode and validate a view received as JSON.
    pub fn from_json(text: &str) -> Result<ValidatedView, ViewError> {
        if text.len() > MAX_VIEW_BYTES {
            return Err(ViewError::ViewLimit);
        }
        let view: View =
            serde_json::from_str(text).map_err(|e| ViewError::Encoding(e.to_string()))?;
        view.validate()
    }
}

fn check_node(node: &Node) -> Result<(), ViewError> {
    let id = || node.id.clone();
    let mut texts: Vec<&str> = Vec::new();
    texts.extend(node.a11y.label.as_deref());
    texts.extend(node.a11y.description.as_deref());
    let label: Option<&str> = match &node.element {
        Element::Panel { title, .. } => {
            texts.extend(title.as_deref());
            None
        }
        Element::Stack { .. } => None,
        Element::Text { value, .. } | Element::Label { value, .. } => {
            texts.push(value);
            None
        }
        Element::Button { label, href, .. } => {
            if href.as_deref().is_some_and(|href| !valid_href(href)) {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::Chip { label, detail, .. } => {
            texts.extend(detail.as_deref());
            Some(label)
        }
        Element::List {
            label,
            selectable,
            selected,
            children,
        } => {
            if let Some(selected) = selected
                && (!selectable || !children.iter().any(|child| &child.id == selected))
            {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::Field {
            label,
            value,
            placeholder,
            max_bytes,
            ..
        } => {
            if *max_bytes == 0
                || *max_bytes as usize > MAX_FIELD_BYTES
                || value.len() > *max_bytes as usize
            {
                return Err(ViewError::TextLimit(id()));
            }
            texts.push(placeholder);
            Some(label)
        }
        Element::Sheet { label, .. } | Element::Dialog { label, .. } => Some(label),
        Element::Toast { message, .. } => Some(message),
        Element::MediaSlot { slot, label, .. } => {
            if !valid_id(slot) {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::EngineSurface { surface, label } => {
            if !valid_id(surface) {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
    };
    if let Some(label) = label {
        if !label_ok(label) {
            return Err(ViewError::MissingLabel(id()));
        }
        texts.push(label);
    }
    if texts.iter().any(|text| text.len() > MAX_TEXT_BYTES) {
        return Err(ViewError::TextLimit(id()));
    }
    Ok(())
}
