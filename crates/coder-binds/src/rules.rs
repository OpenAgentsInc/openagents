//! The one table of window rules.
//!
//! `RULES` holds a row per rule: what it matches, on the app-id or the
//! title, and the effects the desktop applies to a window that matches.
//! Two readers keep the copies from drifting: [`hyprland_rule_lines`]
//! renders the rows as the `windowrule` lines
//! `os/modules/coderos/desktop.nix` writes for Hyprland 0.55, which a
//! repository test compares to that file, and the Coder compositor calls
//! [`matching`] when a window maps and when its app-id or title changes.
//!
//! A match is a literal: the whole string or its start, with ASCII case
//! either exact or ignored. The Hyprland renderer turns the literal into
//! the regular expression Hyprland reads, and nothing else in the
//! repository holds one.

/// Which string of a window a rule reads. An X11 window's app-id is its
/// class, the second string of its `WM_CLASS` pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    AppId,
    Title,
}

/// How a rule compares its literals to the window's string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Byte for byte.
    Exact,
    /// ASCII case ignored. Wine reports a class as the executable's name in
    /// whichever case the launcher spelled it, so a host's rule for a game
    /// under Wine reads it this way.
    Any,
}

/// One literal a rule accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// The whole string.
    Exact(&'static str),
    /// The start of the string.
    Prefix(&'static str),
    /// The start of the string, and a word somewhere after it.
    PrefixHolding {
        prefix: &'static str,
        holds: &'static str,
    },
    /// The whole string, as a `coderos.desktop` option names it. The
    /// compositor matches the option's default; `desktop.nix` writes the
    /// option's value.
    Configured {
        nix: &'static str,
        default: &'static str,
    },
}

/// A literal as one comparison, whichever table holds it.
#[derive(Clone, Copy)]
enum Literal<'a> {
    Exact(&'a str),
    Prefix(&'a str),
    PrefixHolding { prefix: &'a str, holds: &'a str },
}

impl Literal<'_> {
    /// Whether one string matches this literal under `case`.
    fn matches(self, text: &str, case: Case) -> bool {
        let fold = |s: &str| match case {
            Case::Exact => s.to_string(),
            Case::Any => s.to_ascii_lowercase(),
        };
        let text = fold(text);
        match self {
            Literal::Exact(want) => text == fold(want),
            Literal::Prefix(want) => text.starts_with(&fold(want)),
            Literal::PrefixHolding { prefix, holds } => text
                .strip_prefix(&fold(prefix))
                .is_some_and(|rest| rest.contains(&fold(holds))),
        }
    }

    /// The literal as one alternative of a Hyprland regular expression.
    fn hypr(self) -> String {
        match self {
            Literal::Exact(want) => escape(want),
            Literal::Prefix(want) => format!("{}.*", escape(want)),
            Literal::PrefixHolding { prefix, holds } => {
                format!("{}.*{}.*", escape(prefix), escape(holds))
            }
        }
    }
}

impl Pattern {
    /// Whether one string matches this literal under `case`.
    fn matches(self, text: &str, case: Case) -> bool {
        match self {
            Pattern::Exact(want) | Pattern::Configured { default: want, .. } => {
                Literal::Exact(want).matches(text, case)
            }
            Pattern::Prefix(want) => Literal::Prefix(want).matches(text, case),
            Pattern::PrefixHolding { prefix, holds } => {
                Literal::PrefixHolding { prefix, holds }.matches(text, case)
            }
        }
    }

    /// The literal as one alternative of a Hyprland regular expression.
    fn hypr(self) -> String {
        match self {
            Pattern::Exact(want) => Literal::Exact(want).hypr(),
            Pattern::Prefix(want) => Literal::Prefix(want).hypr(),
            Pattern::PrefixHolding { prefix, holds } => {
                Literal::PrefixHolding { prefix, holds }.hypr()
            }
            Pattern::Configured { nix, .. } => format!("${{{nix}}}"),
        }
    }
}

/// A literal with the characters RE2 reads as syntax escaped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if r"\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// What a rule matches: a field, the literals it accepts, and how case is
/// read. A window matches when any one literal does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub field: Field,
    pub patterns: &'static [Pattern],
    pub case: Case,
}

