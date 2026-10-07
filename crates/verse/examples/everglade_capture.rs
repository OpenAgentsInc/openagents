//! Offline visual acceptance of Everglade with the shared renderer.
//! Usage: [VERSE_TOWN_HOUR=H] everglade_capture OUTPUT.png [approach|winds|stone|stone-crumble|stone-settled|sky|yard|hall|lane-east|lane-west|reverse|reverse-top|overhead|town-north|town-west|tooltip|city-market|city-stoop|city-lantern|city-brownstone|city-observatory|city-foundry|studio-yard|studio-hall|studio-atrium|eyes|hall-eyes|at:X,Z,YAW,TILT[,Y]|air:EX,EY,EZ,TX,TZ|look:EX,EY,EZ,TX,TY,TZ] [FRAME]
//!
//! Installs Everglade from the committed, pinned pack, as a portal entry
//! does after the download, and renders one of these views with the zone
//! HUD:
//!
//! - `approach` (the default): from the stepping stones near the return
//!   portal, up the path through the gate toward the workshop.
//! - `tooltip`: the approach with the pointer resting on the hotbar's Wind
//!   Wall slot, so its card shows over the bar.
//! - `winds`: four Wind Walls cast on the approach, fanned left to right,
//!   seen from behind the caster.
//! - `sky`: from the approach, turned toward the Sun and looking up, for
//!   the daylight sky and its clouds.
//! - `yard`: from above the yard's south edge, over the Task Wall, the
//!   proving ring, and the podium to the hall's facade.
//! - `hall`: inside the hall, over the desks and their monitors toward the
//!   gallery and the hearth.
//! - `eyes` and `hall-eyes`: the approach and the hall in first person,
//!   zoomed all the way in, with the player's character hidden.
//! - `stone`, `stone-crumble`, and `stone-settled`: two Walls of Stone
//!   cast on the approach and seen from behind the caster: standing, just
//!   after their lifetime runs out as they crumble, and with the chunks
//!   lying on the ground before they shrink away.
//! - `reverse`: Reverse Gravity cast on the approach, seen from outside
//!   its cylinder after its particles have climbed and gathered.
//! - `reverse-top`: the same cylinder from the caster hovering at its top,
//!   looking down through the rising particles.
//! - `overhead`: high above the approach, looking down over the whole town,
//!   with the fog pushed back so the far districts show.
//! - `town-north`: from the commons walk, north over Lantern Pond toward
//!   Main Street.
//! - `town-west`: from Stoop Lane, south past the homes toward Walden
//!   Woods.
//! - `city-market`, `city-stoop`, `city-lantern`, `city-brownstone`,
//!   `city-observatory`, and `city-foundry`: the city's districts from
//!   their streets: the Fountain Plaza and the Market Hall, Stoop Lane's
//!   townhouses, Hearth Road into the Lantern Quarter, Brownstone Row,
//!   Observatory Hill, and Foundry Road.
//! - `at:X,Z,YAW,TILT[,Y]`: the player standing anywhere, at `(X, Z)`
//!   facing `YAW` radians, with the camera tilted by `TILT`. With `Y`, the
//!   player drops from height `Y` and lands on what is below, such as a
//!   roof.
//! - `air:EX,EY,EZ,TX,TZ`: as `overhead`, from an eye at `(EX, EY, EZ)`
//!   looking at `(TX, 0, TZ)`, for a closer look at one district.
//! - `look:EX,EY,EZ,TX,TY,TZ`: a free camera at `(EX, EY, EZ)` looking at
//!   `(TX, TY, TZ)`, such as a close look at a placed character's face.
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
/// Where the `reverse` views cast Reverse Gravity, on the approach.
const REVERSE_Z: f32 = -26.0;

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
    // VERSE_ALICE_OUTFIT dresses Alice as `verse --alice-outfit` does.
    if let Ok(outfit) = std::env::var("VERSE_ALICE_OUTFIT") {
        zones::everglade::npcs::set_alice_outfit(&outfit)?;
    }
    let mut runtime = WorldRuntime::new();
    // With VERSE_CAPTURE_PRIVATE set to Verse's home, the owner's private
    // characters load through the broker as on the desktop
    // (docs/verse/private-assets.md).
    let private = std::env::var_os("VERSE_CAPTURE_PRIVATE").map(PathBuf::from);
    if let Some(home) = &private {
        runtime.configure_private_assets(home.clone());
    }
    // The town clock: late morning, as Everglade looked before it had one,
    // unless VERSE_TOWN_HOUR pins another hour (`18`, `6:30`) or
    // VERSE_TOWN_CLOCK=live follows the real clock.
    let clock = town_clock::Clock::DEFAULT;
    let hour = match std::env::var("VERSE_TOWN_HOUR") {
        Ok(hour) => Some(town_clock::parse_hour(&hour)?),
        Err(_) if std::env::var("VERSE_TOWN_CLOCK").is_ok_and(|v| v == "live") => None,
        Err(_) => Some(10.5),
    };
    runtime.set_town_clock(clock.pinned(hour));
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    if private.is_some() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while runtime.private_guests() == 0 && std::time::Instant::now() < deadline {
            runtime.zone_tick();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        eprintln!("private guests: {}", runtime.private_guests());
    }
    let first_person = view.ends_with("eyes");
    let (at, yaw, tilt) = match view.as_str() {
        "eyes" => (glam::Vec3::new(0.0, 0.0, -29.0), 0.0, -60.0),
        "hall-eyes" => (glam::Vec3::new(0.0, 0.0, 5.0), 0.0, -40.0),
        "approach" | "tooltip" | "winds" | "stone" | "stone-crumble" | "stone-settled" => {
            (glam::Vec3::new(0.0, 0.0, -29.0), 0.0, 0.0)
        }
        // From the approach, turned toward the Sun and tilted up at the sky.
        "sky" => (glam::Vec3::new(0.0, 0.0, -29.0), SKY_YAW, SKY_TILT),
        "yard" | "studio-yard" => (glam::Vec3::new(-3.0, 0.0, -15.0), 0.25, 80.0),
        // At the desks station; the camera stays inside, by the doors.
        // From the yard toward the café pavilion and the reading room.
        "lane-east" => (glam::Vec3::new(10.0, 0.0, -20.0), 0.65, 40.0),
        // From the yard toward the cottage.
        "lane-west" => (glam::Vec3::new(-7.0, 0.0, -12.0), -0.68, 40.0),
        // Reverse Gravity's caster on the approach.
        "reverse" | "reverse-top" => (glam::Vec3::new(0.0, 0.0, REVERSE_Z), 0.0, 0.0),
        "hall" | "studio-hall" => (glam::Vec3::new(0.0, 0.0, 5.0), 0.0, 20.0),
        "overhead" => (glam::Vec3::new(0.0, 0.0, -20.0), 0.0, 0.0),
        other if other.starts_with("air:") => (glam::Vec3::new(0.0, 0.0, -20.0), 0.0, 0.0),
        // Alice, the workshop agent, as a host answered for her: the
        // player stands in front of her wherever her work puts her.
        other if other.starts_with("alice") => (glam::Vec3::new(0.0, 0.0, 5.0), 0.0, 20.0),
        // A free camera; the player stands out of its way, at the spawn.
        other if other.starts_with("look:") => (glam::Vec3::new(0.0, 0.0, -29.0), 0.0, 0.0),
        "town-north" => (glam::Vec3::new(-11.0, 0.0, 14.0), 0.35, 30.0),
        "town-west" => (glam::Vec3::new(-34.0, 0.0, 40.0), 2.9, 30.0),
        // The city's districts, from their streets.
        "city-market" => (glam::Vec3::new(0.0, 0.0, 47.0), 0.0, 30.0),
        "city-stoop" => (
            glam::Vec3::new(-60.0, 0.0, 30.0),
            std::f32::consts::PI,
            30.0,
        ),
        "city-lantern" => (glam::Vec3::new(-62.0, 0.0, -8.0), -1.57, 30.0),
        "city-brownstone" => (glam::Vec3::new(-30.0, 0.0, -78.0), -1.57, 30.0),
        "city-observatory" => (glam::Vec3::new(64.0, 0.0, -46.0), 2.24, 40.0),
        "city-foundry" => (glam::Vec3::new(64.0, 0.0, 6.0), 1.2, 30.0),
        // Inside the gate, looking up at the goal board.
        "studio-atrium" => (glam::Vec3::new(2.8, 0.0, -13.3), 0.5, 10.0),
        // Anywhere: `at:X,Z,YAW,TILT[,Y]` stands the player at (X, Z)
        // facing YAW radians with the camera tilted by TILT; with Y, it
        // drops the player from height Y, onto a roof below it.
        other if other.starts_with("at:") => {
            let v: Vec<f32> = other[3..]
                .split(',')
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| format!("`{other}` is not at:X,Z,YAW,TILT[,Y]"))?;
            let (x, z, yaw, tilt, y) = match v[..] {
                [x, z, yaw, tilt] => (x, z, yaw, tilt, 0.0),
                [x, z, yaw, tilt, y] => (x, z, yaw, tilt, y),
                _ => return Err(format!("`{other}` is not at:X,Z,YAW,TILT[,Y]")),
            };
            (glam::Vec3::new(x, y, z), yaw, tilt)
        }
        other => {
            return Err(format!(
                "unknown view `{other}`; use approach, winds, stone, stone-crumble, stone-settled, sky, yard, hall, lane-east, lane-west, reverse, reverse-top, \
                 overhead, town-north, town-west, tooltip, city-market, city-stoop, city-lantern, \
                 city-brownstone, city-observatory, city-foundry, studio-yard, studio-hall, studio-atrium, eyes, \
                 hall-eyes, at:X,Z,YAW,TILT, or air:EX,EY,EZ,TX,TZ"
            ));
        }
    };
    runtime.settle_zone_light();
    runtime.set_spawn(at, yaw)?;
    // With VERSE_CAPTURE_ALICE set, Alice stands at her desk as the workshop
    // agent's resident seat does in a desktop window, with no host. With
    // VERSE_CAPTURE_ALICE_WALK set too, she then sets off for the console, so
    // the shot catches her mid-stride.
    if std::env::var_os("VERSE_CAPTURE_ALICE").is_some() {
        use coder_access::studio::{Activity, Role, Spend, Station};
        let agent = zones::everglade::studio::WORKSHOP_AGENT;
        let seat = |activity, station| coder_access::studio::Seat {
            seat: agent.into(),
            role: Role::Worker,
            route: "idle".into(),
            look: agent.into(),
            desk: 3,
            activity,
            station,
            task: None,
            paused: false,
            spend: Spend::default(),
        };
        runtime.set_studio_resident(vec![seat(Activity::Idle, Station::Desk)]);
        runtime.update_studio(true, 0.0);
        if std::env::var_os("VERSE_CAPTURE_ALICE_WALK").is_some() {
            let idle = InputState::default();
            for _ in 0..20 {
                runtime.update_studio(true, 0.05);
                runtime.tick(&idle, 0.05);
            }
            runtime.set_studio_resident(vec![seat(Activity::Running, Station::Workbench)]);
            runtime.update_studio(true, 0.0);
        }
        eprintln!("alice at {:?}", runtime.studio().seat_position(agent));
    }
    // Kept until the shot is rendered; the recording holds no file in it.
    let _scratch = if view.starts_with("studio-") {
        Some(studio(&mut runtime, &view, frame)?)
    } else {
        None
    };
    let idle = InputState::default();
    let workshop = match view.split_once(':') {
        Some((how @ ("alice" | "alice-door" | "alice-walk" | "alice-board"), files)) => {
            Some(alice(&mut runtime, how, files, &idle)?)
        }
        _ => None,
    };
    if view.starts_with("reverse") {
        reverse(&mut runtime, &view, &idle)?;
    }
    if view == "winds" {
        // Four walls, each from its own spot and heading, all standing.
        for (dx, turn) in [(-6.0, -0.5), (-2.0, -0.15), (2.0, 0.15), (6.0, 0.5)] {
            runtime.set_spawn(glam::Vec3::new(dx, 0.0, -29.0), turn)?;
            runtime.zone_intent(zones::Intent::WindWall)?;
            runtime.tick(&idle, 0.05);
        }
        runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -36.0), 0.0)?;
        runtime.apply(Action::Orbit { dx: 0.0, dy: 60.0 })?;
    }
    if view.starts_with("stone") {
        // Two walls from the approach, turned a little apart.
        for turn in [-0.35, 0.35] {
            runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -29.0), turn)?;
            runtime.zone_intent(zones::Intent::WallOfStone)?;
        }
        runtime.set_spawn(glam::Vec3::new(0.0, 0.0, -37.0), 0.0)?;
        runtime.apply(Action::Orbit { dx: 0.0, dy: 40.0 })?;
        // The shot itself idles half a second more below.
        let lifetime = zones::everglade::spells::STONE_LIFETIME as f32;
        let wait = match view.as_str() {
            "stone-crumble" => lifetime - 0.2,
            "stone-settled" => lifetime + 1.6,
            _ => 0.0,
        };
        for _ in 0..(wait / 0.05).round() as usize {
            runtime.tick(&idle, 0.05);
        }
    }
    runtime.apply(Action::Orbit { dx: 0.0, dy: tilt })?;
    if first_person {
        runtime.apply(Action::Zoom { lines: 100.0 })?;
    }
    // A player dropped from a height lands before the frame.
    let settle = if at.y > 0.0 { 80 } else { 10 };
    for _ in 0..settle {
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
        if view == "tooltip" {
            // A simulated hover: the pointer rests on Wind Wall's slot.
            let wind = zones::everglade::hotbar::SLOTS
                .iter()
                .position(|(intent, ..)| *intent == zones::Intent::WindWall)
                .ok_or("the hotbar has no Wind Wall")?;
            zones::everglade::hotbar::draw_tip(
                &mut ui,
                &atlas,
                [1280.0, 800.0],
                14.0,
                slots.len(),
                wind,
            );
        }
    }
    if let Some(workshop) = &workshop {
        // Her desk panel, anchored over the hotbar as the app draws it.
        let size = [1280.0, 800.0];
        let cols = verse::hud::workshop_cols(&atlas, size, 1.0);
        let rows = workshop.rows(cols, verse::hud::WORKSHOP_ROWS);
        let _ = verse::hud::workshop_panel(&mut ui, &atlas, size, 1.0, 60.0, &rows);
    }
    if let Some(summary) = runtime
        .studio()
        .view()
        .and_then(zones::everglade::signals::Summary::of)
    {
        let _ = verse::hud::studio_strip(&mut ui, &atlas, [1280.0, 800.0], 1.0, &summary, false);
    }
    let mut shot = runtime.view(1.6);
    let mut dynamic = runtime.dynamic_mesh();
    let mut air = zones::atmosphere(runtime.zone);
    if let Some((eye, target)) = aerial(&view)? {
        // A camera high over the approach, looking down across the city,
        // or over any point (`air:`), with the haze pushed back past the
        // tree ring.
        shot.eye = eye;
        shot.view_proj = glam::Mat4::perspective_rh(0.9, 1.6, 0.5, 2000.0)
            * glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
        air.fog_start = 400.0;
        air.fog_end = 900.0;
        air.height_fog = None;
        if let Some(neon) = dynamic.neon.as_mut() {
            neon.fog_start = air.fog_start;
            neon.fog_end = air.fog_end;
            neon.height_fog = None;
        }
    }
    verse::render::capture_with_atmosphere(
        &output,
        1280,
        800,
        &runtime.world.mesh,
        shot,
        &dynamic,
        &ui,
        &atlas,
        air,
    )
}

