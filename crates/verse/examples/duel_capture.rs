//! Offline visual check of the Prussian duel with the shared renderer.
//! Usage: duel_capture OUTPUT.png [SECONDS]
//!
//! Enters the duel as `verse --duel` does, walks the knight in, raises his
//! shield against Skurdo's first blow, and strikes back, then renders the
//! moment at SECONDS (default 3.2) with the duel's screen: both fighters'
//! bars, the incoming-blow cue, the direction mark, and the callouts.
use std::path::PathBuf;
use verse::{controller::InputState, runtime::WorldRuntime, zones};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let seconds: f32 = args
        .next()
        .map_or(Ok(3.2), |s| s.parse())
        .map_err(|_| "SECONDS must be a number")?;
    let mut runtime = WorldRuntime::new();
    runtime.enter_duel()?;
    let idle = InputState::default();
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    let frame = 1.0 / 60.0;
    let frames = (seconds / frame).round() as usize;
    for i in 0..frames {
        let (gap, threat) = {
            let fight = runtime.duel().ok_or("The duel did not open")?.fight();
            let gap = (fight.player().pos - fight.enemy().pos).length();
            (gap, fight.enemy().threat(fight.now).is_some())
        };
        let input = if gap > 1.9 { &walk } else { &idle };
        if let Some(duel) = runtime.duel_mut() {
            duel.set_shield(threat);
            if !threat && gap < 2.0 && i % 50 == 0 {
                let _ = duel.attack(Some(zones::duel::Dir::Right));
            }
        }
        runtime.tick(input, frame);
    }
    let (width, height) = (1600u32, 1000u32);
    let aspect = width as f32 / height as f32;
    let atlas = verse::ui::Atlas::new(16.0);
    let mut ui = verse::ui::UiBatch::default();
    let view = runtime.view(aspect);
    let duel = runtime.duel().ok_or("The duel did not open")?;
    zones::duel::hud::draw(
        &mut ui,
        &atlas,
        [width as f32, height as f32],
        view.view_proj,
        duel,
    );
    eprintln!("{}", runtime.zone_snapshot(aspect).caption);
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &runtime.world.mesh,
        view,
        &runtime.dynamic_mesh(),
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
