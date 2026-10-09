//! Button, ButtonLink and CopyButton, ported from Apps SDK UI
//! `src/components/Button` (MIT). Styles: `static/components/button.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag, safe_url};
use super::{Color, ControlSize, glyphs};

/// Button style variant. `Transparent` is an OpenAgents addition (text only,
/// no background), matching ChatGPT's toolbar buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    #[default]
    Solid,
    Soft,
    Outline,
    Ghost,
    Transparent,
}

impl ButtonVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Soft => "soft",
            Self::Outline => "outline",
            Self::Ghost => "ghost",
            Self::Transparent => "transparent",
        }
    }
}

/// Icon size override, from the `--control-icon-size-*` tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconSize {
    Sm,
    Md,
    Lg,
    Xl,
    Xl2,
}

impl IconSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
            Self::Xl2 => "2xl",
        }
    }
}

/// Edge gutter override (`--control-gutter-*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GutterSize {
    Xs2,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl GutterSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xs2 => "2xs",
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
        }
    }
}

/// Negative margin by the gutter to optically align with surrounding content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpticalAlign {
    Start,
    End,
}

impl OpticalAlign {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// `Relaxed` keeps the disabled look with a default cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisabledTone {
    Relaxed,
}

/// The `type` attribute of a `<button>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonType {
    #[default]
    Button,
    Submit,
    Reset,
}

impl ButtonType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Submit => "submit",
            Self::Reset => "reset",
        }
    }
}

#[derive(Clone, Debug)]
struct Style {
    color: Color,
    variant: ButtonVariant,
    pill: bool,
    size: ControlSize,
    icon_size: Option<IconSize>,
    gutter_size: Option<GutterSize>,
    block: bool,
    optically_align: Option<OpticalAlign>,
    disabled: bool,
    disabled_tone: Option<DisabledTone>,
    uniform: bool,
    selected: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: Color::Primary,
            variant: ButtonVariant::Solid,
            pill: true,
            size: ControlSize::Md,
            icon_size: None,
            gutter_size: None,
            block: false,
            optically_align: None,
            disabled: false,
            disabled_tone: None,
            uniform: false,
            selected: false,
        }
    }
}

impl Style {
    /// The data attributes in the order the React component writes them.
    fn data(&self, tag: Tag) -> Tag {
        tag.attr("data-color", self.color.as_str())
            .attr("data-variant", self.variant.as_str())
            .flag("data-pill", self.pill)
            .flag("data-uniform", self.uniform)
            .attr("data-size", self.size.as_str())
            .attr_opt("data-gutter-size", self.gutter_size.map(GutterSize::as_str))
            .attr_opt("data-icon-size", self.icon_size.map(IconSize::as_str))
    }

    fn placement(&self, tag: Tag) -> Tag {
        tag.flag("data-selected", self.selected)
            .flag("data-block", self.block)
            .attr_opt(
                "data-optically-align",
                self.optically_align.map(OpticalAlign::as_str),
            )
    }

    fn disabled_data(&self, tag: Tag) -> Tag {
        tag.flag("data-disabled", self.disabled).attr_opt(
            "data-disabled-tone",
            match (self.disabled, self.disabled_tone) {
                (true, Some(DisabledTone::Relaxed)) => Some("relaxed"),
                _ => None,
            },
        )
    }
}

/// Label plus optional leading and trailing icons. Text with icon siblings
/// is wrapped in a `<span>`, as `wrapTextNodeSiblings` does in React.
#[derive(Clone, Debug, Default)]
struct Content {
    label: Option<String>,
    start: Option<Markup>,
    end: Option<Markup>,
    custom: Option<Markup>,
}

impl Content {
    fn render(&self) -> Markup {
        if let Some(custom) = &self.custom {
            return custom.clone();
        }
        let has_icons = self.start.is_some() || self.end.is_some();
        html! {
            @if let Some(start) = &self.start { (start) }
            @if let Some(label) = &self.label {
                @if has_icons { span { (label) } } @else { (label) }
            }
            @if let Some(end) = &self.end { (end) }
        }
    }
}

