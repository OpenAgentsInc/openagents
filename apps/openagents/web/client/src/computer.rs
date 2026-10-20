//! The browser as a client of one of the account's computers (#12264), the
//! way the phone is: host RPC through the computer's device room on
//! `coder-sync`.
//!
//! The browser holds its own key, kept in IndexedDB ([`Key`]). It opens the
//! device room with a room token from the site ([`super::Client::room_token`]);
//! `coder-sync` names the connection `web-…`, and the computer serves it
//! nothing until a hello signed with that key names this connection
//! (`coder_access::browser`). The first time, the computer doesn't know the
//! key: the browser asks for access, and the person at the computer allows
//! it in the computer's own app (Settings, Devices). The grant is narrow
//! (watch and run, no delegation), lasts a week, and the computer checks it
//! on every call, so removing it stops the next one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Result, anyhow};
use futures::channel::{mpsc, oneshot};
use js_sys::Uint8Array;
use k256::schnorr::SigningKey;
use openagents_proto::device_frame::{self, DeviceFrameHeader};
use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::types::Room;
use crate::web::Client;

/// The computer's method names (`openagents_rpc::browsers`).
pub const BROWSER_REQUEST: &str = "BrowserRequest";
pub const BROWSER_REQUEST_STATUS: &str = "BrowserRequestStatus";
pub const BROWSER_HELLO: &str = "BrowserHello";
pub const BROWSER_FORGET: &str = "BrowserForget";
/// `coder_access::browser::HELLO_KIND`.
const HELLO_KIND: u64 = 27_237;
/// `coder_access::browser::CONN_PREFIX`.
const CONN_PREFIX: &str = "web-";
/// The relay's control kind (`openagents_rpc::device_room::RELAY_KIND`;
/// yes, with its leading space).
const RELAY_KIND: &str = " relay";
const RPC_KIND: &str = "rpc";
/// The computer's answer to a call before the browser is allowed.
const ALLOW_FIRST: &str = "Allow this browser on the computer first.";

const DB: &str = "openagents";
const STORE: &str = "keys";
const KEY_NAME: &str = "browser-device";

fn js(value: JsValue) -> anyhow::Error {
    anyhow!("{value:?}")
}

use crate::event::hex;

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    web_sys::window()
        .ok_or_else(|| anyhow!("no window"))?
        .crypto()
        .map_err(js)?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(js)?;
    Ok(bytes)
}

/// Wait for an IndexedDB request.
async fn done(request: &web_sys::IdbRequest) -> Result<JsValue> {
    let (tx, rx) = oneshot::channel::<std::result::Result<JsValue, JsValue>>();
    let tx = Rc::new(RefCell::new(Some(tx)));
    let ok = {
        let tx = tx.clone();
        let request = request.clone();
        Closure::once(move |_: web_sys::Event| {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(request.result());
            }
        })
    };
    let failed = Closure::once(move |_: web_sys::Event| {
        if let Some(tx) = tx.borrow_mut().take() {
            let _ = tx.send(Err(JsValue::from_str("IndexedDB refused the request")));
        }
    });
    request.set_onsuccess(Some(ok.as_ref().unchecked_ref()));
    request.set_onerror(Some(failed.as_ref().unchecked_ref()));
    let result = rx.await.map_err(|_| anyhow!("IndexedDB went away"))?;
    drop((ok, failed));
    result.map_err(js)
}

/// The browser's key: 32 secret bytes in IndexedDB (`openagents`, `keys`,
/// `browser-device`), made the first time.
pub struct Key {
    signing: SigningKey,
}

