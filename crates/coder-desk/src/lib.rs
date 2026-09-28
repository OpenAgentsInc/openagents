//! The desk client: one way for a Coder program to ask a desktop session
//! about the windows on its screens, and to change one.
//!
//! The verbs are the desk protocol's, which [`protocol`] defines. A
//! caller names a verb; the backend translates it for the desk in front of
//! it. Hyprland's wire format stops at [`Hyprland`], so a caller ported to
//! this crate runs unchanged on the Coder compositor.
//!
//! # Which desk a run reaches
//!
//! [`Announcement::from_env`] reads what the session announced, and
//! [`Desk::found`] reads an announcement in this order:
//!
//! 1. `CODER_DESK_SOCKET`, the desk protocol's own socket.
//! 2. `CODER_QUEST_SOCKET`, when this process is a pane inside CoderQuest.
//!    It names a socket that answers the protocol.
//! 3. `HYPRLAND_INSTANCE_SIGNATURE` with `XDG_RUNTIME_DIR`, which name the
//!    Hyprland control socket at
//!    `$XDG_RUNTIME_DIR/hypr/<signature>/.socket.sock`.
//!
//! [`Desk::here`] is the two together. A host with no compositor and a run
//! reached over SSH announce none, so the answer is [`Absent::NoSession`]
//! and a caller says so rather than answering for a screen nobody looked
//! at. A session that ended leaves its socket file behind, so a socket
//! nothing answers on is [`Absent::Unreachable`].
//!
//! # Asking
//!
//! Every verb is an async method on [`Desk`] and a method with the same
//! name on [`Blocking`], which speaks over a blocking socket for a caller
//! that runs a frame loop rather than a reactor. The async half is the
//! blocking half on the blocking pool, so both send the same bytes.
//!
//! # Answering
//!
//! [`serve`] is the other half: the socket, the framing, and the dispatch
//! a desk answers with. The Coder compositor serves the windows on a
//! CoderOS screen and Coder Desktop serves the panes in its own window,
//! and both go through one server, so a verb behaves the same wherever a
//! caller asks it.
//!
//! # Testing against it
//!
//! Under the `test-support` feature, [`fake`] holds a desk that keeps its
//! windows and screens in memory and records every request, and the
//! Hyprland session a caller's tests already stand up.

mod hyprland;
mod native;
pub mod protocol;
#[cfg(unix)]
pub mod serve;
mod wire;

#[cfg(any(test, feature = "test-support"))]
pub mod fake;

use std::path::PathBuf;
use std::sync::Arc;

pub use crate::protocol::{
    Answer, Border, Button, Chord, GENERATION, Modifier, Modifiers, Motion, Point, Refusal, Reply,
    Request, Screen, Selector, Size, Stroke, Verb, Window, refusal, socket_named,
};

pub use hyprland::Hyprland;
pub use native::Native;
pub use wire::Codec;

/// What a desktop session tells the programs it starts.
///
/// A session sets these in the environment of every program it starts, and
/// a terminal started that way passes them on, so a run inside the session
/// carries them and a run reached over SSH does not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Announcement {
    /// The desk protocol's own socket, which the session names as
    /// `CODER_DESK_SOCKET`. The Coder compositor, Coder Desktop, and
    /// CoderQuest each announce one.
    pub desk_socket: Option<PathBuf>,
    /// The name the running session goes by.
    pub signature: Option<String>,
    /// The directory the session keeps its control socket under.
    pub runtime_dir: Option<PathBuf>,
    /// CoderQuest control socket, when this process is a pane inside it.
    /// CoderQuest answers the desk protocol on it.
    pub quest_socket: Option<PathBuf>,
    /// Whether the desk draws its own panes rather than opening windows,
    /// which Coder Desktop says with
    /// [`crate::protocol::PANES_VAR`].
    pub panes: bool,
}

