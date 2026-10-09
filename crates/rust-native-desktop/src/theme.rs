//! The adapter's defaults and the OpenAgents theme tokens
//! ([#10022](https://github.com/OpenAgentsInc/openagents/issues/10022)).
//!
//! Rust Native's core has no palette; the application supplies one. A
//! node's own `style.foreground` and `style.background` override these.
//!
//! **Light or dark, chosen by the app.** The defaults are dark
//! ([`APPEARANCE`]). An app may paint Coder Light instead (#11028): the
//! window reads the system's appearance in one place and hands it to
//! [`crate::App::system_appearance`], the app resolves its scheme there
//! (the person's choice, or the system's), and the window asks for the
//! returned [`Theme::appearance`] for its title bar and controls. Nothing
//! that lays out or paints reads the system's appearance itself.
//!
//! The [`Theme`] is the single source for the token groups:
//!
//! | Group | Tokens |
//! | --- | --- |
//! | Color | `background`, `text`, `muted`, `rule`, `button`, `button_text`, `link`, `focus`, and the [`Roles`] a transcript's [`ColorRole`]s map to; the [`ladder`] they are drawn from |
//! | Radius | `card_radius`, `button_radius`, and [`Radii`] |
//! | Border | [`Borders`] |
//! | Shadow | [`Shadows`] |
//! | Opacity | [`Opacities`] |
//! | Motion | [`Motion`]: durations and an easing, zero for decoration under reduced motion ([`motion`]) |

use rust_native::layout::display::ColorRole;
use rust_native::style::{Color, Space};
use std::time::Duration;

/// The OpenAgents white ladder: white, white 75, white 50, and white 25 on
/// neutral near-black, the same values as the website and CoderOS
/// (`os/modules/coderos/desktop.nix`).
pub mod ladder {
    use rust_native::style::Color;

    /// White: the brightest text and marks.
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// White 75: a lit control (`c8c8c8`).
    pub const WHITE_75: Color = Color::rgb(200, 200, 200);
    /// White 50: quiet text and dim marks (`8a8a8a`).
    pub const WHITE_50: Color = Color::rgb(138, 138, 138);
    /// White 25: the dimmest marks (`4a4a4a`).
    pub const WHITE_25: Color = Color::rgb(74, 74, 74);
    /// The near-black ground (`0a0a0a`).
    pub const GROUND: Color = Color::rgb(10, 10, 10);
    /// A raised surface on the ground (`1a1a1a`).
    pub const RAISED: Color = Color::rgb(26, 26, 26);
}

/// The app's appearance: the scheme it paints and the one its window asks
/// the system for (title bar, traffic lights, scroll bars).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

/// The default appearance: a theme that does not choose is dark, whatever
/// the system's own setting.
pub const APPEARANCE: Appearance = Appearance::Dark;

/// The bundled vector control artwork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconSet {
    #[default]
    Standard,
    /// Solar Icons by 480 Design (CC BY 4.0).
    Solar,
}

/// The colors a transcript's [`ColorRole`]s are painted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roles {
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub link: Color,
    pub bubble: Color,
    pub surface: Color,
    pub raised: Color,
    pub border: Color,
    pub inline_code: Color,
}

impl Roles {
    /// The desktop transcript's roles, for rows that name a role rather
    /// than their own color.
    pub const DESKTOP: Roles = Roles {
        primary: Color::rgb(230, 232, 235),
        secondary: Color::rgb(150, 155, 163),
        tertiary: Color::rgb(103, 111, 122),
        link: Color::rgb(158, 201, 242),
        bubble: Color::rgb(35, 40, 48),
        surface: Color::rgb(20, 23, 28),
        raised: Color::rgb(38, 43, 51),
        border: Color::rgb(61, 68, 78),
        inline_code: Color::rgb(41, 46, 55),
    };

    /// The color `role` is painted in.
    pub const fn color(&self, role: ColorRole) -> Color {
        match role {
            ColorRole::Primary => self.primary,
            ColorRole::Secondary => self.secondary,
            ColorRole::Tertiary => self.tertiary,
            ColorRole::Link => self.link,
            ColorRole::Bubble => self.bubble,
            ColorRole::Surface => self.surface,
            ColorRole::Raised => self.raised,
            ColorRole::Border => self.border,
            ColorRole::InlineCode => self.inline_code,
        }
    }
}

/// Corner radii besides a card's and a button's, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// A checkbox's corners.
    pub check: f32,
}

