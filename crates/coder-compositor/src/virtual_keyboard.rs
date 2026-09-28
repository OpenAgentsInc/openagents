//! `zwp_virtual_keyboard_v1`: the keyboard `wtype` and `os/bin/dictate-toggle`
//! type through.
//!
//! Smithay serves the protocol and sends each key straight to the focused
//! window, under the keymap the client uploaded, which is what typing text
//! needs. A chord went the same way and reached no bind table, so `wtype -M
//! logo -k t` opened no shell.
//!
//! This module reads each key the way that client's own keymap reads it,
//! runs the chord through the bind table `crate::input` runs a press
//! through, and keeps the keys that run a bind. Every other request goes on
//! to Smithay untouched, so the text a dictation types reaches the window
//! as it did before.

use std::collections::HashMap;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

use smithay::input::keyboard::{xkb, Keycode, Keysym};
use smithay::reexports::wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use smithay::reexports::wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_v1::{
    self, ZwpVirtualKeyboardV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, ObjectId};
use smithay::reexports::wayland_server::{
    delegate_dispatch, delegate_global_dispatch, Client, DataInit, Dispatch, DisplayHandle,
    Resource,
};
use smithay::wayland::virtual_keyboard::{
    VirtualKeyboardManagerGlobalData, VirtualKeyboardManagerState, VirtualKeyboardUserData,
};

use crate::binds::{self, Mods};
use crate::keys;
use crate::state::Coder;

/// The `state` a `key` request carries for a press. The protocol writes it
/// as a number rather than an enum.
const PRESSED: u32 = 1;

/// The largest keymap this compositor reads from a client, which is room
/// for every layout xkb ships several times over.
const MAX_KEYMAP_BYTES: u32 = 4 * 1024 * 1024;

/// The virtual keyboards the clients of this session made.
#[derive(Default)]
pub struct Keyboards {
    boards: HashMap<ObjectId, Board>,
}

impl Keyboards {
    /// Keeps the keymap one keyboard uploaded, in place of the one it
    /// uploaded before.
    fn remember(&mut self, id: ObjectId, keymap: &xkb::Keymap) {
        self.boards.insert(
            id,
            Board {
                state: xkb::State::new(keymap),
                kept: Vec::new(),
            },
        );
    }

    /// Forgets a keyboard whose client destroyed it.
    fn forget(&mut self, id: &ObjectId) {
        self.boards.remove(id);
    }
}

/// One virtual keyboard: the keymap its client uploaded with the modifiers
/// it last named, and the keys this compositor kept.
struct Board {
    state: xkb::State,
    /// The keys whose press ran a bind, so their release is kept too and
    /// the window never reads half a chord.
    kept: Vec<u32>,
}

impl Board {
    /// What one key reads as: the keysym this keyboard's keymap gives it,
    /// and the modifiers the keyboard holds.
    fn reading(&self, key: u32) -> (Keysym, Mods) {
        let sym = self.state.key_get_one_sym(Keycode::new(key + 8));
        let held = |name: &str| {
            self.state
                .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
        };
        (
            sym,
            Mods {
                logo: held(xkb::MOD_NAME_LOGO),
                shift: held(xkb::MOD_NAME_SHIFT),
                ctrl: held(xkb::MOD_NAME_CTRL),
                alt: held(xkb::MOD_NAME_ALT),
            },
        )
    }

    /// Keeps one key's press, so its release is kept as well.
    fn keep(&mut self, key: u32) {
        self.kept.push(key);
    }

    /// Whether one key's release belongs to a press this compositor kept,
    /// and forgets it.
    fn release(&mut self, key: u32) -> bool {
        match self.kept.iter().position(|held| *held == key) {
            Some(index) => {
                self.kept.remove(index);
                true
            }
            None => false,
        }
    }
}

