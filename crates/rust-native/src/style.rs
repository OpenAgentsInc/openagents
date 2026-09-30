//! Typed declarations with ordered, property-by-property composition.
//!
//! This is not CSS or a StyleX compiler. Shorthands expand to leaf properties
//! before composition. Reset explicitly restores a supplied renderer default.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Semantic spacing. Each renderer documents its own physical mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    None,
    Xs,
    Sm,
    Md,
    Lg,
}

/// A device-independent sRGB color. Applications supply palettes and defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextWeight {
    Normal,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Start,
    Center,
    End,
}

/// How a stack offers some of its buttons as a native menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Menu {
    /// A card with a context menu. The stack's first child is the card, and
    /// every other child is a button the card's menu offers: on a long press
    /// on a phone (`UIMenu`, `PopupMenu`), or on a secondary click on a
    /// desktop. Each item activates through the view like any button, so a
    /// stale menu refuses. An adapter without menus lays the children out on
    /// the stack's axis.
    Context,
}

/// Presentation of a button label's secondary line, after its first newline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonDetail {
    pub text_size: u16,
    pub line_height: u16,
    pub color: Color,
    pub leading: bool,
}

/// Unset preserves an earlier declaration. Reset removes it at this layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Patch<T> {
    #[default]
    Unset,
    Set(T),
    Reset,
}

impl<T: Copy> Patch<T> {
    fn overlay(self, later: Self) -> Self {
        match later {
            Self::Unset => self,
            explicit => explicit,
        }
    }

    fn resolve(self, default: Option<T>) -> Option<T> {
        match self {
            Self::Set(value) => Some(value),
            Self::Unset | Self::Reset => default,
        }
    }
}

/// Resolved style tokens. None delegates that property to the adapter default.
/// There is no implicit parent inheritance in this initial contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub padding_top: Option<Space>,
    pub padding_end: Option<Space>,
    pub padding_bottom: Option<Space>,
    pub padding_start: Option<Space>,
    pub gap: Option<Space>,
    pub weight: Option<TextWeight>,
    pub align: Option<TextAlign>,
    /// Rounded corners in logical points; zero produces square corners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<u16>,
    /// A one-point inset border around a stack's background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Color>,
    /// Fill the remaining height of a bounded vertical container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_height: Option<bool>,
    /// Keep measured content width in a horizontal stack instead of sharing space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_width: Option<bool>,
    /// Exact top, end, bottom, and start padding in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_points: Option<[u16; 4]>,
    /// Exact spacing between children in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_points: Option<u16>,
    /// Text size in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_size: Option<u16>,
    /// Text line height in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<u16>,
    /// Minimum box height in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<u16>,
    /// Horizontal and vertical button content insets in logical points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_padding: Option<[u16; 2]>,
    /// Keep one semantic label while styling its secondary line independently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_detail: Option<ButtonDetail>,
    /// An explicit hover fill for a framed button; absence uses the adapter default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover_background: Option<Color>,
    /// The vector glyph size, independent of its button's hit area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph_size: Option<u16>,
    /// Space between a leading glyph and its label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph_gap: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph_color: Option<Color>,
    /// Use the monospaced member of the selected font family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monospace: Option<bool>,
    /// Offer the stack's other buttons as its first child's menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<Menu>,
}

/// A declaration contains only canonical leaf properties. A later explicit
/// leaf wins regardless of style name, insertion order, or selector priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StylePatch {
    pub foreground: Patch<Color>,
    pub background: Patch<Color>,
    pub padding_top: Patch<Space>,
    pub padding_end: Patch<Space>,
    pub padding_bottom: Patch<Space>,
    pub padding_start: Patch<Space>,
    pub gap: Patch<Space>,
    pub weight: Patch<TextWeight>,
    pub align: Patch<TextAlign>,
    pub radius: Patch<u16>,
    pub border: Patch<Color>,
    pub fill_height: Patch<bool>,
    pub intrinsic_width: Patch<bool>,
    pub padding_points: Patch<[u16; 4]>,
    pub gap_points: Patch<u16>,
    pub text_size: Patch<u16>,
    pub line_height: Patch<u16>,
    pub min_height: Patch<u16>,
    pub button_padding: Patch<[u16; 2]>,
    pub button_detail: Patch<ButtonDetail>,
    pub hover_background: Patch<Color>,
    pub glyph_size: Patch<u16>,
    pub glyph_gap: Patch<u16>,
    pub glyph_color: Patch<Color>,
    pub monospace: Patch<bool>,
    pub menu: Patch<Menu>,
}

