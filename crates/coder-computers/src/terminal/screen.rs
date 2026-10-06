//! The phone terminal screen a linked host's **Terminal** control opens
//! (NIP-TERM), shared by the Coder and OpenAgents mobile libraries.
//!
//! The Computers surface resolves **Terminal** on a host only after the
//! shared authority check passes (the host is online and this device holds
//! the `terminal` right), then the mobile app calls [`Terminal::open`] with
//! the host key and label. The screen is a Rust Native view with its own
//! instance and revisions; taps return as activations.
//!
//! Rust owns the session, the emulator, the modifier latch, and every byte
//! sent. The native host reports the grid the HUD fits, forwards keystrokes
//! and the clipboard, and polls with `terminal_poll`, whose smaller packet
//! carries the view only when it changed. The session starts once the host
//! reports a size, so the shell opens at the size it is shown at.
//!
//! [`Terminal::reference`] names the terminal once attached. A screen
//! recreated after the app returns from the background, or after its
//! surface was torn down, passes it to [`Terminal::reattach`] and attaches
//! to the same terminal instead of opening another, optionally only to
//! watch it.

use crate::live::Terminals;
use crate::terminal::session::Session;
use crate::terminal::{Blocks, Model, Phase, Saved, SavedMember, TerminalIntent, view};
use coder_host::pty::wire::TerminalRef;
use coder_vt::{Key, Modifiers};
use rust_native::{Activation, ValidatedView};
use serde::Serialize;
use tokio::runtime::Handle;

/// What an accepted activation did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The screen changed or a key was sent.
    Updated,
    /// Return to the Computers surface.
    Closed,
}

/// The terminal screen's reply to a terminal request.
#[derive(Serialize)]
pub struct TerminalPacket {
    pub schema: &'static str,
    /// A terminal screen is open.
    pub open: bool,
    /// The current view's revision.
    pub revision: u64,
    /// The view, when newer than the revision the host said it has.
    pub view: Option<serde_json::Value>,
    /// The person asked to paste: the host reads its clipboard and sends
    /// the text with `terminal_paste`.
    pub paste: bool,
}

impl TerminalPacket {
    pub fn closed() -> Self {
        TerminalPacket {
            schema: "coder.mobile.terminal.v1",
            open: false,
            revision: 0,
            view: None,
            paste: false,
        }
    }
}

/// One terminal screen for one host.
pub struct Terminal {
    host: String,
    label: String,
    terminals: Option<Terminals>,
    runtime: Handle,
    /// The model before the host reported a size.
    idle: Option<Model>,
    session: Option<Session>,
    instance: String,
    revision: u64,
    drawn: u64,
    current: Option<ValidatedView<TerminalIntent>>,
    paste: bool,
    /// The terminal to attach to instead of opening one.
    target: Option<TerminalRef>,
}

/// A key the native host names: an editing key's name or one character.
fn named_key(name: &str) -> Option<Key> {
    Some(match name {
        "enter" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "backtab" => Key::BackTab,
        "escape" => Key::Escape,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "page_up" => Key::PageUp,
        "page_down" => Key::PageDown,
        "insert" => Key::Insert,
        "delete" => Key::Delete,
        _ => {
            if let Some(number) = name.strip_prefix('f')
                && let Ok(number @ 1..=12) = number.parse::<u8>()
            {
                return Some(Key::F(number));
            }
            let mut characters = name.chars();
            let character = characters.next()?;
            if characters.next().is_some() || character.is_control() {
                return None;
            }
            Key::Char(character)
        }
    })
}

impl Terminal {
    /// Open the terminal screen for `host`. The caller has already checked
    /// the host's `terminal` right; the host checks it again on
    /// `terminal.open`. Without `terminals` (a build with no live service)
    /// the screen says it can't open one.
    pub fn open(
        host: String,
        label: String,
        terminals: Option<Terminals>,
        runtime: Handle,
    ) -> Result<Self, String> {
        let mut idle = Model::new(host.clone(), label.clone(), 24, 80);
        if terminals.is_none() {
            idle.phase = Phase::Refused(
                "This build can't reach computers, so it can't open a terminal.".into(),
            );
        }
        let mut terminal = Terminal {
            host,
            label,
            terminals,
            runtime,
            idle: Some(idle),
            session: None,
            instance: format!("terminal:{}", coder_connect::protocol::random_id()),
            revision: 0,
            drawn: 0,
            current: None,
            paste: false,
            target: None,
        };
        terminal.redraw();
        if terminal.current.is_none() {
            return Err("The terminal screen could not be drawn.".into());
        }
        Ok(terminal)
    }

