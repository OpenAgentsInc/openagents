//! Hand tracking as desk input: the reader on the camera daemon's hands
//! socket, and what the loop does with each frame.
//!
//! `coderos-camera` publishes one JSON line a frame on
//! `$XDG_RUNTIME_DIR/coderos-camera/hands.sock` while its tracker is on.
//! A thread here connects to it, parses each line with `coder_hands::wire::Line`,
//! and hands it to the loop over a channel; it reconnects while the daemon
//! is away and logs each change of state once. The loop feeds every line
//! to `coder_hands::gestures` and applies what comes back through the same
//! functions a mouse and a keyboard use in `crate::input`, so a pinch is a
//! press to the client and a swipe is the desk chord.
//!
//! Super+H toggles it, and so does the host: a session whose grant names
//! `hands` among its launchers starts with tracking on. Turning it on asks
//! the daemon for `hands on` over its control socket, and turning it off
//! asks for `hands off`, both from the reader's thread so a daemon that
//! is fetching its model never holds the loop.
//!
//! The session starts the compositor before it starts the camera daemon,
//! so at a login the first `hands on` has nothing to reach. The reader
//! therefore asks again before each attempt at the landmark socket until
//! the daemon takes it, and again after a daemon goes away, because a
//! daemon that comes back comes back with its tracker off. Asking once
//! left the host with a reader attached to a socket nothing published
//! on.
//!
//! Beside the rules sits the Jev seam in `coder_hands::watch`, which the
//! host turns on with `coderos.desktop.hands.judge`. It reads the same
//! frames, asks about the windows the rules cannot settle on a thread of
//! its own, and in shadow records the answers and changes nothing.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use coder_hands::Landmark;
use coder_hands::gestures::{self, Act, Gestures, Label};
use coder_hands::socket::{CAPTURE_VAR, Sockets, TRACK_OFF, TRACK_ON, ask};
use coder_hands::watch::Watch;
use coder_hands::wire::Line;
use coder_wm::Dir;

use crate::binds::Action;
use crate::hands_overlay::{self, Picture};
use crate::input;
use crate::state::Coder;

/// The launcher option that grants hands, the `option` of the Super+H row
/// in `crates/coder-binds` and the name `desktop.nix` writes into the
/// grant when `coderos.desktop.hands` is on.
pub const OPTION: &str = "hands";

/// How long the reader waits before it tries the socket again.
const RECONNECT: Duration = Duration::from_millis(500);
/// The evdev code of the left button.
const BTN_LEFT: u32 = 0x110;
/// The evdev code of Escape.
const KEY_ESC: u32 = 1;

/// What the reader's thread sends the loop.
enum Read {
    /// One frame off the socket.
    Line(Line),
    /// The reader connected, or lost the daemon.
    Linked(bool),
}

/// The reader, the gesture machine, and what the overlay draws.
pub struct Hands {
    sockets: Sockets,
    on: bool,
    linked: bool,
    rx: Option<Receiver<Read>>,
    stop: Option<Arc<AtomicBool>>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    gestures: Gestures,
    /// The Jev seam beside the rules. Off unless the session's
    /// configuration turns it on, and then it asks and records while the
    /// rules keep the desk.
    judge: Watch,
    label: Label,
    margin: f32,
    /// The smoothed hand of the last frame, which the overlay draws.
    hand_drawn: Option<[Landmark; 21]>,
    /// What the overlay draws, in logical pixels of the focused screen.
    /// The frame takes a handle rather than a copy, because the rasters
    /// are redrawn a frame at a time and read on every draw.
    pictures: Arc<Vec<Picture>>,
}

impl Hands {
    /// Tracking off, reading the sockets and the camera's aspect from the
    /// environment.
    pub fn from_environment() -> Hands {
        let value = |name: &str| std::env::var(name).ok();
        let mut hands = Hands::new(
            Sockets::from_environment(value),
            gestures::aspect_from(std::env::var(CAPTURE_VAR).ok()),
        );
        hands.judge = Watch::from_environment();
        hands
    }

    /// Tracking off, on these sockets.
    pub fn new(sockets: Sockets, aspect: f32) -> Hands {
        Hands {
            sockets,
            on: false,
            linked: false,
            rx: None,
            stop: None,
            wake: None,
            gestures: Gestures::new(aspect),
            judge: Watch::off(),
            label: Label::None,
            margin: 0.0,
            hand_drawn: None,
            pictures: Arc::new(Vec::new()),
        }
    }

