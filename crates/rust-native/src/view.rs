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
/// Explicit opt-in for fields, styled text, choices, and dialogs. Existing
/// constructors continue to emit v2, and v2 refuses these added elements.
pub const SCHEMA_V3: &str = "rust-native.view.v3";
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
    /// Left to right, continuing on the next line when the next child does
    /// not fit, as a row of suggestion chips. An adapter without it lays
    /// the children out vertically.
    Wrap,
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
    /// One row, or one run within a row, of a fixed-cell character grid such
    /// as a terminal screen. Adapters draw it monospaced on one line without
    /// wrapping and keep every space. The application sizes its grid from the
    /// cell size the adapter reports.
    Terminal,
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
    /// Selectable text whose runs preserve application-supplied presentation.
    /// Runs contain no markup, links, or executable payloads.
    RichText {
        runs: Vec<RichRun>,
        role: TextRole,
    },
    /// An editable value. Changes carry this node's activation identity and
    /// a bounded value; the current tree supplies `on_change`.
    Field {
        label: String,
        value: String,
        placeholder: String,
        secret: bool,
        multiline: bool,
        enabled: bool,
        max_bytes: usize,
        on_change: I,
    },
    /// A selectable action. Its selected state is explicit, not inferred
    /// from a color or label. Selection remains application state.
    Choice {
        label: String,
        selected: bool,
        enabled: bool,
        intent: I,
        children: Vec<Node<I>>,
    },
    /// A modal focus scope. Dismissal resolves `on_close`; the application
    /// owns whether it remains open. The adapter restores originating focus.
    Dialog {
        label: String,
        open: bool,
        on_close: I,
        children: Vec<Node<I>>,
    },
    /// A control that runs `intent`. `label` is its visible label, or,
    /// with a circular `icon`, its spoken name.
    Button {
        label: String,
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<Icon>,
        /// Display-only shortcut hint. The application owns the key binding.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shortcut: Option<String>,
        intent: I,
    },
    /// A conversation, oldest row first. Adapters keep the newest row in view
    /// while the reader is at the bottom, and offer a jump to the bottom when
    /// the reader scrolls up. Activating the node runs `earlier`.
    ///
    /// With `source`, the rows are not in the view: the application published
    /// them to the in-process transcript source of that name
    /// (`layout::source`), and the adapter's transcript layout reads them
    /// there. `children` is then empty, and the view's node and byte bounds
    /// do not count the rows.
    Transcript {
        label: String,
        children: Vec<Node<I>>,
        earlier: Option<Earlier<I>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// One message, drawn by its role. `note` is a short status, such as a
    /// time or "Not sent".
    Message {
        role: MessageRole,
        note: Option<String>,
        children: Vec<Node<I>>,
    },
    /// Markdown the application parsed. Only a link that
    /// [`crate::markdown::opens`] admits opens, in the system browser.
    Markdown {
        blocks: Vec<crate::markdown::Block>,
    },
    /// A tool call, one line until the reader expands it to its children.
    Tool {
        name: String,
        detail: String,
        state: ToolState,
        children: Vec<Node<I>>,
    },
    /// The assistant is working, such as "Coder is working".
    Working {
        label: String,
    },
    /// A text field with a send control. The adapter answers a send as an
    /// input answer bound to `token`; activating the node while `busy` runs
    /// `stop`. `choices` are other ways to send the same text, which the
    /// adapter offers as a long press on the send control (a menu); choosing
    /// one answers with that choice's token instead. `draft` is text the
    /// adapter puts in the field when `token` is new, such as a message to
    /// edit; otherwise the field keeps what the person typed. `focus` puts
    /// the text cursor in the field when `token` is new, so a screen whose
    /// purpose is to write, such as a new chat, opens ready to type.
    Composer {
        token: String,
        placeholder: String,
        max_bytes: usize,
        enabled: bool,
        busy: bool,
        stop: Option<I>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        choices: Vec<ComposerChoice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        focus: bool,
    },
}

