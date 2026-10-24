//! The web stylesheet Kiki and Verse import:
//! `apps/verse-ui/src/aethyr/tokens.css`, which `apps/verse-ui/src/theme.css`
//! imports. It carries shadcn's theme variables, Tailwind's `@theme inline`
//! names for them, and the HUD, blur, motion, and spacing variables.
//!
//! Every value comes from the table. A shadcn color that means the same as a
//! Coder Noir role takes the Noir value, so a dark Kiki page matches the
//! desktop and the mobile apps; the rest come from [`product`], [`hud`],
//! [`scale`](crate::scale), and [`typography::geist`]. A view tree never
//! carries a raw color: it names a role and the renderer looks the value up.
//!
//! Regenerate the stylesheet with `cargo run -p oa-tokens -- --write`; the
//! `generated_stylesheet_is_current` test fails when it drifts.

use std::fmt::Write as _;

use crate::scale::{blur, motion, space};
use crate::{Rgba8, hud, noir, product, typography};

/// What a token is for. The stylesheet writes variables in kind order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// shadcn theme colors.
    Color,
    Radius,
    Type,
    /// The HUD layer: glass panels, lines, corner ticks, glow, header.
    Hud,
    Blur,
    Motion,
    Space,
}

/// A token's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Color(Rgba8),
    /// Any other CSS value, written as is.
    Css(&'static str),
}

impl Value {
    /// The value as CSS.
    #[must_use]
    pub fn css(self) -> String {
        match self {
            Value::Color(color) => color.hex(),
            Value::Css(text) => text.to_string(),
        }
    }
}

/// One custom property, with an optional value at phone width
/// (`max-width: hud::PHONE_MAX_WIDTH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    /// The CSS custom property, with its leading `--`.
    pub name: &'static str,
    pub value: Value,
    pub phone: Option<&'static str>,
}

const fn role(name: &'static str, rgb: u32) -> Token {
    color(name, Rgba8::rgb(rgb))
}

const fn color(name: &'static str, value: Rgba8) -> Token {
    Token {
        kind: Kind::Color,
        name,
        value: Value::Color(value),
        phone: None,
    }
}

const fn hud_color(name: &'static str, value: Rgba8) -> Token {
    Token {
        kind: Kind::Hud,
        ..color(name, value)
    }
}

const fn css(kind: Kind, name: &'static str, value: &'static str) -> Token {
    Token {
        kind,
        name,
        value: Value::Css(value),
        phone: None,
    }
}

/// Every token, in stylesheet order.
pub const TOKENS: &[Token] = &[
    // shadcn's theme, on Coder Noir's roles.
    role("--background", noir::CANVAS),
    role("--foreground", noir::CONTENT),
    // Cards float over the live world, so they keep a little translucency.
    color("--card", Rgba8::rgb(noir::SURFACE_RAISED).with_alpha(0xef)),
    role("--popover", noir::SURFACE_RAISED),
    role("--primary", noir::ACCENT_SOLID),
    role("--primary-foreground", noir::ACCENT_ON_SOLID),
    role("--secondary", noir::CONTROL_ON_OVERLAY),
    role("--muted", noir::SURFACE_SUBTLE),
    role("--muted-foreground", noir::CONTENT_SECONDARY),
    color("--accent", Rgba8::rgb(product::HOVER_FILL)),
    role("--border", noir::STROKE_SUBTLE),
    role("--input", noir::STROKE),
    role("--ring", noir::ACCENT),
    role("--destructive", noir::DANGER),
    css(Kind::Radius, "--radius", hud::RADIUS_PANEL),
    css(Kind::Radius, "--hud-radius-control", hud::RADIUS_CONTROL),
    css(Kind::Type, "--font-mono", typography::geist::MONO),
    css(Kind::Type, "--hud-label-size", hud::LABEL_SIZE),
    css(Kind::Type, "--hud-label-tracking", hud::LABEL_TRACKING),
    hud_color("--hud-glass", hud::GLASS),
    hud_color("--hud-line", hud::LINE),
    hud_color("--hud-tick", hud::TICK),
    css(Kind::Hud, "--hud-glow", hud::GLOW),
    css(Kind::Hud, "--hud-tick-size", hud::TICK_SIZE),
    Token {
        kind: Kind::Hud,
        name: "--hud-header",
        value: Value::Css(hud::HEADER),
        phone: Some(hud::HEADER_PHONE),
    },
    css(Kind::Blur, "--hud-blur", blur::PANEL),
    css(Kind::Blur, "--hud-blur-bar", blur::BAR),
    css(Kind::Motion, "--hud-motion", motion::FAST),
    css(Kind::Motion, "--hud-fade", motion::FADE),
    css(Kind::Motion, "--hud-pulse", motion::PULSE),
    css(Kind::Space, "--hud-space-xs", space::XS),
    css(Kind::Space, "--hud-space-sm", space::SM),
    css(Kind::Space, "--hud-space-md", space::MD),
    css(Kind::Space, "--hud-space-lg", space::LG),
    css(Kind::Space, "--hud-space-xl", space::XL),
];

