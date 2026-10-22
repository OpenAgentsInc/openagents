//! The suggestion box on the Grid homepage (#12460): a mailbox on the
//! landing plaza with SUGGESTION BOX floating over it. When the visitor
//! walks up, a prompt floats over it ("Press E to leave a suggestion", or
//! "Tap here…" on a touch screen). E or a tap calls the page's
//! `homeSuggest(open)`, which opens the box in the GPUI chat; walking away
//! closes it.
//!
//! The GPUI chat hands a written suggestion over in the page global
//! `openagentsSuggestOut` (JSON: `text`, `category`). This module seals it
//! with NIP-17 from the player's Grid key to the suggestion box's key
//! (`playtest::suggestion`), publishes the box's wrap and the player's own
//! copy, and reports in `openagentsSuggestStatus` (JSON: `state`, one of
//! `sending`, `delivered`, `saved`, or `error`, and `line`). "Delivered"
//! means the relay took the wrap: the drop-in alone is not proof. A wrap
//! the relay hasn't taken stays queued in this browser and is sent again,
//! so a suggestion is never silently dropped.

use std::cell::RefCell;

use glam::{Mat4, Vec3};
use playtest::suggestion::{self, Category, Client, Suggestion};
use verse::ui::{Atlas, UiBatch};
use wasm_bindgen::{JsCast, JsValue};

use crate::grid::Presence;
use nostr::domain::{Event, RelaySigner, Tag};
use verse::net::{In, Link, Out};

/// The page global that is `true` while the box is open.
const OPEN_GLOBAL: &str = "openagentsChatSuggest";
/// The page's function that opens and closes the box.
const OPEN_CALL: &str = "homeSuggest";
/// Where the GPUI chat leaves a suggestion to send.
const OUT_GLOBAL: &str = "openagentsSuggestOut";
/// Where this module says how the last one went.
const STATUS_GLOBAL: &str = "openagentsSuggestStatus";
/// Where wraps the relay hasn't taken wait, in this browser.
const QUEUE_KEY: &str = "openagents.grid.suggestions";
/// How long the relay has to take a wrap before it waits for a retry, ms.
const ANSWER_WAIT: f64 = 10_000.0;
/// How often waiting wraps are sent again, ms.
const RETRY_EVERY: f64 = 30_000.0;
/// The prompt's padding, CSS pixels.
const PAD: [f32; 2] = [10.0, 6.0];

/// One suggestion on its way: the box's wrap and the sender's copy, as
/// JSON events.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Waiting {
    code: String,
    wrap: String,
    copy: String,
    #[serde(default)]
    relay: String,
    #[serde(default)]
    wrap_ok: bool,
    #[serde(default)]
    copy_ok: bool,
}

#[derive(Default)]
struct BoxState {
    near: bool,
    prompt: Option<[f32; 4]>,
    touch: Option<bool>,
    /// The wrap last sent from this visit, its code, and when, ms.
    sent: Option<(String, String, f64)>,
    /// When waiting wraps were last sent again, ms.
    retried: f64,
    config: Option<suggestion::InboxConfig>,
    loading: bool,
    config_checked: f64,
    links: std::collections::HashMap<String, Link>,
    player: [f32; 2],
}

thread_local! {
    static BOX: RefCell<BoxState> = RefCell::default();
}

fn global(name: &str) -> JsValue {
    js_sys::Reflect::get(&js_sys::global(), &name.into()).unwrap_or(JsValue::UNDEFINED)
}

fn open_now() -> bool {
    global(OPEN_GLOBAL).as_bool().unwrap_or(false)
}

fn call(open: bool) {
    if let Some(callback) = global(OPEN_CALL).dyn_ref::<js_sys::Function>() {
        let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(open));
    }
}