/// A styled Unicode run. Colors use the same generic sRGB values as `Style`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RichRun {
    pub text: String,
    pub foreground: Option<crate::style::Color>,
    pub background: Option<crate::style::Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub dim: bool,
}

/// The most choices a composer offers.
pub const MAX_COMPOSER_CHOICES: usize = 4;

/// Another way to send a composer's text: its own input token and label.
/// The application gives each choice's token its meaning; the adapter never
/// reads one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposerChoice {
    pub token: String,
    pub label: String,
}

/// A glyph a button draws. An adapter that cannot draw it shows the
/// button's label instead, so a button's meaning never depends on the glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Icon {
    pub glyph: Glyph,
    /// Draw only the glyph in a circle, with the label as its spoken name.
    /// Otherwise the glyph leads the visible label, as a back link does.
    pub circular: bool,
    /// Draw the glyph and the visible label together in a filled capsule,
    /// as a suggestion chip or a selector. Ignored when `circular`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pill: bool,
}

/// The closed set of button glyphs. Adapters map each to a native symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Glyph {
    /// Go back to the previous screen.
    Back,
    /// Go forward again after going back.
    Forward,
    /// Start something new, such as a chat.
    Compose,
    /// Edit an item.
    Edit,
    /// Open a list of earlier items beside the screen, such as previous
    /// chats: a hamburger.
    Menu,
    /// Continue something earlier, such as a recent chat.
    History,
    /// A folder, such as a computer's workspace.
    Folder,
    /// A computer.
    Computer,
    /// A hosted service, such as the cloud.
    Cloud,
    /// Add something, such as a computer.
    Add,
    /// A plain plus, such as a titlebar's new-session control.
    Plus,
    /// Submit or move up.
    ArrowUp,
    /// Return to the latest content below the viewport.
    ArrowDown,
    /// Stop the current work.
    Stop,
    /// Attach a file.
    Paperclip,
    /// Paste from the clipboard.
    Clipboard,
    /// Open additional actions.
    More,
    /// Find an item.
    Search,
    /// Open application settings.
    Settings,
    /// Keep an item at the top of a list.
    Pin,
    /// Archive an item without deleting it.
    Archive,
    /// Restore an archived item.
    Restore,
    /// The current choice among several.
    Check,
    /// Ask a suggested question, such as a follow-up under an answer.
    Ask,
    /// Mark or report something, such as a wrong answer.
    Flag,
    /// Run a command.
    Terminal,
    /// A wallet.
    Wallet,
    /// A key, such as identity keys.
    Key,
    /// A person, such as the trainer's profile.
    Person,
    /// A checkbox that is off. The label says what turning it on allows;
    /// activating the button asks the application to turn it on.
    Unchecked,
    /// A checkbox that is on. Activating the button asks the application
    /// to turn it off.
    Checked,
    /// A map of how things connect: a node linked to three others.
    Map,
}

/// The control that loads older rows at a transcript's top.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Earlier<I> {
    pub label: String,
    /// Older rows are loading; the control shows progress and is inert.
    pub loading: bool,
    pub intent: I,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    Running,
    Done,
    Failed,
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

/// A field change contains identity and data, never a replacement intent.
/// It is local input data and must not be logged for secret fields.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldChange {
    pub activation: Activation,
    pub value: String,
}

