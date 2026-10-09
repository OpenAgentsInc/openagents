//! Action and display components (UI-02): Button, ButtonLink, CopyButton,
//! TextLink, Badge, the loading indicators, Avatar and AvatarGroup, Alert,
//! EmptyMessage, Image and ShimmerText.
//!
//! Each builder emits the class names of its stylesheet under
//! `static/components/` and the same data attributes the Apps SDK UI React
//! component puts on the DOM (`data-color`, `data-variant`, `data-size`,
//! `data-pill`, ...), so the ported CSS applies unchanged. Builders escape
//! every text and attribute value they are given, and never emit inline
//! `style` attributes or inline scripts (the site CSP is `style-src 'self'`
//! and `script-src 'self'`).
//!
//! Every builder also takes `.id(..)`, `.class(..)` (extra classes, for
//! example Tailwind layout utilities) and `.attr(name, value)` /
//! `.flag(name)` for anything else, such as `hx-post` or `aria-controls`.

/// Generates the shared `.id`, `.class`, `.attr` and `.flag` setters for a
/// builder with an `attrs: html::Attrs` field.
macro_rules! impl_attrs {
    ($ty:ty) => {
        impl $ty {
            /// Sets the element `id`.
            pub fn id(mut self, id: impl Into<String>) -> Self {
                self.attrs.id = Some(id.into());
                self
            }

            /// Adds extra classes (for example Tailwind layout utilities).
            pub fn class(mut self, class: impl Into<String>) -> Self {
                self.attrs.classes.push(class.into());
                self
            }

            /// Adds an attribute, such as `hx-post` or `aria-controls`. The
            /// value is escaped; names outside `[A-Za-z0-9_:.@-]` are dropped.
            pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
                self.attrs.extra.push((name.into(), Some(value.into())));
                self
            }

            /// Adds a boolean attribute, such as `hx-boost` or `autofocus`.
            pub fn flag(mut self, name: impl Into<String>) -> Self {
                self.attrs.extra.push((name.into(), None));
                self
            }
        }
    };
}

mod alert;
mod avatar;
mod badge;
mod button;
mod empty_message;
mod glyphs;
mod html;
mod image;
mod indicator;
mod shimmer_text;
mod text_link;

pub use alert::{Alert, AlertActionsPlacement};
pub use avatar::{Avatar, AvatarGroup, AvatarSize, AvatarStack};
pub use badge::{Badge, BadgeSize};
pub use button::{
    Button, ButtonLink, ButtonType, ButtonVariant, CopyButton, DisabledTone, GutterSize, IconSize,
    OpticalAlign,
};
pub use empty_message::{EmptyMessage, EmptyMessageFill, EmptyMessageIconSize};
pub use image::Image;
pub use indicator::{CircularProgress, LoadingDots, LoadingIndicator};
pub use shimmer_text::{ShimmerTag, ShimmerText};
pub use text_link::TextLink;

/// The CopyButton script hook. Serve it as a static file and load it with
/// `<script src=... defer>`; it handles every `[data-oa-copy]` button on the
/// page through one delegated click listener (no inline JavaScript).
pub const COPY_BUTTON_JS: &str = include_str!("../../static/components/copy-button.js");

/// Semantic color, mirroring Apps SDK UI's `SemanticColor`. Each component
/// styles the subset its React counterpart accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Primary,
    Secondary,
    Danger,
    Success,
    Warning,
    Caution,
    Discovery,
    Info,
}

impl Color {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Danger => "danger",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Caution => "caution",
            Self::Discovery => "discovery",
            Self::Info => "info",
        }
    }
}

/// Visual style for Badge, Alert and Avatar (Avatar uses Solid and Soft).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    Solid,
    Soft,
    Outline,
}

impl Variant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Soft => "soft",
            Self::Outline => "outline",
        }
    }
}

/// Control height scale, mirroring Apps SDK UI's `ControlSize`
/// (22px at `Xs3` to 48px at `Xl3`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControlSize {
    Xs3,
    Xs2,
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
    Xl2,
    Xl3,
}

impl ControlSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xs3 => "3xs",
            Self::Xs2 => "2xs",
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
            Self::Xl2 => "2xl",
            Self::Xl3 => "3xl",
        }
    }
}

#[cfg(test)]
mod tests;
