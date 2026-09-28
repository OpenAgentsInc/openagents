//! Checks extended-range (HDR) output offscreen, as an EDR display receives it.
//!
//! For each scene it renders a linear RGBA16F frame at headroom 1.0 and at 4.0
//! and reports: the brightest value, the share of pixels above reference
//! white, and the largest change among pixels that stay below the tone
//! curve's shoulder (which must be zero: standard-range content is unchanged
//! on an HDR display). Usage: `hdr_probe`.
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones,
};

fn frame(runtime: &WorldRuntime, headroom: f32, sun: bool) -> Result<Vec<[f32; 4]>, String> {
    let atlas = verse::ui::Atlas::new(16.0);
    let mut dynamic = runtime.dynamic_mesh();
    let player = runtime.view(1.6);
    let view = match (&mut dynamic.sky, sun) {
        (Some(sky), true) => {
            // The Sun through a telephoto lens at a sunlit exposure.
            sky.camera.auto_exposure = false;
            verse::render::View {
                view_proj: glam::Mat4::perspective_rh(0.2, 1.6, 0.1, 2000.0)
                    * glam::Mat4::look_to_rh(player.eye, sky.sun_dir, glam::Vec3::Y),
                eye: player.eye,
            }
        }
        _ => player,
    };
    verse::render::capture_extended(
        640,
        400,
        &runtime.world.mesh,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        &atlas,
        zones::atmosphere(runtime.zone),
        headroom,
    )
}

fn report(name: &str, sdr: &[[f32; 4]], hdr: &[[f32; 4]], headroom: f32) -> Result<(), String> {
    let peak = |f: &[[f32; 4]]| {
        f.iter()
            .flat_map(|p| p[..3].iter().copied())
            .fold(0.0, f32::max)
    };
    let above = hdr
        .iter()
        .filter(|p| p[..3].iter().any(|&c| c > 1.001))
        .count();
    let mut change: f32 = 0.0;
    for (a, b) in sdr.iter().zip(hdr) {
        if a[..3].iter().all(|&c| c < 0.7) {
            for i in 0..3 {
                change = change.max((a[i] - b[i]).abs());
            }
        }
    }
    println!(
        "{name}: SDR peak {:.3}, HDR peak {:.3} (ceiling {headroom}), {:.2}% above white, \
         largest change below the shoulder {change:.5}",
        peak(sdr),
        peak(hdr),
        above as f32 * 100.0 / hdr.len() as f32,
    );
    if peak(sdr) > 1.001 || peak(hdr) > headroom + 0.01 || change > 1e-3 {
        return Err(format!("{name}: extended output out of bounds"));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let headroom = 4.0;
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(glam::Vec3::new(12.0, 0.0, 9.0), 0.0)?;
    let plaza_sdr = frame(&runtime, 1.0, false)?;
    let plaza_hdr = frame(&runtime, headroom, false)?;
    report("plaza", &plaza_sdr, &plaza_hdr, headroom)?;
    let plaza_peak = plaza_hdr
        .iter()
        .flat_map(|p| p[..3].iter().copied())
        .fold(0.0, f32::max);
    if plaza_peak > 1.001 {
        return Err("the plaza must keep its standard-range look".into());
    }
    runtime.zone_intent(zones::Intent::Enter)?;
    let idle = InputState::default();
    runtime.apply(Action::Orbit { dx: 90.0, dy: 0.0 })?;
    for _ in 0..4 {
        runtime.tick(&idle, 0.05);
    }
    runtime.settle_zone_light();
    let station_sdr = frame(&runtime, 1.0, false)?;
    let station_hdr = frame(&runtime, headroom, false)?;
    report("station", &station_sdr, &station_hdr, headroom)?;
    let sun_sdr = frame(&runtime, 1.0, true)?;
    let sun_hdr = frame(&runtime, headroom, true)?;
    report("sun", &sun_sdr, &sun_hdr, headroom)?;
    let sun_peak = sun_hdr
        .iter()
        .flat_map(|p| p[..3].iter().copied())
        .fold(0.0, f32::max);
    if sun_peak < 2.0 {
        return Err("the Sun should rise well above reference white in HDR".into());
    }
    Ok(())
}
