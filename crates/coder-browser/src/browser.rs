//! DOM WebSocket adapters. Keys and grants remain in Rust page memory.
use super::*;
use futures_util::future::{Either, select};
use nostr::domain::Tag;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, task::Waker};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event as DomEvent, MessageEvent, WebSocket};

/// Opens the explicitly disclosed direct route and proves its host and current generation.
pub async fn direct(
    url: &str,
    admission: Admission,
    now: u64,
) -> Result<Direct<coder_reach::browser::Socket>> {
    let socket = match select(
        Box::pin(coder_reach::browser::Socket::open(url)),
        Box::pin(gloo_timers::future::TimeoutFuture::new(10_000)),
    )
    .await
    {
        Either::Left((result, _)) => result.map_err(|_| Error::Disconnected)?,
        Either::Right(_) => return Err(Error::Disconnected),
    };
    Direct::connect(socket, admission, now).await
}
struct Inbox {
    messages: VecDeque<String>,
    bytes: usize,
    ended: bool,
    waker: Option<Waker>,
}
impl Inbox {
    fn wake(&mut self) {
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }
}
/// One authenticated, bounded relay socket. Offline commands are never retained.
pub struct Socket {
    socket: WebSocket,
    inbox: Rc<RefCell<Inbox>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _state: Closure<dyn FnMut(DomEvent)>,
    _end: Closure<dyn FnMut(DomEvent)>,
    subscription: Option<String>,
    frames: VecDeque<Event>,
}
impl Drop for Socket {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onopen(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}
impl Socket {
    /// Only the relay named by the host-signed grant receives authentication or terminal artifacts.
    pub async fn open(admission: &Admission, now: u64) -> Result<Self> {
        admission
            .access
            .verify(&admission.secret, now, admission.policy)
            .map_err(|_| Error::NotAdmitted)?;
        Self::open_key(&admission.access.grant.relay, &admission.secret, now).await
    }
    async fn open_key(url: &str, secret: &SecretKey, now: u64) -> Result<Self> {
        let socket = WebSocket::new(url).map_err(|_| Error::Disconnected)?;
        let inbox = Rc::new(RefCell::new(Inbox {
            messages: VecDeque::new(),
            bytes: 0,
            ended: false,
            waker: None,
        }));
        let state = inbox.clone();
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let mut s = state.borrow_mut();
            let Some(text) = event.data().as_string() else {
                s.ended = true;
                s.wake();
                return;
            };
            if text.len() > 256 * 1024
                || s.bytes.saturating_add(text.len()) > 512 * 1024
                || s.messages.len() >= 64
            {
                s.ended = true;
                s.messages.clear();
                s.bytes = 0;
            } else {
                s.bytes += text.len();
                s.messages.push_back(text);
            }
            s.wake();
        }) as Box<dyn FnMut(MessageEvent)>);
        let state = inbox.clone();
        let opened = Closure::wrap(
            Box::new(move |_: DomEvent| state.borrow_mut().wake()) as Box<dyn FnMut(DomEvent)>
        );
        let state = inbox.clone();
        let end = Closure::wrap(Box::new(move |_: DomEvent| {
            let mut s = state.borrow_mut();
            s.ended = true;
            s.wake();
        }) as Box<dyn FnMut(DomEvent)>);
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        socket.set_onopen(Some(opened.as_ref().unchecked_ref()));
        socket.set_onclose(Some(end.as_ref().unchecked_ref()));
        socket.set_onerror(Some(end.as_ref().unchecked_ref()));
        let mut this = Self {
            socket,
            inbox,
            _message: message,
            _state: opened,
            _end: end,
            subscription: None,
            frames: VecDeque::new(),
        };
        this.authenticate(secret, url, now).await?;
        Ok(this)
    }
    async fn next(&mut self) -> Result<Value> {
        let next = futures_util::future::poll_fn(|cx| {
            let mut s = self.inbox.borrow_mut();
            if s.ended {
                return std::task::Poll::Ready(Err(Error::Disconnected));
            }
            if let Some(text) = s.messages.pop_front() {
                s.bytes -= text.len();
                return std::task::Poll::Ready(
                    nostr::contracts::parse_strict_bounded(text.as_bytes(), 256 * 1024)
                        .map_err(|_| Error::Malformed),
                );
            }
            s.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        });
        match select(
            Box::pin(next),
            Box::pin(gloo_timers::future::TimeoutFuture::new(12_000)),
        )
        .await
        {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(Error::Unknown),
        }
    }
    fn send(&self, value: Value) -> Result<()> {
        let text = value.to_string();
        if text.len() > 256 * 1024 || self.socket.buffered_amount() > 256 * 1024 {
            return Err(Error::Limit);
        }
        if self.socket.ready_state() != WebSocket::OPEN {
            return Err(Error::Disconnected);
        }
        self.socket.send_with_str(&text).map_err(|_| Error::Unknown)
    }
    async fn authenticate(&mut self, secret: &SecretKey, url: &str, now: u64) -> Result<()> {
        let challenge = self.next().await?;
        let token = challenge[1]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 4096)
            .ok_or(Error::Malformed)?;
        if challenge.as_array().is_none_or(|a| a.len() != 2) || challenge[0] != "AUTH" {
            return Err(Error::Malformed);
        }
        let mut event = Event {
            id: String::new(),
            pubkey: coder_access::protocol::pubkey(secret),
            created_at: now,
            kind: 22242,
            tags: vec![
                Tag::new(vec!["relay".into(), url.into()]),
                Tag::new(vec!["challenge".into(), token.into()]),
            ],
            content: String::new(),
            sig: String::new(),
        };
        let id = event.computed_id_bytes().map_err(|_| Error::Malformed)?;
        event.id = event.computed_id().map_err(|_| Error::Malformed)?;
        let secp = secp256k1::Secp256k1::signing_only();
        let mut pair = secp256k1::Keypair::from_secret_key(&secp, secret);
        event.sig = secp.sign_schnorr_no_aux_rand(&id, &pair).to_string();
        pair.non_secure_erase();
        self.send(json!(["AUTH", event]))?;
        let ack = self.next().await?;
        if ack.as_array().is_none_or(|a| a.len() != 4)
            || ack[0] != "OK"
            || ack[1] != event.id
            || ack[2] != true
        {
            return Err(Error::NotAdmitted);
        }
        Ok(())
    }
}
impl Relay for Socket {
    fn take_event(&mut self) -> Result<Option<Event>> {
        if let Some(event) = self.frames.pop_front() {
            return Ok(Some(event));
        }
        for _ in 0..64 {
            let value = {
                let mut inbox = self.inbox.borrow_mut();
                if inbox.ended {
                    return Err(Error::Disconnected);
                }
                let Some(text) = inbox.messages.pop_front() else {
                    return Ok(None);
                };
                inbox.bytes -= text.len();
                nostr::contracts::parse_strict_bounded(text.as_bytes(), 256 * 1024)
                    .map_err(|_| Error::Malformed)?
            };
            if value[0] == "EVENT"
                && value[1].as_str() == self.subscription.as_deref()
                && value.as_array().is_some_and(|a| a.len() == 3)
            {
                return serde_json::from_value(value[2].clone())
                    .map(Some)
                    .map_err(|_| Error::Malformed);
            }
            if !matches!(value[0].as_str(), Some("EOSE" | "OK" | "NOTICE")) {
                return Err(Error::Malformed);
            }
        }
        Err(Error::Limit)
    }
    async fn subscribe(&mut self, host: &str, recipient: &str, attachment: &str) -> Result<()> {
        let id = format!("frames-{attachment}");
        self.send(json!(["REQ", id, {"kinds":[3188],"authors":[host],"#p":[recipient],"#h":[attachment],"limit":64}]))?;
        self.subscription = Some(id);
        Ok(())
    }
    async fn next(&mut self) -> Result<Event> {
        if let Some(event) = self.take_event()? {
            return Ok(event);
        }
        for _ in 0..64 {
            let value = Socket::next(self).await?;
            if value[0] == "EVENT"
                && value[1].as_str() == self.subscription.as_deref()
                && value.as_array().is_some_and(|a| a.len() == 3)
            {
                return serde_json::from_value(value[2].clone()).map_err(|_| Error::Malformed);
            }
            if value[0] != "EOSE" {
                return Err(Error::Malformed);
            }
        }
        Err(Error::Limit)
    }

