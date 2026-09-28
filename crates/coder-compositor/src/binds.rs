//! The bind table: every chord the compositor answers and what it does.
//!
//! The rows mirror the binds `os/modules/coderos/desktop.nix` writes for
//! tiling, desks, floats, fullscreen, monitors, and the two `exec` rows, so
//! a person who learned the Hyprland session learns nothing new. The
//! launcher chords, Super+C for the camera and Super+Shift+D for the deck
//! among them, are read from `crates/coder-binds`, the table `desktop.nix`
//! writes its `bind` lines from: each launcher row there names the
//! `coderos.desktop` option that gates it, and the compositor answers the
//! rows whose option the host turned on, which the session hands it in
//! `CODER_COMPOSITOR_LAUNCHERS`.
//!
//! The layout rows live in this crate for now. They move to the one table
//! the compositor and Coder Desktop both read, `crates/coder-binds`, once
//! it holds layout rows. The window rules
//! that sit beside it are already read from that crate, in `crate::rules`.

use coder_wm::Dir;

/// The modifiers a chord holds. Super is the desktop's modifier. A Mac
/// window reads Control and Option for it instead, which
/// `crates/coder-binds/src/modifier.rs` holds and this compositor, which
/// never runs on a Mac, never needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    /// The Super key, which every chord in this table holds but the two
    /// monitor chords Hyprland binds to Ctrl+Alt+Tab.
    pub logo: bool,
    /// The Shift key.
    pub shift: bool,
    /// The Ctrl key.
    pub ctrl: bool,
    /// The Alt key.
    pub alt: bool,
}

impl Mods {
    /// Super alone.
    pub const fn logo() -> Mods {
        Mods {
            logo: true,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    /// Super with another modifier held.
    pub const fn with(self, shift: bool, ctrl: bool, alt: bool) -> Mods {
        Mods {
            logo: self.logo,
            shift,
            ctrl,
            alt,
        }
    }
}

/// The key half of a chord, named by what the key prints rather than by a
/// keysym, so the table reads the way `desktop.nix` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// The Return key.
    Return,
    /// A letter key, held as its lowercase character.
    Letter(char),
    /// The space bar.
    Space,
    /// One of the four arrow keys.
    Arrow(Dir),
    /// A digit key, 1 through 9.
    Digit(u8),
    /// The Tab key.
    Tab,
}

/// One chord: the modifiers held and the key pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The modifiers the chord holds.
    pub mods: Mods,
    /// The key the chord presses.
    pub key: Key,
}

/// What a chord does. The layout actions go to the layout crate, the exec
/// actions start a program, and `Exit` ends the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Open a Coder in a new tile, the Super+Return row.
    OpenCoder,
    /// Open a shell in a new tile, the Super+T row.
    OpenShell,
    /// Ask the focused window to close.
    Close,
    /// Move the focus to the next tile in a direction.
    MoveFocus(Dir),
    /// Move the focused tile in a direction.
    MoveWindow(Dir),
    /// Resize the focused tile in a direction, by the pixels
    /// `desktop.nix` resizes by.
    Resize(Dir),
    /// Show a desk, 1 through 9.
    Desk(u8),
    /// Send the focused window to a desk, 1 through 9.
    MoveToDesk(u8),
    /// Flip the focused pair's split axis.
    ToggleSplit,
    /// Float the focused window, or put it back in the layout.
    ToggleFloat,
    /// Fill the screen with the focused window.
    Fullscreen,
    /// Fill the layout's area with the focused window, keeping the gaps.
    Maximize,
    /// Give the next screen the focus, Ctrl+Alt+Tab.
    FocusScreenNext,
    /// Give the previous screen the focus, Ctrl+Alt+Shift+Tab.
    FocusScreenPrevious,
    /// Give the screen in a direction the focus.
    FocusScreen(Dir),
    /// Send the focused window to the screen in a direction.
    MoveWindowToScreen(Dir),
    /// Send the focused screen's desk, with every window on it, to the
    /// screen in a direction.
    MoveDeskToScreen(Dir),
    /// Run a command on the host: a launcher row of `crates/coder-binds`,
    /// such as `coder-deck-open` for Super+Shift+D.
    Exec(&'static str),
    /// Hands on, or off: the Super+H row of `crates/coder-binds`, which
    /// the host grants with `coderos.desktop.hands`.
    ToggleHands,
    /// End the session.
    Exit,
}

/// The pixels a resize chord moves an edge by, which is the `resizeactive`
/// argument in `desktop.nix`.
pub const RESIZE_STEP: i32 = 40;

