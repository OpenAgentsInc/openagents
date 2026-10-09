//! The type system for GUI surfaces that do not run CSS: the web's font
//! families and type scale, resolved to points (1rem = 16pt).
//!
//! [`SANS`] and [`MONO`] are the `--font-sans` and `--font-mono` stacks,
//! split into family names in order. [`TEXT`] and [`HEADING`] are the
//! `--font-text-*` and `--font-heading-*` sizes and line heights. Tests
//! hold every value to the token table, so a native app and the web cannot
//! drift apart.

use crate::apps_sdk;

/// One step of the type scale, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The token's name without its prefix, such as `sm`.
    pub name: &'static str,
    pub size: f32,
    pub line_height: f32,
}

const fn step(name: &'static str, size: f32, line_height: f32) -> Step {
    Step {
        name,
        size,
        line_height,
    }
}

/// `--font-text-{name}-size` and `-line-height`, smallest first.
pub const TEXT: [Step; 6] = [
    step("3xs", 8.0, 12.0),
    step("2xs", 10.0, 14.0),
    step("xs", 12.0, 18.0),
    step("sm", 14.0, 20.0),
    step("md", 16.0, 24.0),
    step("lg", 18.0, 29.0),
];

/// `--font-heading-{name}-size` and `-line-height`, smallest first. Every
/// heading is semibold (`--font-heading-*-weight`).
pub const HEADING: [Step; 9] = [
    step("xs", 16.0, 24.0),
    step("sm", 18.0, 26.0),
    step("md", 20.0, 26.0),
    step("lg", 24.0, 28.0),
    step("xl", 32.0, 38.0),
    step("2xl", 36.0, 42.0),
    step("3xl", 48.0, 48.0),
    step("4xl", 60.0, 60.0),
    step("5xl", 72.0, 72.0),
];

/// The text sizes, by name.
pub mod text {
    use super::{Step, TEXT};
    pub const XS: Step = TEXT[2];
    pub const SM: Step = TEXT[3];
    pub const MD: Step = TEXT[4];
    pub const LG: Step = TEXT[5];
}

/// The heading sizes, by name.
pub mod heading {
    use super::{HEADING, Step};
    pub const XS: Step = HEADING[0];
    pub const SM: Step = HEADING[1];
    pub const MD: Step = HEADING[2];
    pub const LG: Step = HEADING[3];
    pub const XL: Step = HEADING[4];
}

/// The weights, `--font-weight-*`.
pub mod weight {
    pub const NORMAL: u16 = 400;
    pub const MEDIUM: u16 = 500;
    pub const SEMIBOLD: u16 = 600;
    pub const BOLD: u16 = 700;
}

/// Whether `size` points is a step of the type scale, text or heading.
#[must_use]
pub fn on_scale(size: f32) -> bool {
    TEXT.iter()
        .chain(HEADING.iter())
        .any(|step| (step.size - size).abs() < 0.01)
}

/// A font-family stack's value, split into family names in order, quotes
/// removed.
#[must_use]
pub fn split_stack(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|family| family.trim().trim_matches('"').to_string())
        .filter(|family| !family.is_empty())
        .collect()
}

fn stack(name: &str) -> Vec<String> {
    apps_sdk::SEMANTIC
        .iter()
        .flat_map(|section| section.tokens.iter())
        .find(|(token, _)| *token == name)
        .map(|(_, value)| split_stack(value))
        .unwrap_or_default()
}

/// The `--font-sans` stack: the family every piece of text draws in.
#[must_use]
pub fn sans() -> Vec<String> {
    stack("--font-sans")
}

/// The `--font-mono` stack: the family code draws in.
#[must_use]
pub fn mono() -> Vec<String> {
    stack("--font-mono")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(name: &str) -> String {
        apps_sdk::SEMANTIC
            .iter()
            .flat_map(|section| section.tokens.iter())
            .find(|(token, _)| *token == name)
            .map(|(_, value)| (*value).to_string())
            .unwrap_or_else(|| panic!("no token {name}"))
    }

    fn points(value: &str) -> f32 {
        let rem: f32 = value
            .strip_suffix("rem")
            .unwrap_or_else(|| panic!("{value} is not in rem"))
            .parse()
            .expect("a number");
        rem * 16.0
    }

    #[test]
    fn the_scale_is_the_token_table() {
        for (prefix, steps) in [("text", &TEXT[..]), ("heading", &HEADING[..])] {
            for step in steps {
                let size = points(&token(&format!("--font-{prefix}-{}-size", step.name)));
                let line = points(&token(&format!(
                    "--font-{prefix}-{}-line-height",
                    step.name
                )));
                assert_eq!(step.size, size, "{prefix} {}", step.name);
                assert_eq!(step.line_height, line, "{prefix} {}", step.name);
            }
        }
    }

    #[test]
    fn the_weights_are_the_token_table() {
        for (name, value) in [
            ("normal", weight::NORMAL),
            ("medium", weight::MEDIUM),
            ("semibold", weight::SEMIBOLD),
            ("bold", weight::BOLD),
        ] {
            assert_eq!(token(&format!("--font-weight-{name}")), value.to_string());
        }
    }

    #[test]
    fn the_stacks_start_with_the_system_faces() {
        assert_eq!(sans()[0], "ui-sans-serif");
        assert!(sans().contains(&"Segoe UI".to_string()));
        assert_eq!(mono()[0], "ui-monospace");
        assert!(mono().contains(&"SF Mono".to_string()));
        assert!(on_scale(14.0) && on_scale(24.0) && !on_scale(13.0));
    }
}
