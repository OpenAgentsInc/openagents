//! The release gate (`docs/mobile/1.0-audit.md`): the features still in
//! development, the Verse tab, the Gym in chat (Train Coder, Profile, its
//! cards and starter chips), Trainer, Playtest, Tailnet, and the display
//! name, show only in a build made with `OPENAGENTS_MOBILE_PREVIEW=on`.
//! Release builds and normal debug builds hide them; their code stays.
//!
//! Rust reads the setting once, when it compiles. The hosts ask
//! [`openagents_mobile_preview`] (iOS) or `OpenAgentsNative.preview()`
//! (Android, `crate::android`), so a host never shows an entry point Rust
//! would refuse.

/// Whether this build shows the preview features.
pub const ON: bool = setting(option_env!("OPENAGENTS_MOBILE_PREVIEW"));

/// Reads the build's `OPENAGENTS_MOBILE_PREVIEW`: exactly `on` turns the
/// preview features on; unset or anything else leaves them hidden.
#[must_use]
pub const fn setting(value: Option<&str>) -> bool {
    let Some(value) = value else { return false };
    let bytes = value.as_bytes();
    bytes.len() == 2 && bytes[0] == b'o' && bytes[1] == b'n'
}

/// Whether this build shows the preview features: the iOS host's question
/// before it draws its tabs and Account rows.
#[unsafe(no_mangle)]
pub extern "C" fn openagents_mobile_preview() -> bool {
    ON
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
        assert_eq!(ON, setting(option_env!("OPENAGENTS_MOBILE_PREVIEW")));
    }
}
