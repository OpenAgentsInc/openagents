//! One chat's chat2 room on `coder-sync`, open in the browser (#12224): a
//! WebSocket with a fresh room token for each connection, the
//! [`ChatReplica`]'s hello, the checkpoint read through the site when the
//! room has compacted, its rows applied as they arrive, a probe each minute,
//! and a reconnect with backoff when the socket closes (the token's expiry
//! closes it every few minutes). Dropping the [`ChatRoom`] closes it.

use std::cell::RefCell;
use std::rc::Rc;

use futures::channel::{mpsc, oneshot};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::chat_replica::{ChatReplica, Step};
use crate::computer::sleep;
use crate::event::hex;
use crate::types::{Refusal, Room};
use crate::web::Client;

const STABLE_MS: f64 = 30_000.0;
const BACKOFF_BASE_MS: i32 = 500;
const BACKOFF_CAP_MS: i32 = 30_000;
const PROBE_MS: i32 = 60_000;

struct State {
    replica: ChatReplica,
    socket: Option<web_sys::WebSocket>,
    changed: mpsc::UnboundedSender<()>,
    /// The page left this chat.
    closed: bool,
    /// The site serves no room for this chat.
    unavailable: bool,
}

/// One chat's room, open until dropped.
pub struct ChatRoom {
    state: Rc<RefCell<State>>,
}

impl Drop for ChatRoom {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.closed = true;
        if let Some(socket) = state.socket.take() {
            let _ = socket.close();
        }
    }
}

impl ChatRoom {
    /// Opens chat `chat`'s room. Each change to its transcript arrives on
    /// the receiver.
    pub fn open(client: Client, chat: &str) -> (Self, mpsc::UnboundedReceiver<()>) {
        let device = format!(
            "web-{}",
            hex(&crate::computer::random::<8>().unwrap_or_default())
        );
        let (tx, rx) = mpsc::unbounded();
        let state = Rc::new(RefCell::new(State {
            replica: ChatReplica::new(device),
            socket: None,
            changed: tx,
            closed: false,
            unavailable: false,
        }));
        wasm_bindgen_futures::spawn_local(run(state.clone(), client, chat.to_owned()));
        (Self { state }, rx)
    }

    /// Reads the room's transcript.
    pub fn read<R>(&self, f: impl FnOnce(&ChatReplica) -> R) -> R {
        f(&self.state.borrow().replica)
    }

    /// Whether the site serves no room for this chat.
    pub fn unavailable(&self) -> bool {
        self.state.borrow().unavailable
    }
}

fn notify(state: &Rc<RefCell<State>>) {
    let _ = state.borrow().changed.unbounded_send(());
}

fn send(socket: &web_sys::WebSocket, bytes: &[u8]) {
    let _ = socket.send_with_u8_array(bytes);
}

fn final_refusal(error: &anyhow::Error) -> bool {
    error.downcast_ref::<Refusal>().is_some_and(|r| {
        matches!(
            r.code.as_str(),
            "unavailable" | "signed_out" | "forbidden" | "http_401" | "http_403" | "http_404"
        )
    })
}

async fn run(state: Rc<RefCell<State>>, client: Client, chat: String) {
    let mut backoff = BACKOFF_BASE_MS;
    while !state.borrow().closed {
        let token = match client.room_token(Room::Chat, Some(&chat)).await {
            Ok(token) => token,
            Err(error) if final_refusal(&error) => {
                log::info!("chat room: not served here: {error:#}");
                state.borrow_mut().unavailable = true;
                notify(&state);
                return;
            }
            Err(error) => {
                log::warn!("chat room: no room token: {error:#}");
                sleep(backoff).await;
                backoff = (backoff * 2).min(BACKOFF_CAP_MS);
                continue;
            }
        };
        let started = js_sys::Date::now();
        if let Err(error) = session(&state, &client, &chat, &token.url).await {
            log::warn!("chat room: connection failed: {error:?}");
        }
        state.borrow_mut().socket = None;
        if state.borrow().closed {
            return;
        }
        if js_sys::Date::now() - started > STABLE_MS {
            backoff = BACKOFF_BASE_MS;
        }
        sleep(backoff).await;
        backoff = (backoff * 2).min(BACKOFF_CAP_MS);
    }
}

/// One connection, until it closes.
async fn session(
    state: &Rc<RefCell<State>>,
    client: &Client,
    chat: &str,
    url: &str,
) -> Result<(), JsValue> {
    let socket = web_sys::WebSocket::new(url)?;
    socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let (closed_tx, closed) = oneshot::channel::<()>();
    let closed_tx = Rc::new(RefCell::new(Some(closed_tx)));
    let on_open = {
        let state = state.clone();
        let socket = socket.clone();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let hello = state.borrow().replica.hello();
            send(&socket, &hello);
        })
    };
    let on_message = {
        let state = state.clone();
        let socket = socket.clone();
        let client = client.clone();
        let chat = chat.to_owned();
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            let step = state.borrow_mut().replica.apply(&bytes);
            match step {
                Step::Nothing => {}
                Step::Changed => notify(&state),
                Step::Send(bytes) => send(&socket, &bytes),
                Step::Checkpoint => {
                    let (state, socket, client, chat) =
                        (state.clone(), socket.clone(), client.clone(), chat.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        let path = format!("/v1/sync/chat2/{chat}/checkpoint");
                        let bytes = client.bytes(&path).await.ok().flatten();
                        let step = state.borrow_mut().replica.checkpoint(bytes.as_deref());
                        notify(&state);
                        if let Step::Send(bytes) = step {
                            send(&socket, &bytes);
                        }
                    });
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
    let probing = Rc::new(RefCell::new(true));
    {
        let socket = socket.clone();
        let probing = probing.clone();
        let probe = state.borrow().replica.probe();
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                sleep(PROBE_MS).await;
                if !*probing.borrow() {
                    break;
                }
                if socket.ready_state() == web_sys::WebSocket::OPEN
                    && socket.send_with_u8_array(&probe).is_err()
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