    /// Open the screen on a terminal the host already runs, named by
    /// [`Terminal::reference`] from an earlier screen, and attach to it
    /// once the native host reports a size. With `watch` the screen
    /// attaches in `observe` mode and sends nothing.
    pub fn reattach(
        host: String,
        label: String,
        terminals: Option<Terminals>,
        runtime: Handle,
        reference: (String, String),
        watch: bool,
    ) -> Result<Self, String> {
        let mut terminal = Self::open(host, label, terminals, runtime)?;
        let (generation, id) = reference;
        terminal.target = Some(TerminalRef {
            generation,
            terminal: id,
        });
        terminal.with_model(|model| {
            model.watch = watch;
            model.touch();
        });
        terminal.redraw();
        Ok(terminal)
    }

    /// The terminal this screen is attached to, as `(generation,
    /// terminal)`, once the host named it.
    pub fn reference(&mut self) -> Option<(String, String)> {
        self.with_model(|model| model.reference.clone())
    }

    fn with_model<T>(&mut self, work: impl FnOnce(&mut Model) -> T) -> T {
        match (&self.session, &mut self.idle) {
            (Some(session), _) => work(&mut session.model()),
            (None, Some(model)) => work(model),
            (None, None) => {
                let mut model = Model::new(self.host.clone(), self.label.clone(), 24, 80);
                let result = work(&mut model);
                self.idle = Some(model);
                result
            }
        }
    }

    /// Draw a new revision when the model changed.
    pub fn redraw(&mut self) {
        let instance = self.instance.clone();
        let model_revision = self.with_model(|model| model.revision);
        if self.current.is_some() && model_revision == self.drawn {
            return;
        }
        let next = self.revision + 1;
        let validated = self.with_model(|model| view(model, &instance, next).validate());
        if let Ok(validated) = validated {
            self.revision = next;
            self.drawn = model_revision;
            self.current = Some(validated);
        }
    }

    /// The current view, as JSON for the native host and the world HUD.
    pub fn view(&self) -> Option<serde_json::Value> {
        self.current
            .as_ref()
            .and_then(|view| serde_json::to_value(view.view()).ok())
    }

    /// Whether the screen asked the native host for the clipboard.
    pub fn wants_paste(&self) -> bool {
        self.paste
    }

