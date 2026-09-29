//! `coder-service`: install and run the Coder host as a background service.
//!
//! ```text
//! coder-service [--root DIR] service install --host-key HEX [options] [-- HOST_ARGS...]
//! coder-service [--root DIR] service status | restart | uninstall | render
//! coder-service [--root DIR] run
//! coder-service [--root DIR] update --to SHA256 [--wait SECONDS]
//! coder-service [--root DIR] descriptor
//! coder-service adopt detect
//! ```
//!
//! The host root defaults to `~/.openagents/host`. Read
//! `docs/coder/runtime/host-service.md` for every option.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use coder_service::descriptor::UpdateState;
use coder_service::launcher::{self, CONFIG_SCHEMA, Config, Launcher, Layout};
use coder_service::service::{self, Platform, SystemRunner};
use coder_service::{Error, Result, bundle};

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

const USAGE: &str = "usage: coder-service [--root DIR] (service (install|status|restart|uninstall|render) | run | update --to SHA256 [--wait SECONDS] | descriptor | adopt detect)";

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| Error::Refused("HOME must be an absolute path".into()))
}

fn print(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn real_main() -> Result<ExitCode> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let home = home()?;
    // `--root` comes before the command, so a host argument after `--`
    // can never be mistaken for it.
    let root = if args.first().map(String::as_str) == Some("--root") {
        match take_option(&mut args, "--root")? {
            Some(root) => absolute(&root)?,
            None => unreachable!("take_option found --root at the front"),
        }
    } else {
        home.join(".openagents/host")
    };
    let layout = Layout::new(root);
    let head: Vec<String> = args.iter().take(2).cloned().collect();
    let command: Vec<&str> = head.iter().map(String::as_str).collect();
    match command.as_slice() {
        ["service", "install", ..] => {
            let rest = args.split_off(2);
            install(&layout, &home, rest)?;
        }
        ["service", "render", ..] => {
            let config = Config::load(&layout)?;
            print!("{}", service::render(&layout, &config)?);
        }
        ["service", "status", ..] => {
            let config = Config::load(&layout)?;
            print(&service::status(&layout, &config, &mut SystemRunner)?)?;
        }
        ["service", "restart", ..] => {
            let config = Config::load(&layout)?;
            service::restart(&config, &mut SystemRunner)?;
            print(&serde_json::json!({ "result": "restarted", "label": config.label }))?;
        }
        ["service", "uninstall", ..] => {
            let config = Config::load(&layout)?;
            print(&service::uninstall(&layout, &config, &mut SystemRunner)?)?;
        }
        ["run", ..] => {
            // SAFETY: the handler only stores to an atomic, which is
            // async-signal-safe.
            unsafe {
                libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
                libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
            }
            let mut launcher = Launcher::open(layout)?;
            let code = launcher.run(&STOP)?;
            return Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)));
        }
        ["update", ..] => {
            let mut rest = args.split_off(1);
            let target = take_option(&mut rest, "--to")?
                .ok_or_else(|| Error::Refused("update needs --to SHA256".into()))?;
            let wait = take_option(&mut rest, "--wait")?
                .map(|value| parse_number(&value, "--wait"))
                .transpose()?;
            no_extra(&rest)?;
            let request = launcher::request_update(&layout, &target)?;
            let Some(wait) = wait else {
                print(
                    &serde_json::json!({ "result": "requested", "request": request.id, "target": target }),
                )?;
                return Ok(ExitCode::SUCCESS);
            };
            let deadline = Instant::now() + Duration::from_secs(wait);
            while Instant::now() < deadline {
                if let Some(descriptor) = launcher::read_descriptor(&layout)?
                    && descriptor.update.request.as_deref() == Some(request.id.as_str())
                {
                    match descriptor.update.state {
                        UpdateState::Committed => {
                            print(&descriptor)?;
                            return Ok(ExitCode::SUCCESS);
                        }
                        UpdateState::RolledBack => {
                            print(&descriptor)?;
                            return Ok(ExitCode::from(2));
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            print(&serde_json::json!({ "result": "waiting", "request": request.id }))?;
            return Ok(ExitCode::from(3));
        }
        ["adopt", "detect"] => {
            // Read-only: what the desktop app would adopt. No secret.
            let paths = coder_service::adopt::Paths::under(&home);
            let now = coder_service::fsx::now_ms() / 1000;
            let detection = coder_service::adopt::detect(&paths, now)?;
            let status = match Config::load(&Layout::new(&paths.host_root)) {
                Ok(config) => Some(service::status(
                    &Layout::new(&paths.host_root),
                    &config,
                    &mut SystemRunner,
                )?),
                Err(_) => None,
            };
            print(&serde_json::json!({ "detection": detection, "service": status }))?;
        }
        ["descriptor", ..] => match launcher::read_descriptor(&layout)? {
            Some(descriptor) => print(&descriptor)?,
            None => {
                return Err(Error::Refused(
                    "no descriptor yet; the launcher has not run".into(),
                ));
            }
        },
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn install(layout: &Layout, home: &Path, mut args: Vec<String>) -> Result<()> {
    let host_args = match args.iter().position(|arg| arg == "--") {
        Some(index) => {
            let tail = args.split_off(index);
            tail[1..].to_vec()
        }
        None => vec!["host".into(), "serve".into()],
    };
    let platform = match take_option(&mut args, "--platform")?.as_deref() {
        Some("macos") => Platform::Macos,
        Some("linux") => Platform::Linux,
        Some(other) => return Err(Error::Refused(format!("unknown platform {other}"))),
        None => Platform::current().ok_or_else(|| {
            Error::Refused("this platform has no supported service manager".into())
        })?,
    };
    let bundle_root = match take_option(&mut args, "--bundle-root")? {
        Some(path) => absolute(&path)?,
        None => home.join(".openagents/host-bundle"),
    };
    let mut state_dirs = Vec::new();
    while let Some(path) = take_option(&mut args, "--state")? {
        state_dirs.push(absolute(&path)?);
    }
    if state_dirs.is_empty() {
        state_dirs.push(home.join(".openagents/tasks"));
    }
    let host_key = take_option(&mut args, "--host-key")?.ok_or_else(|| {
        Error::Refused("install needs --host-key with the host's public key".into())
    })?;
    let label =
        take_option(&mut args, "--label")?.unwrap_or_else(|| "org.openagents.coder-host".into());
    let listen = take_option(&mut args, "--listen")?.unwrap_or_else(|| "127.0.0.1:47100".into());
    let registration_dir = match take_option(&mut args, "--registration-dir")? {
        Some(path) => absolute(&path)?,
        None => platform.default_registration_dir(home),
    };
    let ready_timeout_secs = take_option(&mut args, "--ready-timeout")?
        .map_or(Ok(60), |value| parse_number(&value, "--ready-timeout"))?;
    let stop_grace_secs = take_option(&mut args, "--stop-grace")?
        .map_or(Ok(10), |value| parse_number(&value, "--stop-grace"))?;
    let snapshot_max_bytes = take_option(&mut args, "--snapshot-max-bytes")?
        .map_or(Ok(1 << 30), |value| {
            parse_number(&value, "--snapshot-max-bytes")
        })?;
    let version = match take_option(&mut args, "--version")? {
        Some(version) => version,
        None => bundle::selected(&bundle_root)?.ok_or_else(|| {
            Error::Refused(
                "no bundle is selected; pass --version or stage one with scripts/coder-host.py"
                    .into(),
            )
        })?,
    };
    let search_path = take_option(&mut args, "--path")?
        .unwrap_or_else(|| launcher::search_path(std::env::var("PATH").ok().as_deref()));
    let linger = take_flag(&mut args, "--linger");
    let start = !take_flag(&mut args, "--no-start");
    no_extra(&args)?;

    coder_service::fsx::private_dir(layout.root())?;
    let current = std::env::current_exe()?;
    let launcher_path = service::install_launcher_binary(layout, &current)?;
    let config = Config {
        schema: CONFIG_SCHEMA.into(),
        label,
        platform,
        registration_dir,
        launcher: launcher_path,
        bundle_root,
        host_args,
        state_dirs,
        listen,
        host_key,
        ready_timeout_secs,
        stop_grace_secs,
        snapshot_max_bytes,
        search_path,
    };
    launcher::initialize(layout, &config, &version)?;
    let report = service::install(layout, &config, &mut SystemRunner, start, linger)?;
    print(&report)
}

fn take_option(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Ok(None);
    };
    if index + 1 >= args.len() {
        return Err(Error::Refused(format!("{name} needs a value")));
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Ok(Some(value))
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    match args.iter().position(|arg| arg == name) {
        Some(index) => {
            args.remove(index);
            true
        }
        None => false,
    }
}

fn no_extra(args: &[String]) -> Result<()> {
    match args.first() {
        Some(extra) => Err(Error::Refused(format!(
            "unexpected argument {extra}; {USAGE}"
        ))),
        None => Ok(()),
    }
}

fn parse_number(value: &str, name: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| Error::Refused(format!("{name} takes a whole number")))
}

fn absolute(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
