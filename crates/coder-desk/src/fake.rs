//! The desks a caller's tests run against.
//!
//! [`Session`] holds windows and screens in memory, answers the desk
//! protocol on a socket of its own, and records every request, so a test
//! reads what its caller asked for and what the desk did about it.
//! [`hyprland_session`] is the stand-in for a Hyprland session: a control
//! socket that answers each request from a list, which is what a test that
//! asserts the exact bytes a verb sends uses.
//!
//! Both are behind the `test-support` feature, so nothing a release builds
//! holds them.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use crate::{
    Announcement, Answer, Chord, Desk, GENERATION, Modifiers, Reply, Request, Screen, Selector,
    Stroke, Verb, Window, refusal,
};

/// A desk that answers the protocol from windows and screens it holds.
pub struct Session {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    /// Every request the desk was sent, in order.
    pub asked: mpsc::Receiver<Request>,
}

impl Session {
    /// A desk holding these windows and screens, with the focus on the
    /// first window it holds.
    pub fn holding(windows: Vec<Window>, screens: Vec<Screen>) -> Session {
        let dir = tempfile::Builder::new()
            .prefix("coder-desk-")
            .tempdir()
            .expect("a temporary directory");
        let socket = dir.path().join("desk.sock");
        let listener = UnixListener::bind(&socket).expect("the desk's socket");
        let (sender, asked) = mpsc::channel();
        let mut held = Held {
            focused: windows.first().map(|window| window.handle.clone()),
            windows,
            screens,
        };
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                held.serve(stream, &sender);
            }
        });
        Session {
            _dir: dir,
            socket,
            asked,
        }
    }

    /// The client for this desk.
    pub fn desk(&self) -> Desk {
        Desk::native(self.socket.clone())
    }

    /// The socket the desk answers on, which `CODER_DESK_SOCKET` would
    /// name.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Every request the desk has been sent since the last read.
    pub fn requests(&self) -> Vec<Request> {
        let mut out = Vec::new();
        while let Ok(request) = self.asked.try_recv() {
            out.push(request);
        }
        out
    }
}

/// What one fake desk holds.
struct Held {
    windows: Vec<Window>,
    screens: Vec<Screen>,
    focused: Option<String>,
}

impl Held {
    /// One connection: read a request, record it, answer it, close.
    fn serve(&mut self, stream: UnixStream, asked: &mpsc::Sender<Request>) {
        let mut line = String::new();
        // The reachability probe connects and asks nothing. It is not a
        // request, so it is not answered and not recorded.
        if BufReader::new(&stream).read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let answer = match serde_json::from_str::<Request>(line.trim()) {
            Ok(request) => {
                let answer = self.answer(&request);
                let _ = asked.send(request);
                answer
            }
            Err(error) => Answer::refused(refusal::MALFORMED, error.to_string()),
        };
        let mut line = serde_json::to_string(&answer).unwrap_or_default();
        line.push('\n');
        let _ = (&stream).write_all(line.as_bytes());
    }

