//! One relay connection over the browser's WebSocket, with NIP-42
//! authentication, and the NIP-CJ ask: discover beacons, choose a pylon,
//! send one sealed request, and wait for its result.

use std::collections::{BTreeSet, VecDeque};

use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt;
use futures_util::future::{Either, select};
use nostr::domain::Event;
use nostr::pylon::BeaconBook;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket};

use super::util::{now_ms, random32, sleep, unix};
use crate::cj::{self, Answer, Key};

enum Msg {
    Text(String),
    Open,
    Closed,
}

pub struct Relay {
    ws: WebSocket,
    rx: UnboundedReceiver<Msg>,
    held: VecDeque<Value>,
    pub closed: bool,
    _handlers: Vec<Closure<dyn FnMut(JsValue)>>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.ws.close();
    }
}

/// Why an ask failed, and the pylon to avoid next time, if one was chosen.
pub struct AskError {
    pub pylon: Option<String>,
    pub message: String,
}

/// What one ask returned.
pub struct Asked {
    pub text: String,
    /// The model's reasoning: the pylon's `reasoning`, or a `<think>` block.
    pub reasoning: Option<String>,
    pub label: String,
    pub model: String,
    pub pylon: String,
    pub discover_ms: f64,
    pub contact_ms: Option<f64>,
    pub answer_ms: f64,
    pub usage: Option<Value>,
}

