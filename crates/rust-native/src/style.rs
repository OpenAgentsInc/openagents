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