fn status(state: &str, line: &str) {
    let json = serde_json::json!({"state": state, "line": line}).to_string();
    let _ = js_sys::Reflect::set(
        &js_sys::global(),
        &STATUS_GLOBAL.into(),
        &JsValue::from_str(&json),
    );
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn queue() -> Result<Vec<Waiting>, String> {
    let storage = storage().ok_or("Enable site storage to save suggestions.")?;
    let text = storage
        .get_item(QUEUE_KEY)
        .map_err(|_| "Saved suggestions couldn't be read.")?;
    text.map(|text| {
        serde_json::from_str(&text).map_err(|_| {
            "Saved suggestions couldn't be read. Keep this browser's site data and contact us."
                .into()
        })
    })
    .unwrap_or_else(|| Ok(Vec::new()))
}

fn save(queue: &[Waiting]) -> Result<(), String> {
    let storage = storage()
        .ok_or("This browser can't save suggestions. Enable site storage and try again.")?;
    storage
        .set_item(
            QUEUE_KEY,
            &serde_json::to_string(queue).map_err(|_| "The suggestion couldn't be saved.")?,
        )
        .map_err(|_| "This device is out of storage. Free some space and try again.".into())
}

fn secret() -> Result<secp256k1::SecretKey, String> {
    let storage = storage().ok_or("Enable site storage to save your suggestion.")?;
    if let Some(hex) = storage
        .get_item("openagents.grid.secret")
        .map_err(|_| "Your key couldn't be read.")?
    {
        return hex.parse().map_err(|_| "Your key couldn't be read.".into());
    }
    let key = verse::identity::random_secret();
    storage
        .set_item("openagents.grid.secret", &key.display_secret().to_string())
        .map_err(|_| "Your key couldn't be saved.")?;
    Ok(key)
}

fn signer() -> Result<RelaySigner, String> {
    RelaySigner::from_secret_hex(&secret()?.display_secret().to_string())
        .map_err(|_| "Your key couldn't be read.".into())
}

fn load_config() {
    let due = BOX.with(|b| {
        let mut b = b.borrow_mut();
        if b.config.is_none() {
            b.config = storage()
                .and_then(|s| s.get_item("openagents.suggestions.config").ok().flatten())
                .and_then(|text| serde_json::from_str::<suggestion::InboxConfig>(&text).ok())
                .filter(|c| c.check().is_ok());
        }
        let now = js_sys::Date::now();
        if b.loading || now - b.config_checked < 30_000. {
            false
        } else {
            b.loading = true;
            b.config_checked = now;
            true
        }
    });
    if !due {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        let found = async {
            let window = web_sys::window().ok_or(())?;
            let response = wasm_bindgen_futures::JsFuture::from(
                window.fetch_with_str("/suggestions/config.json"),
            )
            .await
            .map_err(|_| ())?;
            let response: web_sys::Response = response.dyn_into().map_err(|_| ())?;
            if !response.ok() {
                return Err(());
            }
            let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|_| ())?)
                .await
                .map_err(|_| ())?
                .as_string()
                .ok_or(())?;
            let config: suggestion::InboxConfig = serde_json::from_str(&text).map_err(|_| ())?;
            config.check().map_err(|_| ())?;
            Ok(config)
        }
        .await;
        BOX.with(|b| {
            let mut b = b.borrow_mut();
            b.loading = false;
            if let Ok(config) = found {
                if let Some(storage) = storage() {
                    let _ = storage.set_item(
                        "openagents.suggestions.config",
                        &serde_json::to_string(&config).unwrap_or_default(),
                    );
                }
                b.config = Some(config);
            }
        });
    });
}

