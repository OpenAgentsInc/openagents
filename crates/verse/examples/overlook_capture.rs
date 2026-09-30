//! The desktop app's backdrop, the Grid from above, rendered to a PNG
//! without a window, at the size the backdrop draws for the default
//! window (half its pixels).
//! Usage: overlook_capture OUTPUT.png [ORBIT_SECONDS] [RELAY_URL WATCH_SECONDS]
//!
//! `OVERLOOK_SIZE=WIDTHxHEIGHT` draws another size (the website's Grid
//! image is 1600x900).
//!
//! ORBIT_SECONDS (default 0) is how far into its slow turn the camera is.
//! With a relay, it watches the Grid there for WATCH_SECONDS first, as a
//! spectator that publishes nothing, and draws who is there.
use std::path::PathBuf;
use std::time::{Duration, Instant};
use verse::spectator::Overlook;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let orbit: f32 = args.next().map_or(Ok(0.0), |s| {
        s.parse().map_err(|_| format!("{s} is not a number"))
    })?;
    let mut overlook = match args.next() {
        Some(relay) => {
            let watch: u64 = args.next().map_or(Ok(10), |s| {
                s.parse().map_err(|_| format!("{s} is not a number"))
            })?;
            let mut overlook = Overlook::new(&relay);
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(watch) {
                overlook.tick(Instant::now());
                std::thread::sleep(Duration::from_millis(33));
            }
            eprintln!(
                "{:?}; {} players",
                overlook.status(),
                overlook.players(Instant::now()).len()
            );
            overlook
        }
        None => Overlook::offline(),
    };
    let now = Instant::now();
    overlook.tick(now);
    let mesh = overlook.mesh(now, 1.0 / 30.0);
    let (width, height) = match std::env::var("OVERLOOK_SIZE") {
        Ok(size) => size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
            .ok_or_else(|| format!("OVERLOOK_SIZE={size} is not WIDTHxHEIGHT"))?,
        Err(_) => (560, 720),
    };
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &overlook.world.world.mesh,
        Overlook::view(width as f32 / height as f32, orbit),
        &mesh,
        &verse::ui::UiBatch::default(),
        &verse::ui::Atlas::new(12.0),
        overlook.atmosphere(),
    )
}