    /// What this desk answers one request with.
    fn answer(&mut self, request: &Request) -> Answer {
        if request.generation != GENERATION {
            return Answer::refused(
                refusal::UNSUPPORTED_GENERATION,
                format!("this desk speaks generation {GENERATION}"),
            );
        }
        match &request.verb {
            Verb::List => Answer::new(Reply::Windows {
                windows: self.windows.clone(),
            }),
            Verb::Screens => Answer::new(Reply::Screens {
                screens: self.screens.clone(),
            }),
            Verb::Focused => Answer::new(Reply::Focused {
                window: self
                    .focused
                    .as_deref()
                    .and_then(|handle| self.windows.iter().find(|w| w.handle == handle))
                    .cloned(),
            }),
            Verb::Open { command, path, .. } => match (command, path) {
                (Some(_), None) | (None, Some(_)) => Answer::done(),
                _ => Answer::refused(
                    refusal::MALFORMED,
                    "an `open` names exactly one of a command and a path",
                ),
            },
            Verb::Focus { handle } => self.on(handle, |held, at| {
                held.focused = Some(held.windows[at].handle.clone());
            }),
            Verb::Place { handle, desk } => {
                let desk = *desk;
                self.on(handle, move |held, at| held.windows[at].desk = desk)
            }
            Verb::Raise { handle } => self.on(handle, |_, _| {}),
            Verb::Close { handle } => self.on(handle, |held, at| {
                held.windows.remove(at);
            }),
            Verb::Shape {
                handle,
                float,
                pin,
                at,
                size,
                ..
            } => {
                let (float, pin, put, size) = (*float, *pin, *at, *size);
                self.on(handle, move |held, index| {
                    let window = &mut held.windows[index];
                    window.floating = float.unwrap_or(window.floating);
                    window.pinned = pin.unwrap_or(window.pinned);
                    window.at = put.unwrap_or(window.at);
                    window.size = size.unwrap_or(window.size);
                })
            }
            Verb::Scale { screen, scale } => {
                match self.screens.iter_mut().find(|held| held.name == *screen) {
                    Some(held) => {
                        held.scale = *scale;
                        Answer::done()
                    }
                    None => Answer::refused(
                        refusal::NO_SUCH_SCREEN,
                        format!("this desk holds no screen named `{screen}`"),
                    ),
                }
            }
            Verb::Notice { .. } | Verb::Reload => Answer::done(),
            // The fake desk drives nothing: it records what it was asked
            // and answers that it went through, and a `shot` writes no
            // file. A chord that does not parse is refused the way a real
            // desk refuses it.
            Verb::Key { chord } => match Chord::parse(chord) {
                Ok(_) => Answer::done(),
                Err(why) => Answer::refused(refusal::MALFORMED, why),
            },
            Verb::Type { .. }
            | Verb::Click { .. }
            | Verb::Move { .. }
            | Verb::ButtonPress { .. }
            | Verb::ButtonRelease { .. }
            | Verb::Shot { .. } => Answer::done(),
            // A key and a drag name a modifier, and a spelling the
            // protocol does not read is refused the way a real desk
            // refuses it, so a caller's test reads its own mistake here.
            Verb::KeyPress { key } | Verb::KeyRelease { key } => match Stroke::parse(key) {
                Ok(_) => Answer::done(),
                Err(why) => Answer::refused(refusal::MALFORMED, why),
            },
            Verb::Drag { modifiers, .. } => match Modifiers::parse(modifiers) {
                Ok(_) => Answer::done(),
                Err(why) => Answer::refused(refusal::MALFORMED, why),
            },
            Verb::Scroll { .. } => Answer::done(),
            Verb::Status => Answer::new(Reply::Status {
                hands: crate::protocol::Hands::default(),
            }),
            Verb::Unknown => Answer::refused(
                refusal::UNKNOWN_TYPE,
                "this desk does not know that request",
            ),
        }
    }

    /// One change to the window a selector names, or the refusal that it
    /// names none.
    fn on(&mut self, handle: &Selector, change: impl FnOnce(&mut Held, usize)) -> Answer {
        match self.found(handle) {
            Some(at) => {
                change(self, at);
                Answer::done()
            }
            None => Answer::refused(
                refusal::NO_SUCH_WINDOW,
                format!("this desk holds no window `{handle}`"),
            ),
        }
    }

    /// Which window a selector names.
    fn found(&self, handle: &Selector) -> Option<usize> {
        self.windows.iter().position(|window| match handle {
            Selector::Address(address) => window.handle == *address,
            Selector::Class(class) => window.app_id == *class,
            Selector::Title(title) => window.title == *title,
        })
    }
}

/// A stand-in for a Hyprland session: a control socket that answers the
/// requests a session answers, and records what it was asked.
pub struct Hypr {
    _dir: tempfile::TempDir,
    /// What the session told the programs it starts.
    pub announced: Announcement,
    /// Every request the session was sent, in order.
    pub asked: mpsc::Receiver<String>,
}

impl Hypr {
    /// The client for this session.
    pub fn desk(&self) -> Result<Desk, crate::Absent> {
        Desk::found(&self.announced)
    }

    /// Every request the session has been sent since the last read.
    pub fn requests(&self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(request) = self.asked.try_recv() {
            out.push(request);
        }
        out
    }
}

/// Bind a control socket where an announcement says one is, and answer each
/// request from the list, matching on the request's own text.
pub fn hyprland_session(answers: Vec<(&'static str, &'static str)>) -> Hypr {
    let dir = tempfile::Builder::new()
        .prefix("coder-desk-hypr-")
        .tempdir()
        .expect("a temporary directory");
    let home = dir.path().join("hypr").join("s");
    std::fs::create_dir_all(&home).expect("the session directory");
    let socket = home.join(".socket.sock");
    let listener = UnixListener::bind(&socket).expect("the control socket");
    let (sender, asked) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buffer = [0u8; 4096];
            let Ok(read) = stream.read(&mut buffer) else {
                continue;
            };
            // The reachability probe connects and asks nothing. It is not a
            // request, so it is not answered and not recorded.
            if read == 0 {
                continue;
            }
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let answer = answers
                .iter()
                .find(|(asked, _)| request.starts_with(asked))
                .map(|(_, answer)| *answer)
                .unwrap_or("");
            let _ = stream.write_all(answer.as_bytes());
            let _ = sender.send(request);
        }
    });
    Hypr {
        _dir: dir,
        announced: Announcement {
            desk_socket: None,
            signature: Some("s".into()),
            runtime_dir: Some(runtime_dir_of(&socket)),
            quest_socket: None,
            panes: false,
        },
        asked,
    }
}

/// The runtime directory a control socket sits three levels under.
fn runtime_dir_of(socket: &Path) -> PathBuf {
    socket
        .ancestors()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or_default()
}