impl Key {
    pub async fn load_or_create() -> Result<Self> {
        let factory = web_sys::window()
            .ok_or_else(|| anyhow!("no window"))?
            .indexed_db()
            .map_err(js)?
            .ok_or_else(|| anyhow!("This browser has no IndexedDB."))?;
        let open = factory.open_with_u32(DB, 1).map_err(js)?;
        let upgrade = Closure::<dyn FnMut(web_sys::Event)>::new({
            let open = open.clone();
            move |_: web_sys::Event| {
                if let Ok(db) = open
                    .result()
                    .and_then(|db| db.dyn_into::<web_sys::IdbDatabase>())
                {
                    if !db.object_store_names().contains(STORE) {
                        let _ = db.create_object_store(STORE);
                    }
                }
            }
        });
        open.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let db: web_sys::IdbDatabase = done(&open).await?.dyn_into().map_err(js)?;
        drop(upgrade);
        let store = |mode| -> Result<web_sys::IdbObjectStore> {
            db.transaction_with_str_and_mode(STORE, mode)
                .map_err(js)?
                .object_store(STORE)
                .map_err(js)
        };
        let found = done(
            &store(web_sys::IdbTransactionMode::Readonly)?
                .get(&JsValue::from_str(KEY_NAME))
                .map_err(js)?,
        )
        .await?;
        if let Some(bytes) = found.dyn_ref::<Uint8Array>().filter(|b| b.length() == 32) {
            let mut secret = [0u8; 32];
            bytes.copy_to(&mut secret);
            if let Ok(signing) = SigningKey::from_bytes(&secret) {
                return Ok(Self { signing });
            }
        }
        let signing = loop {
            if let Ok(key) = SigningKey::from_bytes(&random::<32>()?) {
                break key;
            }
        };
        let bytes = Uint8Array::from(signing.to_bytes().as_slice());
        done(
            &store(web_sys::IdbTransactionMode::Readwrite)?
                .put_with_key(&bytes, &JsValue::from_str(KEY_NAME))
                .map_err(js)?,
        )
        .await?;
        Ok(Self { signing })
    }

    /// Lowercase hex, x-only: the NIP-HOST device key.
    pub fn public(&self) -> String {
        crate::event::public(&self.signing)
    }

    /// A NIP-01 event with this key.
    fn sign(&self, kind: u64, tags: Value, created_at: u64) -> Result<Value> {
        crate::event::sign(&self.signing, kind, tags, created_at, &random::<32>()?)
    }
}

/// A call waiting for its answer: one value, or a stream's items until it
/// ends (`{id, item}` frames, then `{id, done: true}`).
enum Waiting {
    Once(oneshot::Sender<Result<Value>>),
    Items(mpsc::UnboundedSender<Result<Value>>),
}

type Pending = Rc<RefCell<HashMap<u64, Waiting>>>;

/// One open device-room connection to a computer.
pub struct Computer {
    device: String,
    conn: String,
    socket: web_sys::WebSocket,
    pending: Pending,
    next: RefCell<u64>,
    _listeners: Vec<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    _closed: Closure<dyn FnMut(web_sys::Event)>,
}

impl Drop for Computer {
    fn drop(&mut self) {
        let _ = self.socket.close();
    }
}

fn fail_all(pending: &Pending, message: &str) {
    for (_, waiting) in pending.borrow_mut().drain() {
        match waiting {
            Waiting::Once(tx) => {
                let _ = tx.send(Err(anyhow!("{message}")));
            }
            Waiting::Items(tx) => {
                let _ = tx.unbounded_send(Err(anyhow!("{message}")));
            }
        }
    }
}

