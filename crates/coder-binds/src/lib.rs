//! The one table of desktop chords.
//!
//! `BINDS` holds a row per chord: modifiers, key, action, and the surfaces
//! the chord applies to. The surfaces are the CoderOS compositor, CoderQuest
//! in its window, and Coder Desktop in its window. A row spells the desktop
//! modifier `Super`, and the reader decides which physical modifier that is:
//! [`modifier`] holds that decision and says why a Mac window reads Control
//! and Option rather than Command.
//!
//! The readers render the table rather than copy it: `hyprland_lines`
//! renders the compositor rows as the `bind`, `binde`, and `bindm` lines
//! `os/modules/coderos/desktop.nix` writes for Hyprland; a window that
//! handles its own chords calls [`find`] with [`Surface::Quest`] or
//! [`Surface::Desktop`]; and [`appendix_markdown`] renders the table as
//! Markdown for a document that lists the chords.
//!
//! The window rules sit beside the chords in [`rules`]: `RULES` holds a
//! row per rule, [`hyprland_rule_lines`] renders them as the `windowrule`
//! lines `desktop.nix` writes, and [`matching`] is what the compositor
//! applies to a window when it maps and when its title changes.

pub mod modifier;
pub mod rules;

pub use modifier::{Held, Modifier, Press, press};
pub use rules::{
    Case, EMULATOR_CLASS, Effects, Field, Match, Pattern, RULES, Rule, hyprland_rule,
    hyprland_rule_lines, matching,
};

/// A direction a focus, move, or resize chord points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    fn hypr(self) -> &'static str {
        match self {
            Dir::Left => "l",
            Dir::Right => "r",
            Dir::Up => "u",
            Dir::Down => "d",
        }
    }
}

/// The modifiers a chord holds. Super is the compositor's modifier; a macOS
/// reader reports Command for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub super_key: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Mods {
    pub const SUPER: Mods = Mods {
        super_key: true,
        shift: false,
        ctrl: false,
        alt: false,
    };
    pub const SUPER_SHIFT: Mods = Mods {
        super_key: true,
        shift: true,
        ctrl: false,
        alt: false,
    };
    pub const SUPER_CTRL: Mods = Mods {
        super_key: true,
        shift: false,
        ctrl: true,
        alt: false,
    };
    pub const SUPER_ALT: Mods = Mods {
        super_key: true,
        shift: false,
        ctrl: false,
        alt: true,
    };
    pub const SUPER_SHIFT_ALT: Mods = Mods {
        super_key: true,
        shift: true,
        ctrl: false,
        alt: true,
    };
    pub const SUPER_CTRL_ALT: Mods = Mods {
        super_key: true,
        shift: false,
        ctrl: true,
        alt: true,
    };
    pub const CTRL_ALT: Mods = Mods {
        super_key: false,
        shift: false,
        ctrl: true,
        alt: true,
    };
    pub const CTRL_ALT_SHIFT: Mods = Mods {
        super_key: false,
        shift: true,
        ctrl: true,
        alt: true,
    };

    /// The modifier field of a Hyprland `bind` line, in the order the file
    /// spells it: `SUPER`, `CTRL`, `ALT`, `SHIFT`.
    fn hypr(self) -> String {
        let mut parts = Vec::new();
        if self.super_key {
            parts.push("SUPER");
        }
        if self.ctrl {
            parts.push("CTRL");
        }
        if self.alt {
            parts.push("ALT");
        }
        if self.shift {
            parts.push("SHIFT");
        }
        parts.join(" ")
    }

    /// The modifier field as a macOS reader says it: Command for Super and
    /// Option for Alt.
    fn mac(self) -> String {
        let mut parts = Vec::new();
        if self.super_key {
            parts.push("Command");
        }
        if self.ctrl {
            parts.push("Control");
        }
        if self.alt {
            parts.push("Option");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.join("+")
    }
}

/// The key half of a chord. Letters and digits are the unshifted key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Return,
    Space,
    Tab,
    /// A lowercase letter, `a` through `z`.
    Char(char),
    /// A digit key, `1` through `9` in the table.
    Digit(u8),
    Arrow(Dir),
    /// A mouse button by its evdev code: `272` is left, `273` is right.
    Mouse(u16),
}

impl Key {
    fn hypr(self) -> String {
        match self {
            Key::Return => "RETURN".to_string(),
            Key::Space => "SPACE".to_string(),
            Key::Tab => "TAB".to_string(),
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
            Key::Digit(d) => d.to_string(),
            Key::Arrow(dir) => match dir {
                Dir::Left => "left".to_string(),
                Dir::Right => "right".to_string(),
                Dir::Up => "up".to_string(),
                Dir::Down => "down".to_string(),
            },
            Key::Mouse(button) => format!("mouse:{button}"),
        }
    }