/// Generates the style setters shared by Button, ButtonLink and CopyButton.
macro_rules! style_setters {
    ($ty:ty) => {
        impl $ty {
            /// Semantic color (default `Primary`).
            pub fn color(mut self, color: Color) -> Self {
                self.style.color = color;
                self
            }

            /// Style variant (default `Solid`).
            pub fn variant(mut self, variant: ButtonVariant) -> Self {
                self.style.variant = variant;
                self
            }

            /// Fully rounded pill shape (default `true`).
            pub fn pill(mut self, pill: bool) -> Self {
                self.style.pill = pill;
                self
            }

            /// Control size (default `Md`, 32px).
            pub fn size(mut self, size: ControlSize) -> Self {
                self.style.size = size;
                self
            }

            /// Icon size override; defaults from `size`.
            pub fn icon_size(mut self, icon_size: IconSize) -> Self {
                self.style.icon_size = Some(icon_size);
                self
            }

            /// Edge gutter override; defaults from `size`.
            pub fn gutter_size(mut self, gutter_size: GutterSize) -> Self {
                self.style.gutter_size = Some(gutter_size);
                self
            }

            /// Takes 100% of the available width.
            pub fn block(mut self, block: bool) -> Self {
                self.style.block = block;
                self
            }

            /// Pulls the button out by its gutter to align its content optically.
            pub fn optically_align(mut self, align: OpticalAlign) -> Self {
                self.style.optically_align = Some(align);
                self
            }

            /// Disables the control visually and for interaction.
            pub fn disabled(mut self, disabled: bool) -> Self {
                self.style.disabled = disabled;
                self
            }

            /// Disabled visuals with a default cursor.
            pub fn disabled_tone(mut self, tone: DisabledTone) -> Self {
                self.style.disabled_tone = Some(tone);
                self
            }

            /// Selected styles, varying by variant (`data-selected`).
            pub fn selected(mut self, selected: bool) -> Self {
                self.style.selected = selected;
                self
            }

            /// Leading icon; any `impl Render`, such as an `Icon` variant.
            pub fn icon_start(mut self, icon: impl Render) -> Self {
                self.content.start = Some(icon.render());
                self
            }

            /// Trailing icon.
            pub fn icon_end(mut self, icon: impl Render) -> Self {
                self.content.end = Some(icon.render());
                self
            }

            /// Accessible name, required when the button shows only an icon.
            pub fn aria_label(mut self, label: impl Into<String>) -> Self {
                self.aria_label = Some(label.into());
                self
            }
        }
    };
}

/// A `<button class="oa-button">`.
///
/// ```
/// use openagents_ui::actions::{Button, ButtonVariant, Color, ControlSize};
/// let save = Button::new("Save").variant(ButtonVariant::Soft).color(Color::Secondary).size(ControlSize::Sm);
/// assert!(maud::Render::render(&save).into_string().contains(r#"data-variant="soft""#));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Button {
    style: Style,
    content: Content,
    kind: ButtonType,
    loading: bool,
    inert: bool,
    name: Option<String>,
    value: Option<String>,
    aria_label: Option<String>,
    attrs: Attrs,
}

