//! Offline visual acceptance of the bare world's ball with the shared renderer.
//! Usage: bare_capture OUTPUT.png [WALK_SECONDS] [IDLE_SECONDS] [ORBIT_PIXELS]
//!
//! Starts the bare world at its spawn, walks forward for WALK_SECONDS
//! (default 0, the ball ahead at rest), waits IDLE_SECONDS (default 0.5),
//! orbits the camera ORBIT_PIXELS sideways (default 0), and renders a
//! portrait phone frame. It prints the ball's position and speed and the
//! last physics step's time.
use std::path::PathBuf;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let number = |value: Option<String>, default: f32| -> Result<f32, String> {
        value.map_or(Ok(default), |s| {
            s.parse().map_err(|_| format!("{s} is not a number"))
        })
    };
    let walk = number(args.next(), 0.0)?;
    let idle = number(args.next(), 0.5)?;
    let orbit = number(args.next(), 0.0)?;
    let mut runtime = WorldRuntime::bare();
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    let frame = 1.0 / 60.0;
    for _ in 0..(walk / frame).round() as usize {
        runtime.tick(&forward, frame);
    }
    for _ in 0..(idle / frame).round() as usize {
        runtime.tick(&InputState::default(), frame);
    }
    if orbit != 0.0 {
        runtime.apply(Action::Orbit { dx: orbit, dy: 0.0 })?;
    }
    if let Some(ball) = runtime.ball() {
        let body = ball.body();
        eprintln!(
            "ball at {:.2?}, {:.2} m/s, {:.2} rad/s; step {:?}",
            body.pos,
            body.vel.length(),
            body.omega.length(),
            ball.world().stats.total
        );
    }
    let (width, height) = (590, 1280);
    let aspect = width as f32 / height as f32;
    let atlas = verse::ui::Atlas::new(16.0);
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &runtime.world.mesh,
        runtime.view(aspect),
        &runtime.dynamic_mesh(),
        &verse::ui::UiBatch::default(),
        &atlas,
        runtime.atmosphere(),
    )
}