    /// The key's `sendshortcut` spelling inside a `coder-chord` argument:
    /// `Return`, `space`, `Left`, or the lowercase letter.
    fn chord(self) -> String {
        match self {
            Key::Return => "Return".to_string(),
            Key::Space => "space".to_string(),
            Key::Tab => "tab".to_string(),
            Key::Char(c) => c.to_string(),
            Key::Digit(d) => d.to_string(),
            Key::Arrow(dir) => format!("{dir:?}"),
            Key::Mouse(button) => format!("mouse:{button}"),
        }
    }

    /// The key name a macOS reader says: `Return`, `W`, `arrows` for the
    /// arrow set handled by [`chords_label`].
    fn mac(self) -> String {
        match self {
            Key::Return => "Return".to_string(),
            Key::Space => "Space".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
            Key::Digit(d) => d.to_string(),
            Key::Arrow(dir) => format!("{dir:?}"),
            Key::Mouse(button) => format!("mouse {button}"),
        }
    }
}

/// A chord: the modifiers and the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

const fn chord(mods: Mods, key: Key) -> Chord {
    Chord { mods, key }
}

/// Which line shape Hyprland writes for a bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `bind`: fires once a press.
    Once,
    /// `binde`: repeats while held, for the resize chords.
    Repeat,
    /// `bindm`: a mouse drag with the modifier held.
    Mouse,
}

/// Which surfaces a chord applies to. A row with several surfaces means the
/// same chord on each; a surface whose chord differs carries its own row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surfaces(u8);

impl Surfaces {
    /// The CoderOS compositor: Hyprland today, `coder-compositor` later.
    pub const COMPOSITOR: Surfaces = Surfaces(1);
    /// CoderQuest's window.
    pub const QUEST: Surfaces = Surfaces(2);
    /// Coder Desktop's window.
    pub const DESKTOP: Surfaces = Surfaces(4);
    /// Every surface.
    pub const ALL: Surfaces = Surfaces(7);

    pub const fn or(self, other: Surfaces) -> Surfaces {
        Surfaces(self.0 | other.0)
    }

    pub const fn contains(self, surface: Surface) -> bool {
        self.0 & (1 << surface as u8) != 0
    }
}

/// A surface the table covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Compositor = 0,
    Quest = 1,
    Desktop = 2,
}

/// What a chord does. The actions are the desk protocol's and the layout
/// crate's; the launchers are `exec` rows that name the command the
/// compositor runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A new Coder: `exec ${openCoder}` on the compositor, a `coder-terminal`
    /// tile in CoderQuest, a pane in Coder Desktop.
    OpenCoder,
    /// A bare shell: `exec coder-pane --shell` on the compositor, a `$SHELL`
    /// tile in CoderQuest.
    OpenShell,
    /// Close the focused window or tile. The compositor runs `coder-close`,
    /// which asks the session before closing a window with work in flight.
    Close,
    /// Move focus between tiles.
    Focus(Dir),
    /// Move the tile inside the layout.
    Move(Dir),
    /// Resize the tile, 40 pixels a press, repeating while held.
    Resize(Dir),
    /// Switch to desk `n`, `1` through `9`.
    Desk(u8),
    /// Move the focused window to desk `n`.
    MoveToDesk(u8),
    /// Focus a monitor: `Cycle` walks them in the order the compositor
    /// holds them, `At` aims at the monitor in a direction.
    FocusScreen(Screen),
    /// Send the focused window to the monitor in a direction.
    MoveToScreen(Dir),
    /// Send the whole desk to the monitor in a direction.
    DeskToScreen(Dir),
    /// Flip the split the next window opens on, dwindle's `togglesplit`.
    FlipSplit,
    /// Toggle the focused window between floating and tiled.
    Float,
    /// Take the whole screen: Hyprland `fullscreen 0`.
    Fullscreen,
    /// Fill the tiling area and leave the layout in place: Hyprland
    /// `fullscreen 1`, what a monocle mode is.
    Maximize,
    /// End the session on the compositor, or quit the window in-window.
    Exit,
    /// Drag a floating window with the left button: `bindm` `movewindow`.
    DragMove,
    /// Resize a floating window with the right button: `bindm`
    /// `resizewindow`.
    DragResize,
    /// Hands on, or off: the compositor's reader of the camera daemon's
    /// landmarks, which `coderos.desktop.hands` grants, and CoderQuest's
    /// own tracker in its window. Hyprland gets no line for it.
    ToggleHands,
    /// Run a command on the host. `doc` is the one-word name the audit
    /// table gives it.
    Exec {
        command: &'static str,
        doc: &'static str,
    },
}

/// The monitor a `FocusScreen` chord reaches for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The next or previous monitor in the order the compositor holds them.
    Cycle(i8),
    /// The monitor in a direction.
    At(Dir),
}

