//! Live mode: the game as a hosted zone that updates while people play
//! (`docs/verse/ice-cream-man-live.md`).
//!
//! A page whose canvas has `data-live-tenant` loads everything else from
//! that livehost tenant's manifest: the rules as a WebAssembly module
//! ([`Logic`]), the tuning, and the meshes as chunks (`icecream_zone`). It
//! follows the tenant's channel through the host's event stream
//! (`/t/TENANT/_live`), and its `<html data-livehost-self>` tells
//! livehost's injected client not to reload it. When the channel moves,
//! it fetches only the objects whose hash it hasn't seen, prepares them
//! off the frame, and hands a [`Ready`] version to the app, which applies
//! it between two steps: a new logic module carries on from the same
//! state, new numbers retune the run, and new chunks replace their meshes
//! in the next frame. The rooms stream in by distance from the player.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use icecream_rules::live::HouseCache;
use icecream_rules::{Game, Input, Tuning};
use icecream_zone::chunk::{self, Part};
use icecream_zone::{Objects, Update, Zone, object_url};
use js_sys::{Function, Reflect, Uint8Array, WebAssembly};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

/// Rooms fetched at once while streaming.
const STREAMING: usize = 4;
/// The session storage key that carries a run across a shell reload.
const CARRY_KEY: &str = "icecream.live.carry.v1";

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

/// The rules module of one manifest.
pub struct Logic {
    pub hash: String,
    memory: WebAssembly::Memory,
    alloc: Function,
    call: Function,
    reply_len: Function,
}

impl Logic {
    async fn load(bytes: Vec<u8>, hash: String) -> Result<Self, String> {
        let source = Uint8Array::from(bytes.as_slice());
        let result = JsFuture::from(WebAssembly::instantiate_buffer(
            &source.to_vec(),
            &js_sys::Object::new(),
        ))
        .await
        .map_err(js_error)?;
        let instance: WebAssembly::Instance = Reflect::get(&result, &"instance".into())
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        let exports = instance.exports();
        let get = |name: &str| -> Result<JsValue, String> {
            Reflect::get(&exports, &name.into()).map_err(js_error)
        };
        let function = |name: &str| -> Result<Function, String> {
            get(name)?
                .dyn_into::<Function>()
                .map_err(|_| format!("the logic module has no {name}"))
        };
        let abi = function("ll_abi")?
            .call0(&JsValue::NULL)
            .map_err(js_error)?;
        if abi.as_f64() != Some(1.0) {
            return Err(format!("the logic module speaks ABI {abi:?}, not 1"));
        }
        let reads = function("ll_state_version")?
            .call0(&JsValue::NULL)
            .map_err(js_error)?
            .as_f64()
            .unwrap_or(0.0);
        if reads < f64::from(icecream_rules::live::STATE_VERSION) {
            return Err("the logic module reads an older game state".into());
        }
        Ok(Self {
            hash,
            memory: get("memory")?.dyn_into().map_err(js_error)?,
            alloc: function("ll_alloc")?,
            call: function("ll_call")?,
            reply_len: function("ll_reply_len")?,
        })
    }

    /// Runs one request; the reply's note and state. A trap in the module
    /// is an error, and the caller's state is untouched.
    fn request(
        &self,
        op: &str,
        args: &str,
        state: &str,
    ) -> Result<(serde_json::Value, String), String> {
        let text = format!("{op}\n{args}\n{state}");
        let len = text.len() as u32;
        let ptr = self
            .alloc
            .call1(&JsValue::NULL, &len.into())
            .map_err(js_error)?
            .as_f64()
            .unwrap_or(0.0) as u32;
        let memory = Uint8Array::new(&self.memory.buffer());
        memory.set(&Uint8Array::from(text.as_bytes()), ptr);
        let out = self
            .call
            .call2(&JsValue::NULL, &ptr.into(), &len.into())
            .map_err(js_error)?
            .as_f64()
            .unwrap_or(0.0) as u32;
        let n = self
            .reply_len
            .call0(&JsValue::NULL)
            .map_err(js_error)?
            .as_f64()
            .unwrap_or(0.0) as u32;
        let memory = Uint8Array::new(&self.memory.buffer());
        let reply = String::from_utf8(memory.subarray(out, out + n).to_vec())
            .map_err(|_| "the logic module replied with bad text".to_owned())?;
        let mut parts = reply.splitn(3, '\n');
        let ok = parts.next() == Some("ok");
        let note: serde_json::Value =
            serde_json::from_str(parts.next().unwrap_or("null")).unwrap_or_default();
        let state = parts.next().unwrap_or_default().to_owned();
        if ok {
            Ok((note, state))
        } else {
            Err(note
                .as_str()
                .unwrap_or("the logic module refused")
                .to_owned())
        }
    }

