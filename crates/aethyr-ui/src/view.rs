use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One published revision of a controller's interface.
///
/// A new controller lifetime takes a fresh `instance`. A revision names one
/// immutable tree in that lifetime; the controller never reuses a revision
/// for different content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// Always `aethyr.ui.v1`.
    pub schema: String,
    #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
    pub instance: String,
    /// Positive, and increasing within an instance.
    #[schemars(range(min = 1))]
    pub revision: u64,
    pub root: Node,
}

/// A semantic node. Its `id` is stable across revisions for the same thing,
/// so a renderer keeps focus, scroll, and mounted media on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Unique in the view: letters, digits, `-`, `_`, `.`, `:`.
    #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
    pub id: String,
    #[serde(default, skip_serializing_if = "Style::is_plain")]
    pub style: Style,
    #[serde(default, skip_serializing_if = "A11y::is_plain")]
    pub a11y: A11y,
    pub element: Element,
}

/// The closed set of node kinds. Anything else needs its own contract first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Element {
    /// A HUD glass panel with corner ticks.
    Panel {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        children: Vec<Node>,
    },
    /// Children in a row, a column, or a wrapping row.
    Stack { axis: Axis, children: Vec<Node> },
    /// Read-only text.
    Text { value: String, role: TextRole },
    /// A mono, uppercase HUD readout, optionally led by a pulsing signal dot.
    Label {
        value: String,
        #[serde(default, skip_serializing_if = "is_false")]
        signal: bool,
    },
    /// A control. Sends `activate`. With `href` (a same-site path) it is a
    /// link: a plain click still sends `activate`, so the controller decides,
    /// and the link keeps working without the controller.
    Button {
        label: String,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Emphasis::is_default")]
        emphasis: Emphasis,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        href: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leading: Option<Glyph>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trailing: Option<Glyph>,
    },
    /// A small toggle or filter. Sends `activate`; `selected` is controller
    /// state, never inferred from color. An optional `detail` is a short
    /// second line, such as a destination's category.
    Chip {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        selected: bool,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// Rows. When `selectable`, choosing a row sends `select` with that
    /// row's (direct child's) ID as the value.
    List {
        label: String,
        #[serde(default, skip_serializing_if = "is_false")]
        selectable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selected: Option<String>,
        children: Vec<Node>,
    },
    /// An editable value. Typing sends `input` with the whole value; Enter
    /// (single line) sends `submit`.
    Field {
        label: String,
        value: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        placeholder: String,
        #[serde(default, skip_serializing_if = "is_false")]
        secret: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        multiline: bool,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
        max_bytes: u32,
    },
    /// A non-modal panel: beside the content on wide screens, a bottom sheet
    /// on phones. Sends `dismiss` when `dismissible`.
    Sheet {
        label: String,
        open: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        dismissible: bool,
        children: Vec<Node>,
    },
    /// A modal focus scope. Sends `dismiss` (Escape) when `dismissible`; the
    /// controller decides whether it closes. The renderer restores focus.
    Dialog {
        label: String,
        open: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        dismissible: bool,
        children: Vec<Node>,
    },
    /// A short status message, announced politely. Sends `dismiss` when
    /// `dismissible`.
    Toast {
        message: String,
        #[serde(default, skip_serializing_if = "Tone::is_default")]
        tone: Tone,
        #[serde(default, skip_serializing_if = "is_false")]
        dismissible: bool,
    },
    /// A place the host fills with an image, video, or illustration it
    /// registered under `slot`. Never a URL.
    MediaSlot {
        #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
        slot: String,
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aspect: Option<Aspect>,
    },
    /// The engine's drawing surface, registered by the host under `surface`.
    /// The renderer keeps it mounted across revisions and never sends
    /// intents for it; the engine takes its own input.
    EngineSurface {
        #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
        surface: String,
        label: String,
    },
    /// Choose one of `options`. Sends `select` with the chosen option's
    /// `value`; `value` is the current choice.
    Select {
        label: String,
        value: String,
        options: Vec<Choice>,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// On or off. Sends `input` with `true` or `false`, the state the person
    /// asked for.
    Checkbox {
        label: String,
        checked: bool,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// A number between `min` and `max` on `step`. Sends `input` with the
    /// new value as a decimal number.
    Range {
        label: String,
        value: Decimal,
        min: Decimal,
        max: Decimal,
        step: Decimal,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// A color, as `#rrggbb` in lowercase. Sends `input` with the new color.
    Color {
        label: String,
        #[schemars(regex(pattern = r"^#[0-9a-f]{6}$"))]
        value: String,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// An upload slot. The renderer checks each chosen file against
    /// `accept` and `max_bytes`, hands the files to the host under `slot`,
    /// and sends `input` with their names, one per line. File contents
    /// never travel in an intent.
    File {
        label: String,
        #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
        slot: String,
        /// Media types (`image/png`) or extensions (`.json`); empty takes any.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        accept: Vec<String>,
        /// The largest file accepted, in bytes.
        max_bytes: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        multiple: bool,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        enabled: bool,
    },
    /// A drawing surface the host registered under `surface`, at `width` by
    /// `height` CSS pixels, scaled down to fit. The renderer keeps it mounted
    /// across revisions and never sends intents for it; the engine takes
    /// its own input (painting, for example).
    Canvas {
        #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
        surface: String,
        label: String,
        #[schemars(range(min = 1, max = 4096))]
        width: u32,
        #[schemars(range(min = 1, max = 4096))]
        height: u32,
    },
}

/// One option of a `select`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

/// A finite decimal number. Equality is by bit pattern, so a view compares
/// equal to itself; validation refuses NaN and infinities. A whole number
/// encodes as an integer (`5`, not `5.0`), as JavaScript writes it, so both
/// sides produce the same JSON.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Decimal(pub f64);

impl Serialize for Decimal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const SAFE: f64 = 9_007_199_254_740_991.0;
        if self.0.fract() == 0.0 && self.0.abs() <= SAFE {
            serializer.serialize_i64(self.0 as i64)
        } else {
            serializer.serialize_f64(self.0)
        }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Decimal {}

impl Element {
    /// The `kind` tag, as serialized.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Panel { .. } => "panel",
            Self::Stack { .. } => "stack",
            Self::Text { .. } => "text",
            Self::Label { .. } => "label",
            Self::Button { .. } => "button",
            Self::Chip { .. } => "chip",
            Self::List { .. } => "list",
            Self::Field { .. } => "field",
            Self::Sheet { .. } => "sheet",
            Self::Dialog { .. } => "dialog",
            Self::Toast { .. } => "toast",
            Self::MediaSlot { .. } => "media_slot",
            Self::EngineSurface { .. } => "engine_surface",
            Self::Select { .. } => "select",
            Self::Checkbox { .. } => "checkbox",
            Self::Range { .. } => "range",
            Self::Color { .. } => "color",
            Self::File { .. } => "file",
            Self::Canvas { .. } => "canvas",
        }
    }

    pub fn children(&self) -> &[Node] {
        match self {
            Self::Panel { children, .. }
            | Self::Stack { children, .. }
            | Self::List { children, .. }
            | Self::Sheet { children, .. }
            | Self::Dialog { children, .. } => children,
            _ => &[],
        }
    }
}

/// Every node kind, in schema order.
pub const ELEMENT_KINDS: &[&str] = &[
    "panel",
    "stack",
    "text",
    "label",
    "button",
    "chip",
    "list",
    "field",
    "sheet",
    "dialog",
    "toast",
    "media_slot",
    "engine_surface",
    "select",
    "checkbox",
    "range",
    "color",
    "file",
    "canvas",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Vertical,
    Horizontal,
    /// Left to right, continuing on the next line.
    Wrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Body,
    /// The main heading of a panel or dialog.
    Title,
    Heading,
    /// Small supporting text.
    Caption,
    Code,
    /// Text that changes and is announced politely.
    Status,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Emphasis {
    /// The one main action of a group.
    Primary,
    #[default]
    Secondary,
    /// No fill until hovered.
    Ghost,
    /// Inline, like a text link.
    Link,
}

impl Emphasis {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A decorative mark beside a button's label. Hidden from assistive
/// technology; the label carries the meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Glyph {
    /// Go somewhere (↗).
    Forward,
    /// Go back (←).
    Back,
    /// Close (×).
    Close,
}

/// A color role, resolved by the renderer through the design tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Default,
    Muted,
    Primary,
    Danger,
}

impl Tone {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Square,
    /// 16:9.
    Wide,
    /// 3:4.
    Tall,
}

/// A step on the spacing scale (`--hud-space-*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Start,
    Center,
    End,
    /// Spread children to both ends (stacks only).
    Between,
}

/// Presentation, by token reference only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Style {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<Space>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<Space>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<Tone>,
    /// A secondary node, hidden at phone width.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hide_on_phone: bool,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}

/// An explicit accessibility role. Each kind has its own default; set this
/// only to change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Group,
    Region,
    Navigation,
    Status,
    Alert,
    /// Decorative: removed from the accessibility tree.
    Presentation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Live {
    Polite,
    Assertive,
}

/// Accessibility overrides. A control's visible label is its accessible
/// name unless `label` says otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct A11y {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<Live>,
    /// Hidden from assistive technology (decorative).
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

impl A11y {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    /// Press a button or chip.
    Activate,
    /// Choose a row of a selectable list (`value` is the row's ID) or an
    /// option of a select (`value` is the option's value).
    Select,
    /// Submit a field.
    Submit,
    /// Close a dismissible dialog, sheet, or toast.
    Dismiss,
    /// A new value, in full: a field's text, a checkbox's `true` or
    /// `false`, a range's number, a color, or the names of chosen files.
    Input,
}

/// What the person did, sent by the renderer. It carries identity and, for
/// `select` and `input`, a value; the current tree says what it means.
/// Input values are personal data: never log them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
    pub instance: String,
    /// The revision the renderer drew. An intent for an older revision is
    /// refused as stale.
    #[schemars(range(min = 1))]
    pub revision: u64,
    #[schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))]
    pub node: String,
    pub kind: IntentKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl std::fmt::Debug for Intent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Intent")
            .field("instance", &self.instance)
            .field("revision", &self.revision)
            .field("node", &self.node)
            .field("kind", &self.kind)
            .field("value", &self.value.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl Node {
    pub fn new(id: impl Into<String>, element: Element) -> Self {
        Self {
            id: id.into(),
            style: Style::default(),
            a11y: A11y::default(),
            element,
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_a11y(mut self, a11y: A11y) -> Self {
        self.a11y = a11y;
        self
    }
}

impl View {
    pub fn new(instance: impl Into<String>, revision: u64, root: Node) -> Self {
        Self {
            schema: crate::SCHEMA.into(),
            instance: instance.into(),
            revision,
            root,
        }
    }
}
