//! The desk protocol server: one JSON object a line over a Unix socket.
//!
//! This is the half a desk answers with, and the client half in this
//! crate's root is what asks. Two desks serve it: the Coder compositor,
//! which answers for the windows on a CoderOS screen, and Coder Desktop,
//! which answers for the panes in its own window.
//!
//! The socket sits at `$XDG_RUNTIME_DIR/coder-desk/<pid>.sock`, and the
//! desk announces it to every program it starts as `CODER_DESK_SOCKET`. A
//! caller connects, writes one [`Request`], reads one [`Answer`], and
//! closes. A host with no runtime directory, which is a Mac, keeps the
//! socket under `~/.openagents/desk/` instead.
//!
//! The listener runs on a thread of its own and owns no desk state: it
//! forwards each request to the desk's own loop as a [`Call`] and writes
//! back the answer that loop hands it. Both desks hold state that stays on
//! one thread, Wayland objects in one and GPUI views in the other, so the
//! socket never touches it.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use crate::protocol::Hands;
use crate::protocol::{
    Answer, Border, Button, Chord, GENERATION, MAX_LINE_BYTES, Modifiers, Motion, Point, Refusal,
    Reply, Request, Screen, Selector, Size, Stroke, Verb, Window, refusal, socket_dir,
};

/// How long the socket thread waits for the desk to answer before it tells
/// the caller the desk is busy. A desk that misses this is one the caller
/// cannot use anyway.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

/// What a `shape` request asks, with each field the request left out as
/// `None`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    /// Float the window over the layout, or put it back in it.
    pub float: Option<bool>,
    /// Show the window on every desk.
    pub pin: Option<bool>,
    /// Move the window's top left corner to this point.
    pub at: Option<Point>,
    /// Size the window to these pixels.
    pub size: Option<Size>,
    /// Keep the window's aspect ratio when the layout resizes it.
    pub aspect: Option<bool>,
    /// The window's border thickness and corner rounding.
    pub border: Option<Border>,
    /// Draw the window's drop shadow.
    pub shadow: Option<bool>,
}

/// Whether a button or a key goes down and stays there, or comes back up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// The button or the key goes down and stays there.
    Down,
    /// It comes back up.
    Up,
}

impl Held {
    /// Whether this holds the button or the key down.
    pub fn down(self) -> bool {
        self == Held::Down
    }
}

/// What a `drag` request asks: the press, the motion, and the release one
/// verb makes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Where the button goes down.
    pub from: Point,
    /// Where it comes back up.
    pub to: Point,
    /// The button the drag holds.
    pub button: Button,
    /// The modifiers held from before the press until after the release.
    pub modifiers: Modifiers,
    /// How the pointer travels between the two points.
    pub motion: Motion,
}

/// What a `scroll` request asks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    /// How far to scroll across, positive to the right.
    pub dx: f64,
    /// How far to scroll down, positive down.
    pub dy: f64,
    /// Send a wheel's notches rather than a trackpad's smooth axis.
    pub discrete: bool,
    /// How the scroll is spread out.
    pub motion: Motion,
}

/// What a `open` request asks.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    /// The program to start, with its arguments.
    pub command: Option<String>,
    /// The file to show.
    pub path: Option<String>,
    /// The desk to open it on, or the desk showing now.
    pub desk: Option<u32>,
    /// Open it without giving it the focus.
    pub silent: bool,
}