fn take_out(now: f64) {
    let Some(text) = global(OUT_GLOBAL).as_string() else {
        return;
    };
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let config = BOX.with(|b| b.borrow().config.clone());
    if config.is_none() {
        load_config();
    }
    let category = value["category"]
        .as_str()
        .and_then(Category::from_name)
        .unwrap_or_default();
    let client = global("openagentsSuggestionClient")
        .as_string()
        .as_deref()
        .and_then(Client::from_name)
        .unwrap_or(Client::VerseGrid);
    let place = (client == Client::VerseGrid).then(|| BOX.with(|b| b.borrow().player));
    let made = (|| {
        let s = Suggestion::new(
            client,
            category,
            value["text"].as_str().unwrap_or_default(),
            place,
            None,
            (js_sys::Date::now() / 1000.) as u64,
        )?;
        let random = || playtest::report::Randomness {
            wrapper: verse::identity::random_secret(),
            seal_nonce: verse::identity::random_bytes(),
            wrap_nonce: verse::identity::random_bytes(),
            seal_earlier: u64::from(u16::from_le_bytes(verse::identity::random_bytes())) * 2,
            wrap_earlier: u64::from(u16::from_le_bytes(verse::identity::random_bytes())) * 2,
        };
        let key = secret()?;
        let recipient = match &config {
            Some(config) => config
                .inbox
                .parse()
                .map_err(|_| "The inbox key couldn't be read.")?,
            None => key.x_only_public_key(&secp256k1::Secp256k1::new()).0,
        };
        let sealed = suggestion::wrap(&s, &key, &recipient, &random(), &random())?;
        let waiting = Waiting {
            code: sealed.code.clone(),
            wrap: serde_json::to_string(&sealed.wrap).unwrap_or_default(),
            copy: serde_json::to_string(&sealed.copy).unwrap_or_default(),
            relay: config
                .as_ref()
                .map(|c| c.relays[0].clone())
                .unwrap_or_default(),
            wrap_ok: false,
            copy_ok: false,
        };
        let mut kept = queue()?;
        kept.push(waiting);
        save(&kept)?;
        BOX.with(|b| {
            let mut b = b.borrow_mut();
            b.sent = Some((sealed.wrap.id, sealed.code, now));
            b.retried = 0.;
        });
        Ok::<_, String>(())
    })();
    let _ = js_sys::Reflect::set(&js_sys::global(), &OUT_GLOBAL.into(), &JsValue::UNDEFINED);
    match made {
        Ok(()) => status(
            if config.is_some() { "sending" } else { "saved" },
            if config.is_some() {
                suggestion::SENDING
            } else {
                suggestion::SAVED
            },
        ),
        Err(line) => {
            let _ = js_sys::Reflect::set(
                &js_sys::global(),
                &"openagentsSuggestFailed".into(),
                &value["text"].as_str().unwrap_or_default().into(),
            );
            status("error", &line);
        }
    }
}