impl Announcement {
    /// What this process was told.
    pub fn from_env() -> Announcement {
        Announcement {
            desk_socket: socket_named(),
            signature: std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            runtime_dir: std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty()),
            quest_socket: std::env::var_os("CODER_QUEST_SOCKET")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty()),
            panes: std::env::var(crate::protocol::PANES_VAR)
                .is_ok_and(|said| !matches!(said.trim(), "" | "0" | "off" | "false")),
        }
    }

    /// Whether anything announced a desk at all. A run reached over SSH
    /// and a host with no session announce none, and a caller that reads
    /// `false` here says so rather than answering for a screen nobody
    /// looked at.
    pub fn names_a_desk(&self) -> bool {
        self.desk_socket.is_some() || self.quest_socket.is_some() || self.signature.is_some()
    }

    /// Whether this desk runs a program in a pane of its own window. A
    /// caller that opens a program then runs it directly, because a
    /// terminal emulator here would open a window beside the desk rather
    /// than a pane inside it.
    pub fn draws_its_own_panes(&self) -> bool {
        self.quest_socket.is_some() || self.panes
    }
}

/// Why there is no desk to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Absent {
    /// Nothing announced a session, so this run is not inside one.
    NoSession,
    /// A session was announced, and its control socket does not answer.
    Unreachable(PathBuf),
}

impl Absent {
    /// The refusal a caller reads. It says which of the two happened,
    /// because they call for different answers: the first means this run is
    /// somewhere without a screen, and the second means the session it was
    /// started in has ended.
    pub fn say(&self) -> String {
        match self {
            // Callers that want copy of their own for a run with no session
            // match on the variant instead.
            Absent::NoSession => "error: no desktop session here, so there are no windows to \
                 read. A desk answers for the session on this computer's own screen. A run \
                 reached over SSH and a host with no compositor have none."
                .to_string(),
            Absent::Unreachable(socket) => unreachable_copy(socket),
        }
    }
}

/// What a caller reads when the socket it names no longer answers. A
/// session leaves its socket file behind when it goes, so the missing file
/// and the one nothing listens on read as the same sentence, and neither
/// reads as a socket error.
fn unreachable_copy(socket: &std::path::Path) -> String {
    format!(
        "error: this run names a desktop session that is no longer there. Its \
         control socket at {} does not answer, so the session has ended or it \
         belongs to another user.",
        socket.display()
    )
}

/// Why one request did not answer what it asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskError {
    /// The socket does not answer. The session has ended.
    Unreachable(PathBuf),
    /// The request did not reach the desk, or the desk stopped answering.
    Asked(String),
    /// The desk answered something this client cannot read.
    Unreadable(String),
    /// The desk refused the request and said why.
    Refused(Refusal),
}

impl DeskError {
    /// The refusal a caller reads.
    pub fn say(&self) -> String {
        match self {
            DeskError::Unreachable(socket) => unreachable_copy(socket),
            DeskError::Asked(what) | DeskError::Unreadable(what) => format!("error: {what}"),
            DeskError::Refused(refused) => {
                format!("error: the desk refused the request: {}", refused.message)
            }
        }
    }

    /// The refusal the desk gave, when it gave one.
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            DeskError::Refused(refused) => Some(refused),
            _ => None,
        }
    }

    /// The error a verb this desk does not do makes.
    pub fn unsupported(message: impl Into<String>) -> DeskError {
        DeskError::Refused(Refusal::new(refusal::UNSUPPORTED, message))
    }

    /// The error a request the desk cannot read makes.
    pub fn malformed(message: impl Into<String>) -> DeskError {
        DeskError::Refused(Refusal::new(refusal::MALFORMED, message))
    }
}

/// What one desk speaks, and what one verb becomes on its socket.
///
/// [`Backend::requests`] turns a verb into the lines that go out, and
/// [`Backend::reply`] turns what came back into the contract's reply. Both
/// are ordinary functions, so a test reads the exact bytes a verb sends
/// without a socket.
pub trait Backend: std::fmt::Debug + Send + Sync {
    /// The name this backend goes by.
    fn name(&self) -> &'static str;

    /// Where one answer on this desk's socket ends.
    fn codec(&self) -> Codec;

    /// The requests one verb makes, in the order they go out.
    fn requests(&self, verb: &Verb) -> Result<Vec<String>, DeskError>;

    /// What the answers to those requests reply.
    fn reply(&self, verb: &Verb, answers: &[String]) -> Result<Reply, DeskError>;