impl fmt::Debug for FieldChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldChange")
            .field("activation", &self.activation)
            .field("value", &"[redacted]")
            .finish()
    }
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
    StyleBounds,
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
            Self::StyleBounds => f.write_str("view style exceeds its geometry bound"),
            Self::MissingLabel => f.write_str("control or surface requires a nonempty label"),
            Self::Encoding(error) => write!(f, "invalid Rust Native view encoding: {error}"),
            Self::StaleActivation => f.write_str("activation does not name the current view"),
            Self::NotInteractive => f.write_str("event does not name an interactive control"),
            Self::Disabled => f.write_str("control is disabled"),
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

    pub fn new_v3(instance: impl Into<String>, revision: u64, root: Node<I>) -> Self {
        Self {
            schema: SCHEMA_V3.into(),
            instance: instance.into(),
            revision,
            root,
        }
    }

    pub fn validate(self) -> Result<ValidatedView<I>, ViewError> {
        if self.schema != SCHEMA && self.schema != SCHEMA_V3 {
            return Err(ViewError::Schema);
        }
        if !crate::valid_id(&self.instance) || self.revision == 0 {
            return Err(ViewError::Identity);
        }
        let mut keys = HashSet::new();
        let mut pending = vec![(&self.root, 1)];
        while let Some((node, depth)) = pending.pop() {
            if self.schema == SCHEMA
                && matches!(
                    node.element,
                    Element::Field { .. }
                        | Element::RichText { .. }
                        | Element::Choice { .. }
                        | Element::Dialog { .. }
                )
            {
                return Err(ViewError::Schema);
            }
            if depth > MAX_DEPTH {
                return Err(ViewError::DepthLimit);
            }
            if node
                .style
                .glyph_size
                .is_some_and(|value| !(1..=128).contains(&value))
                || node.style.glyph_gap.is_some_and(|value| value > 128)
                || node.style.radius.is_some_and(|radius| radius > 128)
                || node
                    .style
                    .padding_points
                    .is_some_and(|padding| padding.iter().any(|value| *value > 128))
                || node
                    .style
                    .button_padding
                    .is_some_and(|padding| padding.iter().any(|value| *value > 128))
                || node.style.gap_points.is_some_and(|value| value > 400)
                || node
                    .style
                    .text_size
                    .is_some_and(|value| !(1..=400).contains(&value))
                || node
                    .style
                    .line_height
                    .is_some_and(|value| !(1..=800).contains(&value))
                || node.style.min_height.is_some_and(|value| value > 4096)
                || node.style.button_avatar.is_some_and(|avatar| {
                    !(1..=128).contains(&avatar.size)
                        || !(1..=128).contains(&avatar.text_size)
                        || avatar.initial.is_control()
                        || avatar.initial.is_whitespace()
                })
                || node.style.button_detail.is_some_and(|detail| {
                    !(1..=400).contains(&detail.text_size)
                        || !(1..=800).contains(&detail.line_height)
                })
                || node.style.viewport.is_some_and(|viewport| {
                    !(1..=4096).contains(&viewport.max_height)
                        || viewport.fade > 128
                        || !matches!(
                            node.element,
                            Element::Stack {
                                axis: Axis::Vertical,
                                ..
                            }
                        )
                })
            {
                return Err(ViewError::StyleBounds);
            }
            if node.style.menu.is_some() {
                // A menu card: the card, then at least one button to offer.
                let Element::Stack { children, .. } = &node.element else {
                    return Err(ViewError::NotInteractive);
                };
                if children.len() < 2
                    || children[1..]
                        .iter()
                        .any(|child| !matches!(child.element, Element::Button { .. }))
                {
                    return Err(ViewError::NotInteractive);
                }
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
                Element::Stack { children, .. }
                | Element::List { children, .. }
                | Element::Transcript { children, .. }
                | Element::Message { children, .. }
                | Element::Dialog { children, .. }
                | Element::Choice { children, .. }
                | Element::Tool { children, .. } => {
                    match &node.element {
                        Element::Transcript {
                            label,
                            earlier,
                            source,
                            children,
                        } => {
                            if let Some(source) = source
                                && (!crate::valid_id(source) || !children.is_empty())
                            {
                                return Err(ViewError::Identity);
                            }
                            check_text(label)?;
                            if label.trim().is_empty() {
                                return Err(ViewError::MissingLabel);
                            }
                            if let Some(earlier) = earlier {
                                check_text(&earlier.label)?;
                                if earlier.label.trim().is_empty() {
                                    return Err(ViewError::MissingLabel);
                                }
                            }
                        }
                        Element::Message {
                            note: Some(note), ..
                        } => check_text(note)?,
                        Element::Tool { name, detail, .. } => {
                            check_text(name)?;
                            check_text(detail)?;
                            if name.trim().is_empty() {
                                return Err(ViewError::MissingLabel);
                            }
                        }
                        Element::Dialog { label, .. } => {
                            check_text(label)?;
                            if label.trim().is_empty() {
                                return Err(ViewError::MissingLabel);
                            }
                        }
                        Element::Choice { label, .. } => {
                            check_text(label)?;
                            if label.trim().is_empty() {
                                return Err(ViewError::MissingLabel);
                            }
                            let mut descendants: Vec<_> = children.iter().collect();
                            while let Some(child) = descendants.pop() {
                                match &child.element {
                                    Element::Button { .. }
                                    | Element::Choice { .. }
                                    | Element::Field { .. }
                                    | Element::Composer { .. }
                                    | Element::Dialog { .. }
                                    | Element::Tool { .. }
                                    | Element::Transcript { .. } => {
                                        return Err(ViewError::NotInteractive);
                                    }
                                    Element::Stack { children, .. }
                                    | Element::List { children, .. }
                                    | Element::Message { children, .. } => {
                                        descendants.extend(children)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
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
                Element::RichText { runs, .. } => {
                    if runs.len() > MAX_NODES {
                        return Err(ViewError::NodeLimit);
                    }
                    if runs.iter().map(|run| run.text.len()).sum::<usize>() > MAX_TEXT_BYTES {
                        return Err(ViewError::TextLimit);
                    }
                }
                Element::Field {
                    label,
                    value,
                    placeholder,
                    secret,
                    multiline,
                    max_bytes,
                    ..
                } => {
                    check_text(label)?;
                    check_text(placeholder)?;
                    if label.trim().is_empty() {
                        return Err(ViewError::MissingLabel);
                    }
                    if *max_bytes == 0
                        || *max_bytes > crate::input::MAX_INPUT_VALUE_BYTES
                        || value.len() > *max_bytes
                    {
                        return Err(ViewError::TextLimit);
                    }
                    // Secret values stay in the owning controller; a view
                    // carries only the empty masked editing field.
                    if (*secret && (!value.is_empty() || *multiline))
                        || (!*multiline && value.contains(['\n', '\r']))
                    {
                        return Err(ViewError::TextLimit);
                    }
                }
                Element::Surface { resource, label } => {
                    if !crate::valid_id(resource) {
                        return Err(ViewError::Identity);
                    }
                    check_text(label)?;
                    if label.trim().is_empty() {
                        return Err(ViewError::MissingLabel);
                    }
                }
                Element::Button { shortcut, .. }
                    if shortcut.as_ref().is_some_and(|s| {
                        s.trim().is_empty() || s.len() > 64 || s.contains(['\n', '\r'])
                    }) =>
                {
                    return Err(ViewError::TextLimit);
                }
                Element::Button { label, .. } | Element::Working { label } => {
                    check_text(label)?;
                    if label.trim().is_empty() {
                        return Err(ViewError::MissingLabel);
                    }
                }
                Element::Markdown { blocks } => {
                    if crate::markdown::text_bytes(blocks) > MAX_TEXT_BYTES {
                        return Err(ViewError::TextLimit);
                    }
                    if depth + crate::markdown::depth(blocks) > MAX_DEPTH {
                        return Err(ViewError::DepthLimit);
                    }
                }
                Element::Composer {
                    token,
                    placeholder,
                    max_bytes,
                    choices,
                    draft,
                    ..
                } => {
                    if !crate::valid_id(token) {
                        return Err(ViewError::Identity);
                    }
                    check_text(placeholder)?;
                    if *max_bytes == 0 || *max_bytes > crate::input::MAX_INPUT_VALUE_BYTES {
                        return Err(ViewError::TextLimit);
                    }
                    if draft.as_ref().is_some_and(|draft| draft.len() > *max_bytes) {
                        return Err(ViewError::TextLimit);
                    }
                    if choices.len() > MAX_COMPOSER_CHOICES {
                        return Err(ViewError::NodeLimit);
                    }
                    for (index, choice) in choices.iter().enumerate() {
                        // A choice's token is distinct from the send's and
                        // every other choice's, so an answer names one.
                        if !crate::valid_id(&choice.token)
                            || choice.token == *token
                            || choices[..index].iter().any(|c| c.token == choice.token)
                        {
                            return Err(ViewError::Identity);
                        }
                        check_text(&choice.label)?;
                        if choice.label.trim().is_empty() {
                            return Err(ViewError::MissingLabel);
                        }
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
                    Element::Choice {
                        enabled: true,
                        intent,
                        ..
                    } => Ok(intent),
                    Element::Choice { enabled: false, .. } => Err(ViewError::Disabled),
                    Element::Dialog {
                        open: true,
                        on_close,
                        ..
                    } => Ok(on_close),
                    Element::Dialog { open: false, .. } => Err(ViewError::Disabled),
                    Element::Transcript {
                        earlier: Some(earlier),
                        ..
                    } if !earlier.loading => Ok(&earlier.intent),
                    Element::Composer {
                        busy: true,
                        stop: Some(stop),
                        ..
                    } => Ok(stop),
                    Element::Transcript { .. } | Element::Composer { .. } => {
                        Err(ViewError::Disabled)
                    }
                    _ => Err(ViewError::NotInteractive),
                };
            }
            if let Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Dialog { children, .. }
            | Element::Choice { children, .. }
            | Element::Tool { children, .. } = &node.element
            {
                if !matches!(node.element, Element::Dialog { open: false, .. }) {
                    pending.extend(children);
                }
            }
        }
        Err(ViewError::NotInteractive)
    }
}

impl<I> ValidatedView<I> {
    /// Resolve a local field change against the current tree. The caller
    /// remains responsible for editing stamps and application validation.
    pub fn change_field<'a>(&'a self, event: &FieldChange) -> Result<&'a I, ViewError> {
        if event.activation.instance != self.0.instance
            || event.activation.revision != self.0.revision
        {
            return Err(ViewError::StaleActivation);
        }
        let mut pending = vec![&self.0.root];
        while let Some(node) = pending.pop() {
            if node.key == event.activation.node {
                return match &node.element {
                    Element::Field { enabled: false, .. } => Err(ViewError::Disabled),
                    Element::Field {
                        max_bytes,
                        multiline,
                        on_change,
                        ..
                    } => {
                        if event.value.len() > *max_bytes
                            || (!multiline && event.value.contains(['\n', '\r']))
                        {
                            Err(ViewError::TextLimit)
                        } else {
                            Ok(on_change)
                        }
                    }
                    _ => Err(ViewError::NotInteractive),
                };
            }
            if let Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. }
            | Element::Dialog { children, .. }
            | Element::Choice { children, .. } = &node.element
            {
                if !matches!(node.element, Element::Dialog { open: false, .. }) {
                    pending.extend(children);
                }
            }
        }
        Err(ViewError::NotInteractive)
    }
}

impl<I> ValidatedView<I> {
    /// Accept a composer's text: `token` must name an enabled composer in
    /// this view, or one of its choices, and the text must fit its bound.
    /// The application still validates what the text means, and gives a
    /// choice's token its meaning.
    pub fn accept_composer(&self, token: &str, text: &str) -> Result<(), crate::InputError> {
        let mut pending = vec![&self.0.root];
        while let Some(node) = pending.pop() {
            match &node.element {
                Element::Composer {
                    token: current,
                    enabled,
                    max_bytes,
                    choices,
                    ..
                } if current == token || choices.iter().any(|choice| choice.token == token) => {
                    if !enabled {
                        return Err(crate::InputError::Stale);
                    }
                    if text.len() > *max_bytes {
                        return Err(crate::InputError::TooLong);
                    }
                    return Ok(());
                }
                Element::Stack { children, .. }
                | Element::List { children, .. }
                | Element::Transcript { children, .. }
                | Element::Message { children, .. }
                | Element::Dialog { children, .. }
                | Element::Choice { children, .. }
                | Element::Tool { children, .. } => {
                    if !matches!(node.element, Element::Dialog { open: false, .. }) {
                        pending.extend(children);
                    }
                }
                _ => {}
            }
        }
        Err(crate::InputError::Stale)
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