    /// A new run.
    pub fn new_game(
        &self,
        seed: u64,
        tuning: &Tuning,
        cache: &mut HouseCache,
    ) -> Result<Game, String> {
        let args = serde_json::json!({ "seed": seed, "tuning": tuning }).to_string();
        let (_, state) = self.request("new", &args, "")?;
        Game::from_state(&state, cache)
    }

    /// One step of `game`.
    pub fn step(&self, game: &Game, input: Input, cache: &mut HouseCache) -> Result<Game, String> {
        let args = serde_json::json!({ "input": input }).to_string();
        let (_, state) = self.request("step", &args, &game.to_state())?;
        Game::from_state(&state, cache)
    }

    /// `game` with new numbers; whether they apply now.
    pub fn retune(
        &self,
        game: &Game,
        tuning: &Tuning,
        cache: &mut HouseCache,
    ) -> Result<(Game, serde_json::Value), String> {
        let args = serde_json::json!({ "tuning": tuning }).to_string();
        let (note, state) = self.request("retune", &args, &game.to_state())?;
        Ok((Game::from_state(&state, cache)?, note))
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Pointer {
    manifest: String,
    #[serde(default)]
    seq: u64,
}

/// A version fetched and prepared, waiting for the next safe point.
pub struct Ready {
    pub manifest: String,
    pub seq: u64,
    pub objects: Objects,
    pub zone: Zone,
    pub update: Update,
    pub logic: Option<Logic>,
    pub tuning: Option<Tuning>,
    /// Decoded chunks by path.
    pub chunks: Vec<(String, Vec<Part>)>,
    /// URLs fetched for it, to count their bytes.
    pub urls: Vec<String>,
    /// `performance.now()` when the pointer arrived.
    pub since: f64,
}

#[derive(Default)]
struct Inbox {
    first: Option<Function>,
    pointers: Vec<Pointer>,
    ready: Vec<Result<Ready, String>>,
    rooms: Vec<(String, String, Vec<Part>)>,
    fetching: bool,
    in_flight: usize,
}

/// A run carried across a reload of the page.
#[derive(serde::Serialize, Deserialize)]
pub struct Carry {
    pub manifest: String,
    pub state: String,
}

/// The zone the page plays.
pub struct Live {
    pub tenant: String,
    pub manifest: String,
    pub seq: u64,
    pub objects: Objects,
    pub zone: Zone,
    pub logic: Logic,
    pub tuning: Tuning,
    pub cache: HouseCache,
    /// The logic module's last error, while the game waits for a fix.
    pub broken: Option<String>,
    /// Room chunks asked for, by path.
    requested: BTreeSet<String>,
    inbox: Rc<RefCell<Inbox>>,
    /// Kept so the stream stays open.
    _stream: web_sys::EventSource,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

async fn fetch_all(urls: &[String]) -> Result<Vec<Vec<u8>>, String> {
    let window = web_sys::window().ok_or("no window")?;
    // Start every request before waiting for any.
    let requests: Vec<_> = urls
        .iter()
        .map(|url| JsFuture::from(window.fetch_with_str(url)))
        .collect();
    let mut bodies = Vec::new();
    for (request, url) in requests.into_iter().zip(urls) {
        let response: web_sys::Response = request
            .await
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        if !response.ok() {
            return Err(format!("{url}: {}", response.status()));
        }
        bodies.push(JsFuture::from(response.array_buffer().map_err(js_error)?));
    }
    let mut out = Vec::new();
    for body in bodies {
        out.push(Uint8Array::new(&body.await.map_err(js_error)?).to_vec());
    }
    Ok(out)
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let bytes = fetch_all(&[url.to_owned()]).await?.remove(0);
    String::from_utf8(bytes).map_err(|_| format!("{url} isn't text"))
}

fn url_of(objects: &Objects, path: &str) -> Result<String, String> {
    objects
        .get(path)
        .map(|hash| object_url(path, hash))
        .ok_or_else(|| format!("the manifest has no {path}"))
}

/// Fetches and prepares the version `pointer` names. With `old`, only what
/// changed; without, the first play.
async fn prepare(
    tenant: String,
    pointer: Pointer,
    old: Option<(Objects, Zone, BTreeSet<String>)>,
) -> Result<Ready, String> {
    let since = now();
    let manifest = fetch_text(&format!("/t/{tenant}/_manifest/{}", pointer.manifest)).await?;
    let objects = icecream_zone::objects(&manifest)?;
    let zone = Zone::parse(&fetch_text(&url_of(&objects, icecream_zone::INDEX)?).await?)?;
    if zone.state_version > icecream_rules::live::STATE_VERSION {
        return Err("this version needs a newer page".into());
    }
    let (update, wanted): (Update, Vec<String>) = match &old {
        Some((old_objects, _, loaded)) => {
            let update = icecream_zone::update(&zone, old_objects, &objects);
            let mut wanted = Vec::new();
            for chunk in &zone.chunks {
                let changed = update.chunks.contains(&chunk.path);
                // A room not loaded yet streams in later with its new hash.
                if changed && (chunk.room.is_none() || loaded.contains(&chunk.path)) {
                    wanted.push(chunk.path.clone());
                }
            }
            (update, wanted)
        }
        None => {
            let first = zone.first_play();
            let chunks = first
                .iter()
                .filter(|p| p.starts_with("chunks/"))
                .cloned()
                .collect();
            (
                Update {
                    logic: true,
                    tuning: true,
                    ..Update::default()
                },
                chunks,
            )
        }
    };
    let mut paths = wanted.clone();
    if update.logic {
        paths.push(zone.logic.clone());
    }
    if update.tuning {
        paths.push(zone.tuning.clone());
    }
    let urls = paths
        .iter()
        .map(|p| url_of(&objects, p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut bodies = fetch_all(&urls).await?;
    let mut tuning = None;
    if update.tuning {
        let bytes = bodies.pop().unwrap_or_default();
        tuning = Some(serde_json::from_slice(&bytes).map_err(|e| format!("tuning.json: {e}"))?);
    }
    let mut logic = None;
    if update.logic {
        let bytes = bodies.pop().unwrap_or_default();
        logic = Some(Logic::load(bytes, objects[&zone.logic].clone()).await?);
    }
    let mut chunks = Vec::new();
    for (path, bytes) in wanted.into_iter().zip(bodies) {
        chunks.push((path, chunk::decode(&bytes)?));
    }
    let mut urls = urls;
    urls.push(format!("/_manifest/{}", pointer.manifest));
    urls.push(url_of(&objects, icecream_zone::INDEX)?);
    Ok(Ready {
        manifest: pointer.manifest,
        seq: pointer.seq,
        objects,
        zone,
        update,
        logic,
        tuning,
        chunks,
        urls,
        since,
    })
}

impl Live {
    /// Joins `tenant`'s channel: waits for its pointer, then fetches the
    /// first play. The chunks it fetched come back for the app to upload.
    ///
    /// # Errors
    /// The host can't be reached, or the version doesn't load.
    pub async fn boot(tenant: String) -> Result<(Self, Ready), String> {
        // Room for every chunk a long session fetches, so each update's
        // bytes can be counted.
        if let Some(performance) = web_sys::window().and_then(|w| w.performance()) {
            performance.set_resource_timing_buffer_size(100_000);
        }
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        // The host's first message is the channel's manifest now.
        let stream = web_sys::EventSource::new(&format!("/t/{tenant}/_live")).map_err(js_error)?;
        let first = js_sys::Promise::new(&mut |resolve, _| {
            inbox.borrow_mut().first = Some(resolve);
        });
        let on_message = {
            let inbox = inbox.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let Some(text) = event.data().as_string() else {
                    return;
                };
                let Ok(pointer) = serde_json::from_str::<Pointer>(&text) else {
                    return;
                };
                let mut inbox = inbox.borrow_mut();
                if let Some(resolve) = inbox.first.take() {
                    let _ = resolve.call1(&JsValue::NULL, &JsValue::from_str(&text));
                } else {
                    inbox.pointers.push(pointer);
                }
            })
        };
        stream.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let text = JsFuture::from(first).await.map_err(js_error)?;
        let pointer: Pointer = serde_json::from_str(&text.as_string().unwrap_or_default())
            .map_err(|e| format!("a bad pointer: {e}"))?;
        let mut ready = prepare(tenant.clone(), pointer, None).await?;
        let logic = ready.logic.take().ok_or("the zone has no logic module")?;
        let tuning = ready.tuning.take().unwrap_or_default();
        let requested = ready.chunks.iter().map(|(p, _)| p.clone()).collect();
        let live = Self {
            tenant,
            manifest: ready.manifest.clone(),
            seq: ready.seq,
            objects: ready.objects.clone(),
            zone: ready.zone.clone(),
            logic,
            tuning,
            cache: HouseCache::default(),
            broken: None,
            requested,
            inbox,
            _stream: stream,
            _on_message: on_message,
        };
        Ok((live, ready))
    }

    /// The run a reload carried, if it was this zone's house.
    #[must_use]
    pub fn take_carry(&mut self) -> Option<Game> {
        let storage = web_sys::window()?.session_storage().ok()??;
        let text = storage.get_item(CARRY_KEY).ok()??;
        let _ = storage.remove_item(CARRY_KEY);
        let carry: Carry = serde_json::from_str(&text).ok()?;
        let game = Game::from_state(&carry.state, &mut self.cache).ok()?;
        (game.house.seed == self.zone.seed).then_some(game)
    }

    /// Keeps `game` for the page that loads next, then reloads.
    pub fn reload_keeping(&self, game: &Game) {
        let carry = Carry {
            manifest: self.manifest.clone(),
            state: game.to_state(),
        };
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.session_storage()
                && let Ok(text) = serde_json::to_string(&carry)
            {
                let _ = storage.set_item(CARRY_KEY, &text);
            }
            let _ = window.location().reload();
        }
    }

    /// Starts fetching what the next pointer brings, and the next rooms by
    /// distance from `at`. Call once a frame.
    pub fn pump(&mut self, at: [f32; 2]) {
        let next = {
            let mut inbox = self.inbox.borrow_mut();
            if inbox.fetching || inbox.pointers.is_empty() {
                None
            } else {
                inbox.fetching = true;
                // Only the newest pointer matters.
                inbox.pointers.drain(..).last()
            }
        };
        if let Some(pointer) = next {
            let old = Some((
                self.objects.clone(),
                self.zone.clone(),
                self.requested.clone(),
            ));
            let inbox = self.inbox.clone();
            let tenant = self.tenant.clone();
            spawn_local(async move {
                let ready = prepare(tenant, pointer, old).await;
                let mut inbox = inbox.borrow_mut();
                inbox.ready.push(ready);
                inbox.fetching = false;
            });
        }
        let room_slots = STREAMING.saturating_sub(self.inbox.borrow().in_flight);
        if room_slots == 0 {
            return;
        }
        for path in self
            .zone
            .stream_order(at, &self.requested)
            .into_iter()
            .take(room_slots)
        {
            let Some(hash) = self.objects.get(&path).cloned() else {
                continue;
            };
            self.requested.insert(path.clone());
            self.inbox.borrow_mut().in_flight += 1;
            let inbox = self.inbox.clone();
            let url = object_url(&path, &hash);
            spawn_local(async move {
                let parts = fetch_all(&[url])
                    .await
                    .and_then(|mut b| chunk::decode(&b.remove(0)));
                let mut inbox = inbox.borrow_mut();
                inbox.in_flight -= 1;
                match parts {
                    Ok(parts) => inbox.rooms.push((path, hash, parts)),
                    Err(error) => web_sys::console::warn_1(&error.into()),
                }
            });
        }
    }

    /// Streamed rooms ready to upload: each room's index and its parts.
    /// A room whose hash moved on while it loaded is asked for again.
    pub fn take_rooms(&mut self) -> Vec<(usize, Vec<Part>)> {
        let arrived = std::mem::take(&mut self.inbox.borrow_mut().rooms);
        let mut out = Vec::new();
        for (path, hash, parts) in arrived {
            if self.objects.get(&path) != Some(&hash) {
                self.requested.remove(&path);
                continue;
            }
            if let Some(room) = self
                .zone
                .chunks
                .iter()
                .find(|c| c.path == path)
                .and_then(|c| c.room)
            {
                out.push((room, parts));
            }
        }
        out
    }

    /// A version ready to apply, or a version that failed to load.
    pub fn take_ready(&mut self) -> Option<Result<Ready, String>> {
        let mut inbox = self.inbox.borrow_mut();
        (!inbox.ready.is_empty()).then(|| inbox.ready.remove(0))
    }

    /// Makes `ready` the zone's version. What it changed is the caller's
    /// to apply; this keeps the bookkeeping: the logic, the tuning, and
    /// which rooms must stream again.
    pub fn adopt(&mut self, ready: &mut Ready) {
        if let Some(logic) = ready.logic.take() {
            self.logic = logic;
            self.broken = None;
        }
        if let Some(tuning) = ready.tuning {
            self.tuning = tuning;
        }
        for path in &ready.update.chunks {
            if !ready.chunks.iter().any(|(p, _)| p == path) {
                self.requested.remove(path);
            }
        }
        self.manifest = ready.manifest.clone();
        self.seq = ready.seq;
        self.objects = ready.objects.clone();
        self.zone = ready.zone.clone();
    }

    /// Tells the host this page now shows the zone's manifest, with
    /// `note`'s fields: livehost's ack stream, which `icecream-live deploy
    /// --wait` and benchmarks read.
    pub fn ack(&self, kind: &str, mut note: serde_json::Value) {
        let Some(window) = web_sys::window() else {
            return;
        };
        note["tenant"] = self.tenant.clone().into();
        note["manifest"] = self.manifest.clone().into();
        note["seq"] = self.seq.into();
        note["logic_module"] = self.logic.hash.get(..12).unwrap_or_default().into();
        note["kind"] = kind.into();
        note["page_epoch_ms"] = js_sys::Date::now().into();
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&JsValue::from_str(&note.to_string()));
        let _ = window.fetch_with_str_and_init("/_livehost/ack", &init);
    }
}

/// Bytes moved over the network for resources fetched since `since`
/// (`performance.now()`) whose address ends with one of `urls` (all
/// resources when `urls` is empty), from the browser's resource timing. A
/// cache hit moves none.
#[must_use]
pub fn transferred(urls: &[String], since: f64) -> (f64, u32) {
    let Some(performance) = web_sys::window().and_then(|w| w.performance()) else {
        return (0.0, 0);
    };
    let entries = performance.get_entries_by_type("resource");
    let mut bytes = 0.0;
    let mut files = 0;
    for entry in entries.iter() {
        let name = Reflect::get(&entry, &"name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        if !urls.is_empty() && !urls.iter().any(|u| name.ends_with(u.as_str())) {
            continue;
        }
        let start = Reflect::get(&entry, &"startTime".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if start < since {
            continue;
        }
        let size = Reflect::get(&entry, &"transferSize".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        bytes += size;
        if size > 0.0 {
            files += 1;
        }
    }
    if urls.is_empty() {
        let navigation = performance.get_entries_by_type("navigation");
        for entry in navigation.iter() {
            bytes += Reflect::get(&entry, &"transferSize".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
        }
    }
    (bytes, files)
}

/// Milliseconds since the page started loading.
#[must_use]
pub fn since_navigation() -> f64 {
    now()
}