    /// The requests a screen reading makes.
    fn reading_requests(&self) -> Vec<String>;

    /// The screen reading those answers make.
    fn reading(&self, answers: &[String]) -> Result<Reading, DeskError>;
}

/// What a screen reading holds: the screens, the name of the one the focus
/// is on, and the desks the session keeps.
///
/// Generation 1's [`Screen`] row carries neither the focus nor a desk's
/// window count, and the protocol folds the desks into `screens`, so a
/// caller that needs either reads them here until the contract carries
/// them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// The screens the session holds.
    pub screens: Vec<Screen>,
    /// The screen the focus is on, by name.
    pub focused: Option<String>,
    /// The desks the session holds.
    pub desks: Vec<DeskRow>,
}

impl Reading {
    /// The desk the screen with the focus is showing, or the first screen's
    /// when no screen holds the focus. `None` when no screen is on.
    pub fn focused_desk(&self) -> Option<u32> {
        let named = self.focused.as_deref();
        self.screens
            .iter()
            .find(|screen| Some(screen.name.as_str()) == named)
            .or_else(|| self.screens.first())
            .map(|screen| screen.desk)
    }
}

/// One desk the session holds: the number it goes by, the screen showing
/// it, and how many windows sit on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeskRow {
    pub id: i64,
    pub screen: String,
    pub windows: i64,
}

/// What an `open` asks for: a program to start or a file to show, the desk
/// to put it on, and whether it takes the focus.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
    pub command: Option<String>,
    pub path: Option<String>,
    pub desk: Option<u32>,
    pub silent: bool,
}

impl Open {
    /// Start a program in a new window beside the others.
    pub fn command(command: impl Into<String>) -> Open {
        Open {
            command: Some(command.into()),
            ..Open::default()
        }
    }

    /// Show a file in a new pane.
    pub fn path(path: impl Into<String>) -> Open {
        Open {
            path: Some(path.into()),
            ..Open::default()
        }
    }

    /// Put it on this desk rather than on the one showing now.
    pub fn on_desk(mut self, desk: u32) -> Open {
        self.desk = Some(desk);
        self
    }

    /// Open it without giving it the focus.
    pub fn silently(mut self) -> Open {
        self.silent = true;
        self
    }
}

/// How the desk draws one window. Every field is optional, and a field left
/// out keeps what the window has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub float: Option<bool>,
    pub pin: Option<bool>,
    pub at: Option<Point>,
    pub size: Option<Size>,
    pub aspect: Option<bool>,
    pub border: Option<Border>,
    pub shadow: Option<bool>,
}

/// A desktop session this run can reach.
#[derive(Clone, Debug)]
pub struct Desk {
    socket: PathBuf,
    backend: Arc<dyn Backend>,
}

impl Desk {
    /// The desk this run was started in, or why there is none. It is
    /// [`Desk::found`] over what this process was told.
    pub fn here() -> Result<Desk, Absent> {
        Desk::found(&Announcement::from_env())
    }

    /// The desk an announcement names, or why there is none.
    ///
    /// The desk protocol's own socket comes first. `CODER_QUEST_SOCKET`
    /// stays readable as a fallback for a pane that an older CoderQuest
    /// started, from before CoderQuest announced `CODER_DESK_SOCKET`.
    /// It names a socket that answers the protocol, so the native backend
    /// speaks to it. A Hyprland signature comes last.
    ///
    /// A session that ended leaves its socket behind, so every test is
    /// whether something answers on the socket and not whether the file
    /// is there.
    pub fn found(announced: &Announcement) -> Result<Desk, Absent> {
        if let Some(socket) = announced
            .desk_socket
            .as_ref()
            .or(announced.quest_socket.as_ref())
        {
            return match wire::answers(socket) {
                true => Ok(Desk::native(socket.clone())),
                false => Err(Absent::Unreachable(socket.clone())),
            };
        }
        let (Some(signature), Some(runtime)) = (&announced.signature, &announced.runtime_dir)
        else {
            return Err(Absent::NoSession);
        };
        let socket = runtime.join("hypr").join(signature).join(".socket.sock");
        match wire::answers(&socket) {
            true => Ok(Desk::hyprland(socket)),
            false => Err(Absent::Unreachable(socket)),
        }
    }