    /// Whether hands drive the desk.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// What the overlay draws.
    pub fn pictures(&self) -> &Arc<Vec<Picture>> {
        &self.pictures
    }

    /// What the reader's thread calls when a frame arrives, so a loop that
    /// waits on its sources wakes for it. The nested loop polls instead
    /// and sets none.
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }

    /// Starts the reader and asks the daemon to track.
    pub fn turn_on(&mut self) {
        if self.on {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let reader = Reader {
            sockets: self.sockets.clone(),
            tx,
            stop: Arc::clone(&stop),
            wake: self.wake.clone(),
        };
        match thread::Builder::new()
            .name("hands-reader".into())
            .spawn(move || reader.run())
        {
            Ok(_) => {
                self.rx = Some(rx);
                self.stop = Some(stop);
                self.on = true;
                log::info!(
                    "hands drive the desk: reading {}, asking {}",
                    self.sockets.hands.display(),
                    self.sockets.control.display()
                );
            }
            Err(err) => log::warn!("the hands reader did not start: {err}"),
        }
    }

    /// Stops the reader, asks the daemon to stop tracking, and answers
    /// the acts that end what the hand was doing.
    pub fn turn_off(&mut self) -> Vec<Act> {
        if !self.on {
            return Vec::new();
        }
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.rx = None;
        self.on = false;
        self.linked = false;
        self.label = Label::None;
        self.margin = 0.0;
        self.hand_drawn = None;
        self.pictures = Arc::new(Vec::new());
        self.judge.reset();
        let control = self.sockets.control.clone();
        let _ = thread::Builder::new()
            .name("hands-off".into())
            .spawn(move || {
                if let Err(err) = ask(&control, TRACK_OFF) {
                    log::info!("the camera daemon was not told hands off: {err}");
                }
            });
        let counts = self.judge.counts();
        if counts.ambiguous() > 0 {
            log::info!(
                "hands: the rules could not settle {} window(s). The seam asked about {}; \
                 the trigger held {} back, {} found a request in flight, and {} found no seam.",
                counts.ambiguous(),
                counts.asked,
                counts.paced,
                counts.in_flight,
                counts.closed
            );
        }
        log::info!("hands no longer drive the desk");
        self.gestures.reset()
    }

    /// Reads every frame the thread sent since the last pass, and answers
    /// the acts they produced with the hand the overlay draws.
    fn take(&mut self) -> Vec<Act> {
        let mut acts = Vec::new();
        let mut hand = None;
        let mut drew = false;
        let Some(rx) = self.rx.as_ref() else {
            return acts;
        };
        loop {
            let read = match rx.try_recv() {
                Ok(read) => read,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            };
            match read {
                Read::Linked(linked) => {
                    self.linked = linked;
                    drew = true;
                    if !linked {
                        acts.extend(self.gestures.reset());
                        hand = None;
                    }
                }
                Read::Line(line) => {
                    let step = self.gestures.feed(&line);
                    for text in &step.log {
                        log::info!("{text}");
                    }
                    for act in &step.acts {
                        if !matches!(act, Act::Point(..) | Act::Drag(..)) {
                            log::info!("hands: {}", act.word());
                        }
                    }
                    self.judge.feed(&line, &step, self.gestures.pointer());
                    self.judge.acted(&step.acts);
                    acts.extend(step.acts);
                    self.label = step.label;
                    self.margin = step.margin;
                    hand = step.hand;
                    drew = true;
                }
            }
        }
        // What the seam answered about an earlier window. It is nothing
        // until the host turns the seam on, and nothing but a record
        // until the rollout reaches the rung that acts.
        acts.extend(self.judge.take());
        if drew {
            self.hand_drawn = hand;
        }
        acts
    }
}

