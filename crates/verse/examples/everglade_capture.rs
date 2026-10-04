//! Offline visual acceptance of Everglade with the shared renderer.
//! Usage: everglade_capture OUTPUT.png [approach|sky|yard|hall|lane-east|lane-west|studio-yard|studio-hall|studio-atrium] [FRAME]
//!
//! Installs Everglade from the committed, pinned pack, as a portal entry
//! does after the download, and renders one of these views with the zone
//! HUD:
//!
//! - `approach` (the default): from the stepping stones near the return
//!   portal, up the path through the gate toward the workshop.
//! - `sky`: from the approach, turned toward the Sun and looking up, for
//!   the daylight sky and its clouds.
//! - `yard`: from above the yard's south edge, over the Task Wall, the
//!   proving ring, and the podium to the hall's facade.
//! - `hall`: inside the hall, over the desks and their monitors toward the
//!   gallery and the hearth.
//! - `studio-atrium`: inside the gate, at the goal board, with the goal
//!   bar and its waiting badge over the view.
//! - `studio-yard`, `studio-hall`, and `studio-atrium`: views of a running
//!   Agent Studio. The example records the simulated team
//!   (`coder::task::studio_sim`) against a scratch repository in a
//!   temporary directory, with no model or network, and shows frame
//!   `FRAME` of the recording: by default, the first with a seat at the
//!   proving ground for the yard, the first with a seat editing for the
//!   hall, and the first with a decision waiting for the atrium. It prints every frame's index and label, so another frame can
//!   be chosen. These views need the `model-host` feature.
use std::path::{Path, PathBuf};
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade_pack},
};

const SKY_YAW: f32 = -2.48;
const SKY_TILT: f32 = -250.0;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "approach".into());
    let frame = args
        .next()
        .map(|v| {
            v.parse::<usize>()
                .map_err(|_| format!("FRAME is a number, got {v}"))
        })
        .transpose()?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let mut runtime = WorldRuntime::new();
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    let (at, yaw, tilt) = match view.as_str() {
        "approach" => (glam::Vec3::new(0.0, 0.0, -29.0), 0.0, 0.0),
        // From the approach, turned toward the Sun and tilted up at the sky.
        "sky" => (glam::Vec3::new(0.0, 0.0, -29.0), SKY_YAW, SKY_TILT),
        "yard" | "studio-yard" => (glam::Vec3::new(-3.0, 0.0, -15.0), 0.25, 80.0),
        // At the desks station; the camera stays inside, by the doors.
        // From the yard toward the café pavilion and the reading room.
        "lane-east" => (glam::Vec3::new(10.0, 0.0, -20.0), 0.65, 40.0),
        // From the yard toward the cottage.
        "lane-west" => (glam::Vec3::new(-7.0, 0.0, -12.0), -0.68, 40.0),
        "hall" | "studio-hall" => (glam::Vec3::new(0.0, 0.0, 5.0), 0.0, 20.0),
        // Inside the gate, looking up at the goal board.
        "studio-atrium" => (glam::Vec3::new(2.8, 0.0, -13.3), 0.5, 10.0),
        other => {
            return Err(format!(
                "unknown view `{other}`; use approach, sky, yard, hall, lane-east, lane-west, studio-yard, studio-hall, \
                 or studio-atrium"
            ));
        }
    };
    runtime.set_spawn(at, yaw)?;
    // Kept until the shot is rendered; the recording holds no file in it.
    let _scratch = if view.starts_with("studio-") {
        Some(studio(&mut runtime, &view, frame)?)
    } else {
        None
    };
    runtime.apply(Action::Orbit { dx: 0.0, dy: tilt })?;
    let idle = InputState::default();
    for _ in 0..10 {
        runtime.tick(&idle, 0.05);
    }
    let mut atlas = verse::ui::Atlas::new(16.0);
    zones::everglade::hotbar::add_sprites(&mut atlas)?;
    let snapshot = runtime.zone_snapshot(1.6);
    eprintln!("{}", snapshot.caption);
    // Everglade's only HUD is the movement hotbar, as the apps draw it.
    let mut ui = verse::ui::UiBatch::default();
    if let Some(slots) = runtime.everglade_hotbar() {
        zones::everglade::hotbar::draw(&mut ui, &atlas, [1280.0, 800.0], 14.0, &slots);
    }
    if let Some(summary) = runtime
        .studio()
        .view()
        .and_then(zones::everglade::signals::Summary::of)
    {
        let _ = verse::hud::studio_strip(&mut ui, &atlas, [1280.0, 800.0], 1.0, &summary, false);
    }
    verse::render::capture_with_atmosphere(
        &output,
        1280,
        800,
        &runtime.world.mesh,
        runtime.view(1.6),
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}

/// Plays the simulated team's recording in `runtime`'s Everglade, held at
/// `frame` or the view's default.
#[cfg(feature = "model-host")]
fn studio(
    runtime: &mut WorldRuntime,
    view: &str,
    frame: Option<usize>,
) -> Result<tempfile::TempDir, String> {
    use coder_access::studio::Activity;
    use verse::zones::everglade::studio::fixture::{Player, Recording};
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let recording = Recording::run(&scratch.path().join("sim"))?;
    for (index, frame) in recording.frames().iter().enumerate() {
        eprintln!("frame {index}: {}", frame.label);
    }
    let wanted = if view == "studio-yard" {
        Activity::Testing
    } else {
        Activity::Editing
    };
    let index = frame
        .or_else(|| {
            if view == "studio-atrium" {
                recording.find(|v| !v.decisions.is_empty())
            } else {
                recording.find(|v| v.seats.iter().any(|s| s.activity == wanted))
            }
        })
        .unwrap_or(0)
        .min(recording.frames().len().saturating_sub(1));
    eprintln!(
        "showing frame {index}: {}",
        recording
            .frames()
            .get(index)
            .map_or("", |f| f.label.as_str())
    );
    runtime.set_studio_source(Box::new(Player::new(recording, index, None)));
    runtime.update_studio(true, 0.0);
    if runtime.studio().view().is_none() {
        return Err("the studio fixture did not load".into());
    }
    Ok(scratch)
}

#[cfg(not(feature = "model-host"))]
fn studio(_: &mut WorldRuntime, _: &str, _: Option<usize>) -> Result<(), String> {
    Err("the studio views need the model-host feature".into())
}