/// What a desk answers the protocol's verbs with. The compositor's event
/// loop implements it over the layout and the Wayland state, Coder
/// Desktop implements it over the panes on its shell screen, and a test
/// implements it over a fixture of tiles and needs no display.
pub trait Desk {
    /// Every window on every desk.
    fn windows(&mut self) -> Vec<Window>;
    /// Every screen, and the desk each one shows.
    fn screens(&mut self) -> Vec<Screen>;
    /// The window that has the keyboard focus.
    fn focused(&mut self) -> Option<Window>;
    /// Start a program in a new window, or show a file in a new pane.
    fn open(&mut self, open: Open) -> Result<(), Refusal>;
    /// Give the window the focus, switching the screen to its desk.
    fn focus(&mut self, handle: &Selector) -> Result<(), Refusal>;
    /// Move the window to a desk without switching the screen to it.
    fn place(&mut self, handle: &Selector, desk: u32) -> Result<(), Refusal>;
    /// Raise the window above the others on its desk.
    fn raise(&mut self, handle: &Selector) -> Result<(), Refusal>;
    /// Ask the window to close.
    fn close(&mut self, handle: &Selector) -> Result<(), Refusal>;
    /// Change how the compositor draws the window.
    fn shape(&mut self, handle: &Selector, shape: Shape) -> Result<(), Refusal>;
    /// Set the scale a screen draws at.
    fn scale(&mut self, screen: &str, scale: f64) -> Result<(), Refusal>;
    /// Raise a notice the operator reads.
    fn notice(&mut self, text: &str) -> Result<(), Refusal>;
    /// Reload the session's configuration.
    fn reload(&mut self) -> Result<(), Refusal>;
    /// Press one chord the way a press on the keyboard runs: a chord the
    /// bind table holds runs its action, and any other reaches the focused
    /// window. A desk that takes its keys from the host that draws its
    /// window, which is Coder Desktop, refuses.
    fn key(&mut self, chord: &Chord) -> Result<(), Refusal> {
        let _ = chord;
        Err(drives_nothing("key"))
    }
    /// Type text into the focused window, one key press per character.
    fn type_text(&mut self, text: &str) -> Result<(), Refusal> {
        let _ = text;
        Err(drives_nothing("type"))
    }
    /// Press and release a pointer button at a point on the screens.
    fn click(&mut self, at: Point, button: Button) -> Result<(), Refusal> {
        let _ = (at, button);
        Err(drives_nothing("click"))
    }
    /// Move the pointer to a point on the screens, in the steps and over
    /// the time the motion names.
    fn move_pointer(&mut self, at: Point, motion: Motion) -> Result<(), Refusal> {
        let _ = (at, motion);
        Err(drives_nothing("move"))
    }
    /// Hold a pointer button down where the pointer is, or let it go.
    fn button(&mut self, button: Button, held: Held) -> Result<(), Refusal> {
        let _ = (button, held);
        Err(drives_nothing("press"))
    }
    /// Hold one key or one modifier down, or let it go, so a modifier is
    /// held across a drag and a chord is built by hand.
    fn stroke(&mut self, stroke: &Stroke, held: Held) -> Result<(), Refusal> {
        let _ = (stroke, held);
        Err(drives_nothing("press"))
    }
    /// Press a button at one point, move to another, and let it go.
    fn drag(&mut self, drag: Drag) -> Result<(), Refusal> {
        let _ = drag;
        Err(drives_nothing("drag"))
    }
    /// Scroll where the pointer is.
    fn scroll(&mut self, scroll: Scroll) -> Result<(), Refusal> {
        let _ = scroll;
        Err(drives_nothing("scroll"))
    }
    /// Write a PNG of one screen to a path.
    fn shot(&mut self, path: &Path, screen: Option<&str>) -> Result<(), Refusal> {
        let _ = (path, screen);
        Err(drives_nothing("shot"))
    }
    /// Whether hands drive the desk. A desk with no hand tracking
    /// answers off.
    fn status(&mut self) -> Hands {
        Hands::default()
    }
}

/// The refusal a desk that drives no input of its own answers a drive verb
/// with. Coder Desktop draws its panes in a window the host draws, and the
/// host holds that window's keyboard, pointer, and screen.
fn drives_nothing(verb: &str) -> Refusal {
    Refusal::new(
        refusal::UNSUPPORTED,
        format!(
            "this desk takes its keys, its pointer, and its screen from the host that draws \
             its window, so it does not answer `{verb}`"
        ),
    )
}