impl Computer {
    /// Open `device`'s room as this browser, with a fresh room token.
    pub async fn open(client: &Client, device: &str) -> Result<Self> {
        let token = client.room_token(Room::Device, Some(device)).await?;
        let tab = hex(&random::<8>()?);
        let url = format!("{}&connId={tab}", token.url);
        let socket = web_sys::WebSocket::new(&url).map_err(js)?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let (opened_tx, opened) = oneshot::channel::<bool>();
        let opened_tx = Rc::new(RefCell::new(Some(opened_tx)));
        let pending: Pending = Rc::default();
        let on_open = {
            let opened_tx = opened_tx.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
                if let Some(tx) = opened_tx.borrow_mut().take() {
                    let _ = tx.send(true);
                }
            })
        };
        let on_message = {
            let pending = pending.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                    return;
                };
                let Ok((header, payload)) =
                    device_frame::decode(&Uint8Array::new(&buffer).to_vec())
                else {
                    return;
                };
                if header.k == RELAY_KIND {
                    let code = device_frame::relay_error_code(&payload).unwrap_or_default();
                    fail_all(
                        &pending,
                        match code.as_str() {
                            "host_outdated" => {
                                "Update the OpenAgents app on this computer to use it from here."
                            }
                            _ => "This computer isn't online right now.",
                        },
                    );
                    return;
                }
                if header.k != RPC_KIND {
                    return;
                }
                let Ok(frame) = serde_json::from_slice::<Value>(&payload) else {
                    return;
                };
                let Some(id) = frame["id"].as_u64() else {
                    return;
                };
                let error = frame
                    .get("err")
                    .and_then(Value::as_str)
                    .map(|error| anyhow!("{}", error.strip_prefix("failed: ").unwrap_or(error)));
                let mut pending = pending.borrow_mut();
                match pending.get(&id) {
                    None => {}
                    Some(Waiting::Items(tx)) => {
                        if let Some(item) = frame.get("item") {
                            if tx.unbounded_send(Ok(item.clone())).is_err() {
                                pending.remove(&id);
                            }
                        } else if let Some(error) = error {
                            let _ = tx.unbounded_send(Err(error));
                            pending.remove(&id);
                        } else if frame["done"] == true {
                            pending.remove(&id);
                        }
                    }
                    Some(Waiting::Once(_)) => {
                        let Some(Waiting::Once(tx)) = pending.remove(&id) else {
                            return;
                        };
                        let _ = tx.send(match error {
                            Some(error) => Err(error),
                            None => Ok(frame.get("ok").cloned().unwrap_or(Value::Null)),
                        });
                    }
                }
            })
        };
        let closed = {
            let pending = pending.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                if let Some(tx) = opened_tx.borrow_mut().take() {
                    let _ = tx.send(false);
                }
                fail_all(&pending, "The connection to this computer closed.");
            })
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(closed.as_ref().unchecked_ref()));
        if !opened.await.unwrap_or(false) {
            return Err(anyhow!("Couldn't reach this computer. Try again."));
        }
        Ok(Self {
            device: device.to_owned(),
            conn: format!("{CONN_PREFIX}{tab}"),
            socket,
            pending,
            next: RefCell::new(1),
            _listeners: vec![on_open, on_message],
            _closed: closed,
        })
    }

    fn next_id(&self) -> u64 {
        let mut next = self.next.borrow_mut();
        *next += 1;
        *next
    }

    fn send(&self, frame: &Value) -> Result<()> {
        let payload = serde_json::to_vec(frame)?;
        let frame = device_frame::encode(&DeviceFrameHeader::new(RPC_KIND, RPC_KIND), &payload)
            .map_err(|e| anyhow!(e))?;
        self.socket.send_with_u8_array(&frame).map_err(js)
    }

    /// One host RPC call.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();
        self.pending.borrow_mut().insert(id, Waiting::Once(tx));
        if let Err(error) = self.send(&json!({"id": id, "method": method, "params": params})) {
            self.pending.borrow_mut().remove(&id);
            return Err(error);
        }
        rx.await
            .map_err(|_| anyhow!("The connection to this computer closed."))?
    }

    /// A streaming host RPC call (`WatchQueue`, `WatchWorkspaceFiles`, ...):
    /// its items until the computer ends it. Dropping the [`Watch`] tells
    /// the computer to stop.
    pub fn watch(self: &Rc<Self>, method: &str, params: Value) -> Result<Watch> {
        let id = self.next_id();
        let (tx, rx) = mpsc::unbounded();
        self.pending.borrow_mut().insert(id, Waiting::Items(tx));
        if let Err(error) = self.send(&json!({"id": id, "method": method, "params": params})) {
            self.pending.borrow_mut().remove(&id);
            return Err(error);
        }
        Ok(Watch {
            computer: self.clone(),
            id,
            items: rx,
        })
    }

    /// Prove the key on this connection. Fails until the computer allows it.
    pub async fn hello(&self, key: &Key) -> Result<Value> {
        let now = (js_sys::Date::now() / 1000.0) as u64;
        let event = key.sign(
            HELLO_KIND,
            json!([["host", self.device], ["conn", self.conn]]),
            now,
        )?;
        self.call(BROWSER_HELLO, json!({ "event": event })).await
    }

    /// Ask the computer for access, then wait (up to ten minutes) for the
    /// person there to allow it. `waiting` runs once the request is asked.
    pub async fn ask(&self, key: &Key, label: &str, waiting: impl FnOnce()) -> Result<()> {
        let asked = self
            .call(
                BROWSER_REQUEST,
                json!({"key": key.public(), "label": label}),
            )
            .await?;
        let request = asked["request"].as_str().unwrap_or_default().to_owned();
        if asked["state"] == "allowed" {
            return Ok(());
        }
        waiting();
        for _ in 0..300 {
            sleep(2_000).await;
            let status = self
                .call(BROWSER_REQUEST_STATUS, json!({ "request": request }))
                .await?;
            match status["state"].as_str() {
                Some("allowed") => return Ok(()),
                Some("denied") => return Err(anyhow!("The computer said no to this browser.")),
                Some("expired") => {
                    return Err(anyhow!("Nobody answered on the computer. Ask again."));
                }
                _ => {}
            }
        }
        Err(anyhow!("Nobody answered on the computer. Ask again."))
    }

    /// Give this browser's access to the computer back.
    pub async fn forget(&self) -> Result<()> {
        self.call(BROWSER_FORGET, json!({})).await.map(drop)
    }
}

