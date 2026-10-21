//! The browser transport: `fetch` for reads and writes, `EventSource` for a
//! live answer. Every request is same-origin, so the site's sign-in cookie
//! goes with it; nothing here holds a token.

use anyhow::{Result, anyhow};
use futures::channel::mpsc;
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::types::*;

/// The header the site checks a cookie-signed write's CSRF value in.
pub const CSRF_HEADER: &str = "x-openagents-csrf";
/// The page's `<meta>` that carries that value.
pub const CSRF_META: &str = "openagents-csrf";

#[derive(Clone)]
pub struct Client {
    csrf: String,
    chat_surface: Option<String>,
}

impl Client {
    /// A client with the CSRF value the page was served with.
    pub fn from_page() -> Self {
        let csrf = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| {
                d.query_selector(&format!("meta[name=\"{CSRF_META}\"]"))
                    .ok()
                    .flatten()
            })
            .and_then(|e| e.get_attribute("content"))
            .unwrap_or_default();
        Self {
            csrf,
            chat_surface: None,
        }
    }

    /// A client whose writes carry `csrf`, for a page that reads its CSRF
    /// value after it loads (`/lander8`).
    pub fn with_csrf(csrf: impl Into<String>) -> Self {
        Self {
            csrf: csrf.into(),
            chat_surface: None,
        }
    }

    /// Identifies the public chat page, for its visitor policy.
    pub fn with_chat_surface(mut self, surface: &str) -> Self {
        self.chat_surface = Some(surface.to_owned());
        self
    }

    pub async fn voice_agent(&self) -> Result<serde_json::Value> {
        self.get("/v1/voice-agent").await
    }

    pub async fn threads(&self) -> Result<ThreadList> {
        self.get("/v1/threads").await
    }

    pub async fn thread(&self, id: &str) -> Result<ThreadDetail> {
        self.get(&format!("/v1/threads/{}", encode(id))).await
    }

    /// Every message of a chat, oldest first, in pages of 200.
    pub async fn messages(&self, id: &str) -> Result<Vec<MessageRow>> {
        let mut out = Vec::new();
        let mut after: Option<String> = None;
        // At most 25 pages: 5,000 messages.
        for _ in 0..25 {
            let mut path = format!("/v1/threads/{}/messages?limit=200", encode(id));
            if let Some(after) = &after {
                path.push_str("&after=");
                path.push_str(&encode(after));
            }
            let page: Page<MessageRow> = self.get(&path).await?;
            out.extend(page.data);
            match page.next {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    pub async fn projects(&self) -> Result<Vec<Project>> {
        let page: Page<Project> = self.get("/v1/projects?limit=100").await?;
        Ok(page.data)
    }

    /// Starts a chat with its first message; answers the new chat's id.
    pub async fn start(
        &self,
        request_id: &str,
        text: &str,
        project: Option<&str>,
    ) -> Result<String> {
        let body = MessageBody {
            request_id,
            text,
            project,
        };
        let started: Started = self.send("POST", "/v1/threads", Some(&body)).await?;
        Ok(started.id)
    }

    pub async fn reply(&self, id: &str, request_id: &str, text: &str) -> Result<()> {
        let body = MessageBody {
            request_id,
            text,
            project: None,
        };
        self.send::<serde_json::Value>(
            "POST",
            &format!("/v1/threads/{}/messages", encode(id)),
            Some(&body),
        )
        .await
        .map(drop)
    }

    pub async fn stop(&self, id: &str, request_id: &str) -> Result<()> {
        self.send::<serde_json::Value>(
            "POST",
            &format!("/v1/threads/{}/stop", encode(id)),
            Some(&serde_json::json!({ "request_id": request_id })),
        )
        .await
        .map(drop)
    }

    pub async fn change(&self, id: &str, change: &Change) -> Result<()> {
        self.send::<serde_json::Value>(
            "PATCH",
            &format!("/v1/threads/{}", encode(id)),
            Some(change),
        )
        .await
        .map(drop)
    }

    /// The running work Coder reports for the account's computers.
    pub async fn agents(&self) -> Result<Boards> {
        self.get("/v1/agents").await
    }

    /// Approve, deny, stop, or message running work.
    pub async fn act(&self, action: &ItemAction<'_>) -> Result<()> {
        self.send::<serde_json::Value>("POST", "/v1/agents/actions", Some(action))
            .await
            .map(drop)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.send::<serde_json::Value>(
            "DELETE",
            &format!("/v1/threads/{}", encode(id)),
            None::<&()>,
        )
        .await
        .map(drop)
    }

    /// Follows the answer to `request_id`. Events arrive on the receiver,
    /// each read by the SDK's [`Event::read`], until one [`Event::ends`] it;
    /// dropping the [`Follow`] closes the connection.
    pub fn follow(
        &self,
        id: &str,
        request_id: &str,
    ) -> Result<(Follow, mpsc::UnboundedReceiver<Event>)> {
        let url = format!(
            "/v1/threads/{}/events?request_id={}",
            encode(id),
            encode(request_id)
        );
        let source = web_sys::EventSource::new(&url).map_err(js_error)?;
        let (sender, receiver) = mpsc::unbounded();
        let mut listeners = Vec::new();
        for name in ["text", "tool", "cards", "done", "error"] {
            let sender = sender.clone();
            let closer = source.clone();
            let request = request_id.to_owned();
            let listener = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
                move |event: web_sys::MessageEvent| {
                    let parsed = match event.data().as_string() {
                        // An event this turn doesn't own, or can't be read,
                        // is left out, as the native client refuses it.
                        Some(data) => Event::read(&data, &request).ok(),
                        // The connection's own error: no data. EventSource
                        // retries by itself; a closed one has ended.
                        None if closer.ready_state() == web_sys::EventSource::CLOSED => {
                            Some(lost(&request))
                        }
                        None => None,
                    };
                    if let Some(parsed) = parsed {
                        if parsed.ends() {
                            closer.close();
                        }
                        let _ = sender.unbounded_send(parsed);
                    }
                },
            );
            source
                .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
                .map_err(js_error)?;
            listeners.push(listener);
        }
        Ok((
            Follow {
                source,
                _listeners: listeners,
            },
            receiver,
        ))
    }

    /// A token to open `room` (`id`: the computer or chat; the registry is
    /// the account's own). Ask again for each connection: a token lasts a
    /// few minutes (#12263).
    pub async fn room_token(&self, room: Room, id: Option<&str>) -> Result<RoomToken> {
        let mut body = serde_json::json!({ "room": room.as_str() });
        if let Some(id) = id {
            body["id"] = id.into();
        }
        self.send("POST", "/v1/sync/room-tokens", Some(&body)).await
    }

    /// A ping each time something in the chat list changes
    /// (`/chats/events?format=json`), so the list is read again only then.
    /// Dropping the [`Follow`] closes the connection.
    pub fn watch_rows(&self) -> Result<(Follow, mpsc::UnboundedReceiver<()>)> {
        let after = (js_sys::Date::now() / 1000.) as u64;
        let source = web_sys::EventSource::new(&format!("/chats/events?format=json&after={after}"))
            .map_err(js_error)?;
        let (sender, receiver) = mpsc::unbounded();
        let listener =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_: web_sys::MessageEvent| {
                let _ = sender.unbounded_send(());
            });
        source
            .add_event_listener_with_callback("rows", listener.as_ref().unchecked_ref())
            .map_err(js_error)?;
        Ok((
            Follow {
                source,
                _listeners: vec![listener],
            },
            receiver,
        ))
    }

    /// Reads `path` on this site as bytes; `None` when it answers 204.
    pub async fn bytes(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let init = web_sys::RequestInit::new();
        init.set_method("GET");
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        let request = web_sys::Request::new_with_str_and_init(path, &init).map_err(js_error)?;
        let window = web_sys::window().ok_or_else(|| anyhow!("no window"))?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        let status = response.status();
        if status == 204 {
            return Ok(None);
        }
        let buffer = JsFuture::from(response.array_buffer().map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        if !(200..300).contains(&status) {
            return Err(anyhow::Error::new(Refusal::read(status, &bytes)));
        }
        Ok(Some(bytes))
    }

    /// Reads `path` on this site as JSON.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send("GET", path, None::<&()>).await
    }

    /// Posts a form body (`a=b&c=d`) to one of this site's pages, as the
    /// page's own form would, and follows its redirect. For pages whose
    /// posts take forms, not JSON (`/environments/*`, #12225).
    pub async fn post_form(&self, path: &str, body: &str) -> Result<()> {
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        let headers = web_sys::Headers::new().map_err(js_error)?;
        headers
            .set("content-type", "application/x-www-form-urlencoded")
            .map_err(js_error)?;
        headers.set(CSRF_HEADER, &self.csrf).map_err(js_error)?;
        if let Some(surface) = &self.chat_surface {
            headers
                .set("x-openagents-chat-surface", surface)
                .map_err(js_error)?;
        }
        init.set_headers(&headers);
        init.set_body(&JsValue::from_str(body));
        let request = web_sys::Request::new_with_str_and_init(path, &init).map_err(js_error)?;
        let window = web_sys::window().ok_or_else(|| anyhow!("no window"))?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| anyhow!("We couldn't reach openagents.com. Check your connection."))?
            .dyn_into()
            .map_err(js_error)?;
        if response.ok() {
            Ok(())
        } else {
            Err(anyhow!("That didn't go through. Try again."))
        }
    }

    /// The page's CSRF value, for a form the page posts itself.
    pub fn csrf(&self) -> &str {
        &self.csrf
    }

    pub async fn send<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T> {
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        let headers = web_sys::Headers::new().map_err(js_error)?;
        headers
            .set("accept", "application/json")
            .map_err(js_error)?;
        if method != "GET" {
            headers.set(CSRF_HEADER, &self.csrf).map_err(js_error)?;
            if let Some(surface) = &self.chat_surface {
                headers
                    .set("x-openagents-chat-surface", surface)
                    .map_err(js_error)?;
                // The Grid terminal's model pick (#12403): the page sets
                // `openagentsPylonModel` while a person asks the V4.1 group,
                // and the site routes the turn to it (#12416).
                if js_sys::Reflect::get(&js_sys::global(), &"openagentsLab".into())
                    .ok()
                    .and_then(|v| v.as_string())
                    .as_deref()
                    == Some("game")
                {
                    headers.set("x-openagents-lab", "game").map_err(js_error)?;
                }
                if let Some(model) =
                    js_sys::Reflect::get(&js_sys::global(), &"openagentsPylonModel".into())
                        .ok()
                        .and_then(|v| v.as_string())
                        .filter(|m| !m.is_empty() && m.len() <= 64)
                {
                    headers
                        .set("x-openagents-pylon-model", &model)
                        .map_err(js_error)?;
                }
            }
        }
        if let Some(body) = body {
            headers
                .set("content-type", "application/json")
                .map_err(js_error)?;
            init.set_body(&JsValue::from_str(&serde_json::to_string(body)?));
        }
        init.set_headers(&headers);
        let request = web_sys::Request::new_with_str_and_init(path, &init).map_err(js_error)?;
        let window = web_sys::window().ok_or_else(|| anyhow!("no window"))?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| {
                anyhow::Error::new(Refusal {
                    code: "offline".into(),
                    message: "We couldn't reach openagents.com. Check your connection.".into(),
                })
            })?
            .dyn_into()
            .map_err(js_error)?;
        let status = response.status();
        let buffer = JsFuture::from(response.array_buffer().map_err(js_error)?)
            .await
            .map_err(js_error)?;
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        if !(200..300).contains(&status) {
            return Err(anyhow::Error::new(Refusal::read(status, &bytes)));
        }
        if bytes.is_empty() {
            return serde_json::from_str("null").map_err(Into::into);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// A live answer's connection; dropping it closes the connection.
pub struct Follow {
    source: web_sys::EventSource,
    _listeners: Vec<Closure<dyn FnMut(web_sys::MessageEvent)>>,
}

impl Drop for Follow {
    fn drop(&mut self) {
        self.source.close();
    }
}

/// The event a closed connection ends a turn's stream with.
fn lost(request_id: &str) -> Event {
    Event::Error {
        request_id: request_id.to_owned(),
        error: TurnRefusal {
            code: RefusalCode::Other("disconnected".into()),
            message: "We lost the connection. Reload to keep following.".into(),
            retry_after_ms: None,
        },
    }
}

fn js_error(value: JsValue) -> anyhow::Error {
    anyhow!("{value:?}")
}

fn encode(value: &str) -> String {
    String::from(js_sys::encode_uri_component(value))
}