impl Action {
    /// The Hyprland dispatcher text after `mods, key`, or `None` when the
    /// action has no compositor form.
    fn dispatcher(self) -> Option<String> {
        let text = match self {
            Action::OpenCoder => ", exec, ${openCoder}".to_string(),
            Action::OpenShell => ", exec, ${cfg.panePackage}/bin/coder-pane --shell".to_string(),
            Action::Close => ", exec, ${closeWindow}/bin/coder-close".to_string(),
            Action::Focus(dir) => format!(", movefocus, {}", dir.hypr()),
            Action::Move(dir) => format!(", movewindow, {}", dir.hypr()),
            Action::MoveToScreen(dir) => format!(", movewindow, mon:{}", dir.hypr()),
            Action::Resize(dir) => {
                let delta = match dir {
                    Dir::Left => "-40 0",
                    Dir::Right => "40 0",
                    Dir::Up => "0 -40",
                    Dir::Down => "0 40",
                };
                format!(", resizeactive, {delta}")
            }
            Action::Desk(desk) => format!(", workspace, {desk}"),
            Action::MoveToDesk(desk) => format!(", movetoworkspace, {desk}"),
            Action::FocusScreen(Screen::Cycle(step)) => {
                format!(", focusmonitor, {step:+}")
            }
            Action::FocusScreen(Screen::At(dir)) => format!(", focusmonitor, {}", dir.hypr()),
            Action::DeskToScreen(dir) => {
                format!(", movecurrentworkspacetomonitor, {}", dir.hypr())
            }
            Action::FlipSplit => ", layoutmsg, togglesplit".to_string(),
            Action::Float => ", togglefloating".to_string(),
            Action::Fullscreen => ", fullscreen, 0".to_string(),
            Action::Maximize => ", fullscreen, 1".to_string(),
            Action::Exit => ", exit".to_string(),
            Action::DragMove => ", movewindow".to_string(),
            Action::DragResize => ", resizewindow".to_string(),
            Action::Exec { command, .. } => format!(", exec, {command}"),
            Action::ToggleHands => return None,
        };
        Some(text)
    }

    /// What the Markdown table calls the action on a surface.
    fn label(self, surface: Surface) -> &'static str {
        match self {
            Action::OpenCoder => match surface {
                Surface::Compositor => "new Coder",
                Surface::Quest => "new Coder tile",
                Surface::Desktop => "new pane",
            },
            Action::OpenShell => match surface {
                Surface::Quest => "shell tile",
                _ => "shell",
            },
            Action::Close => match surface {
                Surface::Compositor => "close, asking",
                Surface::Quest => "close tile",
                Surface::Desktop => "close pane",
            },
            Action::Focus(_) => "focus",
            Action::Move(_) => "move",
            Action::Resize(_) => "resize",
            Action::Desk(_) => "desk",
            Action::MoveToDesk(_) => "move to desk",
            Action::FocusScreen(_) => "focus a monitor",
            Action::MoveToScreen(_) => "send the window to a monitor",
            Action::DeskToScreen(_) => "send the desk to a monitor",
            Action::FlipSplit => "flip split",
            Action::Float => "float",
            Action::Fullscreen => "fullscreen",
            Action::Maximize => "maximize",
            Action::Exit => match surface {
                Surface::Compositor => "exit session",
                _ => "quit",
            },
            Action::DragMove => "move a float",
            Action::DragResize => "resize a float",
            Action::ToggleHands => "hand tracking",
            Action::Exec { doc, .. } => doc,
        }
    }
}

/// One row of the table.
#[derive(Clone, Copy, Debug)]
pub struct Bind {
    pub mods: Mods,
    pub key: Key,
    pub kind: Kind,
    pub action: Action,
    pub surfaces: Surfaces,
    /// Whether the compositor's copy routes through `coder-chord`, which
    /// sends the press into a focused `coder-quest` window and runs the
    /// bind's action on anything else. A chord a session level owns, such
    /// as a desk or a monitor move, answers the compositor directly even
    /// when CoderQuest handles the same press in a window.
    pub routed: bool,
    /// The `coderos.desktop.*` option that gates the compositor's copy of
    /// the bind, such as `dictation` for `coderos.desktop.dictation`.
    pub option: Option<&'static str>,
}

const fn bind(mods: Mods, key: Key, action: Action, surfaces: Surfaces) -> Bind {
    Bind {
        mods,
        key,
        kind: Kind::Once,
        action,
        surfaces,
        routed: false,
        option: None,
    }
}

impl Bind {
    /// Mark the compositor's copy routed through `coder-chord`.
    const fn routed(mut self) -> Bind {
        self.routed = true;
        self
    }
}

const fn repeat(mods: Mods, key: Key, action: Action, surfaces: Surfaces) -> Bind {
    Bind {
        kind: Kind::Repeat,
        ..bind(mods, key, action, surfaces)
    }
}

const fn drag(mods: Mods, key: Key, action: Action, surfaces: Surfaces) -> Bind {
    Bind {
        kind: Kind::Mouse,
        ..bind(mods, key, action, surfaces)
    }
}

const fn gated(
    option: &'static str,
    mods: Mods,
    key: Key,
    action: Action,
    surfaces: Surfaces,
) -> Bind {
    Bind {
        option: Some(option),
        ..bind(mods, key, action, surfaces)
    }
}