/// Tailwind's theme names for the tokens above (`@theme inline`), so shadcn
/// utilities such as `bg-card` resolve to the same variables. Tailwind owns
/// `--font-sans`, so the sans stack is written here rather than in `:root`.
pub const TAILWIND_THEME: &[(&str, &str)] = &[
    ("--color-background", "var(--background)"),
    ("--color-foreground", "var(--foreground)"),
    ("--color-card", "var(--card)"),
    ("--color-card-foreground", "var(--foreground)"),
    ("--color-popover", "var(--popover)"),
    ("--color-popover-foreground", "var(--foreground)"),
    ("--color-primary", "var(--primary)"),
    ("--color-primary-foreground", "var(--primary-foreground)"),
    ("--color-secondary", "var(--secondary)"),
    ("--color-secondary-foreground", "var(--foreground)"),
    ("--color-muted", "var(--muted)"),
    ("--color-muted-foreground", "var(--muted-foreground)"),
    ("--color-accent", "var(--accent)"),
    ("--color-accent-foreground", "var(--foreground)"),
    ("--color-border", "var(--border)"),
    ("--color-input", "var(--input)"),
    ("--color-ring", "var(--ring)"),
    ("--color-destructive", "var(--destructive)"),
    ("--radius-sm", "calc(var(--radius) - 4px)"),
    ("--radius-md", "calc(var(--radius) - 2px)"),
    ("--radius-lg", "var(--radius)"),
    ("--font-sans", typography::geist::SANS),
];

/// Where the stylesheet lives, relative to the repository root.
pub const STYLESHEET_PATH: &str = "apps/verse-ui/src/aethyr/tokens.css";

/// The stylesheet `theme.css` imports.
#[must_use]
pub fn css_text() -> String {
    let mut out = String::from(
        "/* Generated from crates/oa-tokens; do not edit.\n   \
         Regenerate: cargo run -p oa-tokens -- --write */\n",
    );
    out.push_str("@theme inline {\n");
    for (name, value) in TAILWIND_THEME {
        let _ = writeln!(out, "  {name}: {value};");
    }
    out.push_str("}\n:root {\n");
    for token in TOKENS {
        let _ = writeln!(out, "  {}: {};", token.name, token.value.css());
    }
    out.push_str("}\n");
    let phone: Vec<_> = TOKENS.iter().filter(|t| t.phone.is_some()).collect();
    if !phone.is_empty() {
        let _ = writeln!(
            out,
            "@media (max-width: {}) {{\n  :root {{",
            hud::PHONE_MAX_WIDTH
        );
        for token in phone {
            let _ = writeln!(
                out,
                "    {}: {};",
                token.name,
                token.phone.unwrap_or_default()
            );
        }
        out.push_str("  }\n}\n");
    }
    out
}

/// Look a token up by its CSS name (`--hud-glass`).
#[must_use]
pub fn get(name: &str) -> Option<&'static Token> {
    TOKENS.iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    fn checked_in() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(STYLESHEET_PATH)
    }

    #[test]
    fn generated_stylesheet_is_current() {
        let path = checked_in();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            text == css_text(),
            "{} is stale; run `cargo run -p oa-tokens -- --write`",
            path.display()
        );
    }

    #[test]
    fn names_are_unique_custom_properties() {
        let mut seen = HashSet::new();
        for token in TOKENS {
            assert!(token.name.starts_with("--"), "{}", token.name);
            assert!(seen.insert(token.name), "duplicate {}", token.name);
            let value = token.value.css();
            assert!(!value.contains(';') && !value.contains('}'));
        }
        for (name, _) in TAILWIND_THEME {
            assert!(seen.insert(name), "{name} is both a token and a theme name");
        }
    }

    /// Where a shadcn color means the same as a Coder Noir role, the
    /// stylesheet writes the Noir value (ADR 0012).
    #[test]
    fn shadcn_colors_are_noir_roles() {
        let rgb = |name: &str| match get(name).map(|t| t.value) {
            Some(Value::Color(color)) => color.rgb_u32(),
            other => panic!("{name}: {other:?}"),
        };
        for (name, role) in [
            ("--background", noir::CANVAS),
            ("--foreground", noir::CONTENT),
            ("--card", noir::SURFACE_RAISED),
            ("--popover", noir::SURFACE_RAISED),
            ("--primary", noir::ACCENT_SOLID),
            ("--primary-foreground", noir::ACCENT_ON_SOLID),
            ("--muted-foreground", noir::CONTENT_SECONDARY),
            ("--border", noir::STROKE_SUBTLE),
            ("--ring", noir::ACCENT),
            ("--destructive", noir::DANGER),
        ] {
            assert_eq!(rgb(name), role, "{name}");
        }
    }

    #[test]
    fn colors_round_trip_through_hex() {
        for token in TOKENS {
            if let Value::Color(color) = token.value {
                assert_eq!(Rgba8::from_hex(&color.hex()), Some(color), "{}", token.name);
            }
        }
        assert_eq!(hud::GLASS.hex(), "#0b0c10cc");
        assert_eq!(Rgba8::from_hex("#0a0a0a"), Some(Rgba8::rgb(noir::CANVAS)));
        assert_eq!(Rgba8::from_hex("#12"), None);
        assert_eq!(Rgba8::from_hex("#ééé"), None);
    }

    #[test]
    fn phone_overrides_are_written_inside_the_breakpoint() {
        let css = css_text();
        let media = css.find("@media (max-width: 760px)").expect("phone block");
        assert!(css[media..].contains("--hud-header: 56px;"));
        assert_eq!(get("--hud-header").and_then(|t| t.phone), Some("56px"));
    }
}