impl Match {
    /// Whether a window with this app-id and title matches.
    pub fn matches(&self, app_id: &str, title: &str) -> bool {
        let text = match self.field {
            Field::AppId => app_id,
            Field::Title => title,
        };
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(text, self.case))
    }

    /// The `match:` field of a Hyprland `windowrule` line, such as
    /// `match:title ^(selfie)$`.
    fn hypr(&self) -> String {
        let alternatives: Vec<String> = self.patterns.iter().map(|p| p.hypr()).collect();
        match_field(self.field, self.case, &alternatives)
    }
}

/// The `match:` field of a Hyprland `windowrule` line from its parts.
fn match_field(field: Field, case: Case, alternatives: &[String]) -> String {
    let field = match field {
        Field::AppId => "class",
        Field::Title => "title",
    };
    let flag = match case {
        Case::Exact => "",
        Case::Any => "(?i)",
    };
    format!("match:{field} {flag}^({})$", alternatives.join("|"))
}

/// What the desktop does to a window a rule matches. Every field left at
/// its default leaves the window as the layout had it, so a table with no
/// matching rule is [`Effects::default`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// `Some(true)` floats the window over the layout, `Some(false)` puts
    /// it in a tile, and `None` leaves it where the layout put it.
    pub float: Option<bool>,
    /// A float opens in the middle of the screen.
    pub center: bool,
    /// The window shows on every desk. A pinned window floats.
    pub pin: bool,
    /// The layout keeps the ratio the window mapped at when it resizes
    /// the window.
    pub keep_aspect: bool,
    /// The border's thickness in pixels. `Some(0)` draws none.
    pub border: Option<i32>,
    /// Whether the window casts a shadow.
    pub shadow: Option<bool>,
    /// A fullscreen or maximize request from the client leaves the
    /// window where the layout has it.
    pub suppress_fullscreen: bool,
}

impl Effects {
    /// These effects with `over` applied on top: a field `over` sets wins,
    /// and a flag either one raises stays raised.
    pub fn merge(self, over: Effects) -> Effects {
        Effects {
            float: over.float.or(self.float),
            center: self.center || over.center,
            pin: self.pin || over.pin,
            keep_aspect: self.keep_aspect || over.keep_aspect,
            border: over.border.or(self.border),
            shadow: over.shadow.or(self.shadow),
            suppress_fullscreen: self.suppress_fullscreen || over.suppress_fullscreen,
        }
    }

    /// The effects as the fields of a Hyprland `windowrule` line, in the
    /// order `desktop.nix` spells them.
    fn hypr(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.float == Some(true) {
            out.push("float on".to_string());
        }
        if self.center {
            out.push("center on".to_string());
        }
        if self.keep_aspect {
            out.push("keep_aspect_ratio on".to_string());
        }
        if let Some(size) = self.border {
            out.push(format!("border_size {size}"));
        }
        if self.shadow == Some(false) {
            out.push("no_shadow on".to_string());
        }
        if self.pin {
            out.push("pin on".to_string());
        }
        if self.float == Some(false) {
            out.push("tile on".to_string());
        }
        if self.suppress_fullscreen {
            out.push("suppress_event maximize fullscreen".to_string());
        }
        out
    }
}

/// One row of the table.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// The one-line name the crate's README gives the rule.
    pub name: &'static str,
    pub matches: Match,
    pub effects: Effects,
    /// The `coderos.desktop.*` option that gates the Hyprland copy of the
    /// rule, such as `camera` for `coderos.desktop.camera`. The compositor
    /// applies every rule: a window only matches when its launcher ran.
    pub option: &'static str,
}

const NONE: Effects = Effects {
    float: None,
    center: false,
    pin: false,
    keep_aspect: false,
    border: None,
    shadow: None,
    suppress_fullscreen: false,
};

/// A float that keeps its shape, with no border and no shadow, on every
/// desk: the camera circle and the recording HUD.
const OVERLAY: Effects = Effects {
    float: Some(true),
    pin: true,
    border: Some(0),
    shadow: Some(false),
    ..NONE
};

