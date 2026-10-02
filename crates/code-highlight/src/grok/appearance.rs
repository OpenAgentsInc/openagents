//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI):
//! `xai-grok-pager-render/src/theme/env_appearance.rs`, the reply parsing
//! and luminance classification of `src/theme/osc11.rs`, and the chain and
//! day/night mapping of `src/theme/system_appearance.rs` and `cache.rs`.
//!
//! Light or dark decides Grok Day or Grok Night. Detection order, each step
//! only when the one before says nothing:
//! 1. Explicit env stamps: `OPENAGENTS_APPEARANCE`, then the SSH-surviving
//!    `LC_OPENAGENTS_APPEARANCE` (`AcceptEnv LC_*`). grok-build's own
//!    `GROK_APPEARANCE` / `LC_GROK_APPEARANCE` are honored after them.
//! 2. An OSC 11 background reply, when the caller has one
//!    ([`parse_osc11_rgb`] + [`classify_luminance`]).
//! 3. Inherited `COLORFGBG`, the last resort.
//!
//! No answer means Grok Night, grok-build's default.
//!
//! Adapted: grok-build consults the desktop's dark-mode setting first, but
//! only in its opt-in auto theme. The question here is the background the
//! code is drawn on, so a surface that paints its own field passes that
//! color to [`Appearance::of_field`] and detection is skipped entirely.

use std::collections::HashMap;

/// The polarity of the background code is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    /// The polarity of a field the surface paints itself.
    pub fn of_field(r: u8, g: u8, b: u8) -> Self {
        classify_luminance(r, g, b)
    }

    /// The terminal's polarity from the process environment and an optional
    /// OSC 11 reply. `None` when nothing says.
    pub fn detect(osc11_reply: Option<&str>) -> Option<Self> {
        let env: HashMap<String, String> = std::env::vars().collect();
        resolve_chain(
            detect_explicit_from_env_map(&env),
            osc11_reply
                .and_then(parse_osc11_rgb)
                .map(|(r, g, b)| classify_luminance(r, g, b)),
            detect_colorfgbg_from_env_map(&env),
        )
    }
}

/// Explicit stamps, then OSC 11, then `COLORFGBG`.
pub fn resolve_chain(
    explicit: Option<Appearance>,
    osc11: Option<Appearance>,
    colorfgbg: Option<Appearance>,
) -> Option<Appearance> {
    explicit.or(osc11).or(colorfgbg)
}

/// Ordered lookup: explicit stamps, then `COLORFGBG`.
#[must_use]
pub fn detect_from_env_map(env: &HashMap<String, String>) -> Option<Appearance> {
    detect_explicit_from_env_map(env).or_else(|| detect_colorfgbg_from_env_map(env))
}

/// Deliberate wrap/SSH stamps only, no inherited `COLORFGBG` guess.
#[must_use]
pub fn detect_explicit_from_env_map(env: &HashMap<String, String>) -> Option<Appearance> {
    [
        "OPENAGENTS_APPEARANCE",
        "LC_OPENAGENTS_APPEARANCE",
        "GROK_APPEARANCE",
        "LC_GROK_APPEARANCE",
    ]
    .into_iter()
    .find_map(|key| parse_appearance_var(env_nonempty(env, key)))
}

/// Inherited `COLORFGBG` polarity guess.
#[must_use]
pub fn detect_colorfgbg_from_env_map(env: &HashMap<String, String>) -> Option<Appearance> {
    parse_colorfgbg(env_nonempty(env, "COLORFGBG"))
}

fn env_nonempty<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

/// Parse `dark`/`light` (plus `night`/`day` aliases). Unknown values are ignored.
#[must_use]
pub fn parse_appearance_var(raw: Option<&str>) -> Option<Appearance> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "dark" | "night" => Some(Appearance::Dark),
        "light" | "day" => Some(Appearance::Light),
        _ => None,
    }
}

/// Parse `COLORFGBG`. Vim/Neovim heuristic: bg `0-6` and `8` are dark; `7` and `9-15` are light.
/// Non-ANSI indexes and a non-numeric last field (`default`) yield `None`.
#[must_use]
pub fn parse_colorfgbg(raw: Option<&str>) -> Option<Appearance> {
    // Last field is bg (`fg;bg` or `fg;default;bg`)
    // A trailing `default` means "unknown polarity", not "skip and use an earlier number"
    let bg = raw?.split(';').next_back()?.trim().parse::<u8>().ok()?;
    match bg {
        0..=6 | 8 => Some(Appearance::Dark),
        7 | 9..=15 => Some(Appearance::Light),
        _ => None,
    }
}