    async fn exchange(
        &mut self,
        event: &Event,
        host: &str,
        recipient: &str,
        now: u64,
    ) -> Result<Event> {
        let mailbox = event.tag_values("h").next().ok_or(Error::Malformed)?;
        let subscription = format!("term-{mailbox}");
        self.send(json!(["REQ", subscription, {"kinds":[3188], "authors":[host], "#p":[recipient], "#h":[mailbox], "since":now, "limit":1}]))?;
        self.send(json!(["EVENT", event]))?;
        let started = js_sys::Date::now();
        for _ in 0..64 {
            if js_sys::Date::now() - started > 12_000.0 {
                return Err(Error::Unknown);
            }
            let message = self.next().await?;
            match message[0].as_str() {
                Some("EVENT")
                    if message[1] == subscription
                        && message.as_array().is_some_and(|a| a.len() == 3) =>
                {
                    let reply =
                        serde_json::from_value(message[2].clone()).map_err(|_| Error::Malformed)?;
                    self.send(json!(["CLOSE", subscription]))?;
                    return Ok(reply);
                }
                Some("OK") if message[1] == event.id && message[2] == false => {
                    return Err(Error::NotAdmitted);
                }
                Some("EVENT") if message[1].as_str() == self.subscription.as_deref() => {
                    if self.frames.len() >= 64 {
                        return Err(Error::Limit);
                    }
                    self.frames.push_back(
                        serde_json::from_value(message[2].clone()).map_err(|_| Error::Malformed)?,
                    );
                }
                Some("EOSE" | "OK" | "NOTICE") => {}
                _ => return Err(Error::Malformed),
            }
        }
        Err(Error::Limit)
    }
}