fn follow(now: f64) {
    let Ok(mut kept) = queue() else {
        return;
    };
    if kept.is_empty() {
        return;
    }
    let config = BOX.with(|b| b.borrow().config.clone());
    if let Some(config) = config {
        let mut changed = false;
        for waiting in &mut kept {
            if !waiting.relay.is_empty() {
                continue;
            }
            let prepared = (|| {
                let key = secret()?;
                let event: Event = serde_json::from_str(&waiting.copy)
                    .map_err(|_| "Saved suggestion couldn't be read.")?;
                let opened = suggestion::open(&event, &key)?;
                let random = || playtest::report::Randomness {
                    wrapper: verse::identity::random_secret(),
                    seal_nonce: verse::identity::random_bytes(),
                    wrap_nonce: verse::identity::random_bytes(),
                    seal_earlier: 0,
                    wrap_earlier: 0,
                };
                let sealed = suggestion::wrap(
                    &opened.suggestion,
                    &key,
                    &config.inbox.parse().map_err(|_| "Invalid inbox key")?,
                    &random(),
                    &random(),
                )?;
                Ok::<_, String>((
                    serde_json::to_string(&sealed.wrap).unwrap_or_default(),
                    serde_json::to_string(&sealed.copy).unwrap_or_default(),
                    sealed.code,
                ))
            })();
            if let Ok((wrap, copy, code)) = prepared {
                waiting.wrap = wrap;
                waiting.copy = copy;
                waiting.code = code;
                waiting.relay = config.relays[0].clone();
                changed = true;
            }
        }
        if changed && save(&kept).is_err() {
            return;
        }
    } else if !kept.is_empty() {
        load_config();
    }
    let Ok(signer) = signer() else {
        return;
    };
    let mut answers = std::collections::HashMap::new();
    BOX.with(|b| {
        let mut b = b.borrow_mut();
        for waiting in &kept {
            if !waiting.relay.is_empty() {
                b.links
                    .entry(waiting.relay.clone())
                    .or_insert_with(|| Link::start(&waiting.relay));
            }
        }
        let mut connected = false;
        for (relay, link) in &b.links {
            for input in link.drain() {
                match input {
                    In::Auth(challenge) => {
                        link.send(Out::Auth(signer.sign(
                            (js_sys::Date::now() / 1000.) as u64,
                            22242,
                            vec![
                                Tag::new(vec!["relay".into(), relay.clone()]),
                                Tag::new(vec!["challenge".into(), challenge]),
                            ],
                            String::new(),
                        )));
                    }
                    In::Ok { id, accepted, .. } => {
                        if accepted {
                            connected = true;
                        }
                        answers.insert(id, accepted);
                    }
                    In::Connected => connected = true,
                    _ => {}
                }
            }
        }
        if connected {
            b.retried = 0.;
        }
    });
    let mut newly_delivered = false;
    for waiting in &mut kept {
        let was_delivered = waiting.wrap_ok;
        if let Ok(wrap) = serde_json::from_str::<Event>(&waiting.wrap) {
            waiting.wrap_ok |= answers.get(&wrap.id) == Some(&true);
        }
        if let Ok(copy) = serde_json::from_str::<Event>(&waiting.copy) {
            waiting.copy_ok |= answers.get(&copy.id) == Some(&true);
        }
        newly_delivered |= !was_delivered && waiting.wrap_ok;
    }
    if newly_delivered {
        status("delivered", suggestion::DELIVERED);
        let _ = js_sys::Reflect::set(
            &js_sys::global(),
            &"openagentsSuggestionDropped".into(),
            &JsValue::from_f64(now),
        );
    }
    let sent = BOX.with(|b| b.borrow().sent.clone());
    if let Some((_, code, at)) = sent {
        if kept.iter().any(|w| w.code == code && w.wrap_ok) {
            status("delivered", suggestion::DELIVERED);
            let _ = js_sys::Reflect::set(
                &js_sys::global(),
                &"openagentsSuggestionDropped".into(),
                &JsValue::from_f64(now),
            );
            BOX.with(|b| b.borrow_mut().sent = None);
        } else if now - at > ANSWER_WAIT {
            status("saved", suggestion::SAVED);
            BOX.with(|b| b.borrow_mut().sent = None);
        }
    }
    kept.retain(|w| !(w.wrap_ok && w.copy_ok));
    // Save acknowledgments before a retry. Storage failures keep the old,
    // durable events; duplicate NIP-17 deliveries are harmless.
    let _ = save(&kept);
    BOX.with(|b| {
        let mut b = b.borrow_mut();
        if now - b.retried < RETRY_EVERY {
            return;
        }
        b.retried = now;
        for waiting in &kept {
            if let Some(link) = b.links.get(&waiting.relay) {
                for (json, accepted) in [
                    (&waiting.wrap, waiting.wrap_ok),
                    (&waiting.copy, waiting.copy_ok),
                ] {
                    if !accepted && let Ok(event) = serde_json::from_str(json) {
                        link.send(Out::Publish(event));
                    }
                }
            }
        }
    });
}

/// Run delivery independently of the world renderer, including offline.
pub fn start() {
    load_config();
    wasm_bindgen_futures::spawn_local(async {
        loop {
            let now = js_sys::Date::now();
            load_config();
            take_out(now);
            follow(now);
            if let Some(dropped) = global("openagentsSuggestionDropped").as_f64() {
                if now - dropped > 1800. && open_now() {
                    call(false);
                }
            }
            gloo_timers::future::TimeoutFuture::new(100).await;
        }
    });
}

