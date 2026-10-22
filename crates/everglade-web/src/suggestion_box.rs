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
/// The most suggestions that wait in this browser.
const QUEUE_MAX: usize = 20;
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

fn queue() -> Vec<Waiting> {
    storage()
        .and_then(|s| s.get_item(QUEUE_KEY).ok().flatten())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(queue: &[Waiting]) {
    if let Some(storage) = storage() {
        let start = queue.len().saturating_sub(QUEUE_MAX);
        let _ = storage.set_item(
            QUEUE_KEY,
            &serde_json::to_string(&queue[start..]).unwrap_or_default(),
        );
    }
}

fn event(json: &str) -> Option<nostr::domain::Event> {
    serde_json::from_str(json).ok()
}

/// Seals and sends what the GPUI chat left in [`OUT_GLOBAL`], from the
/// visitor at `player`.
fn take_out(player: Vec3, presence: Option<&mut Presence>, now: f64) {
    let out = global(OUT_GLOBAL);
    let Some(text) = out.as_string() else {
        return;
    };
    let _ = js_sys::Reflect::set(&js_sys::global(), &OUT_GLOBAL.into(), &JsValue::UNDEFINED);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let category = value["category"]
        .as_str()
        .and_then(Category::from_name)
        .unwrap_or_default();
    let at = (js_sys::Date::now() / 1000.0) as u64;
    let made = Suggestion::new(
        Client::VerseGrid,
        category,
        value["text"].as_str().unwrap_or_default(),
        Some([player.x, player.z]),
        None,
        at,
    );
    let suggestion = match made {
        Ok(s) => s,
        Err(line) => return status("error", &line),
    };
    let Some(presence) = presence else {
        return status(
            "error",
            "The Grid isn't connected. Reload the page and try again.",
        );
    };
    let sealed = match presence.seal_suggestion(&suggestion) {
        Ok(sealed) => sealed,
        Err(line) => return status("error", &line),
    };
    let waiting = Waiting {
        code: sealed.code.clone(),
        wrap: serde_json::to_string(&sealed.wrap).unwrap_or_default(),
        copy: serde_json::to_string(&sealed.copy).unwrap_or_default(),
    };
    // Kept until the relay takes it.
    let mut kept = queue();
    kept.push(waiting);
    save(&kept);
    let id = sealed.wrap.id.clone();
    presence.publish(sealed.wrap);
    presence.publish(sealed.copy);
    crate::visit::send([(
        "suggestion_sent",
        serde_json::json!({"category": category.name()}),
    )]);
    BOX.with(|b| b.borrow_mut().sent = Some((id, sealed.code, now)));
    status("sending", suggestion::SENDING);
}

/// Watches the relay's answer to this visit's suggestion, and sends
/// waiting ones again.
fn follow(presence: Option<&mut Presence>, now: f64) {
    let Some(presence) = presence else {
        return;
    };
    let sent = BOX.with(|b| b.borrow().sent.clone());
    if let Some((id, code, at)) = sent {
        match presence.answer(&id) {
            Some(true) => {
                let kept: Vec<Waiting> = queue().into_iter().filter(|w| w.code != code).collect();
                save(&kept);
                status("delivered", suggestion::DELIVERED);
                BOX.with(|b| b.borrow_mut().sent = None);
            }
            Some(false) => {
                status("saved", suggestion::SAVED);
                BOX.with(|b| b.borrow_mut().sent = None);
            }
            None if now - at > ANSWER_WAIT => {
                status("saved", suggestion::SAVED);
                BOX.with(|b| b.borrow_mut().sent = None);
            }
            None => {}
        }
    }
    // Waiting wraps go out again, and leave the queue once taken.
    let due = BOX.with(|b| {
        let mut b = b.borrow_mut();
        let due = b.sent.is_none() && now - b.retried > RETRY_EVERY;
        if due {
            b.retried = now;
        }
        due
    });
    if !due {
        return;
    }
    let kept = queue();
    if kept.is_empty() {
        return;
    }
    let mut left = Vec::new();
    for waiting in kept {
        let Some(wrap) = event(&waiting.wrap) else {
            continue;
        };
        if presence.answer(&wrap.id) == Some(true) {
            continue;
        }
        presence.publish(wrap);
        if let Some(copy) = event(&waiting.copy) {
            presence.publish(copy);
        }
        left.push(waiting);
    }
    save(&left);
}

/// Follows the visitor at `player`; `plaza` is whether they are on the
/// Grid plaza rather than in a zone. Sends what the box was given.
pub fn frame(player: Vec3, plaza: bool, mut presence: Option<&mut Presence>, now: f64) {
    take_out(player, presence.as_deref_mut(), now);
    follow(presence, now);
    BOX.with(|b| {
        let mut b = b.borrow_mut();
        let at = player.as_dvec3().to_array();
        let near = plaza && grid_suggestion_box::in_reach(at, b.near);
        if near && !b.near {
            crate::visit::send([("suggestion_box_reached", serde_json::json!({}))]);
        }
        if !near && b.near && open_now() {
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