/// Line widths, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Borders {
    /// A card's outline.
    pub hairline: f32,
    /// An empty checkbox's outline.
    pub control: f32,
    /// The keyboard focus ring.
    pub focus: f32,
    /// How far the focus ring stands outside its control.
    pub focus_offset: f32,
}

/// One drop shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub color: Color,
}

impl Shadow {
    /// No shadow.
    pub const NONE: Shadow = Shadow {
        x: 0.0,
        y: 0.0,
        blur: 0.0,
        color: Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        },
    };
}

/// Elevation. The dark theme is flat: surfaces separate by fill and
/// hairline, as on the website, so every level is [`Shadow::NONE`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadows {
    /// A card or a panel on the window's field.
    pub raised: Shadow,
    /// A menu or a dialog over the views.
    pub overlay: Shadow,
}

/// How far a control's colors mix toward another, 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacities {
    /// A disabled glyph, toward the field.
    pub disabled: f32,
    /// A disabled control's fill, toward the field.
    pub disabled_fill: f32,
    /// A hovered or pressed transparent control, the field toward the text.
    pub hover_tint: f32,
    /// A hovered or pressed filled control, toward the field.
    pub pressed_tint: f32,
    /// A pressed checked checkbox, toward the field.
    pub pressed_check: f32,
    /// A hovered empty checkbox's outline, toward the text.
    pub hover_edge: f32,
}

/// Whether an animation carries meaning or only decorates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Need {
    /// Shows something the person needs: progress, a caret. Kept under
    /// reduced motion.
    Essential,
    /// Decoration: a transition, the Grid's moving camera. Stopped under
    /// reduced motion.
    Decorative,
}

/// How long a transition takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Speed {
    /// Hover and press feedback.
    Fast,
    /// A panel, a menu, a disclosure.
    Standard,
    /// A large surface entering or leaving.
    Slow,
}

/// A timing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    /// `cubic-bezier(x1, y1, x2, y2)`, as in CSS.
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// The website's default: `cubic-bezier(0.4, 0, 0.2, 1)`.
    pub const STANDARD: Easing = Easing::CubicBezier(0.4, 0.0, 0.2, 1.0);

    /// The curve's progress at `t` of its duration, both 0 to 1.
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::CubicBezier(..) if t <= 0.0 || t >= 1.0 => t,
            Easing::CubicBezier(x1, y1, x2, y2) => {
                let bezier = |a: f32, b: f32, s: f32| {
                    let r = 1.0 - s;
                    3.0 * r * r * s * a + 3.0 * r * s * s * b + s * s * s
                };
                // Solve x(s) = t by bisection; x is monotonic for x1, x2 in 0..=1.
                let (mut low, mut high) = (0.0f32, 1.0f32);
                for _ in 0..32 {
                    let mid = (low + high) / 2.0;
                    if bezier(x1, x2, mid) < t {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                bezier(y1, y2, (low + high) / 2.0)
            }
        }
    }
}

/// Motion tokens: durations and an easing, and whether motion is reduced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Whether the system or the person asked for less motion
    /// ([`motion::reduced`]).
    pub reduced: bool,
    pub fast: Duration,
    pub standard: Duration,
    pub slow: Duration,
    pub easing: Easing,
}

impl Motion {
    /// The tokens, with `reduced` as given.
    pub const fn new(reduced: bool) -> Motion {
        Motion {
            reduced,
            fast: Duration::from_millis(150),
            standard: Duration::from_millis(200),
            slow: Duration::from_millis(300),
            easing: Easing::STANDARD,
        }
    }

    /// The tokens under the current setting ([`motion::reduced`]).
    pub fn current() -> Motion {
        Motion::new(motion::reduced())
    }

    /// How long an animation of `speed` runs: zero for decoration under
    /// reduced motion, so it jumps to its end.
    pub fn duration(&self, speed: Speed, need: Need) -> Duration {
        if !self.animates(need) {
            return Duration::ZERO;
        }
        match speed {
            Speed::Fast => self.fast,
            Speed::Standard => self.standard,
            Speed::Slow => self.slow,
        }
    }

    /// Whether an animation of this `need` runs at all.
    pub fn animates(&self, need: Need) -> bool {
        !(self.reduced && need == Need::Decorative)
    }
}

/// The reduced-motion setting, shared by the whole process: the system's
/// setting, which the app reads from the platform (macOS "Reduce motion",
/// the desktop portal or GNOME's animations switch on Linux, Windows'
/// "Show animations"), and the person's own "Reduce motion" switch in
/// Settings (#10021), which [`follow`] shares. Motion is reduced when
/// either asks.
pub mod motion {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    static SYSTEM: AtomicBool = AtomicBool::new(false);
    static PERSON: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

