//! Which physical modifier a row's Super means, and where.
//!
//! Every row of the table spells the desktop modifier `Super`, because the
//! CoderOS compositor takes Super and a window under it reads the same key.
//! A window on macOS cannot. The system answers Command+H by hiding the
//! window, Command+Q by quitting it, Command+M by minimizing it,
//! Command+Space by opening Spotlight, and Command+W by closing the window,
//! so those presses never reach the window that wants them. CoderQuest's
//! hand-tracking chord was one of them, which is why the run takes
//! `--hands`.
//!
//! A Mac window reads Control and Option together instead. macOS binds
//! nothing to that pair, VoiceOver aside, and it is the pair Rectangle,
//! yabai, and skhd reach for, so a person who moves between them presses
//! the same keys. Control is down already, so a row that also holds Ctrl
//! reads that as Command: Super+Ctrl+Left is Control+Option+Command+Left,
//! the third layer those tools spell the same way. A row that holds Alt has
//! no Mac reading, because Option is half of the desktop modifier; every
//! such row belongs to the compositor alone, and
//! `crates/coder-binds/tests/binds.rs` keeps that true.
//!
//! Coder Desktop stays on Command. It is a Mac application rather than a
//! window manager: it owns its menu, macOS hands it Command+Return,
//! Command+T, Command+W, and Command+arrows, and it quits on the Command+Q
//! the system expects, which is the [`Surface::Desktop`] row of its own the
//! table already holds. Its shell screen reads the rows through [`find`],
//! not through this module.

use crate::{Action, BINDS, Key, Mods, Surface, find};

/// Which physical modifier a row's Super names where the table is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    /// The Super key, which the CoderOS compositor and a window under it
    /// read.
    Super,
    /// Control and Option together, which a window on macOS reads because
    /// the system takes Command first.
    ControlOption,
}

impl Modifier {
    /// What a window on the host this build runs on reads.
    pub const fn of_host() -> Modifier {
        if cfg!(target_os = "macos") {
            Modifier::ControlOption
        } else {
            Modifier::Super
        }
    }

    /// Whether a press holding `held` holds the desktop modifier, whatever
    /// else is down. The mouse rows read this: a drag with the desktop
    /// modifier moves or resizes a float, and the buttons carry no other
    /// modifier.
    pub const fn is_down(self, held: Held) -> bool {
        match self {
            Modifier::Super => held.logo,
            Modifier::ControlOption => held.ctrl && held.alt,
        }
    }
}

/// The modifiers a press holds, as a window toolkit names them: on macOS
/// `alt` is Option and `logo` is Command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Mods {
    /// The modifiers a window holds down for this row where its Super is
    /// `modifier`, or `None` when the row has no chord there. A row without
    /// Super, and a row holding Alt, have no Mac reading: the first is a
    /// compositor monitor chord, and the second would ask for the Option
    /// key that is already half of the desktop modifier.
    pub const fn held(self, modifier: Modifier) -> Option<Held> {
        match modifier {
            Modifier::Super => Some(Held {
                shift: self.shift,
                ctrl: self.ctrl,
                alt: self.alt,
                logo: self.super_key,
            }),
            Modifier::ControlOption => {
                if !self.super_key || self.alt {
                    return None;
                }
                Some(Held {
                    shift: self.shift,
                    ctrl: true,
                    alt: true,
                    logo: self.ctrl,
                })
            }
        }
    }
}

/// What a surface does with a press the window read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Run this row's action.
    Run(Action),
    /// Swallow the press. It is a desktop chord another surface answers,
    /// such as a monitor chord the compositor takes, so the focused pane
    /// never sees it.
    Swallow,
    /// Send the press on to the focused pane.
    Pane,
}