/// The aerial camera's eye and target: `overhead`'s, or
/// `air:EX,EY,EZ,TX,TZ`'s eye at (EX, EY, EZ) looking at (TX, 0, TZ).
fn aerial(view: &str) -> Result<Option<(glam::Vec3, glam::Vec3)>, String> {
    if view == "overhead" {
        return Ok(Some((
            glam::Vec3::new(0.0, 210.0, -230.0),
            glam::Vec3::new(0.0, 0.0, 0.0),
        )));
    }
    if let Some(rest) = view.strip_prefix("look:") {
        let v: Vec<f32> = rest
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| format!("`{view}` is not look:EX,EY,EZ,TX,TY,TZ"))?;
        let [ex, ey, ez, tx, ty, tz] = v[..] else {
            return Err(format!("`{view}` is not look:EX,EY,EZ,TX,TY,TZ"));
        };
        return Ok(Some((
            glam::Vec3::new(ex, ey, ez),
            glam::Vec3::new(tx, ty, tz),
        )));
    }
    let Some(rest) = view.strip_prefix("air:") else {
        return Ok(None);
    };
    let v: Vec<f32> = rest
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| format!("`{view}` is not air:EX,EY,EZ,TX,TZ"))?;
    let [ex, ey, ez, tx, tz] = v[..] else {
        return Err(format!("`{view}` is not air:EX,EY,EZ,TX,TZ"));
    };
    Ok(Some((
        glam::Vec3::new(ex, ey, ez),
        glam::Vec3::new(tx, 0.0, tz),
    )))
}