/// Redeem one explicitly disclosed invitation using a fresh page-memory device key.
pub async fn enroll(
    pairing: crate::pairing::Pairing,
    code: &str,
    now: u64,
) -> Result<crate::pairing::Enrolled> {
    let (invitation, pending) = pairing.prepare(code, now)?;
    let mut socket = Socket::open_key(&pairing.pins.relay, &pairing.secret, now).await?;
    let event = Relay::exchange(
        &mut socket,
        &pending.event,
        &pairing.pins.host,
        &coder_access::protocol::pubkey(&pairing.secret),
        now,
    )
    .await?;
    let current = (js_sys::Date::now() / 1000.0) as u64;
    pairing.finish(&invitation, &pending, &event, current)
}

/// Read an original host thread only when this page's grant separately includes Observe.
pub async fn read_thread(
    read: crate::ThreadRead,
    now: u64,
) -> Result<coder_access::thread::ThreadPage> {
    if !read.admission.can_observe(now) {
        return Err(Error::NotAdmitted);
    }
    read.client
        .validate_pending(&read.pending, now)
        .map_err(|_| Error::NotAdmitted)?;
    let mut socket = Socket::open(&read.admission, now).await?;
    let event = Relay::exchange(
        &mut socket,
        &read.pending.event,
        read.admission.host(),
        read.admission.device(),
        now,
    )
    .await?;
    let current = (js_sys::Date::now() / 1000.0) as u64;
    if !read.admission.can_observe(current) {
        return Err(Error::NotAdmitted);
    }
    read.verify(&event, current)
}