/// The reader's thread.
struct Reader {
    sockets: Sockets,
    tx: Sender<Read>,
    stop: Arc<AtomicBool>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Reader {
    fn run(self) {
        let mut told = false;
        let mut said_absent = false;
        let mut refusal: Option<String> = None;
        while !self.stop.load(Ordering::Relaxed) {
            // The daemon may not be up yet, and one that went away and
            // came back is tracking nothing, so the verb goes out until
            // it lands and again after every loss of the socket.
            if !told {
                match ask(&self.sockets.control, TRACK_ON) {
                    Ok(()) => {
                        told = true;
                        refusal = None;
                        log::info!("the camera daemon tracks hands");
                    }
                    Err(err) => {
                        if refusal.as_deref() != Some(err.as_str()) {
                            log::info!(
                                "the camera daemon was not told hands on: {err}; \
                                 asking again until it is"
                            );
                            refusal = Some(err);
                        }
                    }
                }
            }
            match UnixStream::connect(&self.sockets.hands) {
                Ok(stream) => {
                    said_absent = false;
                    log::info!("hands: connected to {}", self.sockets.hands.display());
                    if self.send(Read::Linked(true)).is_err() {
                        return;
                    }
                    let reason = self.read(stream);
                    if self.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    told = false;
                    log::info!(
                        "hands: {} went away ({reason}); reconnecting",
                        self.sockets.hands.display()
                    );
                    if self.send(Read::Linked(false)).is_err() {
                        return;
                    }
                }
                Err(err) => {
                    if !said_absent {
                        said_absent = true;
                        log::info!(
                            "hands: no daemon at {} ({err}); waiting for one",
                            self.sockets.hands.display()
                        );
                    }
                }
            }
            thread::sleep(RECONNECT);
        }
    }

    /// Reads lines until the socket ends or the reader is stopped, and
    /// says why it stopped.
    fn read(&self, stream: UnixStream) -> String {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
        let mut lines = BufReader::new(stream);
        let mut text = String::new();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return "stopped".to_string();
            }
            text.clear();
            match lines.read_line(&mut text) {
                Ok(0) => return "the daemon closed the socket".to_string(),
                Ok(_) => match Line::parse(&text) {
                    Ok(line) => {
                        if self.send(Read::Line(line)).is_err() {
                            return "the compositor stopped reading".to_string();
                        }
                    }
                    Err(err) => log::debug!("{err}"),
                },
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(err) => return err.to_string(),
            }
        }
    }

    fn send(&self, read: Read) -> Result<(), ()> {
        self.tx.send(read).map_err(|_| ())?;
        if let Some(wake) = &self.wake {
            wake();
        }
        Ok(())
    }
}

impl Coder {
    /// The Super+H chord: hands on, or off.
    pub fn toggle_hands(&mut self) {
        if self.hands.is_on() {
            let acts = self.hands.turn_off();
            self.apply_hands(acts);
        } else {
            self.hands.turn_on();
        }
    }

    /// Reads what the hands thread sent since the last pass and acts on
    /// it. Both loops call it: the nested loop every pass, the hardware
    /// loop when the reader wakes it.
    pub fn hands_pass(&mut self) {
        if !self.hands.is_on() {
            return;
        }
        let acts = self.hands.take();
        self.apply_hands(acts);
        self.draw_hands();
    }

    /// Applies each act through the input path a device takes.
    fn apply_hands(&mut self, acts: Vec<Act>) {
        for act in acts {
            match act {
                Act::Point(x, y) | Act::Drag(x, y) => {
                    if let Some(at) = self.hands_point(x, y) {
                        input::pointer_to(self, at);
                    }
                }
                Act::Press(x, y) => {
                    if let Some(at) = self.hands_point(x, y) {
                        input::pointer_to(self, at);
                    }
                    input::button(self, BTN_LEFT, true);
                }
                Act::Release => input::button(self, BTN_LEFT, false),
                Act::Swipe(dir) => {
                    let desk = self.manager.workspace();
                    let next = match dir {
                        Dir::Left => (desk + 1).min(8),
                        Dir::Right => desk.saturating_sub(1),
                        _ => desk,
                    };
                    if next != desk {
                        self.run(Action::Desk(next as u8 + 1));
                    }
                }
                Act::Escape => input::key_tap(self, KEY_ESC),
            }
        }
    }

    /// A fraction of the focused screen as a point in the shared space.
    fn hands_point(&self, x: f32, y: f32) -> Option<(f64, f64)> {
        let head = self.screens.focused()?;
        let size = head.logical();
        Some((
            f64::from(head.at.0) + f64::from(x) * f64::from(size.width),
            f64::from(head.at.1) + f64::from(y) * f64::from(size.height),
        ))
    }

