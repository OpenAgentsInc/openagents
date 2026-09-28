//! The Coder compositor: a Wayland compositor for a CoderOS desktop.
//!
//! Run it inside a session and it opens a window there, and that window is
//! a compositor. Run it from a TTY and it takes the seat, drives the
//! monitors through DRM and KMS, and reads the keyboard and the pointer
//! through `libinput`. `--backend` names either one; `backend.rs` says how
//! a start with no flag picks. Clients that connect to the socket it
//! announces as `WAYLAND_DISPLAY` tile inside it, the chords the bind table
//! holds work there, and the desk protocol answers on the socket it
//! announces as `CODER_DESK_SOCKET`.
//!
//! Beyond `xdg-shell` it answers `wlr-layer-shell`, `wlr-screencopy`, the
//! clipboard, the primary selection, drag and drop, server-side
//! decorations, text input, idle notification, fractional scale with the
//! viewporter, and explicit sync on the hardware backend. It starts
//! Xwayland the first time it starts a program, lays X11 windows out with
//! the rest, and announces the server to every program it starts as
//! `DISPLAY`.
//!
//! The window rules are the rows `crates/coder-binds` holds, the same rows
//! `desktop.nix` writes for Hyprland, read when a window maps and when its
//! app-id or title changes.
//!
//! Hand tracking reads the CoderOS camera daemon's landmarks in `hands.rs`
//! and draws the hand over every window in `hands_overlay.rs`. Super+H,
//! or a host grant that names `hands`, turns it on.
//!
//! The hardware backend's latency waits for a run on the hardware; the
//! nested backend's was measured on 2026-09-16.

mod backend;
mod binds;
mod closing;
mod connectors;
mod constraints;
mod cursor;
mod desk_server;
mod drag;
mod drive;
mod exec;
mod extras;
mod focus;
mod handlers;
mod hands;
mod hands_overlay;
mod idle;
mod input;
mod keys;
mod layers;
mod layout;
mod nested;
mod outputs;
mod render;
mod rules;
mod screencopy;
mod screens;
mod selection;
mod stacking;
mod state;
mod udev;
mod virtual_keyboard;
mod xwayland;

use backend::{Backend, Command};

/// What `--help` prints.
const USAGE: &str = "\
coder-compositor: the Coder Wayland compositor.

Usage:
  coder-compositor                   Pick the backend from the environment.
  coder-compositor --backend winit   Open a compositor in a window of this session.
  coder-compositor --backend udev    Take this TTY's seat and drive its monitors.
  coder-compositor --help            Print this text.
  coder-compositor --version         Print the version.

With no --backend, a process that finds WAYLAND_DISPLAY or DISPLAY in its
environment opens a window, and one that finds neither takes the seat.

The compositor announces its Wayland socket as WAYLAND_DISPLAY, its desk
protocol socket as CODER_DESK_SOCKET, and its Xwayland server as DISPLAY to
every program it starts. On a TTY, Ctrl+Alt with a function key switches
the virtual terminal, and Super+Shift+E ends the compositor.";

fn main() {
    let filter = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(filter).init();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let named = match backend::parse(&arguments) {
        Ok(Command::Run(named)) => named,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return;
        }
        Ok(Command::Version) => {
            println!("coder-compositor {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Err(err) => {
            eprintln!("coder-compositor: {err}");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let chosen = backend::choose(named, |name| std::env::var(name).ok());
    log::info!("the compositor starts on the {} backend", chosen.name());
    let ran = match chosen {
        Backend::Winit => nested::run(),
        Backend::Udev => udev::run(),
    };
    if let Err(err) = ran {
        eprintln!("coder-compositor: {err}");
        std::process::exit(1);
    }
}