/// The four arrow directions, in the order `desktop.nix` binds them.
const ARROWS: [Dir; 4] = [Dir::Left, Dir::Right, Dir::Up, Dir::Down];

/// Every chord the compositor answers, in the order `desktop.nix` writes
/// them: the layout rows below, then the launcher rows of
/// `crates/coder-binds` whose option `granted` names, such as `deck` for
/// `coderos.desktop.deck`. With no list at all every launcher row joins,
/// which is what a checkout run by hand answers; an empty list leaves
/// every launcher chord to the client, the way a Hyprland session with
/// every option off writes no `bind` line for them.
pub fn table(granted: Option<&[String]>) -> Vec<(Chord, Action)> {
    let mut rows = layout_rows();
    rows.extend(launcher_rows(granted));
    rows
}

/// The launcher rows the compositor answers: every `exec` row of
/// `coder_binds::BINDS` for the compositor surface whose option is in
/// `granted`, or every one when `granted` is `None`, and the Super+H row
/// under the same rule, since `coderos.desktop.hands` gates it the way
/// `coderos.desktop.deck` gates the deck.
fn launcher_rows(granted: Option<&[String]>) -> Vec<(Chord, Action)> {
    coder_binds::BINDS
        .iter()
        .filter(|bind| bind.surfaces.contains(coder_binds::Surface::Compositor))
        .filter_map(|bind| {
            let action = match bind.action {
                coder_binds::Action::Exec { command, .. } => Action::Exec(command),
                coder_binds::Action::ToggleHands => Action::ToggleHands,
                _ => return None,
            };
            let allowed = match (bind.option, granted) {
                (None, _) | (Some(_), None) => true,
                (Some(option), Some(list)) => list.iter().any(|name| name == option),
            };
            if !allowed {
                return None;
            }
            let key = key_from(bind.key)?;
            Some((chord(mods_from(bind.mods), key), action))
        })
        .collect()
}

/// The modifiers of a row in the shared table, as this table holds them.
fn mods_from(mods: coder_binds::Mods) -> Mods {
    Mods {
        logo: mods.super_key,
        shift: mods.shift,
        ctrl: mods.ctrl,
        alt: mods.alt,
    }
}

/// The key of a row in the shared table, or nothing for a mouse button,
/// which no launcher row carries.
fn key_from(key: coder_binds::Key) -> Option<Key> {
    Some(match key {
        coder_binds::Key::Return => Key::Return,
        coder_binds::Key::Space => Key::Space,
        coder_binds::Key::Tab => Key::Tab,
        coder_binds::Key::Char(letter) => Key::Letter(letter),
        coder_binds::Key::Digit(digit) => Key::Digit(digit),
        coder_binds::Key::Arrow(dir) => Key::Arrow(match dir {
            coder_binds::Dir::Left => Dir::Left,
            coder_binds::Dir::Right => Dir::Right,
            coder_binds::Dir::Up => Dir::Up,
            coder_binds::Dir::Down => Dir::Down,
        }),
        coder_binds::Key::Mouse(_) => return None,
    })
}

/// The layout rows, in the order `desktop.nix` writes them.
fn layout_rows() -> Vec<(Chord, Action)> {
    let logo = Mods::logo();
    let mut rows = vec![
        (chord(logo, Key::Return), Action::OpenCoder),
        (chord(logo, Key::Letter('t')), Action::OpenShell),
        (chord(logo, Key::Letter('w')), Action::Close),
        (chord(logo, Key::Letter('q')), Action::Close),
        (chord(logo, Key::Letter('j')), Action::ToggleSplit),
        (chord(logo, Key::Space), Action::ToggleFloat),
        (chord(logo, Key::Letter('d')), Action::ToggleFloat),
        (chord(logo, Key::Letter('f')), Action::Fullscreen),
        (
            chord(logo.with(false, true, false), Key::Letter('f')),
            Action::Maximize,
        ),
        (
            chord(logo.with(true, false, false), Key::Letter('e')),
            Action::Exit,
        ),
    ];
    for dir in ARROWS {
        rows.push((chord(logo, Key::Arrow(dir)), Action::MoveFocus(dir)));
        rows.push((
            chord(logo.with(true, false, false), Key::Arrow(dir)),
            Action::MoveWindow(dir),
        ));
        rows.push((
            chord(logo.with(false, true, false), Key::Arrow(dir)),
            Action::Resize(dir),
        ));
    }
    let ctrl_alt = Mods {
        logo: false,
        shift: false,
        ctrl: true,
        alt: true,
    };
    rows.push((chord(ctrl_alt, Key::Tab), Action::FocusScreenNext));
    rows.push((
        chord(ctrl_alt.with(true, true, true), Key::Tab),
        Action::FocusScreenPrevious,
    ));
    for dir in ARROWS {
        rows.push((
            chord(logo.with(false, false, true), Key::Arrow(dir)),
            Action::FocusScreen(dir),
        ));
        rows.push((
            chord(logo.with(true, false, true), Key::Arrow(dir)),
            Action::MoveWindowToScreen(dir),
        ));
        rows.push((
            chord(logo.with(false, true, true), Key::Arrow(dir)),
            Action::MoveDeskToScreen(dir),
        ));
    }
    for desk in 1..=9u8 {
        rows.push((chord(logo, Key::Digit(desk)), Action::Desk(desk)));
        rows.push((
            chord(logo.with(true, false, false), Key::Digit(desk)),
            Action::MoveToDesk(desk),
        ));
    }
    rows
}

