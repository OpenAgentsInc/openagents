//! The desk protocol: the one Coder-owned, versioned contract a desktop
//! session answers for the windows on its screens.
//!
//! A session announces the socket to every program it starts as
//! `CODER_DESK_SOCKET`, under `$XDG_RUNTIME_DIR/coder-desk/`, and a run
//! inside the session inherits it. A run reached over SSH carries none and
//! says so rather than answering for a screen nobody looked at.
//!
//! The socket carries one JSON object per line: a [`Request`] from the
//! caller, then the desk's [`Answer`], and the connection closes. Every
//! request carries a `generation`, and the desk echoes the generation it
//! speaks on the answer. The
//! generation changes only when a frame in this module changes in a way an
//! older side cannot read: a verb or a reply a side does not know decodes
//! as [`Verb::Unknown`] or [`Reply::Unknown`], a request at a generation the
//! desk does not speak is refused with
//! [`refusal::UNSUPPORTED_GENERATION`], and a new field carries
//! `#[serde(default)]`.
//!
//! The verbs name what they mean rather than what one compositor calls
//! them. The Hyprland wire format stops at the desk's Hyprland backend, so
//! a caller ported to this contract runs unchanged on the Coder compositor.
//! Two rules hold on every desk: a run with no desk says so, and Coder
//! closes no window it did not open.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The generation this module speaks. A request at another generation is
/// refused with [`refusal::UNSUPPORTED_GENERATION`].
pub const GENERATION: u32 = 1;

/// The environment variable that names the desk's socket. A session sets it
/// on every program it starts, and a terminal started that way passes it
/// on, so a run inside the session carries it and a run reached over SSH
/// does not.
pub const SOCKET_VAR: &str = "CODER_DESK_SOCKET";

/// The directory under `$XDG_RUNTIME_DIR` the desk keeps its socket in. The
/// runtime directory is what a sandboxed client can reach, and a socket is
/// a temporary file, so it does not live under `~/.openagents`.
pub const SOCKET_DIR: &str = "coder-desk";

/// The environment variable a desk that draws its own panes sets, beside
/// the socket.
///
/// A compositor opens a window, so a program that wants one runs a
/// terminal emulator in front of itself. A desk that draws its panes in
/// one window of its own opens no window, and an emulator there would open
/// a second window beside it rather than a pane inside it, so a caller
/// that reads this runs the program on its own. Coder Desktop sets it, and
/// CoderQuest says the same thing with `CODER_QUEST_SOCKET`.
pub const PANES_VAR: &str = "CODER_DESK_PANES";

/// The longest line either side reads, in bytes. A `list` answer on a busy
/// session is the frame that approaches it.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// The socket `CODER_DESK_SOCKET` names, when it is set to one. The
/// variable names a path and not a switch, so a caller that finds `None`
/// here concludes the run has no desk rather than inventing a path.
pub fn socket_named() -> Option<PathBuf> {
    std::env::var_os(SOCKET_VAR)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// The directory a desk keeps its socket in when nothing configures the
/// path: [`SOCKET_DIR`] under the session's runtime directory. A host with
/// no runtime directory has no desk.
pub fn socket_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|dir| dir.join(SOCKET_DIR))
}

/// The codes a [`Refusal`] carries.
pub mod refusal {
    /// The request named a generation the desk does not speak.
    pub const UNSUPPORTED_GENERATION: &str = "unsupported_generation";
    /// A line was not a request the desk can read, or carried a field the
    /// verb does not take, such as an `open` naming both a `command` and a
    /// `path` or neither.
    pub const MALFORMED: &str = "malformed";
    /// A request arrived with a type this desk does not know. The request
    /// is refused and the socket stays up.
    pub const UNKNOWN_TYPE: &str = "unknown_type";
    /// The selector named no window the desk holds.
    pub const NO_SUCH_WINDOW: &str = "no_such_window";
    /// The request named a screen the desk does not hold.
    pub const NO_SUCH_SCREEN: &str = "no_such_screen";
    /// The desk does not do what the verb asks: an `open` with a `path` on
    /// a session with no file viewer, or a `close` from a caller that opens
    /// no windows.
    pub const UNSUPPORTED: &str = "unsupported";
}

/// Why the desk refused a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// One of the codes in [`refusal`].
    pub code: String,
    /// The reason, for the caller to show.
    pub message: String,
}