/// Follows the visitor at `player`; `plaza` is whether they are on the
/// Grid plaza rather than in a zone. Sends what the box was given.
pub fn frame(player: Vec3, plaza: bool, _presence: Option<&mut Presence>, _now: f64) {
    BOX.with(|b| {
        let mut b = b.borrow_mut();
        b.player = [player.x, player.z];
        let at = player.as_dvec3().to_array();
        let near = plaza && grid_suggestion_box::in_reach(at, b.near);
        if near && !b.near {
            crate::visit::send([("suggestion_box_reached", serde_json::json!({}))]);
        }
        if !near
            && b.near
            && open_now()
            && global("openagentsSuggestionClient").as_string().is_none()
        {
            call(false);
        }
        b.near = near;
        if !near {
            b.prompt = None;
        }
    });
}

fn plate(ui: &mut UiBatch, atlas: &Atlas, text: &str, x: f32, y: f32) -> [f32; 4] {
    let w = atlas.measure(text) + PAD[0] * 2.0;
    let h = atlas.line + PAD[1] * 2.0;
    let (left, top) = (x - w / 2.0, y - h);
    ui.rect(
        atlas,
        left,
        top,
        w,
        h,
        crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.92),
    );
    ui.frame(
        atlas,
        left,
        top,
        w,
        h,
        1.0,
        crate::theme::linear(coder_ui::coder_noir::STROKE, 1.0),
    );
    ui.text(
        atlas,
        left + PAD[0],
        top + PAD[1],
        text,
        crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
    );
    [left, top, w, h]
}

/// Draws the label over the box from across the plaza, and the prompt
/// while the visitor is in reach and the box is closed.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, eye: Vec3) {
    if let Some(dropped) = global("openagentsSuggestionDropped").as_f64() {
        let progress = ((js_sys::Date::now() - dropped) / 600.).clamp(0., 1.) as f32;
        if progress < 1. {
            let slot =
                Vec3::from_array(grid_suggestion_box::world([0., 1.4, 0.31]).map(|v| v as f32));
            if let Some([x, y]) = verse::hud::project(view_proj, size, slot) {
                ui.rect(
                    atlas,
                    x - 6.,
                    y - 24. + progress * 24.,
                    12.,
                    7.,
                    crate::theme::linear(coder_ui::coder_noir::CONTENT, 1. - progress),
                );
            }
        }
    }
    BOX.with(|b| {
        let mut b = b.borrow_mut();
        b.prompt = None;
        let label = Vec3::from_array(grid_suggestion_box::label_at().map(|v| v as f32));
        let near = b.near && !open_now();
        if !near
            && eye.distance(label) <= grid_suggestion_box::LABEL_RANGE as f32
            && let Some([x, y]) = verse::hud::project(view_proj, size, label)
        {
            plate(ui, atlas, grid_suggestion_box::LABEL, x, y);
        }
        if !near {
            return;
        }
        let touch = *b.touch.get_or_insert_with(crate::help_desk::touch);
        let at = Vec3::from_array(grid_suggestion_box::prompt_at().map(|v| v as f32));
        let Some([x, y]) = verse::hud::project(view_proj, size, at) else {
            return;
        };
        let text = if touch {
            grid_suggestion_box::PROMPT_TOUCH
        } else {
            grid_suggestion_box::PROMPT_KEYS
        };
        b.prompt = Some(plate(ui, atlas, text, x, y));
    });
}

/// A key the world would take: E in reach opens the box. Returns whether
/// the box used it.
pub fn key(code: &str) -> bool {
    let near = BOX.with(|b| b.borrow().near);
    if near && code == grid_suggestion_box::KEY && !open_now() {
        call(true);
        return true;
    }
    false
}

/// A press at `at`, CSS pixels: a press on the prompt opens the box.
/// Returns whether the box used it.
pub fn press(at: [f32; 2]) -> bool {
    let hit = BOX.with(|b| {
        b.borrow().prompt.is_some_and(|[x, y, w, h]| {
            at[0] >= x && at[0] <= x + w && at[1] >= y && at[1] <= y + h
        })
    });
    if hit && !open_now() {
        call(true);
        return true;
    }
    false
}