/// The answer one request earns from one desk.
pub fn answer(request: Request, desk: &mut dyn Desk) -> Answer {
    if request.generation != GENERATION {
        return Answer::refused(
            refusal::UNSUPPORTED_GENERATION,
            format!(
                "this desk speaks generation {GENERATION}, the request named {}",
                request.generation
            ),
        );
    }
    match request.verb {
        Verb::List => Answer::new(Reply::Windows {
            windows: desk.windows(),
        }),
        Verb::Screens => Answer::new(Reply::Screens {
            screens: desk.screens(),
        }),
        Verb::Focused => Answer::new(Reply::Focused {
            window: desk.focused(),
        }),
        Verb::Open {
            command,
            path,
            desk: on,
            silent,
        } => {
            if command.is_some() == path.is_some() {
                return Answer::refused(
                    refusal::MALFORMED,
                    "an open names exactly one of command and path",
                );
            }
            done(desk.open(Open {
                command,
                path,
                desk: on,
                silent,
            }))
        }
        Verb::Focus { handle } => done(desk.focus(&handle)),
        Verb::Place { handle, desk: on } => done(desk.place(&handle, on)),
        Verb::Raise { handle } => done(desk.raise(&handle)),
        Verb::Close { handle } => done(desk.close(&handle)),
        Verb::Shape {
            handle,
            float,
            pin,
            at,
            size,
            aspect,
            border,
            shadow,
        } => done(desk.shape(
            &handle,
            Shape {
                float,
                pin,
                at,
                size,
                aspect,
                border,
                shadow,
            },
        )),
        Verb::Scale { screen, scale } => done(desk.scale(&screen, scale)),
        Verb::Notice { text } => done(desk.notice(&text)),
        Verb::Reload => done(desk.reload()),
        Verb::Key { chord } => match Chord::parse(&chord) {
            Ok(chord) => done(desk.key(&chord)),
            Err(why) => Answer::refused(refusal::MALFORMED, why),
        },
        Verb::Type { text } => done(desk.type_text(&text)),
        Verb::Click { x, y, button } => done(desk.click(Point { x, y }, button)),
        Verb::Move { x, y, steps, ms } => match Motion::read(steps, ms, Motion::JUMP) {
            Ok(motion) => done(desk.move_pointer(Point { x, y }, motion)),
            Err(why) => Answer::refused(refusal::MALFORMED, why),
        },
        Verb::ButtonPress { button } => done(desk.button(button, Held::Down)),
        Verb::ButtonRelease { button } => done(desk.button(button, Held::Up)),
        Verb::KeyPress { key } => held_key(desk, &key, Held::Down),
        Verb::KeyRelease { key } => held_key(desk, &key, Held::Up),
        Verb::Drag {
            from,
            to,
            button,
            modifiers,
            steps,
            ms,
        } => {
            let modifiers = match Modifiers::parse(&modifiers) {
                Ok(modifiers) => modifiers,
                Err(why) => return Answer::refused(refusal::MALFORMED, why),
            };
            match Motion::read(steps, ms, Motion::HAND) {
                Ok(motion) => done(desk.drag(Drag {
                    from,
                    to,
                    button,
                    modifiers,
                    motion,
                })),
                Err(why) => Answer::refused(refusal::MALFORMED, why),
            }
        }
        Verb::Scroll {
            dx,
            dy,
            discrete,
            steps,
            ms,
        } => {
            if !dx.is_finite() || !dy.is_finite() {
                return Answer::refused(refusal::MALFORMED, "a scroll names two whole distances");
            }
            match Motion::read(steps, ms, Motion::JUMP) {
                Ok(motion) => done(desk.scroll(Scroll {
                    dx,
                    dy,
                    discrete,
                    motion,
                })),
                Err(why) => Answer::refused(refusal::MALFORMED, why),
            }
        }
        Verb::Shot { path, screen } => {
            if path.is_empty() {
                return Answer::refused(refusal::MALFORMED, "a shot names the file to write");
            }
            done(desk.shot(Path::new(&path), screen.as_deref()))
        }
        Verb::Status => Answer::new(Reply::Status {
            hands: desk.status(),
        }),
        Verb::Unknown => {
            Answer::refused(refusal::UNKNOWN_TYPE, "this desk does not know that verb")
        }
    }
}

/// The answer a `key_press` or a `key_release` earns: the stroke it names,
/// or the refusal that it names none.
fn held_key(desk: &mut dyn Desk, key: &str, held: Held) -> Answer {
    match Stroke::parse(key) {
        Ok(stroke) => done(desk.stroke(&stroke, held)),
        Err(why) => Answer::refused(refusal::MALFORMED, why),
    }
}

fn done(outcome: Result<(), Refusal>) -> Answer {
    match outcome {
        Ok(()) => Answer::done(),
        Err(refusal) => Answer::new(Reply::Refused(refusal)),
    }
}