impl Refusal {
    pub fn new(code: &str, message: impl Into<String>) -> Refusal {
        Refusal {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Whether a `silent` flag is set, so `false` stays off the wire.
fn is_false(value: &bool) -> bool {
    !*value
}

/// What a verb that acts on one window names it by.
///
/// On the wire a selector is one string: an address the desk printed in a
/// `list` answer, `class:<app-id>` for the window a program owns, or
/// `title:<title>` for the window titled so. A regular expression never
/// reaches the desk: a caller that wants one resolves it against `list`
/// itself, and a backend that needs one keeps it inside the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// An address the desk printed, such as `0x55d1`.
    Address(String),
    /// `class:<app-id>`: the window the named program owns. An agent pane's
    /// app-id is `coder-child-<thread>`, and nothing else on the desk
    /// carries that prefix.
    Class(String),
    /// `title:<title>`: the window whose title matches.
    Title(String),
}

impl Selector {
    /// The selector one string names: `class:` and `title:` name their
    /// kinds, `address:` names an address, and a bare word is the address
    /// `list` printed.
    pub fn parse(named: &str) -> Selector {
        let named = named.trim();
        if let Some(class) = named.strip_prefix("class:") {
            return Selector::Class(class.to_string());
        }
        if let Some(title) = named.strip_prefix("title:") {
            return Selector::Title(title.to_string());
        }
        let address = named.strip_prefix("address:").unwrap_or(named);
        Selector::Address(address.to_string())
    }

    /// The string a request carries for this selector.
    pub fn wire(&self) -> String {
        match self {
            Selector::Address(address) => address.clone(),
            Selector::Class(class) => format!("class:{class}"),
            Selector::Title(title) => format!("title:{title}"),
        }
    }
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.wire())
    }
}

impl Serialize for Selector {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Selector {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Selector, D::Error> {
        Ok(Selector::parse(&String::deserialize(deserializer)?))
    }
}

/// A point on the screen, in pixels from the layout's top left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

/// A window or a screen's size, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

/// A pointer button a `click` presses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    /// The left button, which a `click` that names none presses.
    #[default]
    Left,
    /// The right button.
    Right,
    /// The middle button.
    Middle,
}

impl Button {
    /// The button one word names.
    pub fn parse(named: &str) -> Result<Button, String> {
        match named.trim().to_ascii_lowercase().as_str() {
            "left" => Ok(Button::Left),
            "right" => Ok(Button::Right),
            "middle" => Ok(Button::Middle),
            other => Err(format!(
                "a button is `left`, `right`, or `middle`, and this is `{other}`"
            )),
        }
    }

    /// The word this button goes by.
    pub fn word(self) -> &'static str {
        match self {
            Button::Left => "left",
            Button::Right => "right",
            Button::Middle => "middle",
        }
    }
}

/// A modifier a chord holds, a drag holds across its motion, or a
/// `key_press` holds until its release.
///
/// This is the one place the modifiers are named. The words are the bind
/// table's, so a modifier is spelled the same way wherever it is named: in
/// a chord, in a drag, and in a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    /// The Super key, which `logo` and `win` also name.
    Super,
    /// The Shift key.
    Shift,
    /// The Ctrl key, which `control` also names.
    Ctrl,
    /// The Alt key.
    Alt,
}

impl Modifier {
    /// Every modifier, in the order a spelling lists them.
    pub const ALL: [Modifier; 4] = [
        Modifier::Super,
        Modifier::Shift,
        Modifier::Ctrl,
        Modifier::Alt,
    ];

    /// The modifier one word names, read without regard to case.
    pub fn parse(named: &str) -> Result<Modifier, String> {
        match named.trim().to_ascii_lowercase().as_str() {
            "super" | "logo" | "win" => Ok(Modifier::Super),
            "shift" => Ok(Modifier::Shift),
            "ctrl" | "control" => Ok(Modifier::Ctrl),
            "alt" => Ok(Modifier::Alt),
            other => Err(format!(
                "a modifier is `super`, `shift`, `ctrl`, or `alt`, and this is `{other}`"
            )),
        }
    }

    /// The word this modifier goes by.
    pub fn word(self) -> &'static str {
        match self {
            Modifier::Super => "super",
            Modifier::Shift => "shift",
            Modifier::Ctrl => "ctrl",
            Modifier::Alt => "alt",
        }
    }
}

impl std::fmt::Display for Modifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.word())
    }
}

/// The modifiers held at once, spelled the way a chord spells them and
/// joined by `+`, such as `super+shift`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    /// The Super key.
    pub super_key: bool,
    /// The Shift key.
    pub shift: bool,
    /// The Ctrl key.
    pub ctrl: bool,
    /// The Alt key.
    pub alt: bool,
}