const COMPOSITOR: Surfaces = Surfaces::COMPOSITOR;
const DESKTOP: Surfaces = Surfaces::DESKTOP;
const COMPOSITOR_QUEST: Surfaces = Surfaces::COMPOSITOR.or(Surfaces::QUEST);
const ALL: Surfaces = Surfaces::ALL;

/// The chord table, in the order `desktop.nix` writes the compositor rows.
/// Rows after the compositor set belong to the in-window surfaces alone.
pub const BINDS: &[Bind] = &[
    // The default window: Super+Return opens another Coder beside the
    // focused one, or a tile or pane inside a Coder window.
    bind(Mods::SUPER, Key::Return, Action::OpenCoder, ALL).routed(),
    // Close asks the session before it closes a window with work in flight.
    // Coder Desktop's Command+Q quits instead, so its Close row is
    // Command+W alone.
    bind(Mods::SUPER, Key::Char('w'), Action::Close, ALL).routed(),
    bind(Mods::SUPER, Key::Char('q'), Action::Close, COMPOSITOR_QUEST).routed(),
    // Focus, desks, move, resize.
    bind(
        Mods::SUPER,
        Key::Arrow(Dir::Left),
        Action::Focus(Dir::Left),
        ALL,
    )
    .routed(),
    bind(
        Mods::SUPER,
        Key::Arrow(Dir::Right),
        Action::Focus(Dir::Right),
        ALL,
    )
    .routed(),
    bind(
        Mods::SUPER,
        Key::Arrow(Dir::Up),
        Action::Focus(Dir::Up),
        ALL,
    )
    .routed(),
    bind(
        Mods::SUPER,
        Key::Arrow(Dir::Down),
        Action::Focus(Dir::Down),
        ALL,
    )
    .routed(),
    bind(
        Mods::SUPER,
        Key::Digit(1),
        Action::Desk(1),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(2),
        Action::Desk(2),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(3),
        Action::Desk(3),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(4),
        Action::Desk(4),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(5),
        Action::Desk(5),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(6),
        Action::Desk(6),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(7),
        Action::Desk(7),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(8),
        Action::Desk(8),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER,
        Key::Digit(9),
        Action::Desk(9),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(1),
        Action::MoveToDesk(1),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(2),
        Action::MoveToDesk(2),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(3),
        Action::MoveToDesk(3),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(4),
        Action::MoveToDesk(4),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(5),
        Action::MoveToDesk(5),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(6),
        Action::MoveToDesk(6),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(7),
        Action::MoveToDesk(7),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(8),
        Action::MoveToDesk(8),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Digit(9),
        Action::MoveToDesk(9),
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Arrow(Dir::Left),
        Action::Move(Dir::Left),
        COMPOSITOR_QUEST,
    )
    .routed(),
    bind(
        Mods::SUPER_SHIFT,
        Key::Arrow(Dir::Right),
        Action::Move(Dir::Right),
        COMPOSITOR_QUEST,
    )
    .routed(),
    bind(
        Mods::SUPER_SHIFT,
        Key::Arrow(Dir::Up),
        Action::Move(Dir::Up),
        COMPOSITOR_QUEST,
    )
    .routed(),
    bind(
        Mods::SUPER_SHIFT,
        Key::Arrow(Dir::Down),
        Action::Move(Dir::Down),
        COMPOSITOR_QUEST,
    )
    .routed(),
    repeat(
        Mods::SUPER_CTRL,
        Key::Arrow(Dir::Left),
        Action::Resize(Dir::Left),
        COMPOSITOR_QUEST,
    )
    .routed(),
    repeat(
        Mods::SUPER_CTRL,
        Key::Arrow(Dir::Right),
        Action::Resize(Dir::Right),
        COMPOSITOR_QUEST,
    )
    .routed(),
    repeat(
        Mods::SUPER_CTRL,
        Key::Arrow(Dir::Up),
        Action::Resize(Dir::Up),
        COMPOSITOR_QUEST,
    )
    .routed(),
    repeat(
        Mods::SUPER_CTRL,
        Key::Arrow(Dir::Down),
        Action::Resize(Dir::Down),
        COMPOSITOR_QUEST,
    )
    .routed(),
    // Monitors: cycle, focus a direction, send the window, send the desk.
    bind(
        Mods::CTRL_ALT,
        Key::Tab,
        Action::FocusScreen(Screen::Cycle(1)),
        COMPOSITOR,
    ),
    bind(
        Mods::CTRL_ALT_SHIFT,
        Key::Tab,
        Action::FocusScreen(Screen::Cycle(-1)),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_ALT,
        Key::Arrow(Dir::Left),
        Action::FocusScreen(Screen::At(Dir::Left)),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_ALT,
        Key::Arrow(Dir::Right),
        Action::FocusScreen(Screen::At(Dir::Right)),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_ALT,
        Key::Arrow(Dir::Up),
        Action::FocusScreen(Screen::At(Dir::Up)),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_ALT,
        Key::Arrow(Dir::Down),
        Action::FocusScreen(Screen::At(Dir::Down)),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_SHIFT_ALT,
        Key::Arrow(Dir::Left),
        Action::MoveToScreen(Dir::Left),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_SHIFT_ALT,
        Key::Arrow(Dir::Right),
        Action::MoveToScreen(Dir::Right),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_SHIFT_ALT,
        Key::Arrow(Dir::Up),
        Action::MoveToScreen(Dir::Up),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_SHIFT_ALT,
        Key::Arrow(Dir::Down),
        Action::MoveToScreen(Dir::Down),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_CTRL_ALT,
        Key::Arrow(Dir::Left),
        Action::DeskToScreen(Dir::Left),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_CTRL_ALT,
        Key::Arrow(Dir::Right),
        Action::DeskToScreen(Dir::Right),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_CTRL_ALT,
        Key::Arrow(Dir::Up),
        Action::DeskToScreen(Dir::Up),
        COMPOSITOR,
    ),
    bind(
        Mods::SUPER_CTRL_ALT,
        Key::Arrow(Dir::Down),
        Action::DeskToScreen(Dir::Down),
        COMPOSITOR,
    ),
    // Layout and the bare shell.
    bind(
        Mods::SUPER,
        Key::Char('j'),
        Action::FlipSplit,
        COMPOSITOR_QUEST,
    )
    .routed(),
    bind(Mods::SUPER, Key::Space, Action::Float, COMPOSITOR_QUEST).routed(),
    bind(Mods::SUPER, Key::Char('d'), Action::Float, COMPOSITOR),
    bind(Mods::SUPER, Key::Char('t'), Action::OpenShell, ALL).routed(),
    bind(
        Mods::SUPER,
        Key::Char('f'),
        Action::Fullscreen,
        COMPOSITOR_QUEST,
    )
    .routed(),
    bind(
        Mods::SUPER_CTRL,
        Key::Char('f'),
        Action::Maximize,
        COMPOSITOR_QUEST,
    )
    .routed(),
    // The launchers, each behind its `coderos.desktop` option.
    gated(
        "dictation",
        Mods::SUPER,
        Key::Char('v'),
        Action::Exec {
            command: "dictate-toggle",
            doc: "dictation",
        },
        COMPOSITOR,
    ),
    gated(
        "presentation",
        Mods::SUPER,
        Key::Char('p'),
        Action::Exec {
            command: "presentation-mode toggle",
            doc: "presentation",
        },
        COMPOSITOR,
    ),
    gated(
        "browser",
        Mods::SUPER,
        Key::Char('b'),
        Action::Exec {
            command: "coder-browser",
            doc: "browser",
        },
        COMPOSITOR,
    ),
    gated(
        "deck",
        Mods::SUPER_SHIFT,
        Key::Char('d'),
        Action::Exec {
            command: "coder-deck-open",
            doc: "deck",
        },
        COMPOSITOR,
    ),
    gated(
        "zoom",
        Mods::SUPER,
        Key::Char('z'),
        Action::Exec {
            command: "coder-zoom",
            doc: "zoom",
        },
        COMPOSITOR,
    ),
    gated(
        "android",
        Mods::SUPER,
        Key::Char('a'),
        Action::Exec {
            command: "android-emulator",
            doc: "emulator",
        },
        COMPOSITOR,
    ),
    gated(
        "battlenet",
        Mods::SUPER,
        Key::Char('g'),
        Action::Exec {
            command: "coder-battlenet",
            doc: "games",
        },
        COMPOSITOR,
    ),
    gated(
        "camera",
        Mods::SUPER,
        Key::Char('c'),
        Action::Exec {
            command: "camera-toggle",
            doc: "camera",
        },
        COMPOSITOR,
    ),
    // A mouse drag with Super moves or resizes a float.
    drag(
        Mods::SUPER,
        Key::Mouse(272),
        Action::DragMove,
        COMPOSITOR_QUEST,
    ),
    drag(
        Mods::SUPER,
        Key::Mouse(273),
        Action::DragResize,
        COMPOSITOR_QUEST,
    ),
    bind(
        Mods::SUPER_SHIFT,
        Key::Char('e'),
        Action::Exit,
        COMPOSITOR_QUEST,
    ),
    // Coder Desktop quits on Command+Q, a row of its own. Super+H toggles
    // hands on the compositor, where `coderos.desktop.hands` grants it and
    // Hyprland writes no line for it, and in CoderQuest's window.
    bind(Mods::SUPER, Key::Char('q'), Action::Exit, DESKTOP),
    gated(
        "hands",
        Mods::SUPER,
        Key::Char('h'),
        Action::ToggleHands,
        COMPOSITOR_QUEST,
    ),
];