impl Button {
    /// A text button.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            content: Content {
                label: Some(label.into()),
                ..Content::default()
            },
            ..Self::default()
        }
    }

    /// A square icon-only button (`data-uniform`) named by `aria_label`.
    pub fn icon(icon: impl Render, aria_label: impl Into<String>) -> Self {
        let mut button = Self {
            content: Content {
                start: Some(icon.render()),
                ..Content::default()
            },
            aria_label: Some(aria_label.into()),
            ..Self::default()
        };
        button.style.uniform = true;
        button
    }

    /// A button with arbitrary inner markup.
    pub fn with_content(content: impl Render) -> Self {
        Self {
            content: Content {
                custom: Some(content.render()),
                ..Content::default()
            },
            ..Self::default()
        }
    }

    /// Matching width and height from `size`.
    pub fn uniform(mut self, uniform: bool) -> Self {
        self.style.uniform = uniform;
        self
    }

    /// Shows a spinner over the content and makes the button inert; it is
    /// announced with `aria-busy` and `aria-disabled`.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Inert without a visual change.
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }

    /// The `type` attribute (default `button`).
    pub fn kind(mut self, kind: ButtonType) -> Self {
        self.kind = kind;
        self
    }

    /// Form field name submitted with the button.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Form field value submitted with the button.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
}

style_setters!(Button);
impl_attrs!(Button);

impl Render for Button {
    fn render(&self) -> Markup {
        let inert = self.style.disabled || self.inert || self.loading;
        let mut tag = Tag::new("button", "oa-button", &self.attrs).attr("type", self.kind.as_str());
        tag = self.style.data(tag).flag("data-loading", self.loading);
        tag = self
            .style
            .placement(tag)
            .attr_opt("name", self.name.as_deref())
            .attr_opt("value", self.value.as_deref())
            .attr_opt("aria-label", self.aria_label.as_deref())
            .flag("disabled", inert);
        if inert {
            tag = tag.attr("aria-disabled", "true").attr("tabindex", "-1");
        }
        if self.loading {
            tag = tag.attr("aria-busy", "true");
        }
        tag = self.style.disabled_data(tag).extra(&self.attrs);
        tag.close(html! {
            @if self.loading {
                span class="oa-button-loader" {
                    span class="oa-loading-indicator" aria-hidden="true" {}
                }
            }
            span class="oa-button-inner" { (self.content.render()) }
        })
    }
}

/// An `<a class="oa-button">`. URLs starting with `http://` or `https://`
/// open in a new tab with `rel="noopener noreferrer"`. A disabled link
/// renders as `<span role="link" aria-disabled="true">`.
#[derive(Clone, Debug, Default)]
pub struct ButtonLink {
    style: Style,
    content: Content,
    href: String,
    external: Option<bool>,
    aria_label: Option<String>,
    attrs: Attrs,
}

impl ButtonLink {
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            content: Content {
                label: Some(label.into()),
                ..Content::default()
            },
            href: href.into(),
            ..Self::default()
        }
    }

    /// A square icon-only link named by `aria_label`.
    pub fn icon(icon: impl Render, aria_label: impl Into<String>, href: impl Into<String>) -> Self {
        let mut link = Self {
            content: Content {
                start: Some(icon.render()),
                ..Content::default()
            },
            href: href.into(),
            aria_label: Some(aria_label.into()),
            ..Self::default()
        };
        link.style.uniform = true;
        link
    }

    /// Forces (or suppresses) external-link behavior instead of detecting it.
    pub fn external(mut self, external: bool) -> Self {
        self.external = Some(external);
        self
    }
}

style_setters!(ButtonLink);
impl_attrs!(ButtonLink);

impl Render for ButtonLink {
    fn render(&self) -> Markup {
        let disabled = self.style.disabled;
        let mut tag = if disabled {
            Tag::new("span", "oa-button", &self.attrs).attr("role", "link")
        } else {
            let external = self.external.unwrap_or_else(|| is_external(&self.href));
            let tag = Tag::new("a", "oa-button", &self.attrs).attr("href", &safe_url(&self.href));
            if external {
                tag.attr("target", "_blank")
                    .attr("rel", "noopener noreferrer")
            } else {
                tag
            }
        };
        if disabled {
            tag = tag.attr("aria-disabled", "true").attr("tabindex", "-1");
        }
        tag = self.style.disabled_data(tag);
        tag = self.style.data(tag);
        tag = self
            .style
            .placement(tag)
            .attr_opt("aria-label", self.aria_label.as_deref())
            .extra(&self.attrs);
        tag.close(html! {
            span class="oa-button-inner" { (self.content.render()) }
        })
    }
}