impl Modifiers {
    /// The modifiers one spelling names. An empty spelling holds none, so
    /// a request that names no modifier reads as none rather than as a
    /// refusal.
    pub fn parse(spelled: &str) -> Result<Modifiers, String> {
        let mut held = Modifiers::default();
        for word in spelled
            .split('+')
            .map(str::trim)
            .filter(|word| !word.is_empty())
        {
            held.hold(Modifier::parse(word)?);
        }
        Ok(held)
    }

    /// Whether one modifier is held.
    pub fn holds(self, modifier: Modifier) -> bool {
        match modifier {
            Modifier::Super => self.super_key,
            Modifier::Shift => self.shift,
            Modifier::Ctrl => self.ctrl,
            Modifier::Alt => self.alt,
        }
    }

    /// Holds one modifier.
    pub fn hold(&mut self, modifier: Modifier) {
        match modifier {
            Modifier::Super => self.super_key = true,
            Modifier::Shift => self.shift = true,
            Modifier::Ctrl => self.ctrl = true,
            Modifier::Alt => self.alt = true,
        }
    }

    /// Whether nothing is held.
    pub fn none(self) -> bool {
        !(self.super_key || self.shift || self.ctrl || self.alt)
    }

    /// The modifiers held, in the order a spelling lists them.
    pub fn held(self) -> Vec<Modifier> {
        Modifier::ALL
            .into_iter()
            .filter(|modifier| self.holds(*modifier))
            .collect()
    }

    /// The spelling these modifiers go by, which [`Modifiers::parse`]
    /// reads back.
    pub fn spelled(self) -> String {
        self.held()
            .into_iter()
            .map(Modifier::word)
            .collect::<Vec<&str>>()
            .join("+")
    }
}

impl std::fmt::Display for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.spelled())
    }
}

/// What a `key_press` or a `key_release` names: a modifier by the word
/// above, or any other key by its keysym name.
///
/// A modifier is read first, so `super` holds the Super key rather than
/// naming a keysym, and a key whose symbol needs Shift is reached by
/// holding `shift` first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stroke {
    /// A modifier, held until its release.
    Modifier(Modifier),
    /// Any other key, by its keysym name in lowercase.
    Key(String),
}

impl Stroke {
    /// The key or the modifier one word names.
    pub fn parse(named: &str) -> Result<Stroke, String> {
        let named = named.trim();
        if named.is_empty() {
            return Err(
                "a press names a key or a modifier, such as `a`, `return`, or `super`".to_string(),
            );
        }
        if let Ok(modifier) = Modifier::parse(named) {
            return Ok(Stroke::Modifier(modifier));
        }
        Ok(Stroke::Key(named.to_ascii_lowercase()))
    }

    /// The word this stroke goes by.
    pub fn word(&self) -> String {
        match self {
            Stroke::Modifier(modifier) => modifier.word().to_string(),
            Stroke::Key(key) => key.clone(),
        }
    }
}

impl std::fmt::Display for Stroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.word())
    }
}

/// The steps a hand's motion takes, which is what a drag runs in when a
/// request names no count. A compositor that reads one jump is not reading
/// what a person makes: a grab samples the motion, and a drag of one step
/// can pass it by.
pub const HAND_STEPS: u32 = 16;

/// The milliseconds a hand's motion spreads its steps over.
pub const HAND_MS: u64 = 220;

/// The most steps one motion takes.
pub const MOST_STEPS: u32 = 256;

/// The longest one motion runs for, in milliseconds. The socket answers a
/// request within two seconds, and a verb that ran longer than this would
/// leave the caller reading a timeout rather than the answer.
pub const MOST_MS: u64 = 1_000;

/// How a driven motion runs: the steps the pointer takes, and the
/// milliseconds it spreads them over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    /// The steps the pointer takes, at least one.
    pub steps: u32,
    /// The milliseconds the steps are spread over.
    pub ms: u64,
}

impl Motion {
    /// One step and no wait, which is what a `move` that names neither
    /// runs: the pointer arrives where a request put it.
    pub const JUMP: Motion = Motion { steps: 1, ms: 0 };

    /// What a hand makes: [`HAND_STEPS`] steps over [`HAND_MS`]
    /// milliseconds, which is what a drag runs in when a request names
    /// neither.
    pub const HAND: Motion = Motion {
        steps: HAND_STEPS,
        ms: HAND_MS,
    };