/// A stream from [`Computer::watch`]. Each item is the computer's JSON, or
/// the error that ended it.
pub struct Watch {
    computer: Rc<Computer>,
    id: u64,
    items: mpsc::UnboundedReceiver<Result<Value>>,
}

impl futures::Stream for Watch {
    type Item = Result<Value>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.items).poll_next(cx)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if self
            .computer
            .pending
            .borrow_mut()
            .remove(&self.id)
            .is_some()
        {
            let _ = self.computer.send(&json!({"id": self.id, "cancel": true}));
        }
    }
}

pub(crate) async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// What the browser calls itself to the computer, from its user agent.
pub fn label() -> String {
    let agent = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default();
    let browser = if agent.contains("Firefox/") {
        "Firefox"
    } else if agent.contains("Edg/") {
        "Edge"
    } else if agent.contains("Chrome/") {
        "Chrome"
    } else if agent.contains("Safari/") {
        "Safari"
    } else {
        "A browser"
    };
    let system = if agent.contains("Mac OS X") {
        " on macOS"
    } else if agent.contains("Windows") {
        " on Windows"
    } else if agent.contains("Linux") {
        " on Linux"
    } else {
        ""
    };
    format!("{browser}{system}")
}

thread_local! {
    static OPEN: RefCell<HashMap<String, Rc<Computer>>> = RefCell::new(HashMap::new());
}

/// An allowed connection to `device`: reuses an open one, or opens one,
/// says hello, and asks for access the first time.
pub async fn connect(
    client: &Client,
    device: &str,
    waiting: impl FnOnce(),
) -> Result<Rc<Computer>> {
    if let Some(open) = OPEN.with(|open| open.borrow().get(device).cloned()) {
        return Ok(open);
    }
    let key = Key::load_or_create().await?;
    let computer = Rc::new(Computer::open(client, device).await?);
    match computer.hello(&key).await {
        Ok(_) => {}
        Err(error) if error.to_string().contains(ALLOW_FIRST) => {
            computer.ask(&key, &label(), waiting).await?;
            computer.hello(&key).await?;
        }
        Err(error) => return Err(error),
    }
    OPEN.with(|open| {
        open.borrow_mut()
            .insert(device.to_owned(), computer.clone())
    });
    Ok(computer)
}

/// Close `device`'s connection, so the next call opens a fresh one.
pub fn disconnect(device: &str) {
    OPEN.with(|open| open.borrow_mut().remove(device));
}

/// From the page's script: call `method` on `device` with JSON `params`,
/// asking for access first when needed. Resolves to the answer's JSON.
#[wasm_bindgen(js_name = computerCall)]
pub async fn computer_call(
    device: String,
    method: String,
    params: String,
) -> std::result::Result<String, JsValue> {
    let params: Value = if params.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&params).map_err(|e| JsValue::from_str(&e.to_string()))?
    };
    let client = Client::from_page();
    let result = async {
        let computer = connect(&client, &device, || {
            web_sys::console::info_1(&JsValue::from_str(
                "Allow this browser on your computer: in the OpenAgents app, open Settings, then Devices.",
            ));
        })
        .await?;
        match computer.call(&method, params.clone()).await {
            Err(error) if error.to_string().contains("closed") => {
                disconnect(&device);
                let computer = connect(&client, &device, || {}).await?;
                computer.call(&method, params).await
            }
            other => other,
        }
    }
    .await;
    match result {
        Ok(value) => Ok(value.to_string()),
        Err(error) => {
            if error.to_string().contains("ended") {
                disconnect(&device);
            }
            Err(JsValue::from_str(&error.to_string()))
        }
    }
}

/// From the page's script: give this browser's access to `device` back.
#[wasm_bindgen(js_name = computerForget)]
pub async fn computer_forget(device: String) -> std::result::Result<(), JsValue> {
    let client = Client::from_page();
    let result = async {
        let computer = connect(&client, &device, || {}).await?;
        computer.forget().await
    }
    .await;
    disconnect(&device);
    result.map_err(|error| JsValue::from_str(&error.to_string()))
}