    /// A desk that speaks the desk protocol on this socket.
    pub fn native(socket: PathBuf) -> Desk {
        Desk::speaking(socket, Arc::new(Native))
    }

    /// A desk that speaks the Hyprland line protocol on this socket.
    pub fn hyprland(socket: PathBuf) -> Desk {
        Desk::speaking(socket, Arc::new(Hyprland::new()))
    }

    /// A desk on one socket with one backend.
    pub fn speaking(socket: PathBuf, backend: Arc<dyn Backend>) -> Desk {
        Desk { socket, backend }
    }

    /// The socket this desk answers on.
    pub fn socket(&self) -> &std::path::Path {
        &self.socket
    }

    /// The name the backend goes by.
    pub fn backend(&self) -> &'static str {
        self.backend.name()
    }

    /// The same desk, asked from a thread rather than from a reactor.
    pub fn blocking(&self) -> Blocking {
        Blocking { desk: self.clone() }
    }

    /// One verb and its reply.
    pub async fn ask(&self, verb: Verb) -> Result<Reply, DeskError> {
        self.spoken(move |desk| desk.ask(verb)).await
    }

    /// Every window the session holds.
    pub async fn list(&self) -> Result<Vec<Window>, DeskError> {
        self.spoken(|desk| desk.list()).await
    }

    /// The screens the session holds.
    pub async fn screens(&self) -> Result<Vec<Screen>, DeskError> {
        self.spoken(|desk| desk.screens()).await
    }

    /// The one window that has the focus, or none.
    pub async fn focused(&self) -> Result<Option<Window>, DeskError> {
        self.spoken(|desk| desk.focused()).await
    }

    /// The screens, the one the focus is on, and the desks.
    pub async fn reading(&self) -> Result<Reading, DeskError> {
        self.spoken(|desk| desk.reading()).await
    }

    /// Start a program in a new window, or show a file in a new pane.
    pub async fn open(&self, open: Open) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.open(open)).await
    }

    /// Give one window the focus.
    pub async fn focus(&self, handle: &Selector) -> Result<(), DeskError> {
        let handle = handle.clone();
        self.spoken(move |desk| desk.focus(&handle)).await
    }

    /// Move one window to a desk, without switching the screen to it.
    pub async fn place(&self, handle: &Selector, desk: u32) -> Result<(), DeskError> {
        let handle = handle.clone();
        self.spoken(move |on| on.place(&handle, desk)).await
    }

    /// Raise one window above the others on its desk.
    pub async fn raise(&self, handle: &Selector) -> Result<(), DeskError> {
        let handle = handle.clone();
        self.spoken(move |desk| desk.raise(&handle)).await
    }

    /// Close one window. Coder closes no window it did not open, so this is
    /// for a caller that opens its own.
    pub async fn close(&self, handle: &Selector) -> Result<(), DeskError> {
        let handle = handle.clone();
        self.spoken(move |desk| desk.close(&handle)).await
    }

    /// Change how the desk draws one window.
    pub async fn shape(&self, handle: &Selector, shape: Shape) -> Result<(), DeskError> {
        let handle = handle.clone();
        self.spoken(move |desk| desk.shape(&handle, shape)).await
    }

    /// Set the scale one screen draws at.
    pub async fn scale(&self, screen: &str, scale: f64) -> Result<(), DeskError> {
        let screen = screen.to_string();
        self.spoken(move |desk| desk.scale(&screen, scale)).await
    }

    /// Raise a notice the operator reads.
    pub async fn notice(&self, text: &str) -> Result<(), DeskError> {
        let text = text.to_string();
        self.spoken(move |desk| desk.notice(&text)).await
    }

    /// Ask the session to reload its configuration.
    pub async fn reload(&self) -> Result<(), DeskError> {
        self.spoken(|desk| desk.reload()).await
    }

    /// Press one chord, the way a press on the keyboard runs.
    pub async fn key(&self, chord: &Chord) -> Result<(), DeskError> {
        let chord = chord.clone();
        self.spoken(move |desk| desk.key(&chord)).await
    }

    /// Type text into the focused window.
    pub async fn type_text(&self, text: &str) -> Result<(), DeskError> {
        let text = text.to_string();
        self.spoken(move |desk| desk.type_text(&text)).await
    }

    /// Press and release a pointer button at a point on the screens.
    pub async fn click(&self, at: Point, button: Button) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.click(at, button)).await
    }

    /// Move the pointer to a point on the screens, in the steps and over
    /// the time the motion names.
    pub async fn move_pointer(&self, at: Point, motion: Motion) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.move_pointer(at, motion)).await
    }

    /// Hold a pointer button down where the pointer is, or let it go.
    pub async fn button(&self, button: Button, down: bool) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.button(button, down)).await
    }

    /// Hold one key or one modifier down, or let it go.
    pub async fn stroke(&self, stroke: &Stroke, down: bool) -> Result<(), DeskError> {
        let stroke = stroke.clone();
        self.spoken(move |desk| desk.stroke(&stroke, down)).await
    }

    /// Press a button at one point, move to another, and let it go.
    pub async fn drag(
        &self,
        from: Point,
        to: Point,
        button: Button,
        modifiers: Modifiers,
        motion: Motion,
    ) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.drag(from, to, button, modifiers, motion))
            .await
    }

    /// Scroll where the pointer is.
    pub async fn scroll(
        &self,
        dx: f64,
        dy: f64,
        discrete: bool,
        motion: Motion,
    ) -> Result<(), DeskError> {
        self.spoken(move |desk| desk.scroll(dx, dy, discrete, motion))
            .await
    }

    /// Write a PNG of one screen, or of the screen the pointer is on.
    pub async fn shot(&self, path: &str, screen: Option<&str>) -> Result<(), DeskError> {
        let path = path.to_string();
        let screen = screen.map(str::to_string);
        self.spoken(move |desk| desk.shot(&path, screen.as_deref()))
            .await
    }

    /// Whether hands drive the desk.
    pub async fn status(&self) -> Result<crate::protocol::Hands, DeskError> {
        self.spoken(|desk| desk.status()).await
    }

    /// One blocking ask on the blocking pool. The socket is local and under
    /// a time limit, so the work is a short blocking call rather than a
    /// reactor's.
    async fn spoken<T, F>(&self, ask: F) -> Result<T, DeskError>
    where
        F: FnOnce(&Blocking) -> Result<T, DeskError> + Send + 'static,
        T: Send + 'static,
    {
        let blocking = self.blocking();
        match tokio::task::spawn_blocking(move || ask(&blocking)).await {
            Ok(answered) => answered,
            Err(error) => Err(DeskError::Asked(format!("the desk was not asked: {error}"))),
        }
    }
}

