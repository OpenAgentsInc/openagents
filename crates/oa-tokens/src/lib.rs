//! The OpenAgents design token table as data with no dependencies (an
//! optional `serde` feature serializes [`ThemeChoice`]): one source for
//! the web components (`openagents-ui`, which generates the token
//! stylesheets from it) and the native GUI themes (`coder-ui`, the desktop
//! and mobile apps). See `docs/web/apps-sdk-ui-adoption-plan.md`
//! (decision 5 and phase 4).
//!
//! - [`apps_sdk`] holds Apps SDK UI's three token layers as written upstream.
//!   Their light values are **Coder Light**, unchanged.
//! - [`noir`] holds **Coder Noir**: every Noir value, and the roles whose
//!   dark value it replaces. Every other dark value is Apps SDK UI's.
//! - [`product`] holds our own roles in the same naming scheme.
//! - [`typography`] holds the font stacks and type scale in points.
//! - [`palette`] resolves the table into the native role palettes, Coder
//!   Light and Coder Noir, that GUI surfaces paint with.
//! - [`ThemeChoice`] is the person's choice (follow the system, Light, or
//!   Dark) and how it resolves to a [`Scheme`].
//!
//! Terminal UIs are Coder Noir only: nothing here offers them a light
//! scheme.

pub mod apps_sdk;
pub mod noir;
pub mod palette;
pub mod product;
pub mod typography;

use std::collections::HashMap;

pub use palette::{Palette, Rgba8};

/// A titled group of `(name, value)` custom properties.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub title: &'static str,
    pub tokens: &'static [(&'static str, &'static str)],
}

/// The two themes. `Light` is Coder Light, `Dark` is Coder Noir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scheme {
    Light,
    #[default]
    Dark,
}

/// The person's theme choice. `System` follows the operating system's light
/// or dark appearance; `Light` and `Dark` override it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    /// The scheme to paint with when the system's appearance is `system`
    /// (`None` when the platform does not say, which reads as dark).
    #[must_use]
    pub const fn resolve(self, system: Option<Scheme>) -> Scheme {
        match self {
            ThemeChoice::Light => Scheme::Light,
            ThemeChoice::Dark => Scheme::Dark,
            ThemeChoice::System => match system {
                Some(scheme) => scheme,
                None => Scheme::Dark,
            },
        }
    }

    /// The name a person reads.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
        }
    }

    /// The stored word: `system`, `light`, or `dark` (the web's
    /// `data-theme` values, with `system` for none).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    /// Reads a stored word; anything else is `None`.
    #[must_use]
    pub fn parse(word: &str) -> Option<ThemeChoice> {
        ThemeChoice::ALL
            .into_iter()
            .find(|choice| choice.as_str() == word)
    }
}

/// Every section of the table, in stylesheet order: primitive, semantic,
/// component, then the product roles.
#[must_use]
pub fn sections() -> Vec<Section> {
    let mut all = apps_sdk::PRIMITIVE.to_vec();
    all.extend_from_slice(apps_sdk::SEMANTIC);
    all.extend_from_slice(apps_sdk::COMPONENTS);
    all.extend_from_slice(product::COMPONENTS);
    all
}

/// The token's value with Coder Noir's dark value applied, still in source
/// form (`alpha()`/`spacing()` not yet lowered).
#[must_use]
pub fn themed_value(name: &str, value: &str) -> String {
    let Some((_, noir)) = noir::OVERRIDES.iter().find(|(token, _)| *token == name) else {
        return value.to_string();
    };
    let light = match split_function(value, "light-dark") {
        Some(args) if args.len() == 2 => args[0].clone(),
        _ => value.to_string(),
    };
    format!("light-dark({light}, #{noir:06x})")
}

/// Every token name and its (Noir-applied, source-form) value, across all
/// three layers and the product roles.
#[must_use]
pub fn all_tokens() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for section in sections() {
        for (name, value) in section.tokens {
            out.push((*name, themed_value(name, value)));
        }
    }
    // Theme scalars defined outside the main block (light values).
    for (name, value) in [
        ("--shadow-hairline-width", "1px"),
        ("--shadow-hairline-color", "rgb(0 0 0 / 8%)"),
        ("--shadow-alpha-100", "0.08"),
        ("--shadow-alpha-200", "0.08"),
        ("--shadow-alpha-300", "0.1"),
        ("--shadow-alpha-400", "0.12"),
    ] {
        out.push((name, value.to_string()));
    }
    out
}

/// The index of the `)` matching the `(` at byte `open`.
#[must_use]
pub fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split top-level comma-separated arguments.
#[must_use]
pub fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for ch in text.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    args.push(current.trim().to_string());
    args
}

