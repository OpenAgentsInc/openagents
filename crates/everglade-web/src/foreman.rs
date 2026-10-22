//! The Grid Foreman on the Grid homepage (#12444): the Grid robot at three
//! times its rig's size beside the shared terminal, with `FOREMAN` over its
//! head and a status line under the tag.
//!
//! Where it stands, its size, and its words are plain data in
//! `crates/grid-foreman`. Its host (`grid-foreman serve`, #12445) walks it
//! around the plaza and publishes it like a player under the key the
//! site's `GET /v1/foreman/status` names, so every viewer sees one Foreman
//! in the same place: this module draws that key's avatar as the giant
//! robot, with the status line its state carries. Until the host is seen,
//! it stands at home with the site's issue counts; when the host has sent
//! nothing for the crowd's stale time, it stands at home with
//! `BACK SOON`. Nobody walks through it.
//!
//! Within talking range (#12446), an offer floats over it ("Press E to talk
//! to the Foreman", or "Tap here…" on a touch screen). E or a tap calls the
//! page's `homeForeman(open)`, which opens the homepage chat with the
//! `foreman` surface; walking away closes it.

use std::cell::RefCell;

use glam::{Mat4, Vec3};
use verse::grid_robot::{self, Pose};
use verse::ui::{Atlas, UiBatch};
use verse_engine::motion::{Selection, State};
use wasm_bindgen::JsCast;

#[derive(Default)]
struct Foreman {
    /// The status line the site last gave.
    line: Option<String>,
    /// Page time of the last ask, ms.
    asked: Option<f64>,
    /// Page time of the first frame, ms.
    opened: Option<f64>,
    /// The host's key, from the site.
    pubkey: Option<String>,
    /// Where the host has it, while frames arrive.
    live: Option<Live>,
    /// Whether the visitor is in talking range.
    near: bool,
    /// Where the talk offer was last drawn, CSS pixels: x, y, width, height.
    prompt: Option<[f32; 4]>,
    touch: Option<bool>,
}

/// The page global that is `true` while the Foreman's chat is open.
const OPEN_GLOBAL: &str = "openagentsChatForeman";
/// The page's function that opens and closes the Foreman's chat.
const OPEN_CALL: &str = "homeForeman";
/// The offer's padding, CSS pixels.
const PAD: [f32; 2] = [10.0, 6.0];

fn open_now() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &OPEN_GLOBAL.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn call(open: bool) {
    if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &OPEN_CALL.into())
        && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
    {
        let _ = callback.call1(
            &wasm_bindgen::JsValue::UNDEFINED,
            &wasm_bindgen::JsValue::from_bool(open),
        );
    }
}