pub(crate) fn is_external(href: &str) -> bool {
    let lower = href.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// A Button that copies text to the clipboard through the CSP-safe script
/// hook in `static/components/copy-button.js` (`COPY_BUTTON_JS`). The value
/// travels in `data-oa-copy`, or `copy_from` names an element whose text is
/// copied. On success the script sets `data-copied` for 1.3s (swapping the
/// Copy icon for a Check) and announces the copied label in a polite live
/// region. Without the script it is an ordinary, harmless button.
#[derive(Clone, Debug)]
pub struct CopyButton {
    style: Style,
    content: Content,
    copy_value: String,
    copy_from: Option<String>,
    copied_label: String,
    aria_label: Option<String>,
    copy_icon: Markup,
    copied_icon: Markup,
    attrs: Attrs,
}

impl CopyButton {
    /// An icon-only copy button (`aria-label="Copy"`) for `copy_value`.
    pub fn new(copy_value: impl Into<String>) -> Self {
        let style = Style {
            uniform: true,
            ..Style::default()
        };
        Self {
            style,
            content: Content::default(),
            copy_value: copy_value.into(),
            copy_from: None,
            copied_label: "Copied".to_string(),
            aria_label: Some("Copy".to_string()),
            copy_icon: glyphs::copy(),
            copied_icon: glyphs::check(),
            attrs: Attrs::default(),
        }
    }

    /// Visible text after the icon; turns off the uniform shape and the
    /// default `aria-label`.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.content.label = Some(label.into());
        self.style.uniform = false;
        self.aria_label = None;
        self
    }

    /// Copies the text content of the element with this id instead of the
    /// value (`data-oa-copy-from`).
    pub fn copy_from(mut self, element_id: impl Into<String>) -> Self {
        self.copy_from = Some(element_id.into());
        self
    }

    /// Text announced after a copy (default "Copied").
    pub fn copied_label(mut self, label: impl Into<String>) -> Self {
        self.copied_label = label.into();
        self
    }

    /// Replaces the resting Copy icon.
    pub fn copy_icon(mut self, icon: impl Render) -> Self {
        self.copy_icon = icon.render();
        self
    }

    /// Replaces the Check icon shown after a copy.
    pub fn copied_icon(mut self, icon: impl Render) -> Self {
        self.copied_icon = icon.render();
        self
    }

    /// Matching width and height from `size`.
    pub fn uniform(mut self, uniform: bool) -> Self {
        self.style.uniform = uniform;
        self
    }
}

style_setters!(CopyButton);
impl_attrs!(CopyButton);

impl Render for CopyButton {
    fn render(&self) -> Markup {
        let mut attrs = self.attrs.clone();
        attrs
            .extra
            .insert(0, ("data-oa-copy".into(), Some(self.copy_value.clone())));
        if let Some(from) = &self.copy_from {
            attrs
                .extra
                .insert(1, ("data-oa-copy-from".into(), Some(from.clone())));
        }
        attrs.extra.push((
            "data-oa-copied-label".into(),
            Some(self.copied_label.clone()),
        ));
        let label = self.content.label.as_deref();
        let inner = html! {
            span class="oa-copy-button-icon" data-copy-icon="copy" { (self.copy_icon) }
            span class="oa-copy-button-icon" data-copy-icon="copied" { (self.copied_icon) }
            @if let Some(label) = label { span { (label) } }
            span class="oa-copy-button-status" role="status" aria-live="polite" {}
        };
        let mut button = Button::with_content(inner);
        button.style = self.style.clone();
        button.aria_label = self.aria_label.clone();
        button.attrs = attrs;
        button.render()
    }
}