const fn chord(mods: Mods, key: Key) -> Chord {
    Chord { mods, key }
}

/// Whether a set of held modifiers can start a chord: Super, or Ctrl with
/// Alt for the two monitor rows. A press that holds neither reaches the
/// client.
pub fn can_chord(mods: Mods) -> bool {
    mods.logo || (mods.ctrl && mods.alt)
}

/// What one chord does, or nothing when the table holds no row for it.
pub fn action(rows: &[(Chord, Action)], chord: Chord) -> Option<Action> {
    rows.iter()
        .find(|(row, _)| *row == chord)
        .map(|(_, action)| *action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_for(mods: Mods, key: Key) -> Option<Action> {
        action(&table(None), Chord { mods, key })
    }

    fn granted(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// The eight launcher chords and the command each one runs, in the
    /// order the shared table holds them.
    const LAUNCHERS: [(Mods, char, &str); 8] = [
        (Mods::logo(), 'v', "dictate-toggle"),
        (Mods::logo(), 'p', "presentation-mode toggle"),
        (Mods::logo(), 'b', "coder-browser"),
        (
            Mods::logo().with(true, false, false),
            'd',
            "coder-deck-open",
        ),
        (Mods::logo(), 'z', "coder-zoom"),
        (Mods::logo(), 'a', "android-emulator"),
        (Mods::logo(), 'g', "coder-battlenet"),
        (Mods::logo(), 'c', "camera-toggle"),
    ];

    #[test]
    fn a_checkout_run_by_hand_answers_every_launcher_chord() {
        for (mods, letter, command) in LAUNCHERS {
            assert_eq!(
                action_for(mods, Key::Letter(letter)),
                Some(Action::Exec(command)),
                "Super+{letter} runs {command}"
            );
        }
    }

    #[test]
    fn the_launchers_the_host_granted_join_the_table_and_the_others_stay_out() {
        let rows = table(Some(&granted(&["deck", "camera"])));
        let deck = Chord {
            mods: Mods::logo().with(true, false, false),
            key: Key::Letter('d'),
        };
        assert_eq!(action(&rows, deck), Some(Action::Exec("coder-deck-open")));
        let camera = Chord {
            mods: Mods::logo(),
            key: Key::Letter('c'),
        };
        assert_eq!(action(&rows, camera), Some(Action::Exec("camera-toggle")));
        for letter in ['v', 'p', 'b', 'z', 'a', 'g'] {
            let chord = Chord {
                mods: Mods::logo(),
                key: Key::Letter(letter),
            };
            assert_eq!(action(&rows, chord), None, "Super+{letter} is not granted");
        }
        // Super+D floats the window whether or not the deck is granted.
        let float = Chord {
            mods: Mods::logo(),
            key: Key::Letter('d'),
        };
        assert_eq!(action(&rows, float), Some(Action::ToggleFloat));
    }

    #[test]
    fn a_host_that_granted_no_launcher_leaves_their_chords_to_the_client() {
        let rows = table(Some(&[]));
        for (mods, letter, _) in LAUNCHERS {
            assert_eq!(
                action(
                    &rows,
                    Chord {
                        mods,
                        key: Key::Letter(letter)
                    }
                ),
                None
            );
        }
        assert_eq!(rows.len(), layout_rows().len());
    }

    #[test]
    fn super_h_toggles_hands_when_the_host_grants_them() {
        let super_h = Chord {
            mods: Mods::logo(),
            key: Key::Letter('h'),
        };
        assert_eq!(action(&table(None), super_h), Some(Action::ToggleHands));
        assert_eq!(
            action(&table(Some(&granted(&["hands"]))), super_h),
            Some(Action::ToggleHands)
        );
        assert_eq!(action(&table(Some(&granted(&["camera"]))), super_h), None);
        assert_eq!(action(&table(Some(&[])), super_h), None);
    }

    #[test]
    fn the_two_exec_rows_are_super_return_and_super_t() {
        assert_eq!(
            action_for(Mods::logo(), Key::Return),
            Some(Action::OpenCoder)
        );
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('t')),
            Some(Action::OpenShell)
        );
    }

    #[test]
    fn both_close_keys_close() {
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('w')),
            Some(Action::Close)
        );
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('q')),
            Some(Action::Close)
        );
    }

    #[test]
    fn the_arrows_move_the_focus_the_tile_and_the_edge() {
        for dir in ARROWS {
            assert_eq!(
                action_for(Mods::logo(), Key::Arrow(dir)),
                Some(Action::MoveFocus(dir))
            );
            assert_eq!(
                action_for(Mods::logo().with(true, false, false), Key::Arrow(dir)),
                Some(Action::MoveWindow(dir))
            );
            assert_eq!(
                action_for(Mods::logo().with(false, true, false), Key::Arrow(dir)),
                Some(Action::Resize(dir))
            );
        }
    }

    #[test]
    fn the_nine_digits_show_a_desk_and_send_a_window_to_one() {
        for desk in 1..=9u8 {
            assert_eq!(
                action_for(Mods::logo(), Key::Digit(desk)),
                Some(Action::Desk(desk))
            );
            assert_eq!(
                action_for(Mods::logo().with(true, false, false), Key::Digit(desk)),
                Some(Action::MoveToDesk(desk))
            );
        }
    }

    #[test]
    fn the_layout_chords_flip_float_and_fill() {
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('j')),
            Some(Action::ToggleSplit)
        );
        assert_eq!(
            action_for(Mods::logo(), Key::Space),
            Some(Action::ToggleFloat)
        );
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('d')),
            Some(Action::ToggleFloat)
        );
        assert_eq!(
            action_for(Mods::logo(), Key::Letter('f')),
            Some(Action::Fullscreen)
        );
        assert_eq!(
            action_for(Mods::logo().with(false, true, false), Key::Letter('f')),
            Some(Action::Maximize)
        );
        assert_eq!(
            action_for(Mods::logo().with(true, false, false), Key::Letter('e')),
            Some(Action::Exit)
        );
    }

    #[test]
    fn the_monitor_chords_are_the_rows_desktop_nix_binds() {
        let ctrl_alt = Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        };
        assert_eq!(
            action_for(ctrl_alt, Key::Tab),
            Some(Action::FocusScreenNext)
        );
        assert_eq!(
            action_for(
                Mods {
                    shift: true,
                    ..ctrl_alt
                },
                Key::Tab
            ),
            Some(Action::FocusScreenPrevious)
        );
        for dir in ARROWS {
            assert_eq!(
                action_for(Mods::logo().with(false, false, true), Key::Arrow(dir)),
                Some(Action::FocusScreen(dir))
            );
            assert_eq!(
                action_for(Mods::logo().with(true, false, true), Key::Arrow(dir)),
                Some(Action::MoveWindowToScreen(dir))
            );
            assert_eq!(
                action_for(Mods::logo().with(false, true, true), Key::Arrow(dir)),
                Some(Action::MoveDeskToScreen(dir))
            );
        }
    }

    #[test]
    fn super_or_ctrl_with_alt_starts_a_chord_and_nothing_else_does() {
        assert!(can_chord(Mods::logo()));
        assert!(can_chord(Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        }));
        assert!(!can_chord(Mods {
            ctrl: true,
            ..Mods::default()
        }));
        assert!(!can_chord(Mods::default()));
    }

    #[test]
    fn a_key_the_table_does_not_hold_does_nothing() {
        assert_eq!(action_for(Mods::logo(), Key::Letter('x')), None);
        assert_eq!(action_for(Mods::default(), Key::Return), None);
    }

    #[test]
    fn no_chord_carries_two_rows() {
        let rows = table(None);
        for (index, (chord, _)) in rows.iter().enumerate() {
            let again = rows.iter().skip(index + 1).any(|(other, _)| other == chord);
            assert!(!again, "{chord:?} carries two rows");
        }
    }
}