/// Whether the page's pointer is a finger.
fn touch() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(query) = js_sys::Reflect::get(&window, &"matchMedia".into()) else {
        return false;
    };
    let Some(query) = query.dyn_ref::<js_sys::Function>() else {
        return false;
    };
    query
        .call1(&window, &"(pointer: coarse)".into())
        .ok()
        .and_then(|list| js_sys::Reflect::get(&list, &"matches".into()).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Follows the visitor at `player`; `plaza` is whether they are on the Grid
/// plaza rather than in a zone.
pub fn talk_frame(player: Vec3, plaza: bool) {
    let foreman = at();
    let (was, near) = FOREMAN.with(|f| {
        let mut f = f.borrow_mut();
        let was = f.near;
        let near = plaza
            && grid_foreman::in_talk_range(
                player.as_dvec3().to_array(),
                foreman.as_dvec3().to_array(),
                was,
            );
        f.near = near;
        if !near {
            f.prompt = None;
        }
        (was, near)
    });
    if near && !was {
        crate::visit::send([("foreman_reached", serde_json::json!({}))]);
    }
    if !near && was && open_now() {
        call(false);
    }
}

/// Draws the talk offer while the visitor is in range and the chat is
/// closed.
pub fn draw_prompt(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4) {
    let anchor = at() + Vec3::Y * grid_foreman::PROMPT_LIFT;
    FOREMAN.with(|f| {
        let mut f = f.borrow_mut();
        f.prompt = None;
        if !f.near || open_now() {
            return;
        }
        let touch = *f.touch.get_or_insert_with(touch);
        let Some([x, y]) = verse::hud::project(view_proj, size, anchor) else {
            return;
        };
        let text = if touch {
            grid_foreman::PROMPT_TOUCH
        } else {
            grid_foreman::PROMPT_KEYS
        };
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
        f.prompt = Some([left, top, w, h]);
    });
}

/// A key the world would take: E in range opens the chat. Returns whether
/// the Foreman used it.
pub fn key(code: &str) -> bool {
    let near = FOREMAN.with(|f| f.borrow().near);
    if near && code == grid_foreman::KEY && !open_now() {
        call(true);
        return true;
    }
    false
}

/// A press at `at`, CSS pixels: a press on the offer opens the chat.
/// Returns whether the Foreman used it.
pub fn press(at: [f32; 2]) -> bool {
    let hit = FOREMAN.with(|f| {
        f.borrow().prompt.is_some_and(|[x, y, w, h]| {
            at[0] >= x && at[0] <= x + w && at[1] >= y && at[1] <= y + h
        })
    });
    if hit && !open_now() {
        call(true);
        return true;
    }
    false
}

#[derive(Clone, Debug)]
struct Live {
    pos: Vec3,
    yaw: f32,
    speed: f32,
    status: Option<String>,
}

/// How long the page waits for the host before it says `BACK SOON`, ms.
const GRACE_MS: f64 = 10_000.0;

thread_local! {
    static FOREMAN: RefCell<Foreman> = RefCell::default();
}

fn home() -> Vec3 {
    Vec3::new(
        grid_foreman::HOME[0] as f32,
        grid_foreman::HOME[1] as f32,
        grid_foreman::HOME[2] as f32,
    )
}

/// Its pose at page time `now` (ms): where the host has it, else idle at
/// home facing the terminal.
fn pose(now: f64) -> Pose {
    let seconds = ((now / 1000.0) % 1000.0) as f32;
    FOREMAN.with(|f| match &f.borrow().live {
        Some(live) if live.speed > 0.1 => Pose {
            pos: live.pos,
            yaw: live.yaw,
            animation: Selection::Named(State::Walk),
            time: seconds * grid_foreman::WALK_CLIP_RATE,
            speed: live.speed,
        },
        Some(live) => Pose {
            pos: live.pos,
            yaw: live.yaw,
            animation: Selection::Named(State::Idle),
            time: seconds,
            speed: 0.0,
        },
        None => Pose {
            pos: home(),
            yaw: grid_foreman::home_yaw() as f32,
            animation: Selection::Named(State::Idle),
            time: seconds,
            speed: 0.0,
        },
    })
}

fn at() -> Vec3 {
    FOREMAN.with(|f| f.borrow().live.as_ref().map_or_else(home, |l| l.pos))
}

/// Takes the Foreman's figures out of `peers`, which the page draws as
/// players, and keeps where the host has it; `presence` learns its key.
pub fn take(peers: &mut Vec<verse::crowd::Figure>, presence: Option<&mut crate::grid::Presence>) {
    let Some(pubkey) = FOREMAN.with(|f| f.borrow().pubkey.clone()) else {
        return;
    };
    let live = peers
        .iter()
        .find(|p| p.pubkey == pubkey && p.role == "avatar" && p.online)
        .map(|p| {
            let (axis, angle) = p.rot.to_axis_angle();
            let yaw = if axis.y < 0.0 { -angle } else { angle };
            Live {
                pos: Vec3::new(p.pos.x, 0.0, p.pos.z),
                yaw,
                speed: p.speed,
                status: None,
            }
        });
    peers.retain(|p| p.pubkey != pubkey);
    let live = match (live, presence) {
        (Some(mut live), Some(presence)) => {
            presence.set_foreman(Some(&pubkey));
            live.status = presence
                .status_of(&pubkey)
                .map(|s| grid_foreman::clean_line(&s))
                .filter(|s| !s.is_empty());
            Some(live)
        }
        (live, Some(presence)) => {
            presence.set_foreman(Some(&pubkey));
            live
        }
        (live, None) => live,
    };
    FOREMAN.with(|f| f.borrow_mut().live = live);
}

/// Asks the site for the status line when a minute has passed since the
/// last ask, at page time `now` (ms).
pub fn frame(now: f64) {
    let due = FOREMAN.with(|f| {
        let mut f = f.borrow_mut();
        f.opened.get_or_insert(now);
        let due = f
            .asked
            .is_none_or(|at| now - at >= grid_foreman::POLL_SECONDS as f64 * 1000.0);
        if due {
            f.asked = Some(now);
        }
        due
    });
    if due {
        ask();
    }
}

fn ask() {
    wasm_bindgen_futures::spawn_local(async {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(answer) =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(grid_foreman::STATUS_PATH))
                .await
        else {
            return;
        };
        let Ok(response) = answer.dyn_into::<web_sys::Response>() else {
            return;
        };
        if !response.ok() {
            return;
        }
        let Ok(text) = response.text() else {
            return;
        };
        let Ok(text) = wasm_bindgen_futures::JsFuture::from(text).await else {
            return;
        };
        let Ok(body) =
            serde_json::from_str::<serde_json::Value>(&text.as_string().unwrap_or_default())
        else {
            return;
        };
        let pubkey = body["pubkey"]
            .as_str()
            .filter(|k| k.len() == 64 && k.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(str::to_ascii_lowercase);
        let line = body["line"]
            .as_str()
            .map(grid_foreman::clean_line)
            .filter(|l| !l.is_empty());
        FOREMAN.with(|f| {
            let mut f = f.borrow_mut();
            f.line = line;
            f.pubkey = pubkey;
        });
    });
}

/// Pushes the visitor's feet at `player` out of the Foreman's body.
pub fn keep_clear(player: &mut Vec3) {
    grid_robot::keep_clear_within(
        player,
        at(),
        grid_foreman::BODY_RADIUS + verse::controller::RADIUS,
    );
}

/// This frame's Foreman instance, at page time `now` (ms) for a camera at
/// `eye`.
pub fn instances(now: f64, eye: Vec3, out: &mut Vec<verse_engine::presentation::Instance>) {
    out.push(grid_robot::instance_scaled(
        &pose(now),
        eye,
        grid_foreman::SCALE,
        grid_foreman::FAR_METERS,
    ));
}

/// The tag and the status line over its head, for a camera at `eye`, at
/// page time `now` (ms).
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, eye: Vec3, now: f64) {
    let head = at() + Vec3::Y * grid_foreman::TAG_LIFT;
    if head.distance(eye) > grid_foreman::TAG_RANGE {
        return;
    }
    let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
        return;
    };
    let line = FOREMAN.with(|f| {
        let f = f.borrow();
        if let Some(live) = &f.live {
            return live
                .status
                .clone()
                .unwrap_or_else(|| grid_foreman::STATUS_WAITING.to_owned());
        }
        let waiting = f.opened.is_none_or(|at| now - at < GRACE_MS);
        if f.pubkey.is_some() && !waiting {
            return grid_foreman::BACK_SOON.to_owned();
        }
        f.line
            .clone()
            .unwrap_or_else(|| grid_foreman::STATUS_WAITING.to_owned())
    });
    let mut top = y - atlas.line * 2.0;
    for (text, color) in [
        (grid_foreman::TAG, coder_ui::coder_noir::CONTENT),
        (line.as_str(), coder_ui::coder_noir::CONTENT_SECONDARY),
    ] {
        let width = atlas.measure(text);
        ui.text(
            atlas,
            x - width / 2.0,
            top,
            text,
            crate::theme::linear(color, 1.0),
        );
        top += atlas.line;
    }
}