/// The same desk, asked from a thread.
///
/// A caller that runs a frame loop rather than a reactor uses this: it
/// connects, writes, reads, and closes on the thread that called it, under
/// the same bounds the async half runs under.
#[derive(Clone, Debug)]
pub struct Blocking {
    desk: Desk,
}

impl Blocking {
    /// The desk this run was started in, asked from a thread.
    pub fn here() -> Result<Blocking, Absent> {
        Desk::here().map(|desk| desk.blocking())
    }

    /// The desk an announcement names, asked from a thread.
    pub fn found(announced: &Announcement) -> Result<Blocking, Absent> {
        Desk::found(announced).map(|desk| desk.blocking())
    }

    /// The desk this asks.
    pub fn desk(&self) -> &Desk {
        &self.desk
    }

    /// One verb and its reply.
    pub fn ask(&self, verb: Verb) -> Result<Reply, DeskError> {
        let backend = &self.desk.backend;
        let requests = backend.requests(&verb)?;
        let mut answers = Vec::with_capacity(requests.len());
        for request in &requests {
            answers.push(wire::speak(&self.desk.socket, request, backend.codec())?);
        }
        match backend.reply(&verb, &answers)? {
            Reply::Refused(refused) => Err(DeskError::Refused(refused)),
            Reply::Unknown => Err(DeskError::Unreadable(
                "the desk answered a reply this client does not know".to_string(),
            )),
            reply => Ok(reply),
        }
    }