impl StylePatch {
    /// Expand the shorthand before layering; a subsequent edge can override it.
    pub fn padding(space: Space) -> Self {
        Self {
            padding_top: Patch::Set(space),
            padding_end: Patch::Set(space),
            padding_bottom: Patch::Set(space),
            padding_start: Patch::Set(space),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn then(self, later: Self) -> Self {
        Self {
            foreground: self.foreground.overlay(later.foreground),
            background: self.background.overlay(later.background),
            padding_top: self.padding_top.overlay(later.padding_top),
            padding_end: self.padding_end.overlay(later.padding_end),
            padding_bottom: self.padding_bottom.overlay(later.padding_bottom),
            padding_start: self.padding_start.overlay(later.padding_start),
            gap: self.gap.overlay(later.gap),
            weight: self.weight.overlay(later.weight),
            align: self.align.overlay(later.align),
            radius: self.radius.overlay(later.radius),
            border: self.border.overlay(later.border),
            fill_height: self.fill_height.overlay(later.fill_height),
            intrinsic_width: self.intrinsic_width.overlay(later.intrinsic_width),
            padding_points: self.padding_points.overlay(later.padding_points),
            gap_points: self.gap_points.overlay(later.gap_points),
            text_size: self.text_size.overlay(later.text_size),
            line_height: self.line_height.overlay(later.line_height),
            min_height: self.min_height.overlay(later.min_height),
            button_padding: self.button_padding.overlay(later.button_padding),
            button_detail: self.button_detail.overlay(later.button_detail),
            hover_background: self.hover_background.overlay(later.hover_background),
            glyph_size: self.glyph_size.overlay(later.glyph_size),
            glyph_gap: self.glyph_gap.overlay(later.glyph_gap),
            glyph_color: self.glyph_color.overlay(later.glyph_color),
            monospace: self.monospace.overlay(later.monospace),
            menu: self.menu.overlay(later.menu),
        }
    }

    /// Resolve after all layers are composed. Defaults are explicit and do not
    /// carry a previous rendered frame's incidental native widget state.
    pub fn resolve(self, defaults: Style) -> Style {
        Style {
            foreground: self.foreground.resolve(defaults.foreground),
            background: self.background.resolve(defaults.background),
            padding_top: self.padding_top.resolve(defaults.padding_top),
            padding_end: self.padding_end.resolve(defaults.padding_end),
            padding_bottom: self.padding_bottom.resolve(defaults.padding_bottom),
            padding_start: self.padding_start.resolve(defaults.padding_start),
            gap: self.gap.resolve(defaults.gap),
            weight: self.weight.resolve(defaults.weight),
            align: self.align.resolve(defaults.align),
            radius: self.radius.resolve(defaults.radius),
            border: self.border.resolve(defaults.border),
            fill_height: self.fill_height.resolve(defaults.fill_height),
            intrinsic_width: self.intrinsic_width.resolve(defaults.intrinsic_width),
            padding_points: self.padding_points.resolve(defaults.padding_points),
            gap_points: self.gap_points.resolve(defaults.gap_points),
            text_size: self.text_size.resolve(defaults.text_size),
            line_height: self.line_height.resolve(defaults.line_height),
            min_height: self.min_height.resolve(defaults.min_height),
            button_padding: self.button_padding.resolve(defaults.button_padding),
            button_detail: self.button_detail.resolve(defaults.button_detail),
            hover_background: self.hover_background.resolve(defaults.hover_background),
            glyph_size: self.glyph_size.resolve(defaults.glyph_size),
            glyph_gap: self.glyph_gap.resolve(defaults.glyph_gap),
            glyph_color: self.glyph_color.resolve(defaults.glyph_color),
            monospace: self.monospace.resolve(defaults.monospace),
            menu: self.menu.resolve(defaults.menu),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleError {
    InvalidName(String),
    DuplicateName(String),
    UnknownName(String),
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid style name: {name}"),
            Self::DuplicateName(name) => write!(f, "duplicate style name: {name}"),
            Self::UnknownName(name) => write!(f, "unknown style name: {name}"),
        }
    }
}

impl std::error::Error for StyleError {}

/// A validated name registry. It stores data and never executes style code.
#[derive(Clone, Debug)]
pub struct StyleSheet(BTreeMap<String, StylePatch>);

impl StyleSheet {
    pub fn new(
        entries: impl IntoIterator<Item = (String, StylePatch)>,
    ) -> Result<Self, StyleError> {
        let mut result = BTreeMap::new();
        for (name, style) in entries {
            if !crate::valid_id(&name) {
                return Err(StyleError::InvalidName(name));
            }
            if result.insert(name.clone(), style).is_some() {
                return Err(StyleError::DuplicateName(name));
            }
        }
        Ok(Self(result))
    }

    pub fn compose<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<StylePatch, StyleError> {
        let mut result = StylePatch::default();
        for name in names {
            let style = self
                .0
                .get(name)
                .ok_or_else(|| StyleError::UnknownName(name.to_owned()))?;
            result = result.then(*style);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_leaf_order_and_reset_survive_composition() {
        let base = StylePatch::padding(Space::Md).then(StylePatch {
            foreground: Patch::Set(Color::rgb(240, 240, 240)),
            ..StylePatch::default()
        });
        let edge = StylePatch {
            padding_start: Patch::Set(Space::Xs),
            foreground: Patch::Reset,
            ..StylePatch::default()
        };
        let layered = base.then(edge).then(StylePatch::default());
        assert_eq!(layered.foreground, Patch::Reset);
        let result = layered.resolve(Style {
            foreground: Some(Color::rgb(90, 90, 90)),
            ..Style::default()
        });
        assert_eq!(result.foreground, Some(Color::rgb(90, 90, 90)));
        assert_eq!(result.padding_start, Some(Space::Xs));
        assert_eq!(result.padding_end, Some(Space::Md));
        assert_eq!(
            edge.then(base).resolve(Style::default()).padding_start,
            Some(Space::Md)
        );
    }

    #[test]
    fn sheet_rejects_ambiguous_and_missing_names() {
        let style = StylePatch::default();
        assert!(matches!(
            StyleSheet::new([("a".into(), style), ("a".into(), style)]),
            Err(StyleError::DuplicateName(_))
        ));
        assert!(StyleSheet::new([("bad name".into(), style)]).is_err());
        let sheet = StyleSheet::new([("a".into(), style)]).unwrap();
        assert!(matches!(
            sheet.compose(["missing"]),
            Err(StyleError::UnknownName(_))
        ));
    }

    #[test]
    fn caller_order_not_registry_order_controls_composition() {
        let sheet = StyleSheet::new([
            (
                "z".into(),
                StylePatch {
                    weight: Patch::Set(TextWeight::Bold),
                    ..StylePatch::default()
                },
            ),
            (
                "a".into(),
                StylePatch {
                    weight: Patch::Set(TextWeight::Normal),
                    ..StylePatch::default()
                },
            ),
        ])
        .unwrap();
        assert_eq!(
            sheet
                .compose(["z", "a"])
                .unwrap()
                .resolve(Style::default())
                .weight,
            Some(TextWeight::Normal)
        );
        assert_eq!(
            sheet
                .compose(["a", "z"])
                .unwrap()
                .resolve(Style::default())
                .weight,
            Some(TextWeight::Bold)
        );
        assert!(serde_json::from_str::<StylePatch>(r#"{"css":"color:red"}"#).is_err());
    }
}