/// Whether the compositor keeps one request rather than passing it on: a
/// press that runs a bind, and the release that follows it.
fn keeps(state: &mut Coder, id: &ObjectId, request: &zwp_virtual_keyboard_v1::Request) -> bool {
    match request {
        zwp_virtual_keyboard_v1::Request::Keymap { fd, size, .. } => {
            // The format is read by compiling: a keymap this compositor
            // cannot read leaves the keyboard without one here, and
            // Smithay still serves its keys.
            match read_keymap(fd, *size) {
                Ok(keymap) => state.virtual_keyboards.remember(id.clone(), &keymap),
                Err(why) => log::debug!("a virtual keyboard's keymap is not read here: {why}"),
            }
            false
        }
        zwp_virtual_keyboard_v1::Request::Modifiers {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        } => {
            if let Some(board) = state.virtual_keyboards.boards.get_mut(id) {
                board
                    .state
                    .update_mask(*mods_depressed, *mods_latched, *mods_locked, 0, 0, *group);
            }
            false
        }
        zwp_virtual_keyboard_v1::Request::Key {
            key,
            state: pressed,
            ..
        } => {
            let Some(board) = state.virtual_keyboards.boards.get_mut(id) else {
                return false;
            };
            if *pressed != PRESSED {
                return board.release(*key);
            }
            let (sym, mods) = board.reading(*key);
            if !binds::can_chord(mods) {
                return false;
            }
            let Some(named) = keys::key_of_sym(sym) else {
                return false;
            };
            let Some(action) = binds::action(&state.binds, binds::Chord { mods, key: named })
            else {
                return false;
            };
            board.keep(*key);
            log::info!("the chord runs {action:?}");
            state.run(action);
            true
        }
        _ => false,
    }
}