    /// The motion one request names, or why the request names none. A
    /// field left out takes what `unnamed` holds.
    pub fn read(steps: Option<u32>, ms: Option<u64>, unnamed: Motion) -> Result<Motion, String> {
        let steps = steps.unwrap_or(unnamed.steps);
        let ms = ms.unwrap_or(unnamed.ms);
        if steps == 0 {
            return Err("a motion takes at least one step".to_string());
        }
        if steps > MOST_STEPS {
            return Err(format!(
                "a motion takes at most {MOST_STEPS} steps, and this one names {steps}"
            ));
        }
        if ms > MOST_MS {
            return Err(format!(
                "a motion runs for at most {MOST_MS} milliseconds, because the socket answers \
                 within two seconds, and this one names {ms}"
            ));
        }
        Ok(Motion { steps, ms })
    }

    /// How long one step waits before the next.
    pub fn pause(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.ms / u64::from(self.steps.max(1)))
    }
}

/// A chord a `key` presses, in the bind table's spelling: the modifiers
/// and the key joined by `+`, such as `super+shift+d`, `ctrl+c`, `return`,
/// or `escape`.
///
/// The modifiers are `super` (also `logo` or `win`), `shift`, `ctrl` (also
/// `control`), and `alt`, in any order and any case. The key is the last
/// word: a letter, a digit, or an xkb keysym name such as `return`,
/// `escape`, `space`, `tab`, `backspace`, `left`, or `f1`, read without
/// regard to case. Which names a desk types depends on the keyboard layout
/// the session loaded, so a desk refuses a key its layout has no key for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    /// The Super key, which every desktop chord holds.
    pub super_key: bool,
    /// The Shift key.
    pub shift: bool,
    /// The Ctrl key.
    pub ctrl: bool,
    /// The Alt key.
    pub alt: bool,
    /// The key, in lowercase.
    pub key: String,
}

impl Chord {
    /// The chord one spelling names, or why the spelling is not one.
    pub fn parse(spelled: &str) -> Result<Chord, String> {
        let mut chord = Chord::default();
        let words: Vec<&str> = spelled
            .split('+')
            .map(str::trim)
            .filter(|word| !word.is_empty())
            .collect();
        let Some((key, modifiers)) = words.split_last() else {
            return Err("a chord names a key, such as `super+t` or `return`".to_string());
        };
        let mut held = Modifiers::default();
        for word in modifiers {
            held.hold(Modifier::parse(word)?);
        }
        chord.super_key = held.super_key;
        chord.shift = held.shift;
        chord.ctrl = held.ctrl;
        chord.alt = held.alt;
        chord.key = key.to_ascii_lowercase();
        Ok(chord)
    }

    /// The modifiers this chord holds.
    pub fn modifiers(&self) -> Modifiers {
        Modifiers {
            super_key: self.super_key,
            shift: self.shift,
            ctrl: self.ctrl,
            alt: self.alt,
        }
    }

    /// The spelling this chord goes by, which [`Chord::parse`] reads back.
    pub fn spelled(&self) -> String {
        let mut out = String::new();
        for modifier in self.modifiers().held() {
            out.push_str(modifier.word());
            out.push('+');
        }
        out.push_str(&self.key);
        out
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.spelled())
    }
}

/// How the desk draws a window's border.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Border {
    /// The border's thickness in pixels. Left out, it keeps what it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    /// The border's corner rounding in pixels. Left out, it keeps what it
    /// has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounding: Option<i64>,
}

/// One window the desk holds, the row a `list` answer prints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// The address the desk printed, which a [`Selector::Address`] names.
    pub handle: String,
    /// The app-id of the program that owns the window, which
    /// `class:<app-id>` selects on.
    pub app_id: String,
    /// The title as the session gives it. A title is content: it can hold a
    /// filename, a branch, or a customer's name.
    pub title: String,
    /// The process that owns the window, when the desk knows it. A pane the
    /// desk draws in its own process carries none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,
    /// The screen showing the window, by name.
    pub screen: String,
    /// The desk the window sits on.
    pub desk: u32,
    /// Where the window's top left corner sits on the screen.
    pub at: Point,
    /// The window's size in pixels.
    pub size: Size,
    /// The window floats over the layout rather than tiling in it.
    pub floating: bool,
    /// The window shows on every desk.
    pub pinned: bool,
    /// The window fills its screen.
    pub fullscreen: bool,
}