/// One request the socket thread hands the desk's loop, with the channel
/// the answer goes back on.
pub struct Call {
    /// What the caller asked.
    pub request: Request,
    reply: Sender<Answer>,
}

impl Call {
    /// Sends the answer back to the socket thread. A caller that hung up
    /// drops the receiver, and the send fails without stopping the loop.
    pub fn answer(self, answer: Answer) {
        let _ = self.reply.send(answer);
    }

    /// The answer's channel on its own, for a desk that answers a request
    /// later than the call that read it: a `shot` answers once the screen's
    /// next frame is written.
    pub fn deferred(self) -> Deferred {
        Deferred { reply: self.reply }
    }
}

/// The channel one request's answer goes back on, kept past the call that
/// read the request. The socket thread waits [`ANSWER_TIMEOUT`] for it.
pub struct Deferred {
    reply: Sender<Answer>,
}

impl Deferred {
    /// Sends the answer back to the socket thread.
    pub fn answer(self, answer: Answer) {
        let _ = self.reply.send(answer);
    }
}

/// The listening socket, the thread that reads it, and the calls it hands
/// the desk's loop. Dropping it removes the socket file.
pub struct Server {
    path: PathBuf,
    /// Every request the socket read, in the order it read them.
    pub calls: Receiver<Call>,
}

impl Server {
    /// The path the desk announces as `CODER_DESK_SOCKET`.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The directory this process keeps its socket in when nothing names one.
///
/// A socket is a temporary file that a sandboxed client has to reach, so
/// the runtime directory is where it belongs. A Mac sets no
/// `XDG_RUNTIME_DIR`, and Coder Desktop is a desk there, so the fallback
/// is the one directory this product writes in. CoderQuest chose the same
/// fallback.
pub fn socket_home() -> Result<PathBuf, String> {
    if let Some(dir) = socket_dir() {
        return Ok(dir);
    }
    let home = std::env::var_os("HOME")
        .ok_or("neither XDG_RUNTIME_DIR nor HOME is set, so this session has no desk socket")?;
    Ok(Path::new(&home).join(".openagents").join(HOME_DIR))
}

/// The directory under `~/.openagents` a desk keeps its socket in when the
/// runtime directory is unset.
pub const HOME_DIR: &str = "desk";

/// What the socket thread calls after it hands the desk's loop a request,
/// so a loop that sleeps until something happens wakes to answer it.
pub type Wake = Box<dyn Fn() + Send>;

/// Binds `$XDG_RUNTIME_DIR/coder-desk/<pid>.sock` and starts reading it.
pub fn bind() -> Result<Server, String> {
    bind_waking(None)
}

/// Binds the same socket as [`bind`], and runs `wake`, when there is one,
/// after each request reaches the desk's loop.
pub fn bind_waking(wake: Option<Wake>) -> Result<Server, String> {
    let dir = socket_home()?;
    std::fs::create_dir_all(&dir).map_err(|err| format!("desk socket directory: {err}"))?;
    sweep(&dir);
    bind_at_waking(dir.join(format!("{}.sock", std::process::id())), wake)
}

/// Removes every `<pid>.sock` under `dir` whose process no longer runs.
///
/// A desk that was killed leaves its socket behind, and a caller that
/// reads the directory finds one file per session that ever ran there: 67
/// on one CoderOS host on 2026-09-17. CoderQuest sweeps its own the same
/// way. A pid
/// is read from `/proc`, so a host without one, which is a Mac, sweeps
/// nothing.
pub fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut swept = 0;
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".sock"))
            .and_then(|stem| stem.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !runs(pid) && std::fs::remove_file(entry.path()).is_ok() {
            swept += 1;
        }
    }
    if swept > 0 {
        eprintln!(
            "swept {swept} desk socket{} of sessions that ended from {}",
            if swept == 1 { "" } else { "s" },
            dir.display()
        );
    }
}

/// Whether a process with this pid runs, read from `/proc`. A host with no
/// `/proc` answers that every pid runs, so nothing is swept there.
fn runs(pid: u32) -> bool {
    if !Path::new("/proc").is_dir() {
        return true;
    }
    Path::new("/proc").join(pid.to_string()).is_dir()
}