/// The action a chord runs on a surface, or `None` when the surface binds
/// nothing to it.
pub fn find(surface: Surface, mods: Mods, key: Key) -> Option<Action> {
    BINDS
        .iter()
        .find(|b| b.surfaces.contains(surface) && b.mods == mods && b.key == key)
        .map(|b| b.action)
}

/// The `bind` line Hyprland reads for a compositor row, such as
/// `bind = SUPER, RETURN, exec, ${openCoder}`. A routed row wraps its
/// action in `coder-chord`, such as
/// `bind = SUPER, RETURN, exec, ${coderChord}/bin/coder-chord 'SUPER, Return' exec ${openCoder}`.
pub fn hyprland(bind: &Bind) -> Option<String> {
    let kind = match bind.kind {
        Kind::Once => "bind",
        Kind::Repeat => "binde",
        Kind::Mouse => "bindm",
    };
    let dispatcher = bind.action.dispatcher()?;
    let head = format!("{} = {}, {}", kind, bind.mods.hypr(), bind.key.hypr());
    Some(match bind.routed {
        false => format!("{head}{dispatcher}"),
        true => {
            // `, exec, <cmd>` becomes `exec <cmd>` inside the chord's
            // argument, and `, <dispatcher>, <args>` becomes
            // `dispatch <dispatcher> <args>`.
            let body = dispatcher.strip_prefix(", ").unwrap_or(&dispatcher);
            let tail = match body.strip_prefix("exec, ") {
                Some(command) => format!("exec {command}"),
                None => format!("dispatch {}", body.replace(", ", " ")),
            };
            format!(
                "{head}, exec, ${{coderChord}}/bin/coder-chord '{}, {}' {tail}",
                bind.mods.hypr(),
                bind.key.chord()
            )
        }
    })
}