/// One screen the desk holds, the row a `screens` answer prints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// The name the session gives the screen, such as `DP-2`.
    pub name: String,
    /// Where the screen sits in the session's layout.
    pub at: Point,
    /// The screen's size in pixels.
    pub size: Size,
    /// The scale the screen draws at, such as `1.0` or `2.0`.
    pub scale: f64,
    /// The desk the screen shows now.
    pub desk: u32,
}

/// Whether hand tracking drives the desk: the Coder compositor reading
/// the camera daemon's landmarks as pointer motion, presses, desk
/// switches, and Escape. A desk with no hand tracking answers `off`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hands {
    pub on: bool,
}

/// What a caller asks the desk to do or to read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Verb {
    /// Every window the session holds, answered as [`Window`] rows.
    List,
    /// The screens and the desk each one shows, answered as [`Screen`]
    /// rows.
    Screens,
    /// The one window that has the focus, answered as a [`Window`] or none.
    Focused,
    /// Start a program in a new window beside the others, or show a file in
    /// a new pane, the read-only viewer a click on a file name opens. A
    /// request names exactly one of `command` and `path`; one that names
    /// both or neither is refused with [`refusal::MALFORMED`].
    Open {
        /// The program to start, with its arguments.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        /// The file to show.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// The desk to open it on. Left out, the desk showing now takes it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        desk: Option<u32>,
        /// Open it without giving it the focus.
        #[serde(default, skip_serializing_if = "is_false")]
        silent: bool,
    },
    /// Give the focus to the window `handle` names, switching the screen to
    /// its desk when it sits on another.
    Focus { handle: Selector },
    /// Move the window `handle` names to `desk`, without switching the
    /// screen to it.
    Place { handle: Selector, desk: u32 },
    /// Raise the window `handle` names above the others on its desk.
    Raise { handle: Selector },
    /// Close the window `handle` names. The desk offers it to its own pane
    /// driver, which closes the panes it opened, and to the close key,
    /// which asks the session first. It is never offered to the `windows`
    /// tool: Coder closes no window it did not open.
    Close { handle: Selector },
    /// Change how the desk draws the window `handle` names. Every field is
    /// optional, and a field left out keeps what the window has.
    Shape {
        handle: Selector,
        /// Float the window over the layout, or put it back in it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        float: Option<bool>,
        /// Pin the window so it shows on every desk.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pin: Option<bool>,
        /// Move the window's top left corner to this point.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Point>,
        /// Size the window to these pixels.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<Size>,
        /// Keep the window's aspect ratio when the layout resizes it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aspect: Option<bool>,
        /// The window's border: its thickness and corner rounding.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        border: Option<Border>,
        /// Draw the window's drop shadow. `false` is the compositor's
        /// `no_shadow`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<bool>,
    },
    /// Set the scale the screen `screen` draws at.
    Scale { screen: String, scale: f64 },
    /// Raise a notice the operator reads.
    Notice { text: String },
    /// Ask the session to reload its configuration.
    Reload,
    /// Press one chord, spelled the way [`Chord`] reads it. The desk runs
    /// it the way a press on the keyboard runs: a chord the bind table
    /// holds runs its action, and any other reaches the focused window. A
    /// spelling that is not a chord is refused with [`refusal::MALFORMED`].
    Key { chord: String },
    /// Type text into the focused window, one key press per character. A
    /// newline presses Return and a tab presses Tab.
    Type { text: String },
    /// Press and release a pointer button at a point in the space every
    /// screen shares, on the window under it in draw order. The pointer
    /// moves there first.
    Click {
        x: i64,
        y: i64,
        /// The button, left when left out.
        #[serde(default)]
        button: Button,
    },
    /// Move the pointer to a point in the space every screen shares, which
    /// moves the focus with it the way a mouse does.
    ///
    /// `steps` and `ms` say how the pointer travels: it takes that many
    /// steps over that many milliseconds, so a grab reads motion rather
    /// than one jump. A request that names neither jumps, which is what a
    /// caller that only wants the pointer somewhere asks for.
    Move {
        x: i64,
        y: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        steps: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    /// Press a pointer button where the pointer is and hold it, until a
    /// [`Verb::ButtonRelease`] lets it go. The window under the pointer
    /// reads the press the way it reads one from the mouse, and a press
    /// the session binds, such as a drag, takes the pointer instead.
    ButtonPress {
        /// The button, left when left out.
        #[serde(default)]
        button: Button,
    },
    /// Let a pointer button go where the pointer is.
    ButtonRelease {
        /// The button, left when left out.
        #[serde(default)]
        button: Button,
    },
    /// Press one key or one modifier and hold it, until a
    /// [`Verb::KeyRelease`] lets it go, so a modifier is held across a
    /// drag and a chord is built by hand. `key` is a modifier by its word
    /// or any other key by its keysym name, which [`Stroke`] reads.
    KeyPress { key: String },
    /// Let one key or one modifier go.
    KeyRelease { key: String },
    /// Press a pointer button at `from`, move to `to` in steps, and let
    /// the button go there: the press, the motion, and the release one
    /// verb makes, which is the drag a person makes with the mouse.
    ///
    /// `modifiers` are held from before the press until after the
    /// release, spelled the way a chord spells them, such as `super`. A
    /// request that names no `steps` or `ms` runs the motion a hand
    /// makes, [`Motion::HAND`].
    Drag {
        from: Point,
        to: Point,
        /// The button, left when left out.
        #[serde(default)]
        button: Button,
        /// The modifiers held across the drag, such as `super+shift`.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        modifiers: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        steps: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    /// Scroll where the pointer is: `dx` across and `dy` down, positive
    /// down and to the right, the way a wheel and a trackpad report.
    ///
    /// `discrete` sends the notches a wheel clicks through; left off, the
    /// axis is the smooth one a trackpad reports, and it ends where a
    /// finger lifts. `steps` and `ms` spread the scroll the way a hand
    /// does, and a request that names neither sends it at once.
    Scroll {
        #[serde(default)]
        dx: f64,
        #[serde(default)]
        dy: f64,
        /// Send a wheel's notches rather than a trackpad's smooth axis.
        #[serde(default, skip_serializing_if = "is_false")]
        discrete: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        steps: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    /// Write a PNG of one screen to `path`: the screen `screen` names, or
    /// the screen the pointer is on when it names none. The desk answers
    /// after the file is written.
    Shot {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screen: Option<String>,
    },
    /// What the desk does beyond holding windows: whether hands drive
    /// it, answered as a [`Reply::Status`].
    Status,
    /// A verb this side does not know. The desk refuses it with
    /// [`refusal::UNKNOWN_TYPE`] rather than dropping the line.
    #[serde(other)]
    Unknown,
}

/// One request on the socket: the generation the caller speaks, and the
/// verb it asks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The generation of this contract the caller speaks. A desk that
    /// speaks another refuses with [`refusal::UNSUPPORTED_GENERATION`].
    pub generation: u32,
    /// What the caller asks.
    #[serde(flatten)]
    pub verb: Verb,
}

impl Request {
    /// The request one verb makes at the generation this build speaks.
    pub fn new(verb: Verb) -> Request {
        Request {
            generation: GENERATION,
            verb,
        }
    }
}

/// What the desk answers one request with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    /// The `list` answer: every window the session holds.
    Windows { windows: Vec<Window> },
    /// The `screens` answer: every screen the session holds.
    Screens { screens: Vec<Screen> },
    /// The `focused` answer: the window that has the focus, or none when
    /// nothing does.
    Focused { window: Option<Window> },
    /// The `status` answer: whether hands drive the desk.
    Status { hands: Hands },
    /// The change a verb asked for went through.
    Done,
    /// The desk refused the request.
    Refused(Refusal),
    /// A reply this side does not know.
    #[serde(other)]
    Unknown,
}