    /// Start the session at the reported size, or resize it.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if let Some(session) = &self.session {
            session.resize(rows, cols);
            return;
        }
        let Some(mut model) = self.idle.take() else {
            return;
        };
        model.resize(rows, cols);
        match &self.terminals {
            Some(terminals) if !model.phase.ended() => {
                let links = terminals.links(&self.host);
                self.session = Some(match self.target.clone() {
                    Some(reference) => Session::attach(&self.runtime, links, model, reference),
                    None => Session::start(&self.runtime, links, model),
                });
            }
            _ => self.idle = Some(model),
        }
    }

    fn send(&mut self, bytes: Vec<u8>) {
        match &self.session {
            Some(session) => session.send(bytes),
            None => self.with_model(|model| {
                model.notice = Some("Not connected. What you typed wasn't sent.".into());
                model.touch();
            }),
        }
    }

    pub fn text(&mut self, text: &str) {
        let bytes = self.with_model(|model| model.text(text));
        self.send(bytes);
    }

    pub fn key(&mut self, name: &str, modifiers: Modifiers) {
        let Some(key) = named_key(name) else {
            return;
        };
        let bytes = self.with_model(|model| model.key(key, modifiers));
        self.send(bytes);
    }

    pub fn paste(&mut self, text: &str) {
        self.paste = false;
        let bytes = self.with_model(|model| model.vt.paste(text));
        if !bytes.is_empty() {
            self.send(bytes);
        }
    }

    /// Resolve a tap against the current view.
    pub fn activate(&mut self, event: &Activation) -> Result<Outcome, String> {
        let intent = self
            .current
            .as_ref()
            .ok_or("The terminal screen is unavailable.")?
            .activate(event)
            .map_err(|_| "The terminal screen changed. Try again.".to_owned())?
            .clone();
        match intent {
            TerminalIntent::Key { key } => {
                let bytes = self.with_model(|model| model.key(key.key(), Modifiers::NONE));
                self.send(bytes);
            }
            TerminalIntent::Ctrl => self.with_model(|model| {
                model.ctrl = !model.ctrl;
                model.touch();
            }),
            TerminalIntent::Interrupt => {
                let bytes = self.with_model(|model| {
                    model.ctrl = false;
                    model.vt.key(Key::Char('c'), Modifiers::CTRL)
                });
                self.send(bytes);
            }
            TerminalIntent::Paste => self.paste = true,
            TerminalIntent::Take => {
                if let Some(session) = &self.session {
                    session.take();
                }
            }
            TerminalIntent::Close => {
                if let Some(session) = &self.session {
                    session.close();
                }
            }
            TerminalIntent::Blocks | TerminalIntent::OlderBlocks { .. } => {
                let before = match intent {
                    TerminalIntent::OlderBlocks { before } => Some(before),
                    _ => None,
                };
                match &self.session {
                    Some(session) => session.blocks(before),
                    None => self.with_model(|model| {
                        model.blocks = Blocks::Unavailable("Not connected.".into());
                        model.touch();
                    }),
                }
            }
            TerminalIntent::HideBlocks => match &self.session {
                Some(session) => session.hide_blocks(),
                None => self.with_model(|model| {
                    model.blocks = Blocks::Hidden;
                    model.touch();
                }),
            },
            TerminalIntent::Saved | TerminalIntent::OpenSaved { .. } => {
                let session = match intent {
                    TerminalIntent::OpenSaved { session } => Some(session),
                    _ => None,
                };
                match &self.session {
                    Some(running) => running.saved(session),
                    None => self.with_model(|model| {
                        model.saved = Saved::Unavailable("Not connected.".into());
                        model.touch();
                    }),
                }
            }
            TerminalIntent::HideSaved => match &self.session {
                Some(session) => session.hide_saved(),
                None => self.with_model(|model| {
                    model.saved = Saved::Hidden;
                    model.touch();
                }),
            },
            TerminalIntent::SwitchTerminal { member } => {
                let (target, size) = self.with_model(|model| {
                    let target = match &model.saved {
                        Saved::Open { members, .. } => members.iter().find_map(|m| match m {
                            SavedMember::Terminal {
                                member: id,
                                generation,
                                terminal,
                                ..
                            } if *id == member => Some(TerminalRef {
                                generation: generation.clone(),
                                terminal: terminal.clone(),
                            }),
                            _ => None,
                        }),
                        _ => None,
                    };
                    (target, model.view)
                });
                if let Some(target) = target {
                    // Leave this terminal running and attach to the other.
                    self.target = Some(target);
                    self.session = None;
                    self.idle = Some(Model::new(self.host.clone(), self.label.clone(), 24, 80));
                    self.resize(size.0, size.1);
                }
            }
            TerminalIntent::Reopen => {
                let size = self.with_model(|model| model.size());
                // A new terminal, never the old one again.
                self.target = None;
                self.session = None;
                self.idle = Some(Model::new(self.host.clone(), self.label.clone(), 24, 80));
                self.resize(size.0, size.1);
            }
            TerminalIntent::Leave => return Ok(Outcome::Closed),
        }
        self.redraw();
        Ok(Outcome::Updated)
    }

    /// The reply to a terminal request, with the view when it is newer than
    /// `known`.
    pub fn packet(&mut self, known: Option<u64>) -> TerminalPacket {
        self.redraw();
        let view = match known {
            Some(known) if known >= self.revision => None,
            _ => self.view(),
        };
        TerminalPacket {
            schema: "coder.mobile.terminal.v1",
            open: true,
            revision: self.revision,
            view,
            paste: self.paste,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_and_characters() {
        assert_eq!(named_key("enter"), Some(Key::Enter));
        assert_eq!(named_key("page_down"), Some(Key::PageDown));
        assert_eq!(named_key("f12"), Some(Key::F(12)));
        assert_eq!(named_key("f"), Some(Key::Char('f')));
        assert_eq!(named_key("é"), Some(Key::Char('é')));
        assert_eq!(named_key("f13"), None);
        assert_eq!(named_key("ab"), None);
        assert_eq!(named_key("\u{7}"), None);
        assert_eq!(named_key(""), None);
    }

    #[test]
    fn a_build_without_the_live_service_refuses_clearly_and_closes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut terminal = Terminal::open(
            "ab".repeat(32),
            "Studio".into(),
            None,
            runtime.handle().clone(),
        )
        .unwrap();
        terminal.resize(20, 40);
        terminal.text("ls\n");
        let packet = terminal.packet(None);
        let text = serde_json::to_string(&packet.view).unwrap();
        assert!(text.contains("Terminal on Studio"));
        assert!(text.contains("can't open a terminal"));
        assert!(packet.open);
        // Nothing newer than what the host already has.
        let revision = packet.revision;
        assert!(terminal.packet(Some(revision)).view.is_none());
        let view = terminal.view().unwrap();
        let event = Activation {
            instance: view["instance"].as_str().unwrap().into(),
            revision: view["revision"].as_u64().unwrap(),
            node: "terminal-close".into(),
        };
        assert_eq!(terminal.activate(&event), Ok(Outcome::Closed));
        let stale = Activation {
            revision: 99,
            ..event
        };
        assert!(terminal.activate(&stale).is_err());
    }
}