    /// Every window the session holds.
    pub fn list(&self) -> Result<Vec<Window>, DeskError> {
        match self.ask(Verb::List)? {
            Reply::Windows { windows } => Ok(windows),
            other => Err(wrong("list", &other)),
        }
    }

    /// The screens the session holds.
    pub fn screens(&self) -> Result<Vec<Screen>, DeskError> {
        match self.ask(Verb::Screens)? {
            Reply::Screens { screens } => Ok(screens),
            other => Err(wrong("screens", &other)),
        }
    }

    /// The one window that has the focus, or none.
    pub fn focused(&self) -> Result<Option<Window>, DeskError> {
        match self.ask(Verb::Focused)? {
            Reply::Focused { window } => Ok(window),
            other => Err(wrong("focused", &other)),
        }
    }

    /// The screens, the one the focus is on, and the desks.
    pub fn reading(&self) -> Result<Reading, DeskError> {
        let backend = &self.desk.backend;
        let requests = backend.reading_requests();
        let mut answers = Vec::with_capacity(requests.len());
        for request in &requests {
            answers.push(wire::speak(&self.desk.socket, request, backend.codec())?);
        }
        backend.reading(&answers)
    }

    /// Start a program in a new window, or show a file in a new pane.
    pub fn open(&self, open: Open) -> Result<(), DeskError> {
        self.done(Verb::Open {
            command: open.command,
            path: open.path,
            desk: open.desk,
            silent: open.silent,
        })
    }

    /// Give one window the focus.
    pub fn focus(&self, handle: &Selector) -> Result<(), DeskError> {
        self.done(Verb::Focus {
            handle: handle.clone(),
        })
    }

    /// Move one window to a desk, without switching the screen to it.
    pub fn place(&self, handle: &Selector, desk: u32) -> Result<(), DeskError> {
        self.done(Verb::Place {
            handle: handle.clone(),
            desk,
        })
    }

    /// Raise one window above the others on its desk.
    pub fn raise(&self, handle: &Selector) -> Result<(), DeskError> {
        self.done(Verb::Raise {
            handle: handle.clone(),
        })
    }

    /// Close one window.
    pub fn close(&self, handle: &Selector) -> Result<(), DeskError> {
        self.done(Verb::Close {
            handle: handle.clone(),
        })
    }

    /// Change how the desk draws one window.
    pub fn shape(&self, handle: &Selector, shape: Shape) -> Result<(), DeskError> {
        self.done(Verb::Shape {
            handle: handle.clone(),
            float: shape.float,
            pin: shape.pin,
            at: shape.at,
            size: shape.size,
            aspect: shape.aspect,
            border: shape.border,
            shadow: shape.shadow,
        })
    }

    /// Set the scale one screen draws at.
    pub fn scale(&self, screen: &str, scale: f64) -> Result<(), DeskError> {
        self.done(Verb::Scale {
            screen: screen.to_string(),
            scale,
        })
    }

    /// Raise a notice the operator reads.
    pub fn notice(&self, text: &str) -> Result<(), DeskError> {
        self.done(Verb::Notice {
            text: text.to_string(),
        })
    }

    /// Ask the session to reload its configuration.
    pub fn reload(&self) -> Result<(), DeskError> {
        self.done(Verb::Reload)
    }

    /// Press one chord, the way a press on the keyboard runs.
    pub fn key(&self, chord: &Chord) -> Result<(), DeskError> {
        self.done(Verb::Key {
            chord: chord.spelled(),
        })
    }

    /// Type text into the focused window.
    pub fn type_text(&self, text: &str) -> Result<(), DeskError> {
        self.done(Verb::Type {
            text: text.to_string(),
        })
    }

    /// Press and release a pointer button at a point on the screens.
    pub fn click(&self, at: Point, button: Button) -> Result<(), DeskError> {
        self.done(Verb::Click {
            x: at.x,
            y: at.y,
            button,
        })
    }

    /// Move the pointer to a point on the screens, in the steps and over
    /// the time the motion names.
    pub fn move_pointer(&self, at: Point, motion: Motion) -> Result<(), DeskError> {
        let (steps, ms) = named(motion, Motion::JUMP);
        self.done(Verb::Move {
            x: at.x,
            y: at.y,
            steps,
            ms,
        })
    }