impl Relay {
    /// Open the socket and answer the relay's `AUTH` challenge, if any.
    pub async fn connect(url: &str, key: &Key) -> Result<Self, String> {
        let ws = WebSocket::new(url).map_err(|e| super::util::text(&e))?;
        let (tx, rx) = unbounded();
        let mut handlers = Vec::new();
        let on = |tx: futures_channel::mpsc::UnboundedSender<Msg>, kind: u8| {
            Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                let msg = match kind {
                    0 => Msg::Open,
                    1 => match event
                        .dyn_ref::<MessageEvent>()
                        .and_then(|m| m.data().as_string())
                    {
                        Some(text) => Msg::Text(text),
                        None => return,
                    },
                    _ => Msg::Closed,
                };
                let _ = tx.unbounded_send(msg);
            })
        };
        let open = on(tx.clone(), 0);
        let message = on(tx.clone(), 1);
        let close = on(tx.clone(), 2);
        let error = on(tx, 2);
        ws.set_onopen(Some(open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(close.as_ref().unchecked_ref()));
        ws.set_onerror(Some(error.as_ref().unchecked_ref()));
        handlers.extend([open, message, close, error]);
        let mut relay = Self {
            ws,
            rx,
            held: VecDeque::new(),
            closed: false,
            _handlers: handlers,
        };
        match relay.raw(10_000).await {
            Some(Msg::Open) => {}
            _ => return Err(format!("could not reach {url}")),
        }
        // A relay that wants NIP-42 sends its challenge first.
        let until = now_ms() + 1_500.0;
        while now_ms() < until {
            let Some(frame) = relay.next((until - now_ms()) as i32).await? else {
                break;
            };
            if frame[0] == "AUTH"
                && let Some(challenge) = frame[1].as_str()
            {
                let auth = key.auth(url, challenge, unix());
                relay.send(&json!(["AUTH", auth]));
                let until = now_ms() + 5_000.0;
                while now_ms() < until {
                    match relay.next((until - now_ms()) as i32).await? {
                        Some(f) if f[0] == "OK" && f[1] == auth.id.as_str() => {
                            if f[2] != true {
                                return Err(format!("the relay refused sign-in: {}", f[3]));
                            }
                            break;
                        }
                        Some(f) => relay.held.push_back(f),
                        None => break,
                    }
                }
                break;
            }
            relay.held.push_back(frame);
        }
        Ok(relay)
    }

    async fn raw(&mut self, timeout_ms: i32) -> Option<Msg> {
        let next = self.rx.next();
        let timer = sleep(timeout_ms.max(0));
        match select(next, timer).await {
            Either::Left((msg, _)) => msg,
            Either::Right(_) => None,
        }
    }

    /// The next relay frame, or `None` after `timeout_ms`.
    pub async fn next(&mut self, timeout_ms: i32) -> Result<Option<Value>, String> {
        if let Some(frame) = self.held.pop_front() {
            return Ok(Some(frame));
        }
        let until = now_ms() + f64::from(timeout_ms);
        loop {
            let left = (until - now_ms()) as i32;
            if left <= 0 {
                return Ok(None);
            }
            match self.raw(left).await {
                Some(Msg::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        return Ok(Some(value));
                    }
                }
                Some(Msg::Open) => {}
                Some(Msg::Closed) => {
                    self.closed = true;
                    return Err("the relay closed the connection".into());
                }
                None => return Ok(None),
            }
        }
    }

    pub fn send(&self, value: &Value) {
        let _ = self.ws.send_with_str(&value.to_string());
    }

    /// Publish `event` and wait for the relay's `OK`.
    pub async fn publish(&mut self, event: &Event) -> Result<(), String> {
        self.send(&json!(["EVENT", event]));
        let until = now_ms() + 10_000.0;
        loop {
            let left = (until - now_ms()) as i32;
            let Some(frame) = self.next(left).await? else {
                return Err("the relay did not confirm the event".into());
            };
            if frame[0] == "OK" && frame[1] == event.id.as_str() {
                return if frame[2] == true {
                    Ok(())
                } else {
                    Err(format!("the relay refused the event: {}", frame[3]))
                };
            }
        }
    }

    /// Run one query and return the events up to `EOSE`.
    pub async fn query(&mut self, sub: &str, filter: &Value) -> Result<Vec<Event>, String> {
        self.send(&json!(["REQ", sub, filter]));
        let mut events = Vec::new();
        let until = now_ms() + 10_000.0;
        loop {
            let left = (until - now_ms()) as i32;
            let Some(frame) = self.next(left).await? else {
                break;
            };
            match frame[0].as_str() {
                Some("EVENT") if frame[1] == sub => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone())
                        && event.validate_crypto().is_ok()
                    {
                        events.push(event);
                    }
                }
                Some("EOSE") if frame[1] == sub => break,
                Some("CLOSED") if frame[1] == sub => {
                    return Err(format!("the relay closed the query: {}", frame[2]));
                }
                _ => {}
            }
        }
        self.send(&json!(["CLOSE", sub]));
        Ok(events)
    }

    /// Read every beacon up to `EOSE`.
    pub async fn beacons(&mut self) -> Result<BeaconBook, String> {
        self.send(&json!(["REQ", "beacons", cj::beacon_filter()]));
        let mut book = BeaconBook::default();
        let until = now_ms() + 10_000.0;
        loop {
            let left = (until - now_ms()) as i32;
            let Some(frame) = self.next(left).await? else {
                break;
            };
            match frame[0].as_str() {
                Some("EVENT") if frame[1] == "beacons" => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                        cj::offer(&mut book, event);
                    }
                }
                Some("EOSE") if frame[1] == "beacons" => break,
                Some("CLOSED") if frame[1] == "beacons" => {
                    return Err(format!("the relay closed the beacon query: {}", frame[2]));
                }
                _ => {}
            }
        }
        self.send(&json!(["CLOSE", "beacons"]));
        Ok(book)
    }

    /// Send one conversation job and wait for its answer.
    #[allow(clippy::too_many_arguments)]
    pub async fn ask(
        &mut self,
        key: &Key,
        book: &BeaconBook,
        want: Option<&str>,
        fallback: bool,
        failing: &BTreeSet<String>,
        instructions: &str,
        prompt: &dyn Fn(usize) -> String,
        on_feedback: &dyn Fn(&str),
        cap: Option<usize>,
    ) -> Result<Asked, AskError> {
        let started = now_ms();
        let mut usable = BeaconBook::default();
        for (event, beacon) in book.iter() {
            if !failing.contains(&beacon.provider) {
                usable.offer(event.clone(), beacon.clone());
            }
        }
        let (beacon, service) =
            cj::choose(&usable, unix(), want, fallback).ok_or_else(|| AskError {
                pylon: None,
                message: match want {
                    Some(want) if !fallback => format!("no ready pylon serves {want}"),
                    _ => "no fresh online pylon with a free slot".to_string(),
                },
            })?;
        let fail = |message: String| AskError {
            pylon: Some(beacon.provider.clone()),
            message: format!("{}: {message}", beacon.label),
        };
        let budget = cj::input_limit(&beacon).min(cap.unwrap_or(usize::MAX));
        let prompt = prompt(budget);
        let (body, wait_secs) = cj::request_body(&beacon, instructions, &prompt, 300);
        on_feedback(&format!(
            "asking {} for {} ({} KiB prompt, wait up to {} s)",
            beacon.label,
            service.model,
            prompt.len() / 1024,
            wait_secs
        ));
        let discover_ms = now_ms() - started;
        let heartbeats = std::cell::Cell::new(0_u32);
        let progress = |_: Option<&str>| {
            heartbeats.set(heartbeats.get() + 1);
            if heartbeats.get() == 1 {
                on_feedback("the pylon is working");
            }
        };
        let got = self
            .exchange(key, &beacon.provider, &body, wait_secs, &progress)
            .await
            .map_err(fail)?;
        let (thinking, rest) = crate::transcript::split_thinking(&got.text);
        Ok(Asked {
            text: cj::clean(&rest),
            reasoning: got.reasoning.clone().or(thinking),
            label: beacon.label.clone(),
            model: got.model.unwrap_or_else(|| service.model.clone()),
            pylon: beacon.provider.clone(),
            discover_ms,
            contact_ms: got.contact_ms,
            answer_ms: got.answer_ms,
            usage: got.usage,
        })
    }

    /// Send one sealed conversation request to `provider` and wait up to
    /// `wait_secs` (plus a grace) for its result. `on_progress` sees each
    /// `processing` feedback.
    pub async fn exchange(
        &mut self,
        key: &Key,
        provider: &str,
        body: &Value,
        wait_secs: u64,
        on_progress: &dyn Fn(Option<&str>),
    ) -> Result<Exchanged, String> {
        self.exchange_tagged(key, provider, body, wait_secs, on_progress, &[])
            .await
    }

    /// [`Self::exchange`] with `extra` tags on the request, such as an
    /// agent's `auth` and `mandate` tags.
    pub async fn exchange_tagged(
        &mut self,
        key: &Key,
        provider: &str,
        body: &Value,
        wait_secs: u64,
        on_progress: &dyn Fn(Option<&str>),
        extra: &[nostr::domain::Tag],
    ) -> Result<Exchanged, String> {
        let request = key.seal_request_tagged(provider, body, random32(), unix(), extra)?;
        self.send(&json!([
            "REQ",
            "answer",
            cj::answer_filter(key.pubkey(), &request.id)
        ]));
        self.send(&json!(["EVENT", request]));
        let sent = now_ms();
        let mut contact_ms = None;
        let until = sent + (wait_secs as f64 + 10.0) * 1000.0;
        let result = loop {
            let left = (until - now_ms()) as i32;
            let frame = match self.next(left).await {
                Ok(Some(frame)) => frame,
                Ok(None) => break Err(format!("no answer within {} s", wait_secs + 10)),
                Err(e) => break Err(e),
            };
            match frame[0].as_str() {
                Some("OK") if frame[1] == request.id.as_str() && frame[2] == false => {
                    break Err(format!("the relay refused the request: {}", frame[3]));
                }
                Some("EVENT") if frame[1] == "answer" => {
                    let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                        continue;
                    };
                    if !cj::answers(&event, &request.id, provider, key.pubkey()) {
                        continue;
                    }
                    contact_ms.get_or_insert(now_ms() - sent);
                    let Ok(plain) = key.open(&event) else {
                        continue;
                    };
                    match cj::read(event.kind, &plain) {
                        Answer::Result {
                            text,
                            model,
                            usage,
                            reasoning,
                        } => {
                            break Ok(Exchanged {
                                text,
                                reasoning,
                                model,
                                usage,
                                contact_ms,
                                answer_ms: now_ms() - sent,
                            });
                        }
                        Answer::Error(error) => break Err(error),
                        Answer::Processing(note) => on_progress(note.as_deref()),
                        Answer::Other => {}
                    }
                }
                _ => {}
            }
        };
        self.send(&json!(["CLOSE", "answer"]));
        result
    }
}

/// What [`Relay::exchange`] returned.
pub struct Exchanged {
    pub text: String,
    pub reasoning: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Value>,
    pub contact_ms: Option<f64>,
    pub answer_ms: f64,
}
