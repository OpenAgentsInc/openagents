use crate::view::Decimal;
use crate::view::{Element, Intent, IntentKind, Node, View};
use crate::{
    MAX_ACCEPT, MAX_CANVAS_PIXELS, MAX_DEPTH, MAX_FIELD_BYTES, MAX_FILES, MAX_ID_BYTES, MAX_NODES,
    MAX_OPTIONS, MAX_TEXT_BYTES, MAX_UPLOAD_BYTES, MAX_VIEW_BYTES, SCHEMA,
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

/// `#rrggbb`, lowercase.
pub(crate) fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A media type (`image/png`, `image/*`) or an extension (`.json`).
pub(crate) fn valid_accept(entry: &str) -> bool {
    let token = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-".contains(&b))
    };
    match entry.strip_prefix('.') {
        Some(extension) => token(extension),
        None => entry
            .split_once('/')
            .is_some_and(|(kind, sub)| token(kind) && (sub == "*" || token(sub))),
    }
}

/// A range value as text: an optional `-`, digits with an optional
/// fraction, and an optional exponent. No `+`, spaces, `inf`, or `NaN`, so
/// Rust and TypeScript read the same strings.
pub(crate) fn decimal_text(text: &str) -> Option<f64> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let body = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            (digits(whole) || whole.is_empty())
                && (digits(fraction) || fraction.is_empty())
                && !(whole.is_empty() && fraction.is_empty())
        }
        None => digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['-', '+']).unwrap_or(e)));
    if text.len() > 64 || !mantissa_ok || !exponent_ok {
        return None;
    }
    text.parse().ok().filter(|v: &f64| v.is_finite())
}

/// A range's value: finite, within bounds, and on the step from `min`.
pub(crate) fn on_range(value: f64, min: f64, max: f64, step: f64) -> bool {
    if !value.is_finite() || value < min || value > max {
        return false;
    }
    let steps = ((value - min) / step).round();
    (min + steps * step - value).abs() <= 1e-9 * step.abs().max(1.0)
}

/// File names, one per line: at least one, none empty or with control
/// characters, and only one unless the slot takes several.
pub(crate) fn valid_file_names(value: &str, multiple: bool) -> bool {
    let names: Vec<&str> = value.split('\n').collect();
    value.len() <= MAX_TEXT_BYTES
        && names.len() <= if multiple { MAX_FILES } else { 1 }
        && names
            .iter()
            .all(|name| !name.trim().is_empty() && !name.chars().any(char::is_control))
}

fn finite(values: &[Decimal]) -> bool {
    values.iter().all(|d| d.0.is_finite())
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
                Element::Select {
                    enabled: on,
                    options,
                    ..
                },
                IntentKind::Select,
            ) => {
                enabled(*on)?;
                let value = value.ok_or(IntentError::BadValue)?;
                if !options.iter().any(|option| option.value == value) {
                    return Err(IntentError::BadValue);
                }
            }
            (Element::Checkbox { enabled: on, .. }, IntentKind::Input) => {
                enabled(*on)?;
                if !matches!(value, Some("true" | "false")) {
                    return Err(IntentError::BadValue);
                }
            }
            (
                Element::Range {
                    enabled: on,
                    min,
                    max,
                    step,
                    ..
                },
                IntentKind::Input,
            ) => {
                enabled(*on)?;
                let value = value.and_then(decimal_text).ok_or(IntentError::BadValue)?;
                if !on_range(value, min.0, max.0, step.0) {
                    return Err(IntentError::BadValue);
                }
            }
            (Element::Color { enabled: on, .. }, IntentKind::Input) => {
                enabled(*on)?;
                if !value.is_some_and(valid_color) {
                    return Err(IntentError::BadValue);
                }
            }
            (
                Element::File {
                    enabled: on,
                    multiple,
                    ..
                },
                IntentKind::Input,
            ) => {
                enabled(*on)?;
                if !value.is_some_and(|v| valid_file_names(v, *multiple)) {
                    return Err(IntentError::BadValue);
                }
            }
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
        Element::Select {
            label,
            value,
            options,
            ..
        } => {
            if options.is_empty() || options.len() > MAX_OPTIONS {
                return Err(ViewError::BadReference(id()));
            }
            let mut values = HashSet::new();
            for option in options {
                if !label_ok(&option.label) {
                    return Err(ViewError::MissingLabel(id()));
                }
                if !values.insert(option.value.as_str()) {
                    return Err(ViewError::BadReference(id()));
                }
                texts.push(&option.label);
                texts.push(&option.value);
            }
            if !values.contains(value.as_str()) {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::Checkbox { label, .. } => Some(label),
        Element::Range {
            label,
            value,
            min,
            max,
            step,
            ..
        } => {
            if !finite(&[*value, *min, *max, *step])
                || min.0 >= max.0
                || step.0 <= 0.0
                || value.0 < min.0
                || value.0 > max.0
            {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::Color { label, value, .. } => {
            if !valid_color(value) {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::File {
            label,
            slot,
            accept,
            max_bytes,
            ..
        } => {
            if !valid_id(slot)
                || accept.len() > MAX_ACCEPT
                || !accept.iter().all(|entry| valid_accept(entry))
                || *max_bytes == 0
                || *max_bytes as usize > MAX_UPLOAD_BYTES
            {
                return Err(ViewError::BadReference(id()));
            }
            Some(label)
        }
        Element::Canvas {
            surface,
            label,
            width,
            height,
        } => {
            if !valid_id(surface)
                || !(1..=MAX_CANVAS_PIXELS).contains(width)
                || !(1..=MAX_CANVAS_PIXELS).contains(height)
            {
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
