//! The Grid Foreman on the Grid homepage (#12444): the Grid robot at three
//! times its rig's size beside the shared terminal, with `FOREMAN` over its
//! head and a status line under the tag.
//!
//! Where it stands, its size, and its words are plain data in
//! `crates/grid-foreman`. The status line comes from the site's
//! `GET /v1/foreman/status`, asked once a minute, so every viewer reads
//! the same line. Nobody walks through it.

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
}

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

/// Its pose at page time `now` (ms): idle at home, facing the terminal.
fn pose(now: f64) -> Pose {
    Pose {
        pos: home(),
        yaw: grid_foreman::home_yaw() as f32,
        animation: Selection::Named(State::Idle),
        time: ((now / 1000.0) % 1000.0) as f32,
        speed: 0.0,
    }
}

/// Asks the site for the status line when a minute has passed since the
/// last ask, at page time `now` (ms).
pub fn frame(now: f64) {
    let due = FOREMAN.with(|f| {
        let mut f = f.borrow_mut();
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
        let Some(line) = body["line"].as_str() else {
            return;
        };
        let line = grid_foreman::clean_line(line);
        FOREMAN.with(|f| f.borrow_mut().line = (!line.is_empty()).then_some(line));
    });
}

/// Pushes the visitor's feet at `player` out of the Foreman's body.
pub fn keep_clear(player: &mut Vec3) {
    grid_robot::keep_clear_within(
        player,
        home(),
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

/// The tag and the status line over its head, for a camera at `eye`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, eye: Vec3) {
    let head = home() + Vec3::Y * grid_foreman::TAG_LIFT;
    if head.distance(eye) > grid_foreman::TAG_RANGE {
        return;
    }
    let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
        return;
    };
    let line = FOREMAN.with(|f| {
        f.borrow()
            .line
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