/// What a surface does with a press, reading the row's Super as `modifier`.
///
/// A key outside the table, and a press without the desktop modifier, go to
/// the pane. Under [`Modifier::Super`] a chord that holds Alt as well is a
/// compositor monitor bind, so it is swallowed rather than typed.
pub fn press(surface: Surface, modifier: Modifier, held: Held, key: Key) -> Press {
    if let Some(action) = BINDS
        .iter()
        .find(|bind| {
            bind.surfaces.contains(surface)
                && bind.key == key
                && bind.mods.held(modifier) == Some(held)
        })
        .map(|bind| bind.action)
    {
        return Press::Run(action);
    }
    if modifier == Modifier::Super && held.logo && held.alt {
        let without_alt = Mods {
            super_key: true,
            shift: held.shift,
            ctrl: held.ctrl,
            alt: false,
        };
        if find(surface, without_alt, key).is_some() {
            return Press::Swallow;
        }
    }
    Press::Pane
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dir;

    const CONTROL_OPTION: Held = Held {
        shift: false,
        ctrl: true,
        alt: true,
        logo: false,
    };

    #[test]
    fn a_mac_window_reads_control_and_option_for_super() {
        assert_eq!(
            Mods::SUPER.held(Modifier::ControlOption),
            Some(CONTROL_OPTION)
        );
        assert_eq!(
            Mods::SUPER_SHIFT.held(Modifier::ControlOption),
            Some(Held {
                shift: true,
                ..CONTROL_OPTION
            })
        );
        assert_eq!(
            Mods::SUPER_CTRL.held(Modifier::ControlOption),
            Some(Held {
                logo: true,
                ..CONTROL_OPTION
            })
        );
        // The monitor chords hold Alt, and Option is half of the modifier.
        assert_eq!(Mods::SUPER_ALT.held(Modifier::ControlOption), None);
        assert_eq!(Mods::CTRL_ALT.held(Modifier::ControlOption), None);
    }

    #[test]
    fn a_linux_window_reads_the_row_as_it_is_written() {
        assert_eq!(
            Mods::SUPER_SHIFT.held(Modifier::Super),
            Some(Held {
                shift: true,
                ctrl: false,
                alt: false,
                logo: true,
            })
        );
    }

    #[test]
    fn the_chords_macos_takes_reach_coderquest_on_control_and_option() {
        for (key, action) in [
            (Key::Char('h'), Action::ToggleHands),
            (Key::Char('q'), Action::Close),
            (Key::Char('w'), Action::Close),
            (Key::Space, Action::Float),
        ] {
            assert_eq!(
                press(Surface::Quest, Modifier::ControlOption, CONTROL_OPTION, key),
                Press::Run(action),
                "{key:?}"
            );
        }
    }

    #[test]
    fn a_mac_window_leaves_command_to_the_system() {
        let command = Held {
            shift: false,
            ctrl: false,
            alt: false,
            logo: true,
        };
        for key in [Key::Char('h'), Key::Char('q'), Key::Return] {
            assert_eq!(
                press(Surface::Quest, Modifier::ControlOption, command, key),
                Press::Pane,
                "{key:?}"
            );
        }
    }

    #[test]
    fn resize_keeps_a_chord_of_its_own_beside_focus() {
        let focus = press(
            Surface::Quest,
            Modifier::ControlOption,
            CONTROL_OPTION,
            Key::Arrow(Dir::Left),
        );
        let resize = press(
            Surface::Quest,
            Modifier::ControlOption,
            Held {
                logo: true,
                ..CONTROL_OPTION
            },
            Key::Arrow(Dir::Left),
        );
        assert_eq!(focus, Press::Run(Action::Focus(Dir::Left)));
        assert_eq!(resize, Press::Run(Action::Resize(Dir::Left)));
    }

    #[test]
    fn a_monitor_chord_is_swallowed_rather_than_typed() {
        let super_alt = Held {
            shift: false,
            ctrl: false,
            alt: true,
            logo: true,
        };
        assert_eq!(
            press(
                Surface::Quest,
                Modifier::Super,
                super_alt,
                Key::Arrow(Dir::Left)
            ),
            Press::Swallow
        );
        assert_eq!(
            press(Surface::Quest, Modifier::Super, super_alt, Key::Char('k')),
            Press::Pane
        );
    }

    #[test]
    fn a_key_outside_the_table_goes_to_the_pane() {
        assert_eq!(
            press(
                Surface::Quest,
                Modifier::ControlOption,
                CONTROL_OPTION,
                Key::Char('k')
            ),
            Press::Pane
        );
        assert_eq!(
            press(
                Surface::Quest,
                Modifier::Super,
                Held::default(),
                Key::Return
            ),
            Press::Pane
        );
    }
}