    /// Records the system's setting, as last read from the platform.
    pub fn set_system(reduced: bool) {
        SYSTEM.store(reduced, Ordering::Relaxed);
    }

    /// The system's setting, as last recorded.
    pub fn system() -> bool {
        SYSTEM.load(Ordering::Relaxed)
    }

    /// Follows the person's switch: set, motion is reduced whatever the
    /// system says; clear, the system's setting decides.
    pub fn follow(switch: Arc<AtomicBool>) {
        *PERSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(switch);
    }

    /// Whether the person's switch is set.
    pub fn person() -> bool {
        PERSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_some_and(|switch| switch.load(Ordering::Relaxed))
    }

    /// Whether motion is reduced when the system says `system` and the
    /// person's switch says `person`.
    pub const fn resolve(system: bool, person: bool) -> bool {
        system || person
    }

    /// Whether motion is reduced now.
    pub fn reduced() -> bool {
        resolve(system(), person())
    }

    /// Reads the `reduced-motion` key of the desktop portal's
    /// `org.freedesktop.appearance` settings as `gdbus` prints it, such as
    /// `(<<uint32 1>>,)`: `Some(true)` for reduced, `Some(false)` for no
    /// preference, `None` when the reply names neither.
    pub fn parse_portal_reply(reply: &str) -> Option<bool> {
        let value = reply.split("uint32").nth(1)?;
        let digits: String = value
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        match digits.as_str() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }
}

/// The colors and sizes a view is painted with, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The scheme the colors below are, and the one the window asks the
    /// system for. [`APPEARANCE`] (dark) unless the app chooses light.
    pub appearance: Appearance,
    pub icons: IconSet,
    /// The bundled font pair used by semantic controls.
    pub font_family: rust_native::layout::display::FontFamily,
    /// The window's field.
    pub background: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text: a `status` text role, a disabled control.
    pub muted: Color,
    /// Rules between list rows and the outline of an empty checkbox.
    pub rule: Color,
    /// A button's fill, and a checked checkbox's.
    pub button: Color,
    /// A button's label, and a checked checkbox's mark.
    pub button_text: Color,
    /// A button drawn as a link (a transparent `style.background`).
    pub link: Color,
    /// The keyboard focus ring.
    pub focus: Color,
    /// The colors transcript roles map to.
    pub roles: Roles,
    /// A card: a stack with a `style.background`.
    pub card_radius: f32,
    /// A button's corner radius.
    pub button_radius: f32,
    /// The other corner radii.
    pub radius: Radii,
    /// Line widths.
    pub border: Borders,
    /// Elevation.
    pub shadow: Shadows,
    /// Mixes for hover, press, and disabled states.
    pub opacity: Opacities,
    /// Durations and easing.
    pub motion: Motion,
    /// Circular control diameter.
    pub icon_size: f32,
    /// Body text.
    pub body: f32,
    /// A heading.
    pub heading: f32,
    /// A status line.
    pub status: f32,
    /// Code and terminal text.
    pub code: f32,
    /// The widest the content column grows.
    pub column: f32,
    /// The space around the column.
    pub margin: f32,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            appearance: APPEARANCE,
            icons: IconSet::Standard,
            font_family: Default::default(),
            background: Color::rgb(0, 0, 0),
            text: Color::rgb(245, 245, 245),
            muted: Color::rgb(150, 150, 150),
            rule: Color::rgb(58, 58, 58),
            button: Color::rgb(245, 245, 245),
            button_text: Color::rgb(0, 0, 0),
            link: Color::rgb(170, 200, 255),
            focus: Color::rgb(120, 170, 255),
            roles: Roles::DESKTOP,
            card_radius: 12.0,
            button_radius: 8.0,
            radius: Radii { check: 4.0 },
            border: Borders {
                hairline: 1.0,
                control: 1.5,
                focus: 2.0,
                focus_offset: 3.0,
            },
            shadow: Shadows {
                raised: Shadow::NONE,
                overlay: Shadow::NONE,
            },
            opacity: Opacities {
                disabled: 0.5,
                disabled_fill: 0.7,
                hover_tint: 0.08,
                pressed_tint: 0.15,
                pressed_check: 0.25,
                hover_edge: 0.3,
            },
            motion: Motion::current(),
            icon_size: 32.0,
            body: 15.0,
            heading: 22.0,
            status: 13.0,
            code: 13.0,
            column: 520.0,
            margin: 28.0,
        }
    }
}