/// The class the Android emulator's windows announce, the default of
/// `coderos.desktop.android.windowClass`.
pub const EMULATOR_CLASS: &str = "Emulator";

/// The rule table, in the order `desktop.nix` writes the rows.
pub const RULES: &[Rule] = &[
    // The emulator floats, because a phone is tall and narrow, and keeps
    // its shape under a mouse resize.
    Rule {
        name: "Android emulator",
        matches: Match {
            field: Field::AppId,
            patterns: &[Pattern::Configured {
                nix: "cfg.android.windowClass",
                default: EMULATOR_CLASS,
            }],
            case: Case::Exact,
        },
        effects: Effects {
            float: Some(true),
            keep_aspect: true,
            ..NONE
        },
        option: "android",
    },
    // The camera circle: `os/bin/camera-overlay` titles the window
    // `selfie` and sizes it square, and the rule keeps it square.
    Rule {
        name: "Camera circle",
        matches: Match {
            field: Field::Title,
            patterns: &[Pattern::Exact("selfie")],
            case: Case::Exact,
        },
        effects: Effects {
            keep_aspect: true,
            ..OVERLAY
        },
        option: "camera",
    },
    // The recording HUD: the strip `os/bin/recording-hud` docks under the
    // circle.
    Rule {
        name: "Recording HUD",
        matches: Match {
            field: Field::Title,
            patterns: &[Pattern::Exact("recording-hud")],
            case: Case::Exact,
        },
        effects: OVERLAY,
        option: "screenRecording",
    },
];

/// The effects every rule that matches a window applies, folded in table
/// order, or [`Effects::default`] when none matches.
pub fn matching(app_id: &str, title: &str) -> Effects {
    matching_with(app_id, title, &[])
}

/// The effects of the table's rules and then a host's extra rules, folded
/// in that order, so a host's rule wins where the two disagree.
pub fn matching_with(app_id: &str, title: &str, extra: &[ExtraRule]) -> Effects {
    let table = RULES
        .iter()
        .filter(|rule| rule.matches.matches(app_id, title))
        .map(|rule| rule.effects);
    let host = extra
        .iter()
        .filter(|rule| rule.matches(app_id, title))
        .map(|rule| rule.effects);
    table
        .chain(host)
        .fold(Effects::default(), |held, effects| held.merge(effects))
}

/// One literal of an [`ExtraRule`], owned because it comes from a grant
/// rather than from this table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtraPattern {
    /// The whole string.
    Exact(String),
    /// The start of the string.
    Prefix(String),
    /// The start of the string, and a word somewhere after it.
    PrefixHolding { prefix: String, holds: String },
}

impl ExtraPattern {
    fn literal(&self) -> Literal<'_> {
        match self {
            ExtraPattern::Exact(want) => Literal::Exact(want),
            ExtraPattern::Prefix(want) => Literal::Prefix(want),
            ExtraPattern::PrefixHolding { prefix, holds } => {
                Literal::PrefixHolding { prefix, holds }
            }
        }
    }
}

/// A window rule a host adds beside the table, from
/// `coderos.desktop.extraWindowRules`.
///
/// The table holds the rules for the windows the public CoderOS modules
/// open. A module in a host's own flake, such as a game launcher, adds the
/// rules for its windows to that option. `desktop.nix` renders each entry
/// as a Hyprland `windowrule` line the same way [`ExtraRule::hyprland`]
/// does, and writes the entries to the Coder compositor's grant as
/// `extraRules`, which the compositor folds in with [`matching_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtraRule {
    pub name: String,
    pub field: Field,
    pub patterns: Vec<ExtraPattern>,
    pub case: Case,
    pub effects: Effects,
}

impl ExtraRule {
    /// Whether a window with this app-id and title matches.
    pub fn matches(&self, app_id: &str, title: &str) -> bool {
        let text = match self.field {
            Field::AppId => app_id,
            Field::Title => title,
        };
        self.patterns
            .iter()
            .any(|pattern| pattern.literal().matches(text, self.case))
    }

