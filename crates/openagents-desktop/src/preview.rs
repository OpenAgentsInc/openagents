//! The 1.0 window (#11120), streamlined as the phone was (#11090,
//! `docs/mobile/1.0-audit.md`): chat, Coder on this computer, connecting a
//! phone, the account, and Settings show in every build. The screens still
//! in development, the Verse (the Grid), the Map, the Gym's hosted runs in
//! chat, and **Give feedback** (playtest reports), show only in a build made
//! with `OPENAGENTS_DESKTOP_PREVIEW=on`. Their code stays; only their entry
//! points hide.
//!
//! Rust reads the setting once, when it compiles, as the phone's
//! `OPENAGENTS_MOBILE_PREVIEW` is read.

/// Whether this build shows the preview features.
pub const ON: bool = setting(option_env!("OPENAGENTS_DESKTOP_PREVIEW"));

/// Reads the build's `OPENAGENTS_DESKTOP_PREVIEW`: exactly `on` turns the
/// preview features on; unset or anything else leaves them hidden.
#[must_use]
pub const fn setting(value: Option<&str>) -> bool {
    let Some(value) = value else { return false };
    let bytes = value.as_bytes();
    bytes.len() == 2 && bytes[0] == b'o' && bytes[1] == b'n'
}

/// The command and profile menu entries a build without the preview
/// features leaves out, by key.
pub const HIDDEN_ENTRIES: [&str; 3] = ["grid", "map", "feedback"];

/// Whether a menu entry shows in this build.
#[must_use]
pub fn shows_entry(key: &str, preview: bool) -> bool {
    preview || !HIDDEN_ENTRIES.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_features_are_off_unless_the_build_says_on() {
        assert!(!setting(None));
        assert!(!setting(Some("")));
        assert!(!setting(Some("off")));
        assert!(!setting(Some("1")));
        assert!(setting(Some("on")));
        assert_eq!(ON, setting(option_env!("OPENAGENTS_DESKTOP_PREVIEW")));
        assert!(!shows_entry("map", false));
        assert!(shows_entry("map", true));
        assert!(shows_entry("new", false));
    }
}