/// Binds one path and starts reading it. A stale file at the path is
/// removed first, because a socket outlives the process that made it.
pub fn bind_at(path: PathBuf) -> Result<Server, String> {
    bind_at_waking(path, None)
}

/// Binds one path the way [`bind_at`] does, and runs `wake`, when there is
/// one, after each request reaches the desk's loop.
pub fn bind_at_waking(path: PathBuf, wake: Option<Wake>) -> Result<Server, String> {
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|err| format!("desk socket: {err}"))?;
    let (calls_out, calls) = mpsc::channel();
    let thread = std::thread::Builder::new().name("coder-desk".into());
    thread
        .spawn(move || read_socket(listener, calls_out, wake))
        .map_err(|err| format!("desk socket thread: {err}"))?;
    Ok(Server { path, calls })
}

fn read_socket(listener: UnixListener, calls: Sender<Call>, wake: Option<Wake>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if serve_one(stream, &calls, wake.as_deref()).is_err() {
            break;
        }
    }
}

/// Reads one request from one connection and writes one answer. The error
/// says the desk's loop is gone, which stops the socket thread.
fn serve_one(
    stream: UnixStream,
    calls: &Sender<Call>,
    wake: Option<&(dyn Fn() + Send)>,
) -> Result<(), ()> {
    let reader = BufReader::new(match stream.try_clone() {
        Ok(copy) => copy,
        Err(_) => return Ok(()),
    });
    let mut line = String::new();
    let mut taken = reader.take(MAX_LINE_BYTES as u64);
    if taken.read_line(&mut line).is_err() || line.trim().is_empty() {
        return Ok(());
    }
    let answer = match serde_json::from_str::<Request>(line.trim()) {
        Ok(request) => {
            let (reply, answers) = mpsc::channel();
            if calls.send(Call { request, reply }).is_err() {
                return Err(());
            }
            if let Some(wake) = wake {
                wake();
            }
            match answers.recv_timeout(ANSWER_TIMEOUT) {
                Ok(answer) => answer,
                Err(RecvTimeoutError::Timeout) => Answer::refused(
                    refusal::UNSUPPORTED,
                    "the desk did not answer in two seconds",
                ),
                Err(RecvTimeoutError::Disconnected) => return Err(()),
            }
        }
        Err(err) => Answer::refused(
            refusal::MALFORMED,
            format!("that line is not a request: {err}"),
        ),
    };
    write_answer(stream, &answer);
    Ok(())
}

