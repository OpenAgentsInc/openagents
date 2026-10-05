//! The browser build's relay link: the browser's WebSocket, driven from its
//! callbacks on the page's thread. It has the native link's interface, so
//! the session, chat, and feeds run unchanged, and it shares the native
//! link's protocol state ([`Wire`]): subscriptions restored after a
//! reconnect or an accepted NIP-42 answer, durable events held while
//! offline, and one-shot subscriptions closed after their stored events.
//! A lost or refused socket reconnects with the native link's backoff,
//! from half a second up to eight.
use super::wire::{Wire, batch_fits};
use super::{INBOX, In, Out};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

const FIRST_BACKOFF_MS: i32 = 500;
const MAX_BACKOFF_MS: i32 = 8_000;
const RETRYING: &str = "Relay connection unavailable; retrying.";

/// A relay connection in the browser.
pub struct Link {
    inner: Rc<RefCell<Inner>>,
    /// The exact relay URL used for NIP-42.
    pub url: String,
}

struct Inner {
    url: String,
    wire: Wire,
    inbox: VecDeque<In>,
    socket: Option<Socket>,
    backoff_ms: i32,
    /// The pending reconnect's timer.
    retry: Option<i32>,
    /// Closed sockets whose callbacks may still be running; dropped when
    /// the next socket opens, outside them.
    closed: Vec<Socket>,
    stopped: bool,
}

/// A socket and the callbacks that keep its handlers alive.
struct Socket {
    ws: WebSocket,
    _open: Closure<dyn FnMut()>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _close: Closure<dyn FnMut(CloseEvent)>,
}

impl Socket {
    fn detach(&self) {
        self.ws.set_onopen(None);
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        let _ = self.ws.close();
    }
}

impl Link {
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn idle() -> Self {
        let link = Self::start("ws://127.0.0.1:1");
        link.inner.borrow_mut().stop();
        link
    }

    /// Opens a socket to `url`. Nothing blocks: the socket opens, fails,
    /// and reconnects on the page's event loop.
    #[must_use]
    pub fn start(url: &str) -> Self {
        let inner = Rc::new(RefCell::new(Inner {
            url: url.into(),
            wire: Wire::default(),
            inbox: VecDeque::new(),
            socket: None,
            backoff_ms: FIRST_BACKOFF_MS,
            retry: None,
            closed: Vec::new(),
            stopped: false,
        }));
        connect(&inner);
        Self {
            inner,
            url: url.into(),
        }
    }

    /// Queues `out`. False means the command is too large or the link has
    /// stopped.
    pub fn send(&self, out: Out) -> bool {
        self.send_batch(vec![out])
    }

    /// Writes every command, or holds them for the next connection when
    /// the socket is closed. A command an offline queue refuses leaves a
    /// notice. False means the batch is out of bounds or the link has
    /// stopped.
    pub fn send_batch(&self, commands: Vec<Out>) -> bool {
        if !batch_fits(&commands) {
            return false;
        }
        let mut inner = self.inner.borrow_mut();
        if inner.stopped {
            return false;
        }
        for command in commands {
            let step = inner.wire.command(command);
            inner.write(&step.writes);
            if let Some(message) = step.message {
                inner.deliver(message);
            }
        }
        true
    }

    /// Takes at most one inbox's worth of messages.
    #[must_use]
    pub fn drain(&self) -> Vec<In> {
        let mut inner = self.inner.borrow_mut();
        let take = inner.inbox.len().min(INBOX);
        inner.inbox.drain(..take).collect()
    }

    /// Closes the socket and stops reconnecting. It always finishes at once.
    pub fn shutdown(&mut self, _wait: Duration) -> bool {
        self.inner.borrow_mut().stop();
        true
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.shutdown(Duration::ZERO);
    }
}

impl Inner {
    fn stop(&mut self) {
        self.stopped = true;
        if let Some(socket) = self.socket.take() {
            socket.detach();
            self.closed.push(socket);
        }
        if let Some(timer) = self.retry.take()
            && let Some(window) = web_sys::window()
        {
            window.clear_timeout_with_handle(timer);
        }
        self.wire.closed();
    }

    fn deliver(&mut self, message: In) {
        if self.inbox.len() >= INBOX {
            // The game stopped draining; drop the connection as the native
            // link does, and resubscribe when it comes back.
            self.lost();
            return;
        }
        self.inbox.push_back(message);
    }

    fn write(&mut self, frames: &[String]) {
        let Some(socket) = &self.socket else {
            return;
        };
        for frame in frames {
            if socket.ws.send_with_str(frame).is_err() {
                self.lost();
                return;
            }
        }
    }

    /// The socket is gone: report it once and schedule a reconnect.
    fn lost(&mut self) {
        if let Some(socket) = self.socket.take() {
            socket.detach();
            self.closed.push(socket);
        }
        self.wire.closed();
        if self.inbox.len() < INBOX {
            self.inbox.push_back(In::Disconnected(RETRYING.into()));
        }
    }
}

/// Opens a socket for `inner`, or schedules another try.
fn connect(inner: &Rc<RefCell<Inner>>) {
    let weak = Rc::downgrade(inner);
    let mut state = inner.borrow_mut();
    if state.stopped {
        return;
    }
    state.retry = None;
    state.closed.clear();
    let ws = match WebSocket::new(&state.url) {
        Ok(ws) => ws,
        Err(_) => {
            state.lost();
            drop(state);
            schedule(inner);
            return;
        }
    };
    let open = {
        let weak = weak.clone();
        Closure::<dyn FnMut()>::new(move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let mut inner = inner.borrow_mut();
            inner.backoff_ms = FIRST_BACKOFF_MS;
            let step = inner.wire.opened();
            if let Some(message) = step.message {
                inner.deliver(message);
            }
            inner.write(&step.writes);
        })
    };
    let message = {
        let weak = weak.clone();
        Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let mut inner = inner.borrow_mut();
            let Some(text) = event.data().as_string() else {
                // A binary frame is not NIP-01.
                inner.lost();
                drop(inner);
                schedule_weak(&weak);
                return;
            };
            match inner.wire.received(&text) {
                Some(step) => {
                    if let Some(message) = step.message {
                        inner.deliver(message);
                    }
                    inner.write(&step.writes);
                }
                None => {
                    inner.lost();
                    drop(inner);
                    schedule_weak(&weak);
                }
            }
        })
    };
    let close = {
        let weak = weak.clone();
        Closure::<dyn FnMut(CloseEvent)>::new(move |_: CloseEvent| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            inner.borrow_mut().lost();
            schedule(&inner);
        })
    };
    ws.set_onopen(Some(open.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
    ws.set_onclose(Some(close.as_ref().unchecked_ref()));
    state.socket = Some(Socket {
        ws,
        _open: open,
        _message: message,
        _close: close,
    });
}

fn schedule_weak(weak: &Weak<RefCell<Inner>>) {
    if let Some(inner) = weak.upgrade() {
        schedule(&inner);
    }
}

/// Reconnects after the current backoff, which then doubles.
fn schedule(inner: &Rc<RefCell<Inner>>) {
    let mut state = inner.borrow_mut();
    if state.stopped || state.retry.is_some() || state.socket.is_some() {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let weak = Rc::downgrade(inner);
    // A one-shot callback frees itself after it runs.
    let callback = Closure::once_into_js(move || {
        if let Some(inner) = weak.upgrade() {
            connect(&inner);
        }
    });
    let delay = state.backoff_ms;
    state.backoff_ms = (state.backoff_ms * 2).min(MAX_BACKOFF_MS);
    if let Ok(timer) = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), delay)
    {
        state.retry = Some(timer);
    }
}