/// The bind lines the table renders for the compositor, in table order.
pub fn hyprland_lines() -> Vec<String> {
    BINDS
        .iter()
        .filter(|bind| bind.surfaces.contains(Surface::Compositor))
        .filter_map(hyprland)
        .collect()
}

/// One row of the Markdown chord table: a chord label and the chords it
/// expands to.
pub struct Group {
    pub chord: &'static str,
    pub chords: &'static [Chord],
}

const ARROWS_SUPER: &[Chord] = &[
    chord(Mods::SUPER, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER, Key::Arrow(Dir::Down)),
];
const ARROWS_SUPER_SHIFT: &[Chord] = &[
    chord(Mods::SUPER_SHIFT, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER_SHIFT, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER_SHIFT, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER_SHIFT, Key::Arrow(Dir::Down)),
];
const ARROWS_SUPER_CTRL: &[Chord] = &[
    chord(Mods::SUPER_CTRL, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER_CTRL, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER_CTRL, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER_CTRL, Key::Arrow(Dir::Down)),
];
const DIGITS_SUPER: &[Chord] = &[
    chord(Mods::SUPER, Key::Digit(1)),
    chord(Mods::SUPER, Key::Digit(2)),
    chord(Mods::SUPER, Key::Digit(3)),
    chord(Mods::SUPER, Key::Digit(4)),
    chord(Mods::SUPER, Key::Digit(5)),
    chord(Mods::SUPER, Key::Digit(6)),
    chord(Mods::SUPER, Key::Digit(7)),
    chord(Mods::SUPER, Key::Digit(8)),
    chord(Mods::SUPER, Key::Digit(9)),
];
const DIGITS_SUPER_SHIFT: &[Chord] = &[
    chord(Mods::SUPER_SHIFT, Key::Digit(1)),
    chord(Mods::SUPER_SHIFT, Key::Digit(2)),
    chord(Mods::SUPER_SHIFT, Key::Digit(3)),
    chord(Mods::SUPER_SHIFT, Key::Digit(4)),
    chord(Mods::SUPER_SHIFT, Key::Digit(5)),
    chord(Mods::SUPER_SHIFT, Key::Digit(6)),
    chord(Mods::SUPER_SHIFT, Key::Digit(7)),
    chord(Mods::SUPER_SHIFT, Key::Digit(8)),
    chord(Mods::SUPER_SHIFT, Key::Digit(9)),
];
const MONITOR_CHORDS: &[Chord] = &[
    chord(Mods::CTRL_ALT, Key::Tab),
    chord(Mods::CTRL_ALT_SHIFT, Key::Tab),
    chord(Mods::SUPER_ALT, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER_ALT, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER_ALT, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER_ALT, Key::Arrow(Dir::Down)),
    chord(Mods::SUPER_SHIFT_ALT, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER_SHIFT_ALT, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER_SHIFT_ALT, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER_SHIFT_ALT, Key::Arrow(Dir::Down)),
    chord(Mods::SUPER_CTRL_ALT, Key::Arrow(Dir::Left)),
    chord(Mods::SUPER_CTRL_ALT, Key::Arrow(Dir::Right)),
    chord(Mods::SUPER_CTRL_ALT, Key::Arrow(Dir::Up)),
    chord(Mods::SUPER_CTRL_ALT, Key::Arrow(Dir::Down)),
];

