//! `openagents-desktop`: OpenAgents for Mac, Linux, and Windows.
//!
//! With no option it opens the window and, off the UI thread, starts Coder:
//! it registers the login agent that runs `coder host serve`, upgrading an
//! earlier setup silently first when this computer has one
//! ([`openagents_desktop::migrate`]). The window talks to the host only
//! over the local control socket.
//!
//! On Linux and Windows the same window runs; [`platform`] holds what
//! differs (the systemd user unit or the `Run` entry, the lock check, the
//! clipboard). On Windows `--start-host`, the `Run` entry's command, starts
//! the host with no console window and exits.

// A GUI program on Windows: no console window behind the app.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod appmenu;
mod benchmark;
#[cfg(test)]
mod chat_test_host;
#[cfg(all(test, not(windows)))]
mod grid_fixtures;
#[cfg(not(any(target_os = "linux", windows)))]
mod mac;
#[cfg(target_os = "macos")]
mod mac_notify;
mod menubar;
mod native;
mod platform;
mod shell;
mod sound;
mod strip;
mod updates;
#[cfg(windows)]
mod win_notify;
mod worker;

use openagents_desktop::control::{HostControl, SocketControl};
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Intent, Model, Screen};
use rust_native_desktop::App;
use shell::DesktopApp;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use worker::Context;

const USAGE: &str = "\
openagents-desktop: OpenAgents for Mac, Linux, and Windows

