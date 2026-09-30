//! `openagents-desktop`: OpenAgents for Mac.
//!
//! With no option it opens the window. On first launch it registers the
//! login agent that runs `coder host serve`, unless this Mac already runs
//! Coder from an earlier setup, in which case it asks whether to use that
//! setup. The window talks to the host only over the local control socket.
//!
//! On Linux and Windows the same window runs; [`platform`] holds what
//! differs (the systemd user unit or the `Run` entry, the lock check, the
//! clipboard). On Windows `--start-host`, the `Run` entry's command, starts
//! the host with no console window and exits.

// A GUI program on Windows: no console window behind the app.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(any(target_os = "linux", windows)))]
mod mac;
mod menubar;
mod platform;
mod shell;
mod worker;

use openagents_desktop::control::{HostControl, SocketControl};
use openagents_desktop::fake::FakeHost;
use openagents_desktop::migrate;
use openagents_desktop::model::{Agent, Intent, Model, Screen};
use rust_native_desktop::App;
use shell::DesktopApp;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use worker::Context;

const USAGE: &str = "\
openagents-desktop: OpenAgents for Mac

Usage: openagents-desktop [options]

  --fake-host          show the screens against an in-process host
  --fake-scan SECONDS  with --fake-host, a phone scans the code after SECONDS
  --no-login-agent     don't register the login agent that runs Coder
  --verse-relay URL    watch the Grid behind the window on URL (default
                       $OPENAGENTS_VERSE_RELAY, else wss://relay.openagents.com)
  --no-backdrop        a plain background, without the Grid
  --capture DIR        paint each screen, against the in-process host, to
                       PNG files in DIR
  --help               this text";

#[derive(Debug, Default)]
struct Options {
    fake_host: bool,
    fake_scan: Option<Duration>,
    no_login_agent: bool,
    capture: Option<PathBuf>,
    verse_relay: Option<String>,
    no_backdrop: bool,
    help: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fake-host" => options.fake_host = true,
            "--fake-scan" => {
                let seconds: u64 = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("--fake-scan takes a number of seconds")?;
                options.fake_scan = Some(Duration::from_secs(seconds));
            }
            "--no-login-agent" => options.no_login_agent = true,
            "--verse-relay" => {
                options.verse_relay = Some(args.next().ok_or("--verse-relay takes a URL")?)
            }
            "--no-backdrop" => options.no_backdrop = true,
            "--capture" => {
                options.capture = Some(PathBuf::from(
                    args.next().ok_or("--capture takes a directory")?,
                ))
            }
            "--help" | "-h" => options.help = true,
            // macOS passes a process serial number to an app opened from
            // the Finder on some versions.
            other if other.starts_with("-psn_") => {}
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    if platform::wants_start_host(&args) {
        return match platform::start_host() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let options = match parse(args.into_iter()) {
        Ok(options) => options,
        Err(complaint) => {
            eprintln!("{complaint}");
            return ExitCode::from(2);
        }
    };
    if options.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if let Some(directory) = &options.capture {
        return match capture(directory) {
            Ok(count) => {
                println!("wrote {count} files to {}", directory.display());
                ExitCode::SUCCESS
            }
            Err(complaint) => {
                eprintln!("{complaint}");
                ExitCode::FAILURE
            }
        };
    }
    let now = Instant::now();
    let (model, context) = if options.fake_host {
        let fake = FakeHost::new("Studio Mac", shell::unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            options.fake_scan,
            None,
            home(),
        );
        (
            Model::new(now, Screen::Connect, Agent::Enabled, None),
            context,
        )
    } else {
        let coder = platform::coder_path();
        // Once this app's own agent is on, the setup is already this app's
        // (adopted, or made by it), so there is nothing to ask about.
        let old = if platform::agent_enabled() {
            None
        } else {
            migrate::old_setup(coder.as_deref(), &home())
        };
        // An earlier setup still runs its own Coder; registering ours
        // beside it would start a second one on the same state.
        let agent = if old.is_some() || options.no_login_agent {
            Agent::NotRegistered
        } else {
            platform::register_agent()
        };
        let control: Box<dyn HostControl> = match platform::control_path() {
            Some(path) => Box::new(SocketControl::new(path)),
            None => Box::new(SocketControl::new(PathBuf::from("/nonexistent"))),
        };
        let screen = Model::first_screen(&old);
        (
            Model::new(now, screen, agent, old),
            Context::new(control, None, None, coder, home()),
        )
    };
    let app = DesktopApp::window(model, context);
    // Nearly the whole display, centered; the views grow with it.
    let window = rust_native_desktop::window::Options {
        fill: Some(WINDOW_FILL),
        zoom: Some(rust_native_desktop::window::Zoom {
            design: (560.0, 720.0),
            max: 1.6,
        }),
        ..rust_native_desktop::window::Options::default()
    };
    let result = match backdrop(&options) {
        Some(backdrop) => rust_native_desktop::window::run_with_backdrop(app, window, backdrop),
        None => rust_native_desktop::window::run(app, window),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(complaint) => {
            eprintln!("{complaint}");
            ExitCode::FAILURE
        }
    }
}

/// The share of the display's usable area the window opens at.
const WINDOW_FILL: f64 = 0.9;

/// The Grid behind the window, watched on the chosen relay, unless the
/// person asked for a plain background. Windows has none.
#[cfg(not(windows))]
fn backdrop(options: &Options) -> Option<Box<dyn rust_native_desktop::backdrop::Backdrop>> {
    if options.no_backdrop {
        return None;
    }
    let relay = options
        .verse_relay
        .clone()
        .or_else(|| std::env::var("OPENAGENTS_VERSE_RELAY").ok())
        .filter(|relay| !relay.is_empty())
        .unwrap_or_else(|| verse::session::PUBLIC_RELAY.to_owned());
    Some(Box::new(openagents_desktop::backdrop::GridBackdrop::new(
        &relay,
        Box::new(platform::reduce_motion),
    )))
}

#[cfg(windows)]
fn backdrop(_: &Options) -> Option<Box<dyn rust_native_desktop::backdrop::Backdrop>> {
    None
}

/// Walks the screens against the in-process host and paints each to a PNG
/// in `directory` at twice the window's default size. Returns how many it
/// wrote.
fn capture(directory: &PathBuf) -> Result<usize, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let fake = FakeHost::new("Studio Mac", shell::unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake.clone()),
        None,
        None,
        home(),
    );
    let start = Instant::now();
    let mut app = DesktopApp::inline(
        Model::new(start, Screen::Connect, Agent::Enabled, None),
        context,
    );
    let mut count = 0;
    let mut write = |app: &mut DesktopApp, name: &str| -> Result<(), String> {
        let (frame, _) = rust_native_desktop::capture(app, 560.0, 720.0, 2.0);
        let path = directory.join(format!("{name}.png"));
        std::fs::write(&path, frame.png()?)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        count += 1;
        Ok(())
    };
    app.tick(start);
    write(&mut app, "dsk-01-connect")?;
    app.click(Intent::CopyCode, start);
    write(&mut app, "dsk-01-copied")?;
    let code = app
        .model()
        .codes
        .shown()
        .ok_or("no code showed")?
        .invitation
        .clone();
    fake.redeem(&code, "Kai's iPhone")
        .map_err(|error| error.to_string())?;
    app.tick(start + Duration::from_secs(3));
    write(&mut app, "dsk-02-connected")?;
    let mut host: Box<dyn HostControl> = Box::new(fake.clone());
    host.add_project("/Users/kai/code/website")
        .map_err(|error| error.to_string())?;
    host.set_autostart(openagents_desktop::control::Autostart {
        enabled: true,
        projects: vec!["website".into()],
        max_running: 1,
    })
    .map_err(|error| error.to_string())?;
    app.tick(start + Duration::from_secs(6));
    write(&mut app, "dsk-02-project")?;
    app.click(Intent::Done, start + Duration::from_secs(7));
    write(&mut app, "dsk-03-home")?;
    let device = app
        .model()
        .phones()
        .first()
        .map(|device| device.device.clone())
        .ok_or("no phone")?;
    app.click(Intent::AskRemove { device }, start + Duration::from_secs(8));
    write(&mut app, "dsk-03-remove")?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_options_parse() {
        let options = parse(
            [
                "--fake-host",
                "--fake-scan",
                "5",
                "--no-login-agent",
                "--verse-relay",
                "ws://127.0.0.1:7447",
                "--no-backdrop",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parses");
        assert!(options.fake_host && options.no_login_agent && options.no_backdrop);
        assert_eq!(options.verse_relay.as_deref(), Some("ws://127.0.0.1:7447"));
        assert_eq!(options.fake_scan, Some(Duration::from_secs(5)));
        assert!(parse(["--bogus".to_string()].into_iter()).is_err());
    }

    /// Every QR code in an RGBA or RGB image, as a phone's scanner reads it.
    fn scan(width: usize, height: usize, channels: usize, pixels: &[u8]) -> Vec<String> {
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
            let at = (y * width + x) * channels;
            let [r, g, b] = [pixels[at], pixels[at + 1], pixels[at + 2]].map(u32::from);
            ((r * 299 + g * 587 + b * 114) / 1000) as u8
        });
        image
            .detect_grids()
            .into_iter()
            .filter_map(|grid| grid.decode().ok().map(|(_, text)| text))
            .collect()
    }

    #[test]
    fn the_code_scans_over_the_brightest_backdrop() {
        use rust_native_desktop::backdrop::{Look, composite};
        use rust_native_desktop::{Frame, Theme};
        let fake = FakeHost::new("Studio Mac", shell::unix_now());
        let context = Context::new(Box::new(fake.clone()), Some(fake), None, None, home());
        let start = Instant::now();
        let mut app = DesktopApp::inline(
            Model::new(start, Screen::Connect, Agent::Enabled, None),
            context,
        );
        app.tick(start);
        let text = app.model().codes.shown().expect("a code").text.clone();
        // The QR code carries the link, so the phone's own camera opens the
        // app; the payload is the same code.
        let text = openagents_connect::code::link(&text).expect("a connect code");
        assert!(
            text.starts_with("https://openagents.com/connect#"),
            "{text}"
        );
        let (views, _) = rust_native_desktop::capture_views(&mut app, 560.0, 720.0, 2.0);
        // Backdrops brighter and busier than the Grid ever is: all white,
        // white grid lines on black at the code's own module pitch, and
        // noise.
        let (w, h) = (560, 720);
        let white = Frame::new(w, h, rust_native::style::Color::rgb(255, 255, 255));
        let mut lines = Frame::new(w, h, rust_native::style::Color::rgb(0, 0, 0));
        let mut noise = Frame::new(w, h, rust_native::style::Color::rgb(0, 0, 0));
        let mut seed = 0x9e37_79b9_u32;
        for y in 0..h {
            for x in 0..w {
                let at = (y * w + x) * 4;
                if x % 5 == 0 || y % 5 == 0 {
                    lines.pixels[at..at + 3].copy_from_slice(&[255, 255, 255]);
                }
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let value = (seed >> 24) as u8;
                noise.pixels[at..at + 3].copy_from_slice(&[value, value, value]);
            }
        }
        for backdrop in [&white, &lines, &noise] {
            let frame = composite(
                &views,
                backdrop,
                Theme::default().background,
                Look::default().dim,
            );
            let found = scan(frame.width, frame.height, 4, &frame.pixels);
            assert_eq!(found, std::slice::from_ref(&text));
        }
    }

    #[test]
    fn the_window_screenshot_with_the_grid_behind_it_scans() {
        // The committed capture of the real window over the live Grid.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/screenshots/dsk-01-connect.png"
        );
        let bytes = std::fs::read(path).expect("the screenshot");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("a PNG");
        let mut pixels = vec![0; reader.output_buffer_size().expect("a size")];
        let info = reader.next_frame(&mut pixels).expect("a frame");
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        let channels = info.color_type.samples();
        let found = scan(info.width as usize, info.height as usize, channels, &pixels);
        assert_eq!(found.len(), 1, "{found:?}");
        // A capture from before the QR code carried the link shows the text
        // form; either names a connect code.
        assert!(
            openagents_connect::code::canonical(&found[0]).is_some(),
            "{found:?}"
        );
    }

    #[test]
    fn a_capture_walks_every_screen() {
        let directory = tempfile::tempdir().expect("a directory");
        let count = capture(&directory.path().to_path_buf()).expect("the capture");
        assert_eq!(count, 6);
    }
}