/// The rows of the audit's Appendix B, in its order. Grouping is
/// documentation structure: the binds in a group share one row of the
/// rendered table.
pub const GROUPS: &[Group] = &[
    Group {
        chord: "Super+Return",
        chords: &[chord(Mods::SUPER, Key::Return)],
    },
    Group {
        chord: "Super+T",
        chords: &[chord(Mods::SUPER, Key::Char('t'))],
    },
    Group {
        chord: "Super+W",
        chords: &[chord(Mods::SUPER, Key::Char('w'))],
    },
    Group {
        chord: "Super+Q",
        chords: &[chord(Mods::SUPER, Key::Char('q'))],
    },
    Group {
        chord: "Super+arrows",
        chords: ARROWS_SUPER,
    },
    Group {
        chord: "Super+Shift+arrows",
        chords: ARROWS_SUPER_SHIFT,
    },
    Group {
        chord: "Super+Ctrl+arrows",
        chords: ARROWS_SUPER_CTRL,
    },
    Group {
        chord: "Super+1..9",
        chords: DIGITS_SUPER,
    },
    Group {
        chord: "Super+Shift+1..9",
        chords: DIGITS_SUPER_SHIFT,
    },
    Group {
        chord: "Super+J",
        chords: &[chord(Mods::SUPER, Key::Char('j'))],
    },
    Group {
        chord: "Super+Space",
        chords: &[chord(Mods::SUPER, Key::Space)],
    },
    Group {
        chord: "Super+D",
        chords: &[chord(Mods::SUPER, Key::Char('d'))],
    },
    Group {
        chord: "Super+F, Super+Ctrl+F",
        chords: &[
            chord(Mods::SUPER, Key::Char('f')),
            chord(Mods::SUPER_CTRL, Key::Char('f')),
        ],
    },
    Group {
        chord: "Super+H",
        chords: &[chord(Mods::SUPER, Key::Char('h'))],
    },
    Group {
        chord: "Super+B",
        chords: &[chord(Mods::SUPER, Key::Char('b'))],
    },
    Group {
        chord: "Super+Shift+D",
        chords: &[chord(Mods::SUPER_SHIFT, Key::Char('d'))],
    },
    Group {
        chord: "Super+V, P, Z, A, G, C",
        chords: &[
            chord(Mods::SUPER, Key::Char('v')),
            chord(Mods::SUPER, Key::Char('p')),
            chord(Mods::SUPER, Key::Char('z')),
            chord(Mods::SUPER, Key::Char('a')),
            chord(Mods::SUPER, Key::Char('g')),
            chord(Mods::SUPER, Key::Char('c')),
        ],
    },
    Group {
        chord: "Monitor chords",
        chords: MONITOR_CHORDS,
    },
    Group {
        chord: "Super+mouse left, right",
        chords: &[
            chord(Mods::SUPER, Key::Mouse(272)),
            chord(Mods::SUPER, Key::Mouse(273)),
        ],
    },
    Group {
        chord: "Super+Shift+E",
        chords: &[chord(Mods::SUPER_SHIFT, Key::Char('e'))],
    },
];

