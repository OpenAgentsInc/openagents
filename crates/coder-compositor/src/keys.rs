//! Keysyms to the keys the bind table names.
//!
//! The table names a key by what it prints, so a chord reads the way
//! `os/modules/coderos/desktop.nix` writes it. A press arrives as a keysym,
//! and this module reads the key the press would print with no modifier
//! held, so Super+Shift+1 is the digit row rather than the exclamation
//! mark.

use coder_wm::Dir;
use smithay::input::keyboard::{Keysym, KeysymHandle, XkbConfig, keysyms};

use crate::binds::Key;

/// The keyboard layout the seat loads.
///
/// A session names its layout in the environment, the way
/// `os/modules/coderos/desktop.nix` and every other Wayland session do, and
/// the compositor reads the same five variables so a person on a layout
/// that is not US gets that layout in a tile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// `XKB_DEFAULT_RULES`.
    pub rules: String,
    /// `XKB_DEFAULT_MODEL`.
    pub model: String,
    /// `XKB_DEFAULT_LAYOUT`, such as `de` or `us,fr`.
    pub layout: String,
    /// `XKB_DEFAULT_VARIANT`, such as `dvorak`.
    pub variant: String,
    /// `XKB_DEFAULT_OPTIONS`, such as `grp:alt_shift_toggle`.
    pub options: String,
}

impl Layout {
    /// The layout this process's environment names.
    pub fn from_environment() -> Self {
        Self::read(|name| std::env::var(name).ok())
    }

    /// The layout one set of variables names. A variable that is missing or
    /// empty leaves its field empty, which loads the xkb default.
    pub fn read(mut value: impl FnMut(&str) -> Option<String>) -> Self {
        let mut named = |name: &str| {
            value(name)
                .filter(|held| !held.is_empty())
                .unwrap_or_default()
        };
        Self {
            rules: named("XKB_DEFAULT_RULES"),
            model: named("XKB_DEFAULT_MODEL"),
            layout: named("XKB_DEFAULT_LAYOUT"),
            variant: named("XKB_DEFAULT_VARIANT"),
            options: named("XKB_DEFAULT_OPTIONS"),
        }
    }

    /// The layout as the keyboard reads it.
    pub fn config(&self) -> XkbConfig<'_> {
        XkbConfig {
            rules: &self.rules,
            model: &self.model,
            layout: &self.layout,
            variant: &self.variant,
            options: if self.options.is_empty() {
                None
            } else {
                Some(self.options.clone())
            },
        }
    }

    /// What the log says the seat loaded.
    pub fn named(&self) -> String {
        if self.layout.is_empty() {
            return "the xkb default".to_string();
        }
        match (self.variant.is_empty(), self.options.is_empty()) {
            (true, true) => self.layout.clone(),
            (false, true) => format!("{} ({})", self.layout, self.variant),
            (true, false) => format!("{} with {}", self.layout, self.options),
            (false, false) => format!("{} ({}) with {}", self.layout, self.variant, self.options),
        }
    }
}

/// The key one press names, or nothing when the table names no such key.
pub fn key_of(handle: &KeysymHandle<'_>) -> Option<Key> {
    let raw = handle.raw_syms().first().copied();
    raw.and_then(key_of_sym)
        .or_else(|| key_of_sym(handle.modified_sym()))
}

/// The virtual terminal a press asks for, 1 through 12.
///
/// Ctrl+Alt with a function key prints `XF86Switch_VT_1` through
/// `XF86Switch_VT_12` in every xkb layout, which is how a person leaves a
/// compositor on a TTY for another one. The hardware backend answers it
/// before any chord; the nested backend leaves it to the session it runs
/// in.
pub fn vt_of(sym: Keysym) -> Option<i32> {
    match sym.raw() {
        raw @ keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12 => {
            Some((raw - keysyms::KEY_XF86Switch_VT_1 + 1) as i32)
        }
        _ => None,
    }
}