    /// The `windowrule` line Hyprland 0.55 reads for the entry.
    pub fn hyprland(&self) -> String {
        let alternatives: Vec<String> = self
            .patterns
            .iter()
            .map(|pattern| pattern.literal().hypr())
            .collect();
        let mut fields = vec![match_field(self.field, self.case, &alternatives)];
        fields.extend(self.effects.hypr());
        format!("windowrule = {}", fields.join(", "))
    }
}

/// The `windowrule` line Hyprland 0.55 reads for one rule, such as
/// `windowrule = match:title ^(selfie)$, float on, keep_aspect_ratio on, border_size 0, no_shadow on, pin on`.
pub fn hyprland_rule(rule: &Rule) -> String {
    let mut fields = vec![rule.matches.hypr()];
    fields.extend(rule.effects.hypr());
    format!("windowrule = {}", fields.join(", "))
}

/// The `windowrule` lines the table renders, in table order.
pub fn hyprland_rule_lines() -> Vec<String> {
    RULES.iter().map(hyprland_rule).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_with_no_rule_gets_no_effects() {
        assert_eq!(matching("foot", "~"), Effects::default());
        assert_eq!(matching("XTerm", "xterm"), Effects::default());
        // The emulator's class is matched in its case.
        assert_eq!(matching("emulator", ""), Effects::default());
        assert_eq!(matching("", "selfies"), Effects::default());
    }

    #[test]
    fn the_overlays_float_pinned_and_borderless() {
        let camera = matching("mpv", "selfie");
        assert_eq!(camera.float, Some(true));
        assert!(camera.pin);
        assert!(camera.keep_aspect);
        assert_eq!(camera.border, Some(0));
        assert_eq!(camera.shadow, Some(false));
        assert!(!camera.center);
        let hud = matching("recording-hud", "recording-hud");
        assert_eq!(
            hud,
            Effects {
                keep_aspect: false,
                ..camera
            }
        );
    }

    /// The rules a host with Battle.net adds through
    /// `coderos.desktop.extraWindowRules`, as `os/tests/extension-points.nix`
    /// sets them: the launcher floats in the middle of the screen, and a
    /// game client tiles as a pane that a fullscreen request leaves there.
    fn game_rules() -> Vec<ExtraRule> {
        let game = Effects {
            float: Some(false),
            suppress_fullscreen: true,
            ..NONE
        };
        let owned = |text: &str| text.to_string();
        vec![
            ExtraRule {
                name: owned("Battle.net launcher"),
                field: Field::AppId,
                patterns: vec![
                    ExtraPattern::Exact(owned("battle.net.exe")),
                    ExtraPattern::Exact(owned("Battle.net.exe")),
                    ExtraPattern::Exact(owned("steam_app_battlenet")),
                ],
                case: Case::Exact,
                effects: Effects {
                    float: Some(true),
                    center: true,
                    ..NONE
                },
            },
            ExtraRule {
                name: owned("StarCraft II client, by class"),
                field: Field::AppId,
                patterns: vec![
                    ExtraPattern::Prefix(owned("sc2")),
                    ExtraPattern::Prefix(owned("starcraft")),
                    ExtraPattern::PrefixHolding {
                        prefix: owned("steam_app_"),
                        holds: owned("sc2"),
                    },
                ],
                case: Case::Any,
                effects: game,
            },
            ExtraRule {
                name: owned("StarCraft II client, by title"),
                field: Field::Title,
                patterns: vec![ExtraPattern::Prefix(owned("StarCraft II"))],
                case: Case::Any,
                effects: game,
            },
        ]
    }

    #[test]
    fn a_host_rule_floats_the_launcher_and_tiles_the_game() {
        let extra = game_rules();
        for class in ["battle.net.exe", "Battle.net.exe", "steam_app_battlenet"] {
            let launcher = matching_with(class, "Battle.net", &extra);
            assert_eq!(launcher.float, Some(true), "{class}");
            assert!(launcher.center, "{class}");
            assert!(!launcher.suppress_fullscreen, "{class}");
        }
        for (class, title) in [
            ("SC2_x64.exe", ""),
            ("sc2.exe", ""),
            ("SC2.EXE", ""),
            ("StarCraft II", ""),
            ("steam_app_sc2", ""),
            ("wine", "StarCraft II"),
        ] {
            let game = matching_with(class, title, &extra);
            assert_eq!(game.float, Some(false), "{class} {title}");
            assert!(game.suppress_fullscreen, "{class} {title}");
            assert!(!game.pin, "{class} {title}");
        }
        // Without the host's rules the table has nothing to say about a
        // game, and a class that holds a game's name past its start is not
        // a game's.
        assert_eq!(matching("SC2_x64.exe", ""), Effects::default());
        assert_eq!(matching_with("websc2", "", &extra), Effects::default());
        // A host rule folds after the table's, so it wins where they meet.
        let over = [ExtraRule {
            name: "selfie tiles".to_string(),
            field: Field::Title,
            patterns: vec![ExtraPattern::Exact("selfie".to_string())],
            case: Case::Exact,
            effects: Effects {
                float: Some(false),
                ..NONE
            },
        }];
        let camera = matching_with("mpv", "selfie", &over);
        assert_eq!(camera.float, Some(false));
        assert!(camera.pin);
    }

    #[test]
    fn a_host_rule_renders_the_line_desktop_nix_writes_for_it() {
        let lines: Vec<String> = game_rules().iter().map(ExtraRule::hyprland).collect();
        assert_eq!(
            lines,
            [
                r"windowrule = match:class ^(battle\.net\.exe|Battle\.net\.exe|steam_app_battlenet)$, float on, center on",
                "windowrule = match:class (?i)^(sc2.*|starcraft.*|steam_app_.*sc2.*)$, tile on, suppress_event maximize fullscreen",
                "windowrule = match:title (?i)^(StarCraft II.*)$, tile on, suppress_event maximize fullscreen",
            ]
        );
    }

    #[test]
    fn the_emulator_floats_and_keeps_its_shape() {
        let emulator = matching(EMULATOR_CLASS, "Android Emulator - coder:5554");
        assert_eq!(emulator.float, Some(true));
        assert!(emulator.keep_aspect);
        assert!(!emulator.pin);
        assert_eq!(emulator.border, None);
    }

    #[test]
    fn a_later_effect_wins_and_a_flag_stays_raised() {
        let tiled = Effects {
            float: Some(false),
            suppress_fullscreen: true,
            ..Effects::default()
        };
        let floated = Effects {
            float: Some(true),
            pin: true,
            border: Some(0),
            ..Effects::default()
        };
        let merged = tiled.merge(floated);
        assert_eq!(merged.float, Some(true));
        assert!(merged.pin);
        assert!(merged.suppress_fullscreen);
        assert_eq!(merged.border, Some(0));
        assert_eq!(floated.merge(Effects::default()), floated);
    }

    #[test]
    fn hyprland_lines_render_every_rule() {
        let lines = hyprland_rule_lines();
        assert_eq!(lines.len(), RULES.len());
        assert_eq!(
            lines[0],
            "windowrule = match:class ^(${cfg.android.windowClass})$, float on, keep_aspect_ratio on"
        );
        assert_eq!(
            lines[1],
            "windowrule = match:title ^(selfie)$, float on, keep_aspect_ratio on, border_size 0, no_shadow on, pin on"
        );
        assert_eq!(
            lines[2],
            "windowrule = match:title ^(recording-hud)$, float on, border_size 0, no_shadow on, pin on"
        );
    }

    #[test]
    fn a_literal_reaches_hyprland_escaped() {
        assert_eq!(escape("battle.net.exe"), r"battle\.net\.exe");
        assert_eq!(escape("a+b(c)"), r"a\+b\(c\)");
        assert_eq!(escape("recording-hud"), "recording-hud");
    }

    #[test]
    fn every_rule_has_a_name_and_an_effect() {
        for rule in RULES {
            assert!(!rule.name.is_empty());
            assert_ne!(rule.effects, Effects::default(), "{}", rule.name);
            assert!(!rule.matches.patterns.is_empty(), "{}", rule.name);
        }
    }
}