impl Theme {
    /// The OpenAgents apps' theme: the desktop shell and the deck paint
    /// with it, so a change here changes both. Near-black, with white and
    /// gradations of white; the same values as the transcript palette.
    pub fn openagents() -> Theme {
        Theme {
            font_family: rust_native::layout::display::FontFamily::PaperMono,
            background: Color::rgb(9, 11, 14),
            text: Color::rgb(230, 232, 235),
            muted: Color::rgb(150, 155, 163),
            rule: Color::rgb(43, 47, 53),
            focus: Color::rgb(184, 207, 231),
            button_radius: 7.0,
            body: 13.0,
            heading: 26.0,
            status: 11.0,
            column: 620.0,
            ..Theme::default()
        }
    }

    /// Every token as one line of text each, colors as `#rrggbbaa`: the
    /// snapshot a test pins, so a token cannot change unnoticed.
    pub fn tokens(&self) -> String {
        fn hex(color: Color) -> String {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                color.red, color.green, color.blue, color.alpha
            )
        }
        let roles = &self.roles;
        let shadow = |shadow: Shadow| {
            format!(
                "{} {} {} {}",
                shadow.x,
                shadow.y,
                shadow.blur,
                hex(shadow.color)
            )
        };
        let easing = match self.motion.easing {
            Easing::Linear => "linear".to_owned(),
            Easing::CubicBezier(a, b, c, d) => format!("cubic-bezier({a}, {b}, {c}, {d})"),
        };
        [
            format!("appearance {:?}", self.appearance),
            format!("color.background {}", hex(self.background)),
            format!("color.text {}", hex(self.text)),
            format!("color.muted {}", hex(self.muted)),
            format!("color.rule {}", hex(self.rule)),
            format!("color.button {}", hex(self.button)),
            format!("color.button_text {}", hex(self.button_text)),
            format!("color.link {}", hex(self.link)),
            format!("color.focus {}", hex(self.focus)),
            format!("role.primary {}", hex(roles.primary)),
            format!("role.secondary {}", hex(roles.secondary)),
            format!("role.tertiary {}", hex(roles.tertiary)),
            format!("role.link {}", hex(roles.link)),
            format!("role.bubble {}", hex(roles.bubble)),
            format!("role.surface {}", hex(roles.surface)),
            format!("role.raised {}", hex(roles.raised)),
            format!("role.border {}", hex(roles.border)),
            format!("role.inline_code {}", hex(roles.inline_code)),
            format!("radius.card {}", self.card_radius),
            format!("radius.button {}", self.button_radius),
            format!("radius.check {}", self.radius.check),
            format!("border.hairline {}", self.border.hairline),
            format!("border.control {}", self.border.control),
            format!("border.focus {}", self.border.focus),
            format!("border.focus_offset {}", self.border.focus_offset),
            format!("shadow.raised {}", shadow(self.shadow.raised)),
            format!("shadow.overlay {}", shadow(self.shadow.overlay)),
            format!("opacity.disabled {}", self.opacity.disabled),
            format!("opacity.disabled_fill {}", self.opacity.disabled_fill),
            format!("opacity.hover_tint {}", self.opacity.hover_tint),
            format!("opacity.pressed_tint {}", self.opacity.pressed_tint),
            format!("opacity.pressed_check {}", self.opacity.pressed_check),
            format!("opacity.hover_edge {}", self.opacity.hover_edge),
            format!(
                "motion.fast {}ms",
                self.motion
                    .duration(Speed::Fast, Need::Decorative)
                    .as_millis()
            ),
            format!(
                "motion.standard {}ms",
                self.motion
                    .duration(Speed::Standard, Need::Decorative)
                    .as_millis()
            ),
            format!(
                "motion.slow {}ms",
                self.motion
                    .duration(Speed::Slow, Need::Decorative)
                    .as_millis()
            ),
            format!("motion.easing {easing}"),
            format!("type.body {}", self.body),
            format!("type.heading {}", self.heading),
            format!("type.status {}", self.status),
            format!("type.code {}", self.code),
            format!("size.icon {}", self.icon_size),
            format!("size.column {}", self.column),
            format!("size.margin {}", self.margin),
        ]
        .join("\n")
            + "\n"
    }
}