/// The keymap one client shared, read from the file it shared it in.
///
/// The read takes a copy of the descriptor and reads at an offset, so the
/// descriptor Smithay maps afterward is where the client left it.
fn read_keymap(fd: &OwnedFd, size: u32) -> Result<xkb::Keymap, String> {
    if size == 0 || size > MAX_KEYMAP_BYTES {
        return Err(format!("a keymap of {size} bytes is not one this reads"));
    }
    let copy = fd
        .try_clone()
        .map_err(|err| format!("the keymap's file: {err}"))?;
    let mut bytes = vec![0u8; size as usize];
    std::fs::File::from(copy)
        .read_exact_at(&mut bytes, 0)
        .map_err(|err| format!("the keymap did not read: {err}"))?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let text = String::from_utf8(bytes[..end].to_vec())
        .map_err(|_| "the keymap is not text".to_string())?;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    xkb::Keymap::new_from_string(
        &context,
        text,
        xkb::KEYMAP_FORMAT_TEXT_V1,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or_else(|| "the keymap did not compile".to_string())
}

impl Dispatch<ZwpVirtualKeyboardV1, VirtualKeyboardUserData<Coder>> for Coder {
    fn request(
        state: &mut Coder,
        client: &Client,
        keyboard: &ZwpVirtualKeyboardV1,
        request: zwp_virtual_keyboard_v1::Request,
        data: &VirtualKeyboardUserData<Coder>,
        handle: &DisplayHandle,
        init: &mut DataInit<'_, Coder>,
    ) {
        if keeps(state, &keyboard.id(), &request) {
            return;
        }
        <VirtualKeyboardManagerState as Dispatch<
            ZwpVirtualKeyboardV1,
            VirtualKeyboardUserData<Coder>,
            Coder,
        >>::request(state, client, keyboard, request, data, handle, init);
    }

    fn destroyed(
        state: &mut Coder,
        client: ClientId,
        keyboard: &ZwpVirtualKeyboardV1,
        data: &VirtualKeyboardUserData<Coder>,
    ) {
        state.virtual_keyboards.forget(&keyboard.id());
        <VirtualKeyboardManagerState as Dispatch<
            ZwpVirtualKeyboardV1,
            VirtualKeyboardUserData<Coder>,
            Coder,
        >>::destroyed(state, client, keyboard, data);
    }
}

// The manager itself is Smithay's: it makes the keyboards and holds the
// global. Only the keyboard's own requests come through this module.
delegate_global_dispatch!(Coder: [ZwpVirtualKeyboardManagerV1: VirtualKeyboardManagerGlobalData] => VirtualKeyboardManagerState);
delegate_dispatch!(Coder: [ZwpVirtualKeyboardManagerV1: ()] => VirtualKeyboardManagerState);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binds::{Action, Key};

    /// The evdev code of the T key, which is the code `wtype -k t` sends.
    const EVDEV_T: u32 = 20;
    /// The evdev code of the H key.
    const EVDEV_H: u32 = 35;
    /// The evdev code of the X key, which no row of the bind table holds.
    const EVDEV_X: u32 = 45;

    /// A keyboard holding the US layout, which is the keymap `wtype`
    /// uploads for a key it names.
    fn board() -> Board {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("the layout compiles");
        Board {
            state: xkb::State::new(&keymap),
            kept: Vec::new(),
        }
    }

    /// The mask one modifier name takes in this board's keymap.
    fn mask(board: &Board, name: &str) -> u32 {
        1 << board.state.get_keymap().mod_get_index(name)
    }

    /// What the bind table answers for one key of one keyboard.
    fn action_of(board: &Board, key: u32) -> Option<Action> {
        let (sym, mods) = board.reading(key);
        if !binds::can_chord(mods) {
            return None;
        }
        let named = keys::key_of_sym(sym)?;
        binds::action(&binds::table(None), binds::Chord { mods, key: named })
    }

    #[test]
    fn a_super_chord_from_a_virtual_keyboard_runs_its_bind() {
        let mut board = board();
        let logo = mask(&board, xkb::MOD_NAME_LOGO);
        board.state.update_mask(logo, 0, 0, 0, 0, 0);
        let (sym, mods) = board.reading(EVDEV_T);
        assert!(mods.logo, "the keyboard holds Super");
        assert_eq!(keys::key_of_sym(sym), Some(Key::Letter('t')));
        assert_eq!(action_of(&board, EVDEV_T), Some(Action::OpenShell));
    }

    #[test]
    fn a_key_with_no_modifier_held_goes_to_the_window() {
        let board = board();
        let (sym, mods) = board.reading(EVDEV_T);
        assert!(!mods.logo && !mods.ctrl, "a dictation holds no modifier");
        assert_eq!(keys::key_of_sym(sym), Some(Key::Letter('t')));
        assert_eq!(
            action_of(&board, EVDEV_T),
            None,
            "the text a dictation types is not a chord"
        );
    }

    #[test]
    fn a_super_chord_the_table_does_not_hold_goes_to_the_window() {
        let mut board = board();
        let logo = mask(&board, xkb::MOD_NAME_LOGO);
        board.state.update_mask(logo, 0, 0, 0, 0, 0);
        assert_eq!(action_of(&board, EVDEV_X), None, "no row holds Super+X");
        assert_eq!(
            action_of(&board, EVDEV_H),
            Some(Action::ToggleHands),
            "Super+H toggles hands"
        );
    }

    #[test]
    fn the_release_of_a_press_the_compositor_kept_is_kept_too() {
        let mut board = board();
        assert!(!board.release(EVDEV_T), "nothing was kept yet");
        board.keep(EVDEV_T);
        assert!(board.release(EVDEV_T), "the release follows its press");
        assert!(!board.release(EVDEV_T), "one press is released once");
        assert!(
            !board.release(EVDEV_H),
            "a key the compositor never kept reaches the window"
        );
    }

    #[test]
    fn a_keymap_that_is_not_one_is_not_read() {
        let path = std::env::temp_dir().join(format!("coder-keymap-{}", std::process::id()));
        std::fs::write(&path, "this is not a keymap").expect("the file");
        let file = std::fs::File::open(&path).expect("the file opens");
        let fd = OwnedFd::from(file);
        assert!(read_keymap(&fd, 20).is_err());
        assert!(read_keymap(&fd, 0).is_err(), "an empty keymap is not read");
        assert!(
            read_keymap(&fd, MAX_KEYMAP_BYTES + 1).is_err(),
            "a keymap past the bound is not read"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_keymap_a_client_shares_reads_back() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("the layout compiles");
        let mut text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        text.push('\0');
        let path = std::env::temp_dir().join(format!("coder-keymap-us-{}", std::process::id()));
        std::fs::write(&path, &text).expect("the file");
        let file = std::fs::File::open(&path).expect("the file opens");
        let fd = OwnedFd::from(file);
        let read = read_keymap(&fd, text.len() as u32).expect("the keymap reads");
        let board = Board {
            state: xkb::State::new(&read),
            kept: Vec::new(),
        };
        assert_eq!(
            keys::key_of_sym(board.reading(EVDEV_T).0),
            Some(Key::Letter('t')),
            "the key reads under the keymap the client shared"
        );
        let _ = std::fs::remove_file(&path);
    }
}