/// Casts Reverse Gravity and lets its particles climb. For `reverse` the
/// caster then walks out of the cylinder and turns back toward it; for
/// `reverse-top` the caster stays, hovering at the top, and looks down.
fn reverse(runtime: &mut WorldRuntime, view: &str, idle: &InputState) -> Result<(), String> {
    runtime.zone_intent(zones::Intent::ReverseGravity)?;
    for _ in 0..240 {
        runtime.tick(idle, 0.05);
    }
    if view == "reverse" {
        runtime.set_spawn(glam::Vec3::new(-10.0, 0.0, REVERSE_Z - 24.0), 0.25)?;
        runtime.apply(Action::Orbit { dx: 0.0, dy: -60.0 })?;
    } else {
        runtime.apply(Action::Orbit { dx: 0.0, dy: 260.0 })?;
    }
    Ok(())
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

/// Alice as a host's `studio.agent.list` answer shows her (the JSON
/// `openagents --json agent list` prints, or one agent's view), in the
/// owner's house:
///
/// - `alice:FILE`: across her workstation, facing her, as `--workshop-ask`
///   stands the player.
/// - `alice-door:FILE`: from just inside the front door, looking down the
///   great room at her.
/// - `alice-board:FILE`: in the great room, facing her day plan's board on
///   the west wall, from a view whose `plan` the board shows.
/// - `alice-walk:FROM,TO`: from the door, a moment after the answer in TO
///   sends her from where FROM put her, so she is on her way.
fn alice(
    runtime: &mut WorldRuntime,
    how: &str,
    files: &str,
    idle: &InputState,
) -> Result<verse::workshop::Workshop, String> {
    use zones::everglade::layout::estate::{AliceSpot, OWNERS_HOUSE};
    let read = |file: &str| -> Result<verse::workshop::Workshop, String> {
        use coder_access::agent::AgentView;
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
        let view: AgentView = match value.get("agents") {
            Some(agents) => serde_json::from_value::<Vec<AgentView>>(agents.clone())
                .map_err(|e| format!("{file}: {e}"))?
                .into_iter()
                .find(|a| a.name == verse::workshop::NAME)
                .ok_or("the answer has no alice")?,
            None => serde_json::from_value(value).map_err(|e| format!("{file}: {e}"))?,
        };
        Ok(verse::workshop::Workshop::showing(view))
    };
    let (first, then) = match files.split_once(',') {
        Some((from, to)) => (from, Some(to)),
        None => (files, None),
    };
    let mut workshop = read(first)?;
    runtime.set_studio_resident(workshop.seats());
    runtime.set_studio_plan(workshop.plan().cloned());
    runtime.update_studio(true, 0.0);
    let floor = zones::everglade::layout::estate::floor();
    match how {
        "alice" => {
            let at = runtime
                .studio()
                .seat_position(verse::workshop::NAME)
                .ok_or("Alice has no seat")?;
            let (_, facing) = AliceSpot::Desk.world();
            let toward = glam::Vec3::new(facing.sin(), 0.0, facing.cos());
            let stand = at + toward * verse::workshop::WALK_UP;
            runtime.set_spawn(
                glam::Vec3::new(stand.x, floor, stand.z),
                (-toward.x).atan2(-toward.z),
            )?;
        }
        "alice-board" => {
            let ([bx, bz], _, _) = zones::everglade::layout::estate::PLAN_BOARD;
            let [x, z] = OWNERS_HOUSE.world([bx + 6.4, bz + 1.2]);
            let [tx, tz] = OWNERS_HOUSE.world([bx, bz]);
            runtime.set_spawn(glam::Vec3::new(x, floor, z), (tx - x).atan2(tz - z))?;
        }
        _ => {
            let [x, z] = OWNERS_HOUSE.world([0.0, -12.6]);
            let [tx, tz] = OWNERS_HOUSE.world([0.0, -20.0]);
            runtime.set_spawn(glam::Vec3::new(x, floor, z), (tx - x).atan2(tz - z))?;
        }
    }
    for _ in 0..20 {
        runtime.update_studio(true, 0.05);
        runtime.tick(idle, 0.05);
    }
    if let Some(to) = then {
        workshop = read(to)?;
        runtime.set_studio_resident(workshop.seats());
        // A second and a half into her walk.
        for _ in 0..30 {
            runtime.update_studio(true, 0.05);
            runtime.tick(idle, 0.05);
        }
    }
    Ok(workshop)
}