    /// Hold a pointer button down where the pointer is, or let it go.
    pub fn button(&self, button: Button, down: bool) -> Result<(), DeskError> {
        self.done(match down {
            true => Verb::ButtonPress { button },
            false => Verb::ButtonRelease { button },
        })
    }

    /// Hold one key or one modifier down, or let it go.
    pub fn stroke(&self, stroke: &Stroke, down: bool) -> Result<(), DeskError> {
        let key = stroke.word();
        self.done(match down {
            true => Verb::KeyPress { key },
            false => Verb::KeyRelease { key },
        })
    }

    /// Press a button at one point, move to another, and let it go.
    pub fn drag(
        &self,
        from: Point,
        to: Point,
        button: Button,
        modifiers: Modifiers,
        motion: Motion,
    ) -> Result<(), DeskError> {
        let (steps, ms) = named(motion, Motion::HAND);
        self.done(Verb::Drag {
            from,
            to,
            button,
            modifiers: modifiers.spelled(),
            steps,
            ms,
        })
    }

    /// Scroll where the pointer is.
    pub fn scroll(
        &self,
        dx: f64,
        dy: f64,
        discrete: bool,
        motion: Motion,
    ) -> Result<(), DeskError> {
        let (steps, ms) = named(motion, Motion::JUMP);
        self.done(Verb::Scroll {
            dx,
            dy,
            discrete,
            steps,
            ms,
        })
    }

    /// Write a PNG of one screen, or of the screen the pointer is on.
    pub fn shot(&self, path: &str, screen: Option<&str>) -> Result<(), DeskError> {
        self.done(Verb::Shot {
            path: path.to_string(),
            screen: screen.map(str::to_string),
        })
    }

    /// Whether hands drive the desk.
    pub fn status(&self) -> Result<crate::protocol::Hands, DeskError> {
        match self.ask(Verb::Status)? {
            Reply::Status { hands } => Ok(hands),
            other => Err(wrong("status", &other)),
        }
    }

    /// One change verb, which answers that it went through.
    fn done(&self, verb: Verb) -> Result<(), DeskError> {
        let named = verb.clone();
        match self.ask(verb)? {
            Reply::Done => Ok(()),
            other => Err(wrong(word(&named), &other)),
        }
    }
}

/// The steps and the milliseconds one motion puts on the wire. A field
/// that matches what the desk runs when a request names none is left out,
/// so a caller that asks for the ordinary motion sends the ordinary frame.
fn named(motion: Motion, unnamed: Motion) -> (Option<u32>, Option<u64>) {
    match motion == unnamed {
        true => (None, None),
        false => (Some(motion.steps), Some(motion.ms)),
    }
}

/// The word a verb goes by, for a message about it.
fn word(verb: &Verb) -> &'static str {
    match verb {
        Verb::List => "list",
        Verb::Screens => "screens",
        Verb::Focused => "focused",
        Verb::Open { .. } => "open",
        Verb::Focus { .. } => "focus",
        Verb::Place { .. } => "place",
        Verb::Raise { .. } => "raise",
        Verb::Close { .. } => "close",
        Verb::Shape { .. } => "shape",
        Verb::Scale { .. } => "scale",
        Verb::Notice { .. } => "notice",
        Verb::Reload => "reload",
        Verb::Key { .. } => "key",
        Verb::Type { .. } => "type",
        Verb::Click { .. } => "click",
        Verb::Move { .. } => "move",
        Verb::ButtonPress { .. } => "press",
        Verb::ButtonRelease { .. } => "release",
        Verb::KeyPress { .. } => "press",
        Verb::KeyRelease { .. } => "release",
        Verb::Drag { .. } => "drag",
        Verb::Scroll { .. } => "scroll",
        Verb::Shot { .. } => "shot",
        Verb::Status => "status",
        Verb::Unknown => "an unknown verb",
    }
}

/// The error an answer that does not match its verb makes.
fn wrong(verb: &str, reply: &Reply) -> DeskError {
    DeskError::Unreadable(format!(
        "the desk answered `{verb}` with a reply that verb does not take: {reply:?}"
    ))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
