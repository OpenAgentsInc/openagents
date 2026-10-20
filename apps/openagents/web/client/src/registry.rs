//! The account's registry room on `coder-sync`, open in the browser
//! (#12265): a WebSocket with a fresh room token for each connection, the
//! [`Replica`]'s hello and pushes, its frames applied as they arrive, a
//! probe each minute, and a reconnect with backoff when the socket closes
//! (the token's expiry closes it with 4401 every few minutes).

use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::{mpsc, oneshot};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::computer::sleep;
use crate::event::hex;
use crate::types::{Refusal, Room};
use crate::web::Client;
use crate::workspace::{Applied, Replica};

/// How long a connection must last before a drop starts the backoff over.
const STABLE_MS: f64 = 30_000.0;
const BACKOFF_BASE_MS: i32 = 500;
const BACKOFF_CAP_MS: i32 = 30_000;
const PROBE_MS: i32 = 60_000;

struct State {
    replica: Replica,
    socket: Option<web_sys::WebSocket>,
    changed: Vec<mpsc::UnboundedSender<()>>,
    /// The site doesn't serve the registry to this page.
    unavailable: bool,
}

/// The registry, shared by the page. Cloning shares it.
#[derive(Clone)]
pub struct Registry {
    state: Rc<RefCell<State>>,
}

fn notify(state: &Rc<RefCell<State>>) {
    state
        .borrow_mut()
        .changed
        .retain(|tx| tx.unbounded_send(()).is_ok());
}

fn flush(state: &Rc<RefCell<State>>) {
    let mut state = state.borrow_mut();
    let pushes = state.replica.pushes();
    if let Some(socket) = &state.socket {
        for push in pushes {
            if socket.send_with_str(&push).is_err() {
                break;
            }
        }
    }
}

impl Registry {
    /// Opens the account's registry and keeps it open for the life of the
    /// page. Each change to it arrives on the receiver.
    pub fn open(client: Client) -> (Self, mpsc::UnboundedReceiver<()>) {
        let device = format!(
            "web-{}",
            hex(&crate::computer::random::<8>().unwrap_or_default())
        );
        let (tx, rx) = mpsc::unbounded();
        let state = Rc::new(RefCell::new(State {
            replica: Replica::new(device),
            socket: None,
            changed: vec![tx],
            unavailable: false,
        }));
        wasm_bindgen_futures::spawn_local(run(state.clone(), client));
        (Self { state }, rx)
    }

    /// Reads the registry.
    pub fn read<R>(&self, f: impl FnOnce(&Replica) -> R) -> R {
        f(&self.state.borrow().replica)
    }

    /// Writes to the registry: the change shows at once and is sent now, or
    /// when the connection is back.
    pub fn write<R>(&self, f: impl FnOnce(&mut Replica) -> R) -> R {
        let out = f(&mut self.state.borrow_mut().replica);
        flush(&self.state);
        notify(&self.state);
        out
    }

    /// Whether the account's rows have arrived.
    pub fn synced(&self) -> bool {
        self.state.borrow().replica.synced()
    }

    /// Whether the site serves no registry to this page.
    pub fn unavailable(&self) -> bool {
        self.state.borrow().unavailable
    }
}

/// Whether a refused room token means "don't ask again".
fn final_refusal(error: &anyhow::Error) -> bool {
    error.downcast_ref::<Refusal>().is_some_and(|r| {
        matches!(
            r.code.as_str(),
            "unavailable" | "signed_out" | "forbidden" | "http_401" | "http_403" | "http_404"
        )
    })
}

async fn run(state: Rc<RefCell<State>>, client: Client) {
    let mut backoff = BACKOFF_BASE_MS;
    loop {
        let token = match client.room_token(Room::Registry, None).await {
            Ok(token) => token,
            Err(error) if final_refusal(&error) => {
                log::info!("registry: not served here: {error:#}");
                state.borrow_mut().unavailable = true;
                notify(&state);
                return;
            }
            Err(error) => {
                log::warn!("registry: no room token: {error:#}");
                sleep(backoff).await;
                backoff = (backoff * 2).min(BACKOFF_CAP_MS);
                continue;
            }
        };
        let started = js_sys::Date::now();
        if let Err(error) = session(&state, &token.url).await {
            log::warn!("registry: connection failed: {error:?}");
        }
        state.borrow_mut().socket = None;
        state.borrow_mut().replica.disconnected();
        notify(&state);
        if js_sys::Date::now() - started > STABLE_MS {
            backoff = BACKOFF_BASE_MS;
        }
        sleep(backoff).await;
        backoff = (backoff * 2).min(BACKOFF_CAP_MS);
    }
}

/// One connection, until it closes.
async fn session(state: &Rc<RefCell<State>>, url: &str) -> Result<(), JsValue> {
    let socket = web_sys::WebSocket::new(url)?;
    let (closed_tx, closed) = oneshot::channel::<()>();
    let closed_tx = Rc::new(RefCell::new(Some(closed_tx)));
    let on_open = {
        let state = state.clone();
        let socket = socket.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let hello = state.borrow_mut().replica.hello();
            let _ = socket.send_with_str(&hello);
        })
    };
    let on_message = {
        let state = state.clone();
        let socket = socket.clone();
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            if text == "pong" {
                return;
            }
            let applied = state.borrow_mut().replica.apply(&text);
            match applied {
                Applied::Nothing => {}
                Applied::Changed => {
                    flush(&state);
                    notify(&state);
                }
                Applied::Resync => {
                    notify(&state);
                    let _ = socket.close();
                }
            }
        })
    };
    let on_close = {
        let closed_tx = closed_tx.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if let Some(tx) = closed_tx.borrow_mut().take() {
                let _ = tx.send(());
            }
        })
    };
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    state.borrow_mut().socket = Some(socket.clone());
    // A probe each minute keeps a quiet room honest; a socket that can't
    // send it is gone.
    let probing = Rc::new(RefCell::new(true));
    {
        let socket = socket.clone();
        let probing = probing.clone();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                sleep(PROBE_MS).await;
                if !*probing.borrow() {
                    break;
                }
                if socket.ready_state() == web_sys::WebSocket::OPEN
                    && socket.send_with_str(r#"{"t":"probe"}"#).is_err()
                {
                    let _ = socket.close();
                    break;
                }
            }
        });
    }
    let _ = closed.await;
    *probing.borrow_mut() = false;
    socket.set_onopen(None);
    socket.set_onmessage(None);
    socket.set_onclose(None);
    drop((on_open, on_message, on_close));
    Ok(())
}