/// What a surface does with a group's chords, as the appendix writes it.
fn group_cell(group: &Group, surface: Surface) -> String {
    let rows: Vec<&Bind> = BINDS
        .iter()
        .filter(|b| {
            b.surfaces.contains(surface)
                && group
                    .chords
                    .iter()
                    .any(|c| c.mods == b.mods && c.key == b.key)
        })
        .collect();
    if rows.is_empty() {
        // The Coder Desktop specification reaches some actions on a chord of
        // its own, the way it quits on Command+Q rather than Super+Shift+E.
        if surface != Surface::Desktop {
            return "none".to_string();
        }
        let actions: Vec<Action> = BINDS
            .iter()
            .filter(|b| {
                group
                    .chords
                    .iter()
                    .any(|c| c.mods == b.mods && c.key == b.key)
            })
            .map(|b| b.action)
            .collect();
        let found: Vec<&Bind> = BINDS
            .iter()
            .filter(|b| {
                b.surfaces.contains(surface)
                    && actions.contains(&b.action)
                    && !group
                        .chords
                        .iter()
                        .any(|c| c.mods == b.mods && c.key == b.key)
            })
            .collect();
        if found.is_empty() {
            return "none".to_string();
        }
        return chords_label(&found, surface);
    }
    let mut labels: Vec<&'static str> = Vec::new();
    for row in &rows {
        let label = row.action.label(surface);
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    // A family count stays readable where the launcher list reads as words.
    let all_exec = rows.iter().all(|b| matches!(b.action, Action::Exec { .. }));
    if labels.len() > 2 && !all_exec {
        return format!("{} families", number_word(labels.len()));
    }
    labels.join(", ")
}

/// How a macOS reader names a set of chords, such as
/// `Command+Option+arrows`.
fn chords_label(binds: &[&Bind], surface: Surface) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut mods: Vec<Mods> = Vec::new();
    for bind in binds {
        if !mods.contains(&bind.mods) {
            mods.push(bind.mods);
        }
    }
    for mods in mods {
        let keys: Vec<Key> = binds
            .iter()
            .filter(|b| b.mods == mods)
            .map(|b| b.key)
            .collect();
        let arrows = [
            Key::Arrow(Dir::Left),
            Key::Arrow(Dir::Right),
            Key::Arrow(Dir::Up),
            Key::Arrow(Dir::Down),
        ];
        let key_label = if keys == arrows {
            "arrows".to_string()
        } else if keys.len() > 1 && keys.iter().all(|k| matches!(k, Key::Digit(_))) {
            let first = match keys[0] {
                Key::Digit(d) => d,
                _ => 0,
            };
            let last = match keys[keys.len() - 1] {
                Key::Digit(d) => d,
                _ => 0,
            };
            format!("{first}..{last}")
        } else {
            keys.iter()
                .map(|k| {
                    if surface == Surface::Desktop {
                        k.mac()
                    } else {
                        k.hypr()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mod_label = if surface == Surface::Desktop {
            mods.mac()
        } else {
            mods.hypr()
        };
        out.push(format!("{mod_label}+{key_label}"));
    }
    out.join(", ")
}

fn number_word(n: usize) -> &'static str {
    match n {
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        7 => "seven",
        8 => "eight",
        9 => "nine",
        _ => "several",
    }
}

/// The audit's Appendix B as a Markdown table, generated from `BINDS`.
pub fn appendix_markdown() -> String {
    let mut out = String::from(
        "| Chord | `desktop.nix` | CoderQuest | Coder Desktop specification |\n\
         | --- | --- | --- | --- |\n",
    );
    for group in GROUPS {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            group.chord,
            group_cell(group, Surface::Compositor),
            group_cell(group, Surface::Quest),
            group_cell(group, Surface::Desktop),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_lookup_finds_the_tile_chords() {
        assert_eq!(
            find(Surface::Quest, Mods::SUPER, Key::Return),
            Some(Action::OpenCoder)
        );
        assert_eq!(
            find(Surface::Quest, Mods::SUPER, Key::Char('h')),
            Some(Action::ToggleHands)
        );
        assert_eq!(
            find(Surface::Quest, Mods::SUPER_SHIFT, Key::Char('e')),
            Some(Action::Exit)
        );
        // The compositor-only chords answer nothing on the window surfaces.
        assert_eq!(find(Surface::Quest, Mods::SUPER, Key::Char('d')), None);
        assert_eq!(find(Surface::Quest, Mods::SUPER, Key::Char('v')), None);
        assert_eq!(
            find(Surface::Quest, Mods::SUPER_ALT, Key::Arrow(Dir::Left)),
            None
        );
    }

    #[test]
    fn hyprland_lines_render_the_compositor_set() {
        let lines = hyprland_lines();
        assert_eq!(lines.len(), 64, "{lines:?}");
        assert_eq!(
            lines[0],
            "bind = SUPER, RETURN, exec, ${coderChord}/bin/coder-chord 'SUPER, Return' exec ${openCoder}"
        );
        assert_eq!(
            lines[3],
            "bind = SUPER, left, exec, ${coderChord}/bin/coder-chord 'SUPER, Left' dispatch movefocus l"
        );
        assert!(lines.contains(
            &"binde = SUPER CTRL, left, exec, ${coderChord}/bin/coder-chord 'SUPER CTRL, Left' dispatch resizeactive -40 0".to_string()
        ));
        assert!(lines.contains(&"bind = SUPER, D, togglefloating".to_string()));
        assert!(lines.contains(&"bindm = SUPER, mouse:272, movewindow".to_string()));
        assert_eq!(lines.last().unwrap(), "bind = SUPER SHIFT, E, exit");
    }

    #[test]
    fn every_compositor_bind_is_in_one_group() {
        for bind in BINDS
            .iter()
            .filter(|b| b.surfaces.contains(Surface::Compositor))
        {
            let groups = GROUPS
                .iter()
                .filter(|g| {
                    g.chords
                        .iter()
                        .any(|c| c.mods == bind.mods && c.key == bind.key)
                })
                .count();
            assert_eq!(
                groups, 1,
                "{:?} {:?} in {groups} groups",
                bind.mods, bind.key
            );
        }
    }

    #[test]
    fn appendix_names_the_desktop_chords() {
        let md = appendix_markdown();
        assert!(md.contains("| Super+Return | new Coder | new Coder tile | new pane |"));
        assert!(md.contains("| Super+arrows | focus | focus | focus |"));
        assert!(md.contains("| Monitor chords | three families | none | none |"));
        assert!(md.contains("| Super+Shift+E | exit session | quit | Command+Q |"));
    }
}
