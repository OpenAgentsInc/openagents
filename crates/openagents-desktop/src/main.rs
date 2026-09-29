//! `openagents-desktop`: OpenAgents for Mac.
//!
//! With no option it opens the window. On first launch it registers the
//! login agent that runs `coder host serve`, unless this Mac already runs
//! Coder from an earlier setup, in which case it asks whether to use that
//! setup. The window talks to the host only over the local control socket.

mod mac;
mod menubar;
mod shell;
mod worker;

use openagents_desktop::control::{HostControl, SocketControl, socket_path};
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
  --capture DIR        paint each screen, against the in-process host, to
                       PNG files in DIR
  --help               this text";

#[derive(Debug, Default)]
struct Options {
    fake_host: bool,
    fake_scan: Option<Duration>,
    no_login_agent: bool,
    capture: Option<PathBuf>,
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
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        let coder = mac::coder_path();
        // Once this app's own agent is on, the setup is already this app's
        // (adopted, or made by it), so there is nothing to ask about.
        let old = if mac::agent_enabled() {
            None
        } else {
            migrate::old_setup(coder.as_deref(), &home())
        };
        // An earlier setup still runs its own Coder; registering ours
        // beside it would start a second one on the same state.
        let agent = if old.is_some() || options.no_login_agent {
            Agent::NotRegistered
        } else {
            mac::register_agent()
        };
        let control: Box<dyn HostControl> = match socket_path() {
            Some(path) => Box::new(SocketControl::new(path)),
            None => Box::new(SocketControl::new(PathBuf::from("/nonexistent"))),
        };
        let screen = Model::first_screen(&old);
        (
            Model::new(now, screen, agent, old),
            Context::new(control, None, None, coder, home()),
        )
    };
    match rust_native_desktop::window::run(
        DesktopApp::window(model, context),
        rust_native_desktop::window::Options::default(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(complaint) => {
            eprintln!("{complaint}");
            ExitCode::FAILURE
        }
    }
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
    app.click(Intent::ToggleTerminal, start);
    app.click(Intent::CopyCode, start);
    write(&mut app, "dsk-01-terminal-copied")?;
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
            ["--fake-host", "--fake-scan", "5", "--no-login-agent"]
                .into_iter()
                .map(String::from),
        )
        .expect("parses");
        assert!(options.fake_host && options.no_login_agent);
        assert_eq!(options.fake_scan, Some(Duration::from_secs(5)));
        assert!(parse(["--bogus".to_string()].into_iter()).is_err());
    }

    #[test]
    fn a_capture_walks_every_screen() {
        let directory = tempfile::tempdir().expect("a directory");
        let count = capture(&directory.path().to_path_buf()).expect("the capture");
        assert_eq!(count, 6);
    }
}