Usage: openagents-desktop [options]

  --fake-host          show the screens against an in-process host
  --fake-scan SECONDS  with --fake-host, a phone scans the code after SECONDS
  --fake-phones N      with --fake-host, N phones are already connected, and
                       the window opens on Phones and computers
  --no-login-agent     don't register the login agent that runs Coder
  --verse-relay URL    watch the Grid on the Verse page from URL (default
                       $OPENAGENTS_VERSE_RELAY, else wss://relay.openagents.com)
  --no-backdrop        no world on the Verse page
  --chat-benchmark DIR  measure an offline 3,300-row / 500-chat native fixture
  --benchmark-minimum   use the minimum window in the benchmark
  --benchmark-scale N   render the benchmark at 1x or 2x
  --capture DIR        paint pairing and shell screens, against the in-process host, to
                       PNG files in DIR
  --check-update       say whether a newer release is published (Linux, Windows)
  --update             install a newer release now: an AppImage replaces itself,
                       the Windows MSI installs after exit (Linux, Windows)
  --notify-test        show a test notification and say how it was delivered
  --open-deck ID       open the deck filed under ID in the slide viewer at launch,
                       for testing (for example three-devdays-later)
  --acceptance DIR     run the release acceptance gate's scenarios against the
                       host on this HOME's control socket, writing results and
                       evidence to DIR (scripts/release/acceptance.sh runs it in
                       a scratch HOME; docs/release/acceptance.md)
  --only NAMES         with --acceptance, run only these comma-separated scenarios
  --help               this text";

#[derive(Debug, Default)]
struct Options {
    fake_host: bool,
    fake_scan: Option<Duration>,
    fake_phones: usize,
    no_login_agent: bool,
    capture: Option<PathBuf>,
    verse_relay: Option<String>,
    no_backdrop: bool,
    help: bool,
    chat_benchmark: Option<PathBuf>,
    benchmark_minimum: bool,
    benchmark_scale: Option<f32>,
    check_update: bool,
    update: bool,
    notify_test: bool,
    open_deck: Option<String>,
    acceptance: Option<PathBuf>,
    only: Option<String>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--chat-benchmark" => {
                options.chat_benchmark = Some(PathBuf::from(
                    args.next()
                        .ok_or("--chat-benchmark takes an output directory")?,
                ))
            }
            "--benchmark-minimum" => options.benchmark_minimum = true,
            "--benchmark-scale" => {
                options.benchmark_scale = Some(
                    args.next()
                        .and_then(|value| value.parse::<f32>().ok())
                        .filter(|value| *value == 1.0 || *value == 2.0)
                        .ok_or("--benchmark-scale takes 1 or 2")?,
                )
            }
            "--fake-host" => options.fake_host = true,
            "--fake-scan" => {
                let seconds: u64 = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("--fake-scan takes a number of seconds")?;
                options.fake_scan = Some(Duration::from_secs(seconds));
            }
            "--fake-phones" => {
                options.fake_phones = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|count| *count <= 32)
                    .ok_or("--fake-phones takes a number of phones, at most 32")?;
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
            "--check-update" => options.check_update = true,
            "--update" => options.update = true,
            "--notify-test" => options.notify_test = true,
            "--open-deck" => {
                options.open_deck = Some(args.next().ok_or("--open-deck takes a deck id")?)
            }
            "--acceptance" => {
                options.acceptance = Some(PathBuf::from(
                    args.next().ok_or("--acceptance takes a directory")?,
                ))
            }
            "--only" => options.only = Some(args.next().ok_or("--only takes scenario names")?),
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
    // Windows sets no `HOME`, and the Coder runner this window runs in
    // process (`coder::task::local`) keeps its state and settings under
    // it, as `coder.exe` does: take the profile folder before any thread
    // starts, the same fallback `coder.exe`'s `main` makes.
    #[cfg(windows)]
    if std::env::var_os("HOME").is_none()
        && let Some(profile) = std::env::var_os("USERPROFILE")
    {
        // SAFETY: no other thread exists yet.
        unsafe { std::env::set_var("HOME", profile) };
    }
    // The app the Start menu shortcut names, so toasts show as its own.
    #[cfg(windows)]
    win_notify::claim_app_id();
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Who pays for this window's model calls (BYOK, #10176): the settings'
    // models.payer with the person's stored keys.
    #[cfg(feature = "app")]
    model_access::install(coder::task::settings::access());
    // Delegates' heavy `cargo` commands take build leases (#10756).
    #[cfg(feature = "app")]
    coder::task::targets::enable_lease_shims();
    reduce_motion();
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
    if options.notify_test {
        return match platform::notify_now(&openagents_desktop::notices::Notice {
            id: "openagents-test".into(),
            title: "OpenAgents".into(),
            body: "Notifications from OpenAgents show here.".into(),
            urgent: false,
        }) {
            Some(how) => {
                println!("notification delivered through {how}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("no notification service answered on this desktop");
                ExitCode::FAILURE
            }
        };
    }
    if options.check_update || options.update {
        return if updates::command(options.update) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    if let Some(directory) = &options.chat_benchmark {
        return match benchmark::run(
            directory,
            options.benchmark_minimum,
            options.benchmark_scale.unwrap_or(2.0),
            !options.no_backdrop,
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    #[cfg(not(windows))]
    if let Some(directory) = &options.acceptance {
        return match shell::acceptance::run(directory, options.only.as_deref()) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(1),
            Err(complaint) => {
                eprintln!("{complaint}");
                ExitCode::from(2)
            }
        };
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
        // Phones that paired by scanning, so they have no names.
        for _ in 0..options.fake_phones {
            let paired = fake
                .clone()
                .invite()
                .and_then(|invite| fake.redeem(&invite.invitation, ""));
            if let Err(error) = paired {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            options.fake_scan,
            None,
            home(),
        );
        (Model::new(now, Screen::Connect, Agent::Enabled), context)
    } else {
        let coder = platform::coder_path();
        // The worker starts Coder, upgrading an earlier setup first.
        let agent = if options.no_login_agent {
            Agent::NotRegistered
        } else {
            Agent::Starting
        };
        let control: Box<dyn HostControl> = match platform::control_path() {
            Some(path) => Box::new(SocketControl::new(path)),
            None => Box::new(SocketControl::new(PathBuf::from("/nonexistent"))),
        };
        (
            Model::new(now, Screen::Connect, agent),
            Context::new(control, None, None, coder, home()),
        )
    };
    // The chat's Gym keeps its trainer and runs for the real window only
    // (#10060); the fake host touches nothing the real app manages.
    #[cfg(not(windows))]
    if !options.fake_host {
        openagents_desktop::chat_gym::configure(home());
    }
    let mut app = DesktopApp::window(model, context);
    // Settings' choices, kept beside Coder's (#10021).
    app.use_settings_file(coder::task::settings::path());
    // The theme's motion tokens follow the Settings switch (#10021).
    rust_native_desktop::theme::motion::follow(app.reduce_motion());
    #[cfg(not(windows))]
    let backdrop = backdrop(&options, &mut app);
    #[cfg(windows)]
    let backdrop = backdrop(&options);
    if options.fake_host && options.fake_phones > 0 {
        app.activate(
            Intent::Navigate {
                action: openagents_desktop::chrome::Action::Computers,
            },
            now,
        );
    }
    if let Some(deck) = &options.open_deck
        && let Err(unknown) = app.open_presentation(deck, now)
    {
        eprintln!("{unknown}");
        return ExitCode::FAILURE;
    }
    // Fill the usable display while preserving logical-point component sizes.
    let window = rust_native_desktop::window::Options {
        fill: Some(WINDOW_FILL),
        size: (1200.0, 840.0),
        min_size: (760.0, 540.0),
        ..rust_native_desktop::window::Options::default()
    };
    let result = match backdrop {
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

/// The system's "Reduce motion" setting, read again each call and shared
/// with the theme's motion tokens (`theme::motion`), which also follow the
/// person's switch in Settings. Decoration such as the Grid's camera stops
/// while either holds.
fn reduce_motion() -> bool {
    let system = platform::reduce_motion();
    rust_native_desktop::theme::motion::set_system(system);
    system
}

/// The Verse page's layer: the Grid, watched on the chosen relay while
/// that page shows, unless the person asked for no world. Other pages have
/// a plain background. Windows has none.
#[cfg(not(windows))]
fn backdrop(
    options: &Options,
    app: &mut DesktopApp,
) -> Option<Box<dyn rust_native_desktop::backdrop::Backdrop>> {
    let relay = options
        .verse_relay
        .clone()
        .or_else(|| std::env::var("OPENAGENTS_VERSE_RELAY").ok())
        .filter(|relay| !relay.is_empty())
        .unwrap_or_else(|| verse::session::PUBLIC_RELAY.to_owned());
    let grid = openagents_desktop::grid::Grid::new(relay.clone(), home(), options.fake_host);
    app.set_grid(grid.clone());
    // The world loads only while the Verse page shows (#10071).
    let preference = app.reduce_motion();
    let watcher = (!options.no_backdrop).then(|| -> openagents_desktop::grid::Watcher {
        Box::new(move || {
            openagents_desktop::backdrop::GridBackdrop::new(&relay, Box::new(reduce_motion))
                .follow(preference.clone())
        })
    });
    Some(Box::new(openagents_desktop::grid::Layer::new(
        grid, watcher,
    )))
}

#[cfg(windows)]
fn backdrop(_: &Options) -> Option<Box<dyn rust_native_desktop::backdrop::Backdrop>> {
    None
}

/// Walks the screens against the in-process host and paints each to a PNG
/// in `directory`. Pairing screens use 2× scale; shell fixtures use desktop
/// dimensions. Returns how many it wrote.
fn capture(directory: &PathBuf) -> Result<usize, String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let fake = FakeHost::new("Studio Mac", shell::unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake.clone()),
        None,
        None,
        directory.join("fixture-home"),
    );
    let start = Instant::now();
    let mut app = DesktopApp::inline(Model::new(start, Screen::Connect, Agent::Enabled), context);
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
    Ok(count + capture_shell(directory)?)
}

fn capture_shell(directory: &std::path::Path) -> Result<usize, String> {
    use openagents_desktop::chrome::Action;
    let fake = FakeHost::new("Studio Mac", shell::unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake),
        None,
        None,
        directory.join("fixture-home"),
    );
    let now = Instant::now();
    let mut app = DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
    app.tick(now);
    let mut write = |name: &str, width: f32, height: f32| -> Result<(), String> {
        let (frame, scene) = rust_native_desktop::capture(&mut app, width, height, 1.0);
        if !scene.unsupported.is_empty() {
            return Err(format!(
                "unsupported shell elements: {:?}",
                scene.unsupported
            ));
        }
        std::fs::write(directory.join(format!("{name}.png")), frame.png()?)
            .map_err(|error| error.to_string())
    };
    write("shell-welcome", 1200.0, 840.0)?;
    app.click(
        Intent::Navigate {
            action: Action::SelectChat { id: 3 },
        },
        now,
    );
    let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    std::fs::write(directory.join("shell-selected-chat.png"), frame.png()?)
        .map_err(|error| error.to_string())?;
    app.click(
        Intent::Navigate {
            action: Action::ToggleSidebar,
        },
        now,
    );
    let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    std::fs::write(directory.join("shell-collapsed.png"), frame.png()?)
        .map_err(|error| error.to_string())?;
    app.click(
        Intent::Navigate {
            action: Action::ToggleSidebar,
        },
        now,
    );
    app.resize_leading_pane(400.0, now);
    let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    std::fs::write(directory.join("shell-wide-sidebar.png"), frame.png()?)
        .map_err(|error| error.to_string())?;
    let (frame, _) = rust_native_desktop::capture(&mut app, 760.0, 540.0, 1.0);
    std::fs::write(directory.join("shell-minimum.png"), frame.png()?)
        .map_err(|error| error.to_string())?;
    app.click(
        Intent::Navigate {
            action: Action::Settings,
        },
        now,
    );
    let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
    std::fs::write(directory.join("shell-settings.png"), frame.png()?)
        .map_err(|error| error.to_string())?;
    Ok(6 + capture_map(directory)?)
}

/// The Map page (#10085): default and minimum sizes at 1x and 2x, zoomed
/// out and in, the inspector open on Coder, and the Gaps panel filtered.
fn capture_map(directory: &std::path::Path) -> Result<usize, String> {
    use openagents_desktop::chrome::Action;
    use openagents_desktop::route_map::{Action as Map, Panel};
    let fake = FakeHost::new("Studio Mac", shell::unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake),
        None,
        None,
        directory.join("fixture-home"),
    );
    let now = Instant::now();
    let mut app = DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
    app.tick(now);
    app.click(
        Intent::Navigate {
            action: Action::Map,
        },
        now,
    );
    let mut count = 0;
    let mut write = |app: &mut DesktopApp, name: &str, width: f32, height: f32, scale: f32| {
        // Lay out once so the surface knows its size, then capture.
        let _ = rust_native_desktop::capture(app, width, height, scale);
        let (frame, scene) = rust_native_desktop::capture(app, width, height, scale);
        if !scene.unsupported.is_empty() {
            return Err(format!("unsupported map elements: {:?}", scene.unsupported));
        }
        std::fs::write(directory.join(format!("{name}.png")), frame.png()?)
            .map_err(|error| error.to_string())?;
        count += 1;
        Ok::<(), String>(())
    };
    write(&mut app, "map-default", 1200.0, 840.0, 1.0)?;
    write(&mut app, "map-default-2x", 1200.0, 840.0, 2.0)?;
    write(&mut app, "map-minimum", 760.0, 540.0, 1.0)?;
    write(&mut app, "map-minimum-2x", 760.0, 540.0, 2.0)?;
    let page = |app: &mut DesktopApp, action: Map| {
        app.click(Intent::Map { action }, now);
    };
    let node = |app: &mut DesktopApp, id: &str| -> Result<usize, String> {
        app.map_view()
            .and_then(|map| map.find(id))
            .ok_or_else(|| format!("no {id} on the map"))
    };
    let dispatch = node(&mut app, "route:work.dispatch")?;
    page(&mut app, Map::Select { node: dispatch });
    app.settle_map();
    write(&mut app, "map-zoomed-in-work-dispatch", 1200.0, 840.0, 1.0)?;
    let coder = node(&mut app, "coder")?;
    page(&mut app, Map::Select { node: coder });
    app.settle_map();
    write(&mut app, "map-inspector-coder", 1200.0, 840.0, 1.0)?;
    write(&mut app, "map-inspector-coder-2x", 1200.0, 840.0, 2.0)?;
    page(&mut app, Map::Fit);
    app.settle_map();
    page(&mut app, Map::GapsOnly);
    page(&mut app, Map::Panel { panel: Panel::Gaps });
    write(&mut app, "map-gaps-filtered", 1200.0, 840.0, 1.0)?;
    page(&mut app, Map::GapsOnly);
    page(&mut app, Map::ZoomOut);
    page(&mut app, Map::ZoomOut);
    app.settle_map();
    write(&mut app, "map-zoomed-out", 1200.0, 840.0, 1.0)?;
    page(
        &mut app,
        Map::Panel {
            panel: Panel::Outline,
        },
    );
    write(&mut app, "map-outline", 1200.0, 840.0, 1.0)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dark only, and reduced motion never restyles (#10022): the shell
    /// paints a dark field with light text, and the same pixels whether
    /// the system asks for reduced motion, the person does, or neither.
    /// No system appearance is an input to the paint, so a light desktop
    /// cannot change them either.
    #[test]
    fn the_shell_stays_dark_and_the_same_under_every_motion_setting() {
        use rust_native_desktop::theme::motion;
        use std::sync::atomic::Ordering;
        let home = tempfile::tempdir().unwrap();
        let fake = FakeHost::new("Studio Mac", shell::unix_now());
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            home.path().join("fixture-home"),
        );
        let now = Instant::now();
        let mut app =
            DesktopApp::inline_shell(Model::new(now, Screen::Home, Agent::Enabled), context);
        app.tick(now);
        // The Settings switch (#10021) the theme's motion tokens follow.
        let switch = app.reduce_motion();
        motion::follow(switch.clone());
        let mut frames = Vec::new();
        for (system, person) in [(false, false), (true, false), (false, true), (true, true)] {
            motion::set_system(system);
            switch.store(person, Ordering::Relaxed);
            assert_eq!(app.theme().motion.reduced, system || person);
            let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
            frames.push(frame.png().unwrap());
        }
        motion::set_system(false);
        switch.store(false, Ordering::Relaxed);
        assert!(frames.windows(2).all(|pair| pair[0] == pair[1]));
        let theme = app.theme();
        let luma = |c: rust_native::style::Color| {
            (u32::from(c.red) * 2126 + u32::from(c.green) * 7152 + u32::from(c.blue) * 722) / 10_000
        };
        assert!(luma(theme.background) < 32);
        assert!(luma(theme.text) > 200);
        assert_eq!(
            theme.appearance,
            rust_native_desktop::theme::Appearance::Dark
        );
    }

    #[test]
    fn the_options_parse() {
        let options = parse(
            [
                "--fake-host",
                "--fake-scan",
                "5",
                "--fake-phones",
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
        assert_eq!(options.fake_phones, 5);
        assert!(parse(["--fake-phones".to_string(), "33".to_string()].into_iter()).is_err());
        assert!(parse(["--bogus".to_string()].into_iter()).is_err());
        let deck = parse(
            ["--open-deck", "three-devdays-later"]
                .into_iter()
                .map(String::from),
        )
        .expect("parses");
        assert_eq!(deck.open_deck.as_deref(), Some("three-devdays-later"));
        assert!(parse(["--open-deck".to_string()].into_iter()).is_err());
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
        let fixture_home = tempfile::tempdir().expect("a fixture home");
        let context = Context::new(
            Box::new(fake.clone()),
            Some(fake),
            None,
            None,
            fixture_home.path().to_path_buf(),
        );
        let start = Instant::now();
        let mut app =
            DesktopApp::inline(Model::new(start, Screen::Connect, Agent::Enabled), context);
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
        // Six pairing screens, six shell pages, and ten of the Map page.
        assert_eq!(count, 22);
    }
}