fn write_answer(mut stream: UnixStream, answer: &Answer) {
    let Ok(mut body) = serde_json::to_string(answer) else {
        return;
    };
    body.push('\n');
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// One request over one connection, which is what a caller of the socket
/// does. It is for a desk's own tests; a program that speaks the protocol
/// asks through [`crate::Blocking`] instead.
#[cfg(any(test, feature = "test-support"))]
pub fn ask(path: &Path, verb: Verb) -> Result<Answer, String> {
    let mut stream =
        UnixStream::connect(path).map_err(|err| format!("connect to the desk socket: {err}"))?;
    let request = Request::new(verb);
    let mut line = serde_json::to_string(&request).map_err(|err| format!("encode: {err}"))?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|err| format!("write: {err}"))?;
    stream.flush().map_err(|err| format!("flush: {err}"))?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|err| format!("read: {err}"))?;
    serde_json::from_str(answer.trim()).map_err(|err| format!("decode: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Reply, Verb};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A desk of two windows on two desks, which answers without a
    /// display. It records what a change verb asked so a test can read it.
    #[derive(Default)]
    struct Fixture {
        focused: Vec<String>,
        placed: Vec<(String, u32)>,
    }

    fn window(handle: &str, app_id: &str, desk: u32) -> Window {
        Window {
            handle: handle.to_string(),
            app_id: app_id.to_string(),
            title: format!("{app_id} window"),
            pid: Some(4242),
            screen: "nested-1".to_string(),
            desk,
            at: Point { x: 6, y: 6 },
            size: Size {
                width: 200,
                height: 300,
            },
            floating: false,
            pinned: false,
            fullscreen: false,
        }
    }

    impl Desk for Fixture {
        fn windows(&mut self) -> Vec<Window> {
            vec![window("0x1", "foot", 1), window("0x2", "coder-quest", 2)]
        }

        fn screens(&mut self) -> Vec<Screen> {
            vec![Screen {
                name: "nested-1".to_string(),
                at: Point { x: 0, y: 0 },
                size: Size {
                    width: 1280,
                    height: 800,
                },
                scale: 1.0,
                desk: 1,
            }]
        }

        fn focused(&mut self) -> Option<Window> {
            Some(window("0x1", "foot", 1))
        }

        fn open(&mut self, _open: Open) -> Result<(), Refusal> {
            Ok(())
        }

        fn focus(&mut self, handle: &Selector) -> Result<(), Refusal> {
            self.focused.push(handle.wire());
            Ok(())
        }

        fn place(&mut self, handle: &Selector, desk: u32) -> Result<(), Refusal> {
            self.placed.push((handle.wire(), desk));
            Ok(())
        }

        fn raise(&mut self, _handle: &Selector) -> Result<(), Refusal> {
            Ok(())
        }

        fn close(&mut self, _handle: &Selector) -> Result<(), Refusal> {
            Ok(())
        }

        fn shape(&mut self, _handle: &Selector, _shape: Shape) -> Result<(), Refusal> {
            Ok(())
        }

        fn scale(&mut self, _screen: &str, _scale: f64) -> Result<(), Refusal> {
            Ok(())
        }

        fn notice(&mut self, _text: &str) -> Result<(), Refusal> {
            Ok(())
        }

        fn reload(&mut self) -> Result<(), Refusal> {
            Ok(())
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A socket path no other test in this process uses.
    fn socket_path() -> PathBuf {
        let count = NEXT.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "coder-compositor-desk-{}-{count}.sock",
            std::process::id()
        ))
    }

    /// Answers `count` requests from one fixture and hands the fixture
    /// back, so a test can read what the change verbs recorded.
    fn worker(server: Server, count: usize) -> std::thread::JoinHandle<Fixture> {
        std::thread::spawn(move || {
            let mut desk = Fixture::default();
            for _ in 0..count {
                let Ok(call) = server.calls.recv() else { break };
                let reply = answer(call.request.clone(), &mut desk);
                call.answer(reply);
            }
            desk
        })
    }

    #[test]
    fn the_socket_answers_list_with_every_tile() {
        let path = socket_path();
        let server = bind_at(path.clone()).expect("the socket binds");
        let answering = worker(server, 1);
        let answer = ask(&path, Verb::List).expect("the desk answers");
        assert_eq!(answer.generation, GENERATION);
        match answer.reply {
            Reply::Windows { windows } => {
                let handles: Vec<String> = windows.iter().map(|w| w.handle.clone()).collect();
                assert_eq!(handles, vec!["0x1".to_string(), "0x2".to_string()]);
                let apps: Vec<String> = windows.iter().map(|w| w.app_id.clone()).collect();
                assert_eq!(apps, vec!["foot".to_string(), "coder-quest".to_string()]);
            }
            other => panic!("the desk answered {other:?}"),
        }
        let _ = answering.join();
    }

    #[test]
    fn the_socket_answers_screens_and_focused() {
        let path = socket_path();
        let server = bind_at(path.clone()).expect("the socket binds");
        let answering = worker(server, 2);
        match ask(&path, Verb::Screens).expect("the desk answers").reply {
            Reply::Screens { screens } => {
                assert_eq!(screens.len(), 1);
                assert_eq!(screens[0].name, "nested-1");
                assert_eq!(screens[0].size.width, 1280);
            }
            other => panic!("the desk answered {other:?}"),
        }
        match ask(&path, Verb::Focused).expect("the desk answers").reply {
            Reply::Focused { window } => {
                assert_eq!(window.map(|w| w.handle), Some("0x1".to_string()));
            }
            other => panic!("the desk answered {other:?}"),
        }
        let _ = answering.join();
    }

    #[test]
    fn focus_and_place_reach_the_desk() {
        let path = socket_path();
        let server = bind_at(path.clone()).expect("the socket binds");
        let answering = worker(server, 2);
        let focused = ask(
            &path,
            Verb::Focus {
                handle: Selector::parse("class:foot"),
            },
        )
        .expect("the desk answers");
        assert_eq!(focused.reply, Reply::Done);
        let placed = ask(
            &path,
            Verb::Place {
                handle: Selector::parse("0x2"),
                desk: 3,
            },
        )
        .expect("the desk answers");
        assert_eq!(placed.reply, Reply::Done);
        let desk = answering.join().expect("the fixture comes back");
        assert_eq!(desk.focused, vec!["class:foot".to_string()]);
        assert_eq!(desk.placed, vec![("0x2".to_string(), 3)]);
    }

    #[test]
    fn another_generation_is_refused() {
        let mut desk = Fixture::default();
        let request = Request {
            generation: GENERATION + 1,
            verb: Verb::List,
        };
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => {
                assert_eq!(refusal.code, refusal::UNSUPPORTED_GENERATION);
            }
            other => panic!("the desk answered {other:?}"),
        }
    }

    #[test]
    fn an_open_that_names_both_a_command_and_a_path_is_refused() {
        let mut desk = Fixture::default();
        let request = Request::new(Verb::Open {
            command: Some("foot".to_string()),
            path: Some("/tmp/a".to_string()),
            desk: None,
            silent: false,
        });
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::MALFORMED),
            other => panic!("the desk answered {other:?}"),
        }
    }

    #[test]
    fn a_desk_that_drives_nothing_refuses_the_drive_verbs_and_says_why() {
        let mut desk = Fixture::default();
        for verb in [
            Verb::Key {
                chord: "super+t".to_string(),
            },
            Verb::Type {
                text: "hi".to_string(),
            },
            Verb::Click {
                x: 1,
                y: 2,
                button: Button::Left,
            },
            Verb::Move {
                x: 1,
                y: 2,
                steps: None,
                ms: None,
            },
            Verb::ButtonPress {
                button: Button::Left,
            },
            Verb::ButtonRelease {
                button: Button::Left,
            },
            Verb::KeyPress {
                key: "super".to_string(),
            },
            Verb::KeyRelease {
                key: "super".to_string(),
            },
            Verb::Drag {
                from: Point { x: 1, y: 2 },
                to: Point { x: 3, y: 4 },
                button: Button::Left,
                modifiers: String::new(),
                steps: None,
                ms: None,
            },
            Verb::Scroll {
                dx: 0.0,
                dy: 1.0,
                discrete: false,
                steps: None,
                ms: None,
            },
            Verb::Shot {
                path: "/tmp/a.png".to_string(),
                screen: None,
            },
        ] {
            match answer(Request::new(verb), &mut desk).reply {
                Reply::Refused(refusal) => {
                    assert_eq!(refusal.code, refusal::UNSUPPORTED);
                    assert!(refusal.message.contains("host"), "{}", refusal.message);
                }
                other => panic!("the desk answered {other:?}"),
            }
        }
    }

    #[test]
    fn a_key_that_is_not_a_chord_and_a_shot_with_no_path_are_malformed() {
        let mut desk = Fixture::default();
        let request = Request::new(Verb::Key {
            chord: "hyper+t".to_string(),
        });
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::MALFORMED),
            other => panic!("the desk answered {other:?}"),
        }
        let request = Request::new(Verb::Shot {
            path: String::new(),
            screen: None,
        });
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::MALFORMED),
            other => panic!("the desk answered {other:?}"),
        }
    }

    #[test]
    fn a_drag_and_a_press_that_name_a_modifier_this_side_cannot_read_are_malformed() {
        let mut desk = Fixture::default();
        for verb in [
            Verb::Drag {
                from: Point { x: 1, y: 2 },
                to: Point { x: 3, y: 4 },
                button: Button::Left,
                modifiers: "hyper".to_string(),
                steps: None,
                ms: None,
            },
            Verb::KeyPress {
                key: "  ".to_string(),
            },
            Verb::Move {
                x: 1,
                y: 2,
                steps: Some(0),
                ms: None,
            },
            Verb::Scroll {
                dx: 0.0,
                dy: 1.0,
                discrete: false,
                steps: None,
                ms: Some(9_000),
            },
        ] {
            match answer(Request::new(verb.clone()), &mut desk).reply {
                Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::MALFORMED, "{verb:?}"),
                other => panic!("the desk answered {other:?}"),
            }
        }
    }

    #[test]
    fn a_drag_reaches_the_desk_with_the_motion_a_hand_makes() {
        /// A desk that records the drags and the holds it was asked for.
        #[derive(Default)]
        struct Held {
            dragged: Vec<Drag>,
            strokes: Vec<(Stroke, bool)>,
        }

        impl Desk for Held {
            fn windows(&mut self) -> Vec<Window> {
                Vec::new()
            }
            fn screens(&mut self) -> Vec<Screen> {
                Vec::new()
            }
            fn focused(&mut self) -> Option<Window> {
                None
            }
            fn open(&mut self, _open: Open) -> Result<(), Refusal> {
                Ok(())
            }
            fn focus(&mut self, _handle: &Selector) -> Result<(), Refusal> {
                Ok(())
            }
            fn place(&mut self, _handle: &Selector, _desk: u32) -> Result<(), Refusal> {
                Ok(())
            }
            fn raise(&mut self, _handle: &Selector) -> Result<(), Refusal> {
                Ok(())
            }
            fn close(&mut self, _handle: &Selector) -> Result<(), Refusal> {
                Ok(())
            }
            fn shape(&mut self, _handle: &Selector, _shape: Shape) -> Result<(), Refusal> {
                Ok(())
            }
            fn scale(&mut self, _screen: &str, _scale: f64) -> Result<(), Refusal> {
                Ok(())
            }
            fn notice(&mut self, _text: &str) -> Result<(), Refusal> {
                Ok(())
            }
            fn reload(&mut self) -> Result<(), Refusal> {
                Ok(())
            }
            fn drag(&mut self, drag: Drag) -> Result<(), Refusal> {
                self.dragged.push(drag);
                Ok(())
            }
            fn stroke(&mut self, stroke: &Stroke, held: super::Held) -> Result<(), Refusal> {
                self.strokes.push((stroke.clone(), held.down()));
                Ok(())
            }
        }

        let mut desk = Held::default();
        let answered = answer(
            Request::new(Verb::Drag {
                from: Point { x: 10, y: 20 },
                to: Point { x: 110, y: 20 },
                button: Button::Right,
                modifiers: "SUPER".to_string(),
                steps: None,
                ms: None,
            }),
            &mut desk,
        );
        assert_eq!(answered.reply, Reply::Done);
        let drag = desk.dragged.first().copied().expect("a drag");
        assert_eq!(drag.button, Button::Right);
        assert!(drag.modifiers.holds(crate::protocol::Modifier::Super));
        assert_eq!(
            drag.motion,
            Motion::HAND,
            "a drag that names no motion runs what a hand runs"
        );
        for (key, down) in [("super", true), ("super", false)] {
            let verb = match down {
                true => Verb::KeyPress {
                    key: key.to_string(),
                },
                false => Verb::KeyRelease {
                    key: key.to_string(),
                },
            };
            assert_eq!(answer(Request::new(verb), &mut desk).reply, Reply::Done);
        }
        assert_eq!(
            desk.strokes,
            vec![
                (Stroke::Modifier(crate::protocol::Modifier::Super), true),
                (Stroke::Modifier(crate::protocol::Modifier::Super), false),
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_sweep_removes_the_socket_of_a_session_that_ended_and_keeps_the_rest() {
        let dir = std::env::temp_dir().join(format!("coder-desk-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the directory");
        let dead = {
            let mut child = std::process::Command::new("true")
                .spawn()
                .expect("a child runs");
            let pid = child.id();
            let _ = child.wait();
            pid
        };
        let stale = dir.join(format!("{dead}.sock"));
        let mine = dir.join(format!("{}.sock", std::process::id()));
        let other = dir.join("notes.txt");
        for path in [&stale, &mine, &other] {
            std::fs::write(path, "").expect("a file");
        }
        sweep(&dir);
        assert!(
            !stale.exists(),
            "the socket of a session that ended is swept"
        );
        assert!(mine.exists(), "this process's own socket stays");
        assert!(other.exists(), "a file that is not a socket stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_verb_this_side_does_not_know_is_refused_and_the_socket_stays_up() {
        let mut desk = Fixture::default();
        let request = Request::new(Verb::Unknown);
        match answer(request, &mut desk).reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::UNKNOWN_TYPE),
            other => panic!("the desk answered {other:?}"),
        }
    }
}