/// If `value` is exactly `name(...)`, its top-level arguments.
#[must_use]
pub fn split_function(value: &str, name: &str) -> Option<Vec<String>> {
    let value = value.trim();
    let inner = value.strip_prefix(name)?.strip_prefix('(')?;
    if matching_paren(value, name.len())? != value.len() - 1 {
        return None;
    }
    Some(split_args(&inner[..inner.len() - 1]))
}

/// An sRGB color with alpha, channels in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    #[must_use]
    pub fn opaque(rgb: u32) -> Self {
        Rgba {
            r: f64::from((rgb >> 16) & 0xff) / 255.0,
            g: f64::from((rgb >> 8) & 0xff) / 255.0,
            b: f64::from(rgb & 0xff) / 255.0,
            a: 1.0,
        }
    }

    /// This color drawn over `below`.
    #[must_use]
    pub fn over(self, below: Rgba) -> Rgba {
        let a = self.a + below.a * (1.0 - self.a);
        if a == 0.0 {
            return Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
        }
        let mix = |top: f64, bottom: f64| (top * self.a + bottom * below.a * (1.0 - self.a)) / a;
        Rgba {
            r: mix(self.r, below.r),
            g: mix(self.g, below.g),
            b: mix(self.b, below.b),
            a,
        }
    }

    /// WCAG relative luminance (alpha ignored).
    #[must_use]
    pub fn luminance(self) -> f64 {
        let channel = |value: f64| {
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG contrast ratio between two opaque colors.
    #[must_use]
    pub fn contrast(self, other: Rgba) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (light, dark) = if a > b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    /// The nearest 8-bit color.
    #[must_use]
    pub fn to_8bit(self) -> Rgba8 {
        let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgba8 {
            r: channel(self.r),
            g: channel(self.g),
            b: channel(self.b),
            a: channel(self.a),
        }
    }
}

/// Resolves token colors per theme, the way a browser would.
pub struct Resolver {
    tokens: HashMap<&'static str, String>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    /// The whole table, Noir applied.
    #[must_use]
    pub fn new() -> Self {
        Resolver {
            tokens: all_tokens().into_iter().collect(),
        }
    }

    /// A resolver over another set of `(name, value)` tokens.
    #[must_use]
    pub fn from_tokens(tokens: HashMap<&'static str, String>) -> Self {
        Resolver { tokens }
    }

    /// Whether a token is defined.
    #[must_use]
    pub fn defines(&self, name: &str) -> bool {
        self.tokens.contains_key(name)
    }

    /// The color a token resolves to in `scheme`, if it is a color.
    #[must_use]
    pub fn color(&self, name: &str, scheme: Scheme) -> Option<Rgba> {
        self.eval(self.tokens.get(name)?, scheme)
    }

    /// The color a source-form value resolves to in `scheme`.
    #[must_use]
    pub fn eval(&self, expr: &str, scheme: Scheme) -> Option<Rgba> {
        self.eval_at(expr, scheme, 0)
    }

    fn eval_at(&self, expr: &str, scheme: Scheme, depth: usize) -> Option<Rgba> {
        if depth > 32 {
            return None;
        }
        let expr = expr.trim();
        if let Some(hex) = expr.strip_prefix('#') {
            let hex = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect()
            } else {
                hex.to_string()
            };
            return u32::from_str_radix(&hex, 16).ok().map(Rgba::opaque);
        }
        if expr == "transparent" {
            return Some(Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            });
        }
        if let Some(args) = split_function(expr, "light-dark") {
            let pick = if scheme == Scheme::Light { 0 } else { 1 };
            return self.eval_at(args.get(pick)?, scheme, depth + 1);
        }
        if let Some(args) = split_function(expr, "var") {
            let value = self.tokens.get(args[0].as_str())?;
            return self.eval_at(value, scheme, depth + 1);
        }
        if let Some(args) = split_function(expr, "alpha") {
            let base = self.eval_at(&args[0], scheme, depth + 1)?;
            let amount = args
                .get(1)?
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .ok()?
                / 100.0;
            return Some(Rgba {
                a: base.a * amount,
                ..base
            });
        }
        for name in ["rgb", "rgba"] {
            if let Some(args) = split_function(expr, name) {
                let inner = args.join(" ");
                let (channels, alpha) = match inner.split_once('/') {
                    Some((c, a)) => (c.to_string(), Some(a.trim().to_string())),
                    None => (inner.clone(), None),
                };
                let parts: Vec<f64> = channels
                    .split_whitespace()
                    .map(|p| p.parse::<f64>().ok())
                    .collect::<Option<_>>()?;
                if parts.len() != 3 {
                    return None;
                }
                let a = match alpha {
                    Some(a) if a.ends_with('%') => {
                        a.trim_end_matches('%').parse::<f64>().ok()? / 100.0
                    }
                    Some(a) => a.parse::<f64>().ok()?,
                    None => 1.0,
                };
                return Some(Rgba {
                    r: parts[0] / 255.0,
                    g: parts[1] / 255.0,
                    b: parts[2] / 255.0,
                    a,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