/// The desk's answer to one request: the generation the desk speaks, and
/// what it answered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// The generation of this contract the desk speaks, so a caller knows
    /// which answer shape it is reading.
    pub generation: u32,
    /// What the desk answered.
    #[serde(flatten)]
    pub reply: Reply,
}

impl Answer {
    /// The answer one reply makes at the generation this build speaks.
    pub fn new(reply: Reply) -> Answer {
        Answer {
            generation: GENERATION,
            reply,
        }
    }

    /// The `done` answer to a change verb.
    pub fn done() -> Answer {
        Answer::new(Reply::Done)
    }

    /// The answer a refusal makes.
    pub fn refused(code: &str, message: impl Into<String>) -> Answer {
        Answer::new(Reply::Refused(Refusal::new(code, message)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(verb: Verb) -> String {
        serde_json::to_string(&Request::new(verb)).expect("the request encodes")
    }

    fn read(line: &str) -> Verb {
        serde_json::from_str::<Request>(line)
            .expect("the request decodes")
            .verb
    }

    #[test]
    fn a_chord_reads_its_modifiers_in_any_order_and_case() {
        let chord = Chord::parse("Shift+SUPER+d").expect("a chord");
        assert!(chord.super_key && chord.shift && !chord.ctrl && !chord.alt);
        assert_eq!(chord.key, "d");
        assert_eq!(chord.spelled(), "super+shift+d");
        assert_eq!(Chord::parse("return").expect("a chord").spelled(), "return");
        assert_eq!(
            Chord::parse("ctrl+alt+Tab").expect("a chord").spelled(),
            "ctrl+alt+tab"
        );
        assert_eq!(
            Chord::parse("logo+t").expect("a chord").spelled(),
            "super+t"
        );
        assert_eq!(
            Chord::parse("control+c").expect("a chord").spelled(),
            "ctrl+c"
        );
    }

    #[test]
    fn a_chord_with_no_key_or_a_modifier_it_does_not_know_is_refused() {
        assert!(Chord::parse("").is_err());
        assert!(Chord::parse("+").is_err());
        let refused = Chord::parse("hyper+t").expect_err("no such modifier");
        assert!(refused.contains("hyper"), "{refused}");
    }

    #[test]
    fn a_button_reads_its_word_and_left_is_the_default() {
        assert_eq!(Button::parse("Right"), Ok(Button::Right));
        assert_eq!(Button::parse("middle"), Ok(Button::Middle));
        assert!(Button::parse("back").is_err());
        assert_eq!(Button::default(), Button::Left);
        assert_eq!(Button::Middle.word(), "middle");
    }

    #[test]
    fn the_drive_verbs_carry_their_fields_on_the_wire() {
        assert_eq!(
            wire(Verb::Key {
                chord: "super+t".into()
            }),
            r#"{"generation":1,"type":"key","chord":"super+t"}"#
        );
        assert_eq!(
            wire(Verb::Type {
                text: "echo hi".into()
            }),
            r#"{"generation":1,"type":"type","text":"echo hi"}"#
        );
        assert_eq!(
            wire(Verb::Click {
                x: 10,
                y: 20,
                button: Button::Right
            }),
            r#"{"generation":1,"type":"click","x":10,"y":20,"button":"right"}"#
        );
        assert_eq!(
            wire(Verb::Move {
                x: 1,
                y: 2,
                steps: None,
                ms: None
            }),
            r#"{"generation":1,"type":"move","x":1,"y":2}"#
        );
        assert_eq!(
            wire(Verb::Shot {
                path: "/tmp/a.png".into(),
                screen: None
            }),
            r#"{"generation":1,"type":"shot","path":"/tmp/a.png"}"#
        );
        assert_eq!(
            wire(Verb::Shot {
                path: "/tmp/a.png".into(),
                screen: Some("DP-2".into())
            }),
            r#"{"generation":1,"type":"shot","path":"/tmp/a.png","screen":"DP-2"}"#
        );
    }

    #[test]
    fn the_holding_verbs_carry_their_fields_on_the_wire() {
        assert_eq!(
            wire(Verb::ButtonPress {
                button: Button::Left
            }),
            r#"{"generation":1,"type":"button_press","button":"left"}"#
        );
        assert_eq!(
            wire(Verb::ButtonRelease {
                button: Button::Right
            }),
            r#"{"generation":1,"type":"button_release","button":"right"}"#
        );
        assert_eq!(
            wire(Verb::KeyPress {
                key: "super".into()
            }),
            r#"{"generation":1,"type":"key_press","key":"super"}"#
        );
        assert_eq!(
            wire(Verb::KeyRelease { key: "a".into() }),
            r#"{"generation":1,"type":"key_release","key":"a"}"#
        );
        assert_eq!(
            wire(Verb::Move {
                x: 1,
                y: 2,
                steps: Some(8),
                ms: Some(100)
            }),
            r#"{"generation":1,"type":"move","x":1,"y":2,"steps":8,"ms":100}"#
        );
        assert_eq!(
            wire(Verb::Drag {
                from: Point { x: 1, y: 2 },
                to: Point { x: 3, y: 4 },
                button: Button::Left,
                modifiers: "super".into(),
                steps: None,
                ms: None
            }),
            r#"{"generation":1,"type":"drag","from":{"x":1,"y":2},"to":{"x":3,"y":4},"button":"left","modifiers":"super"}"#
        );
        assert_eq!(
            wire(Verb::Scroll {
                dx: 0.0,
                dy: -2.0,
                discrete: true,
                steps: None,
                ms: None
            }),
            r#"{"generation":1,"type":"scroll","dx":0.0,"dy":-2.0,"discrete":true}"#
        );
    }

    #[test]
    fn a_drag_and_a_scroll_that_name_the_least_read_the_rest_as_their_defaults() {
        assert_eq!(
            read(r#"{"generation":1,"type":"drag","from":{"x":1,"y":2},"to":{"x":3,"y":4}}"#),
            Verb::Drag {
                from: Point { x: 1, y: 2 },
                to: Point { x: 3, y: 4 },
                button: Button::Left,
                modifiers: String::new(),
                steps: None,
                ms: None
            }
        );
        assert_eq!(
            read(r#"{"generation":1,"type":"scroll","dy":1.0}"#),
            Verb::Scroll {
                dx: 0.0,
                dy: 1.0,
                discrete: false,
                steps: None,
                ms: None
            }
        );
        assert_eq!(
            read(r#"{"generation":1,"type":"button_press"}"#),
            Verb::ButtonPress {
                button: Button::Left
            }
        );
    }

    #[test]
    fn the_modifiers_are_spelled_one_way_wherever_they_are_named() {
        let held = Modifiers::parse("WIN+control").expect("the modifiers");
        assert!(held.holds(Modifier::Super) && held.holds(Modifier::Ctrl));
        assert!(!held.holds(Modifier::Shift));
        assert_eq!(held.spelled(), "super+ctrl");
        assert_eq!(Modifiers::parse("").expect("none"), Modifiers::default());
        assert!(Modifiers::default().none());
        let refused = Modifiers::parse("hyper").expect_err("no such modifier");
        assert!(refused.contains("hyper"), "{refused}");
        // A chord spells them the same way, because it reads them here.
        let chord = Chord::parse("logo+shift+d").expect("a chord");
        assert_eq!(chord.modifiers().spelled(), "super+shift");
        assert_eq!(chord.spelled(), "super+shift+d");
    }

    #[test]
    fn a_press_names_a_modifier_before_it_names_a_key() {
        assert_eq!(
            Stroke::parse("Super"),
            Ok(Stroke::Modifier(Modifier::Super))
        );
        assert_eq!(Stroke::parse("ctrl"), Ok(Stroke::Modifier(Modifier::Ctrl)));
        assert_eq!(Stroke::parse("Return"), Ok(Stroke::Key("return".into())));
        assert_eq!(Stroke::parse("a"), Ok(Stroke::Key("a".into())));
        assert_eq!(Stroke::parse("left"), Ok(Stroke::Key("left".into())));
        assert!(Stroke::parse("  ").is_err());
        assert_eq!(Stroke::Modifier(Modifier::Alt).word(), "alt");
    }

    #[test]
    fn a_motion_that_names_neither_a_count_nor_a_time_takes_what_it_is_given() {
        let jump = Motion::read(None, None, Motion::JUMP).expect("a motion");
        assert_eq!(jump, Motion::JUMP);
        assert_eq!(jump.pause(), std::time::Duration::ZERO);
        let hand = Motion::read(None, None, Motion::HAND).expect("a motion");
        assert_eq!(hand.steps, HAND_STEPS);
        assert_eq!(
            hand.pause().as_millis() as u64,
            HAND_MS / u64::from(HAND_STEPS)
        );
        let named = Motion::read(Some(4), Some(200), Motion::HAND).expect("a motion");
        assert_eq!(named, Motion { steps: 4, ms: 200 });
        assert_eq!(named.pause().as_millis(), 50);
        assert!(Motion::read(Some(0), None, Motion::HAND).is_err());
        assert!(Motion::read(Some(MOST_STEPS + 1), None, Motion::HAND).is_err());
        let long = Motion::read(None, Some(MOST_MS + 1), Motion::HAND).expect_err("too long");
        assert!(long.contains("two seconds"), "{long}");
    }

    #[test]
    fn a_click_that_names_no_button_presses_the_left_one() {
        assert_eq!(
            read(r#"{"generation":1,"type":"click","x":5,"y":6}"#),
            Verb::Click {
                x: 5,
                y: 6,
                button: Button::Left
            }
        );
        assert_eq!(
            read(r#"{"generation":1,"type":"shot","path":"a.png"}"#),
            Verb::Shot {
                path: "a.png".into(),
                screen: None
            }
        );
    }

    #[test]
    fn a_reply_to_a_drive_verb_is_the_done_the_other_change_verbs_answer() {
        let answer = serde_json::to_string(&Answer::done()).expect("the answer encodes");
        assert_eq!(answer, r#"{"generation":1,"type":"done"}"#);
        let read: Answer = serde_json::from_str(&answer).expect("the answer decodes");
        assert_eq!(read.reply, Reply::Done);
    }
}