    /// Rasterizes the overlay for the focused screen.
    fn draw_hands(&mut self) {
        let Some(head) = self.screens.focused() else {
            self.hands.pictures = Arc::new(Vec::new());
            return;
        };
        let size = head.logical();
        let screen = hands_overlay::Screen {
            x: head.at.0,
            y: head.at.1,
            width: size.width,
            height: size.height,
        };
        let status = if self.hands.linked {
            None
        } else {
            Some("hands: waiting for the camera daemon")
        };
        self.hands.pictures = Arc::new(hands_overlay::draw(
            screen,
            self.hands.hand_drawn.as_ref(),
            self.hands.label,
            self.hands.margin,
            self.hands.gestures.holding(),
            status,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// The session opens the compositor before it opens the camera
    /// daemon, so the first `hands on` reaches nothing. Before the reader
    /// asked again, the host was left with a reader attached to a socket whose tracker
    /// was never asked to run.
    #[test]
    fn a_daemon_that_starts_later_is_still_asked_to_track() {
        let dir = std::env::temp_dir().join(format!("coder-hands-late-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory");
        let sockets = Sockets {
            hands: dir.join("hands.sock"),
            control: dir.join("control.sock"),
        };
        let mut hands = Hands::new(sockets.clone(), 16.0 / 9.0);
        hands.turn_on();
        // No daemon at all: the first ask fails the way it does at a
        // login, and the reader carries on.
        thread::sleep(Duration::from_millis(100));
        assert!(hands.take().is_empty());

        // The daemon arrives. It has to be asked again, or its tracker
        // publishes nothing for the reader that is about to attach. The
        // wait is bounded, so a reader that never asks fails this test
        // rather than holding it.
        let listener =
            std::os::unix::net::UnixListener::bind(&sockets.control).expect("the control socket");
        listener
            .set_nonblocking(true)
            .expect("a listener that answers whether anyone is there");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut asked = None;
        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).expect("a blocking stream");
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut request = String::new();
                    BufReader::new(&stream)
                        .read_line(&mut request)
                        .expect("the request");
                    let mut answer = &stream;
                    answer
                        .write_all(b"{\"generation\":1,\"type\":\"done\"}\n")
                        .expect("the answer");
                    asked = Some(request);
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(err) => panic!("the control socket: {err}"),
            }
        }
        let asked = asked.expect("the reader asked the daemon that arrived late");
        assert_eq!(
            asked.trim(),
            "{\"generation\":1,\"type\":\"hands_on\"}",
            "the verb is coderos-camera's Verb::HandsOn"
        );
        let acts = hands.turn_off();
        assert!(acts.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_verbs_are_the_daemons_own() {
        assert_eq!(TRACK_ON, "hands_on");
        assert_eq!(TRACK_OFF, "hands_off");
    }

    #[test]
    fn the_reader_reconnects_and_hands_the_loop_every_line() {
        let dir = std::env::temp_dir().join(format!("coder-hands-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory");
        let sockets = Sockets {
            hands: dir.join("hands.sock"),
            control: dir.join("control.sock"),
        };
        let mut hands = Hands::new(sockets.clone(), 16.0 / 9.0);
        hands.turn_on();
        assert!(hands.is_on());
        // No daemon yet: the reader waits, and a pass takes nothing.
        thread::sleep(Duration::from_millis(50));
        assert!(hands.take().is_empty());
        let listener = std::os::unix::net::UnixListener::bind(&sockets.hands).expect("bind");
        let (mut stream, _) = listener.accept().expect("the reader connects");
        let line = Line {
            timestamp: 100.0,
            hands: Vec::new(),
            status: "No hands detected".into(),
            dropped: 0,
        };
        stream.write_all(line.render().as_bytes()).expect("a line");
        drop(stream);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut linked = false;
        while std::time::Instant::now() < deadline {
            hands.take();
            if hands.linked {
                linked = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(linked, "the reader connected");
        // The socket closed, so the reader reports the daemon away and
        // tries again.
        let (mut again, _) = listener.accept().expect("the reader reconnects");
        again.write_all(line.render().as_bytes()).expect("a line");
        let acts = hands.turn_off();
        assert!(acts.is_empty());
        assert!(!hands.is_on());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