/// The points a semantic space stands for.
pub fn space(space: Option<Space>) -> f32 {
    match space {
        None | Some(Space::None) => 0.0,
        Some(Space::Xs) => 4.0,
        Some(Space::Sm) => 8.0,
        Some(Space::Md) => 16.0,
        Some(Space::Lg) => 28.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dark theme's tokens, pinned. A change here is a visual change:
    /// update the snapshot only with the change that means it.
    #[test]
    fn the_openagents_tokens_match_their_snapshot() {
        let tokens = Theme {
            motion: Motion::new(false),
            ..Theme::openagents()
        }
        .tokens();
        assert_eq!(tokens, include_str!("../tests/theme_tokens.txt"));
    }

    #[test]
    fn the_ladder_is_the_websites_and_coderos() {
        let hex = |c: Color| (c.red, c.green, c.blue);
        assert_eq!(hex(ladder::WHITE), (0xff, 0xff, 0xff));
        assert_eq!(hex(ladder::WHITE_75), (0xc8, 0xc8, 0xc8));
        assert_eq!(hex(ladder::WHITE_50), (0x8a, 0x8a, 0x8a));
        assert_eq!(hex(ladder::WHITE_25), (0x4a, 0x4a, 0x4a));
        assert_eq!(hex(ladder::GROUND), (0x0a, 0x0a, 0x0a));
        assert_eq!(hex(ladder::RAISED), (0x1a, 0x1a, 0x1a));
    }

    /// The defaults are dark; the window asks for the app's appearance and
    /// reads the system's in one place only, for the app's seam; nothing
    /// that lays out or paints reads it.
    #[test]
    fn the_default_theme_is_dark_and_only_the_window_reads_the_system() {
        assert_eq!(APPEARANCE, Appearance::Dark);
        assert_eq!(Theme::default().appearance, Appearance::Dark);
        assert_eq!(Theme::openagents().appearance, Appearance::Dark);
        for theme in [Theme::default(), Theme::openagents()] {
            let luma = |c: Color| {
                0.2126 * f32::from(c.red) + 0.7152 * f32::from(c.green) + 0.0722 * f32::from(c.blue)
            };
            assert!(luma(theme.background) < 32.0, "a dark field");
            assert!(luma(theme.text) > 200.0, "light text");
        }
        let window = include_str!("window.rs");
        assert!(
            window.contains(".with_theme(Some(winit_theme(self.app.theme().appearance)))"),
            "the window asks for the app's appearance"
        );
        assert!(
            window.contains("event_loop.system_theme()"),
            "the window reads the system's appearance for the app"
        );
        let sources = [
            include_str!("wayland.rs"),
            include_str!("layout.rs"),
            include_str!("paint.rs"),
            include_str!("lib.rs"),
        ];
        for source in sources {
            for follows in [
                "effectiveAppearance",
                "AppleInterfaceStyle",
                "color-scheme",
                "ThemeChanged",
                "window.theme()",
                "prefers-color-scheme",
            ] {
                assert!(
                    !source.contains(follows),
                    "reads the system's appearance: {follows}"
                );
            }
        }
    }

    #[test]
    fn reduced_motion_zeroes_decoration_and_keeps_what_is_essential() {
        let full = Motion::new(false);
        let reduced = Motion::new(true);
        for speed in [Speed::Fast, Speed::Standard, Speed::Slow] {
            assert!(full.duration(speed, Need::Decorative) > Duration::ZERO);
            assert_eq!(reduced.duration(speed, Need::Decorative), Duration::ZERO);
            assert_eq!(
                reduced.duration(speed, Need::Essential),
                full.duration(speed, Need::Essential)
            );
        }
        assert!(!reduced.animates(Need::Decorative));
        assert!(reduced.animates(Need::Essential));
        let tokens = Theme {
            motion: reduced,
            ..Theme::openagents()
        }
        .tokens();
        assert!(tokens.contains("motion.fast 0ms"));
        assert!(tokens.contains("motion.standard 0ms"));
        assert!(tokens.contains("motion.slow 0ms"));
    }

    #[test]
    fn the_persons_switch_or_the_system_reduces_motion() {
        use motion::resolve;
        assert!(!resolve(false, false));
        assert!(resolve(true, false));
        assert!(resolve(false, true));
        assert!(resolve(true, true));
    }

    #[test]
    fn the_portal_reply_reads_as_reduced_or_not() {
        assert_eq!(motion::parse_portal_reply("(<<uint32 1>>,)"), Some(true));
        assert_eq!(motion::parse_portal_reply("(<uint32 0>,)"), Some(false));
        assert_eq!(motion::parse_portal_reply("Error: No such key"), None);
    }

    #[test]
    fn the_standard_easing_starts_slow_and_lands() {
        let ease = Easing::STANDARD;
        assert_eq!(ease.at(0.0), 0.0);
        assert_eq!(ease.at(1.0), 1.0);
        assert!(ease.at(0.5) > 0.5, "decelerates");
        assert_eq!(Easing::Linear.at(0.25), 0.25);
    }
}