/// The key one keysym names.
pub fn key_of_sym(sym: Keysym) -> Option<Key> {
    match sym.raw() {
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => Some(Key::Return),
        keysyms::KEY_space => Some(Key::Space),
        keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab => Some(Key::Tab),
        keysyms::KEY_Left => Some(Key::Arrow(Dir::Left)),
        keysyms::KEY_Right => Some(Key::Arrow(Dir::Right)),
        keysyms::KEY_Up => Some(Key::Arrow(Dir::Up)),
        keysyms::KEY_Down => Some(Key::Arrow(Dir::Down)),
        raw @ keysyms::KEY_1..=keysyms::KEY_9 => {
            let digit = (raw - keysyms::KEY_1 + 1) as u8;
            Some(Key::Digit(digit))
        }
        raw @ keysyms::KEY_a..=keysyms::KEY_z => {
            char::from_u32(raw).map(|letter| Key::Letter(letter.to_ascii_lowercase()))
        }
        raw @ keysyms::KEY_A..=keysyms::KEY_Z => {
            char::from_u32(raw).map(|letter| Key::Letter(letter.to_ascii_lowercase()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(raw: u32) -> Keysym {
        Keysym::from(raw)
    }

    #[test]
    fn the_keys_the_table_names_read_back() {
        assert_eq!(key_of_sym(sym(keysyms::KEY_Return)), Some(Key::Return));
        assert_eq!(key_of_sym(sym(keysyms::KEY_KP_Enter)), Some(Key::Return));
        assert_eq!(key_of_sym(sym(keysyms::KEY_space)), Some(Key::Space));
        assert_eq!(
            key_of_sym(sym(keysyms::KEY_Left)),
            Some(Key::Arrow(Dir::Left))
        );
        assert_eq!(
            key_of_sym(sym(keysyms::KEY_Down)),
            Some(Key::Arrow(Dir::Down))
        );
    }

    #[test]
    fn the_digit_row_reads_as_one_through_nine() {
        assert_eq!(key_of_sym(sym(keysyms::KEY_1)), Some(Key::Digit(1)));
        assert_eq!(key_of_sym(sym(keysyms::KEY_9)), Some(Key::Digit(9)));
        assert_eq!(key_of_sym(sym(keysyms::KEY_0)), None);
    }

    #[test]
    fn a_letter_reads_the_same_in_either_case() {
        assert_eq!(key_of_sym(sym(keysyms::KEY_t)), Some(Key::Letter('t')));
        assert_eq!(key_of_sym(sym(keysyms::KEY_T)), Some(Key::Letter('t')));
        assert_eq!(key_of_sym(sym(keysyms::KEY_F)), Some(Key::Letter('f')));
    }

    #[test]
    fn tab_reads_as_tab_with_shift_held_or_not() {
        assert_eq!(key_of_sym(sym(keysyms::KEY_Tab)), Some(Key::Tab));
        assert_eq!(key_of_sym(sym(keysyms::KEY_ISO_Left_Tab)), Some(Key::Tab));
    }

    #[test]
    fn ctrl_alt_and_a_function_key_names_a_virtual_terminal() {
        assert_eq!(vt_of(sym(keysyms::KEY_XF86Switch_VT_1)), Some(1));
        assert_eq!(vt_of(sym(keysyms::KEY_XF86Switch_VT_2)), Some(2));
        assert_eq!(vt_of(sym(keysyms::KEY_XF86Switch_VT_12)), Some(12));
        assert_eq!(vt_of(sym(keysyms::KEY_F1)), None);
    }

    #[test]
    fn a_key_outside_the_table_reads_as_nothing() {
        assert_eq!(key_of_sym(sym(keysyms::KEY_Escape)), None);
        assert_eq!(key_of_sym(sym(keysyms::KEY_F1)), None);
    }

    #[test]
    fn an_empty_environment_loads_the_xkb_default() {
        let layout = Layout::read(|_| None);
        assert_eq!(layout, Layout::default());
        assert_eq!(layout.named(), "the xkb default");
        assert_eq!(layout.config().options, None);
    }

    #[test]
    fn a_layout_the_session_names_reaches_the_keyboard() {
        let layout = Layout::read(|name| match name {
            "XKB_DEFAULT_LAYOUT" => Some("de".to_string()),
            "XKB_DEFAULT_VARIANT" => Some("neo".to_string()),
            "XKB_DEFAULT_OPTIONS" => Some("grp:alt_shift_toggle".to_string()),
            _ => None,
        });
        let config = layout.config();
        assert_eq!(config.layout, "de");
        assert_eq!(config.variant, "neo");
        assert_eq!(config.options.as_deref(), Some("grp:alt_shift_toggle"));
        assert_eq!(layout.named(), "de (neo) with grp:alt_shift_toggle");
    }

    #[test]
    fn a_variable_set_to_nothing_reads_as_unset() {
        let layout = Layout::read(|name| match name {
            "XKB_DEFAULT_LAYOUT" => Some("fr".to_string()),
            _ => Some(String::new()),
        });
        assert_eq!(layout.layout, "fr");
        assert!(layout.variant.is_empty());
        assert_eq!(layout.config().options, None);
        assert_eq!(layout.named(), "fr");
    }
}
