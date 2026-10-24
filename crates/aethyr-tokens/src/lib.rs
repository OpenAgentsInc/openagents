//! Aethyr design tokens, defined once (ADR 0006).
//!
//! Every value here is the single source for the website's CSS variables
//! (`css()` writes `apps/verse-ui/src/aethyr/tokens.css`, which
//! `apps/verse-ui/src/theme.css` imports) and for native and GPU renderers,
//! which read the constants directly. A view tree never carries a raw color;
//! it names a token role and the renderer looks the value up here.
//!
//! Regenerate the stylesheet with `cargo run -p aethyr-tokens -- --write`;
//! the `generated_stylesheet_is_current` test fails when it drifts.

use std::fmt::Write as _;

/// What a token is for. The CSS writer groups variables by kind.
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

/// One design token: a CSS custom property and its value, with an optional
/// value at phone width (`max-width: PHONE_MAX_WIDTH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    /// The CSS custom property, with its leading `--`.
    pub name: &'static str,
    pub value: &'static str,
    pub phone: Option<&'static str>,
}

macro_rules! tokens {
    ($(
        $kind:ident {
            $( $(#[$meta:meta])* $konst:ident $css:literal = $value:literal $(, phone $phone:literal)? ; )*
        }
    )*) => {
        $( $( $(#[$meta])* pub const $konst: &str = $value; )* )*
        /// Every token, in stylesheet order.
        pub const TOKENS: &[Token] = &[
            $( $( Token { kind: Kind::$kind, name: $css, value: $value, phone: tokens!(@phone $($phone)?) }, )* )*
        ];
    };
    (@phone) => { None };
    (@phone $phone:literal) => { Some($phone) };
}

tokens! {
    Color {
        /// Page background.
        BACKGROUND "--background" = "#000000";
        /// Default text.
        FOREGROUND "--foreground" = "#fafafa";
        CARD "--card" = "#18181bef";
        POPOVER "--popover" = "#18181b";
        /// Primary actions.
        PRIMARY "--primary" = "#fafafa";
        PRIMARY_FOREGROUND "--primary-foreground" = "#18181b";
        SECONDARY "--secondary" = "#27272a";
        MUTED "--muted" = "#18181b";
        MUTED_FOREGROUND "--muted-foreground" = "#a1a1aa";
        ACCENT "--accent" = "#3f3f46";
        BORDER "--border" = "#ffffff1f";
        INPUT "--input" = "#ffffff28";
        /// Focus outline.
        RING "--ring" = "#d4d4d8";
        DESTRUCTIVE "--destructive" = "#f87171";
    }
    Radius {
        /// Panels and cards.
        RADIUS "--radius" = "0.75rem";
        /// Buttons, toasts, and other controls.
        RADIUS_CONTROL "--hud-radius-control" = "8px";
    }
    Type {
        /// Labels, indexes, and other HUD readouts.
        FONT_MONO "--font-mono" = "\"Geist Mono\", ui-monospace, SFMono-Regular, Menlo, monospace";
        LABEL_SIZE "--hud-label-size" = "11px";
        LABEL_TRACKING "--hud-label-tracking" = "0.18em";
    }
    Hud {
        /// The glass behind panels and toasts.
        HUD_GLASS "--hud-glass" = "#0b0c10cc";
        /// Panel and button outlines.
        HUD_LINE "--hud-line" = "#ffffff26";
        /// Panel corner ticks.
        HUD_TICK "--hud-tick" = "#ffffff73";
        HUD_GLOW "--hud-glow" = "0 0 0 1px #ffffff14, 0 18px 60px #00000080";
        HUD_TICK_SIZE "--hud-tick-size" = "14px";
        /// Height of the top bar.
        HUD_HEADER "--hud-header" = "64px", phone "56px";
    }
    Blur {
        /// Behind panels and cards.
        BLUR_PANEL "--hud-blur" = "18px";
        /// Behind the top bar.
        BLUR_BAR "--hud-blur-bar" = "14px";
    }
    Motion {
        /// Hover and press transitions.
        MOTION_FAST "--hud-motion" = "0.15s";
        /// A still image fading out when the live view is ready.
        MOTION_FADE "--hud-fade" = "0.8s";
        /// One beat of the signal dot.
        MOTION_PULSE "--hud-pulse" = "2.4s";
    }
    Space {
        SPACE_XS "--hud-space-xs" = "4px";
        SPACE_SM "--hud-space-sm" = "8px";
        SPACE_MD "--hud-space-md" = "12px";
        SPACE_LG "--hud-space-lg" = "16px";
        SPACE_XL "--hud-space-xl" = "24px";
    }
}

/// The sans-serif family. Tailwind owns the `--font-sans` name, so it is
/// written into the `@theme` block rather than `:root`. Inter is
/// self-hosted (`theme.css`); the named system faces follow so a computer
/// whose default font is monospace still draws a proportional sans.
pub const FONT_SANS: &str = "\"Geist\", \"Inter\", ui-sans-serif, system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", \"Helvetica Neue\", Arial, sans-serif";

/// Widths up to this are phones: panels become sheets, secondary controls hide.
pub const PHONE_MAX_WIDTH: &str = "760px";

/// Tailwind's theme names for the tokens above (`@theme inline`), so shadcn
/// utilities such as `bg-card` resolve to the same variables.
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
    ("--font-sans", FONT_SANS),
];

/// Where the generated stylesheet lives, relative to the repository root.
pub const STYLESHEET_PATH: &str = "apps/verse-ui/src/aethyr/tokens.css";

/// The token stylesheet `theme.css` imports.
pub fn css() -> String {
    let mut out = String::from(
        "/* Generated from crates/aethyr-tokens; do not edit.\n   \
         Regenerate: cargo run -p aethyr-tokens -- --write */\n",
    );
    out.push_str("@theme inline {\n");
    for (name, value) in TAILWIND_THEME {
        let _ = writeln!(out, "  {name}: {value};");
    }
    out.push_str("}\n:root {\n");
    for token in TOKENS {
        let _ = writeln!(out, "  {}: {};", token.name, token.value);
    }
    out.push_str("}\n");
    let phone: Vec<_> = TOKENS.iter().filter(|t| t.phone.is_some()).collect();
    if !phone.is_empty() {
        let _ = writeln!(out, "@media (max-width: {PHONE_MAX_WIDTH}) {{\n  :root {{");
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
pub fn get(name: &str) -> Option<&'static Token> {
    TOKENS.iter().find(|t| t.name == name)
}

/// A `#rrggbb` or `#rrggbbaa` color as straight (not premultiplied) RGBA
/// bytes, for renderers that do not read CSS.
pub fn rgba(hex: &str) -> Option<[u8; 4]> {
    let digits = hex.strip_prefix('#')?;
    if !digits.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok();
    match digits.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
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
            text == css(),
            "{} is stale; run `cargo run -p aethyr-tokens -- --write`",
            path.display()
        );
    }

    #[test]
    fn names_are_unique_custom_properties() {
        let mut seen = HashSet::new();
        for token in TOKENS {
            assert!(token.name.starts_with("--"), "{}", token.name);
            assert!(seen.insert(token.name), "duplicate {}", token.name);
            assert!(!token.value.contains(';') && !token.value.contains('}'));
        }
        for (name, _) in TAILWIND_THEME {
            assert!(seen.insert(name), "{name} is both a token and a theme name");
        }
    }

    #[test]
    fn every_color_parses() {
        for token in TOKENS.iter().filter(|t| t.kind == Kind::Color) {
            assert!(rgba(token.value).is_some(), "{}", token.name);
        }
        assert_eq!(rgba(HUD_GLASS), Some([0x0b, 0x0c, 0x10, 0xcc]));
        assert_eq!(rgba(BACKGROUND), Some([0, 0, 0, 255]));
        assert_eq!(rgba("#12"), None);
        assert_eq!(rgba("#ééé"), None);
    }

    #[test]
    fn phone_overrides_are_written_inside_the_breakpoint() {
        let css = css();
        let media = css.find("@media (max-width: 760px)").expect("phone block");
        assert!(css[media..].contains("--hud-header: 56px;"));
        assert_eq!(get("--hud-header").and_then(|t| t.phone), Some("56px"));
    }
}