/// Luminance threshold: backgrounds with Y < 0.5 are considered dark.
const LUMINANCE_THRESHOLD: f64 = 0.5;

/// Uses ITU-R BT.709 luminance coefficients with sRGB gamma correction.
pub fn classify_luminance(r: u8, g: u8, b: u8) -> Appearance {
    let luminance =
        0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b);

    if luminance < LUMINANCE_THRESHOLD {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

/// Handles both 4-digit (`rgb:RRRR/GGGG/BBBB`) and 2-digit (`rgb:RR/GG/BB`) hex formats.
pub fn parse_osc11_rgb(response: &str) -> Option<(u8, u8, u8)> {
    let rgb_start = response.find("rgb:")? + 4;
    let rgb_part = response.get(rgb_start..)?;

    // Split on channel separator `/` and terminators (BEL, ESC).
    let parts: Vec<&str> = rgb_part.split(['/', '\x07', '\x1b']).take(3).collect();
    let [r, g, b] = parts.as_slice() else {
        return None;
    };

    Some((parse_channel(r)?, parse_channel(g)?, parse_channel(b)?))
}

/// For 3- or 4-digit values, extracts the high byte (`>> 8`) to map to 0-255.
/// For 1- or 2-digit values, uses the value directly as 0-255.
fn parse_channel(s: &str) -> Option<u8> {
    let trimmed = s.trim();
    let val = u16::from_str_radix(trimmed, 16).ok()?;
    Some(if trimmed.len() > 2 {
        (val >> 8) as u8
    } else {
        val as u8
    })
}

/// Applies the sRGB transfer function inverse (IEC 61966-2-1).
fn srgb_to_linear(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn explicit_stamps_win_over_colorfgbg() {
        assert_eq!(
            detect_from_env_map(&env(&[
                ("OPENAGENTS_APPEARANCE", "dark"),
                ("LC_OPENAGENTS_APPEARANCE", "light"),
                ("COLORFGBG", "0;15"),
            ])),
            Some(Appearance::Dark)
        );
        assert_eq!(
            detect_from_env_map(&env(&[("LC_OPENAGENTS_APPEARANCE", "day")])),
            Some(Appearance::Light)
        );
        assert_eq!(
            detect_from_env_map(&env(&[("GROK_APPEARANCE", "Light")])),
            Some(Appearance::Light)
        );
        assert_eq!(
            detect_from_env_map(&env(&[
                ("OPENAGENTS_APPEARANCE", "solarized"),
                ("COLORFGBG", "15;0"),
            ])),
            Some(Appearance::Dark)
        );
        assert_eq!(detect_from_env_map(&HashMap::new()), None);
    }

    #[test]
    fn colorfgbg_dark_and_light() {
        assert_eq!(parse_colorfgbg(Some("15;0")), Some(Appearance::Dark));
        assert_eq!(parse_colorfgbg(Some("0;15")), Some(Appearance::Light));
        assert_eq!(
            parse_colorfgbg(Some("15;default;0")),
            Some(Appearance::Dark)
        );
        assert_eq!(parse_colorfgbg(Some("7;8")), Some(Appearance::Dark));
        assert_eq!(parse_colorfgbg(Some("0;7")), Some(Appearance::Light));
        assert_eq!(parse_colorfgbg(Some("15;default")), None);
        assert_eq!(parse_colorfgbg(Some("1;99")), None);
    }

    #[test]
    fn osc11_replies_classify() {
        assert_eq!(
            parse_osc11_rgb("\x1b]11;rgb:ffff/ffff/ffff\x07"),
            Some((255, 255, 255))
        );
        assert_eq!(
            parse_osc11_rgb("\x1b]11;rgb:1e/1e/1e\x1b\\"),
            Some((0x1e, 0x1e, 0x1e))
        );
        assert_eq!(parse_osc11_rgb("garbage"), None);
        assert_eq!(classify_luminance(255, 255, 255), Appearance::Light);
        assert_eq!(classify_luminance(0x0a, 0x0a, 0x0a), Appearance::Dark);
        assert_eq!(
            resolve_chain(None, Some(Appearance::Light), Some(Appearance::Dark)),
            Some(Appearance::Light)
        );
    }
}
