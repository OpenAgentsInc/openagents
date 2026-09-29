//! The resident host's service lifecycle, through `coder_service`: a
//! launchd agent on macOS or a systemd user unit on Linux that runs the
//! `coder-service` launcher, which starts the host, trials updates, and
//! rolls them back. Every command reads and writes the same host root
//! (`~/.openagents/host`) and prints the state the crate keeps there.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use coder_service::bundle;
use coder_service::descriptor::{HostDescriptor, UpdateState};
use coder_service::launcher::{self, CONFIG_SCHEMA, Config, Layout};
use coder_service::service::{self, Platform, SystemRunner};
use serde_json::{Value, json};

use crate::{Args, Output, out};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents service COMMAND [OPTIONS]
  install --host-key HEX [--launcher PATH] [--version SHA256]
          [--bundle-root DIR] [--state DIR]... [--label LABEL]
          [--listen ADDR] [--ready-timeout SECONDS] [--stop-grace SECONDS]
          [--snapshot-max-bytes N] [--path PATH] [--platform macos|linux]
          [--registration-dir DIR] [--linger] [--no-start] [-- HOST_ARGS...]
        Install the launcher and register it with the service manager.
  status    Show the service manager's view and the host descriptor.
  restart   Restart the service.
  uninstall Stop and unregister the service; state and bundles stay.
  update --to SHA256 [--wait SECONDS]
        Ask the launcher to trial a staged bundle. With --wait, wait for the
        trial to commit or roll back.
  descriptor
        Print the host descriptor the launcher publishes.
Every command takes --root DIR (default ~/.openagents/host). --launcher is
the coder-service binary the service runs; it defaults to coder-service in
the directory that holds this openagents binary. HOST_ARGS default to
`host serve`.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("install", Effect::LocalWrite),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("restart", Effect::LocalWrite),
    Declared::computer("uninstall", Effect::LocalWrite),
    Declared::computer("update", Effect::LocalWrite),
    Declared::computer("descriptor", Effect::ReadOnly),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("service", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let (rest, host_args) = match rest.iter().position(|word| word == "--") {
        Some(index) => (&rest[..index], Some(rest[index + 1..].to_vec())),
        None => (rest, None),
    };
    let args = match Args::parse(rest, &["linger", "no-start"]) {
        Ok(args) => args,
        Err(message) => return output.usage("service", &message, USAGE),
    };
    if let Some(extra) = args.positional().first() {
        return output.usage("service", &format!("unexpected argument `{extra}`"), USAGE);
    }
    if host_args.is_some() && command != "install" {
        return output.usage("service", "only install takes host arguments", USAGE);
    }
    let result = match command.as_str() {
        "install" => install(output, &args, host_args),
        "status" => status(output, &args),
        "restart" => restart(output, &args),
        "uninstall" => uninstall(output, &args),
        "update" => update(output, &args),
        "descriptor" => descriptor(output, &args),
        other => {
            return output.usage("service", &format!("unknown command `{other}`"), USAGE);
        }
    };
    match result {
        Ok(code) => code,
        Err(Failure::Usage(message)) => output.usage("service", &message, USAGE),
        Err(Failure::Refused(message)) => output.fail("service", &message),
    }
}

#[derive(Debug)]
enum Failure {
    Usage(String),
    Refused(String),
}

impl From<coder_service::Error> for Failure {
    fn from(error: coder_service::Error) -> Self {
        Failure::Refused(error.to_string())
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Failure::Refused(error.to_string())
    }
}

fn home() -> Result<PathBuf, Failure> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| Failure::Refused("HOME must be an absolute path".into()))
}

fn absolute(path: &str) -> Result<PathBuf, Failure> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn layout(args: &Args) -> Result<Layout, Failure> {
    let root = match args.option("root") {
        Some(root) => absolute(root)?,
        None => home()?.join(".openagents/host"),
    };
    Ok(Layout::new(root))
}

fn number(args: &Args, name: &str, fallback: u64) -> Result<u64, Failure> {
    args.number(name, fallback).map_err(Failure::Usage)
}

fn serialized(value: &impl serde::Serialize) -> Result<Value, Failure> {
    serde_json::to_value(value).map_err(|error| Failure::Refused(error.to_string()))
}

/// The `coder-service` binary the service runs: `--launcher`, or the one
/// beside the running `openagents` binary.
fn launcher_source(flag: Option<&str>) -> Result<PathBuf, Failure> {
    let path = match flag {
        Some(path) => absolute(path)?,
        None => {
            let current = std::env::current_exe()?;
            let directory = current.parent().ok_or_else(|| {
                Failure::Refused("the openagents binary has no parent directory".into())
            })?;
            let beside = directory.join("coder-service");
            if !beside.is_file() {
                return Err(Failure::Refused(format!(
                    "no coder-service binary at {}; build it with `cargo build --release -p coder-service` and pass --launcher PATH",
                    beside.display()
                )));
            }
            beside
        }
    };
    if !path.is_file() {
        return Err(Failure::Refused(format!(
            "no launcher binary at {}",
            path.display()
        )));
    }
    Ok(path)
}

fn install(output: &Output, args: &Args, host_args: Option<Vec<String>>) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let home = home()?;
    let platform = match args.option("platform") {
        Some("macos") => Platform::Macos,
        Some("linux") => Platform::Linux,
        Some(other) => return Err(Failure::Usage(format!("unknown platform `{other}`"))),
        None => Platform::current().ok_or_else(|| {
            Failure::Refused("this platform has no supported service manager".into())
        })?,
    };
    let host_key = args
        .option("host-key")
        .ok_or_else(|| {
            Failure::Usage("install needs --host-key with the host's public key".into())
        })?
        .to_owned();
    let bundle_root = match args.option("bundle-root") {
        Some(path) => absolute(path)?,
        None => home.join(".openagents/host-bundle"),
    };
    let mut state_dirs = args
        .options("state")
        .into_iter()
        .map(absolute)
        .collect::<Result<Vec<_>, _>>()?;
    if state_dirs.is_empty() {
        state_dirs.push(home.join(".openagents/tasks"));
    }
    let registration_dir = match args.option("registration-dir") {
        Some(path) => absolute(path)?,
        None => platform.default_registration_dir(&home),
    };
    let version = match args.option("version") {
        Some(version) => version.to_owned(),
        None => bundle::selected(&bundle_root)?.ok_or_else(|| {
            Failure::Refused(
                "no bundle is selected; pass --version or stage one with scripts/coder-host.py"
                    .into(),
            )
        })?,
    };
    let source = launcher_source(args.option("launcher"))?;
    coder_service::fsx::private_dir(layout.root())?;
    let installed = service::install_launcher_binary(&layout, &source)?;
    let config = Config {
        schema: CONFIG_SCHEMA.into(),
        label: args
            .option("label")
            .unwrap_or("org.openagents.coder-host")
            .to_owned(),
        platform,
        registration_dir,
        launcher: installed,
        bundle_root,
        host_args: host_args.unwrap_or_else(|| vec!["host".into(), "serve".into()]),
        state_dirs,
        listen: args
            .option("listen")
            .unwrap_or("127.0.0.1:47100")
            .to_owned(),
        host_key,
        ready_timeout_secs: number(args, "ready-timeout", 60)?,
        stop_grace_secs: number(args, "stop-grace", 10)?,
        snapshot_max_bytes: number(args, "snapshot-max-bytes", 1 << 30)?,
        search_path: args.option("path").map_or_else(
            || launcher::search_path(std::env::var("PATH").ok().as_deref()),
            str::to_owned,
        ),
    };
    launcher::initialize(&layout, &config, &version)?;
    let report = service::install(
        &layout,
        &config,
        &mut SystemRunner,
        !args.switch("no-start"),
        args.switch("linger"),
    )?;
    let mut value = serialized(&report)?;
    value["label"] = json!(config.label);
    value["launcher"] = json!(config.launcher);
    value["platform"] = serialized(&config.platform)?;
    value["root"] = json!(layout.root());
    value["version"] = json!(version);
    output.emit(&value, render_install);
    Ok(0)
}

fn render_install(value: &Value) -> String {
    format!(
        "installed {} ({}) at version {}\n{}\ndefinition {}\nregistration {}",
        text(&value["label"]),
        text(&value["platform"]),
        text(&value["version"]),
        if value["started"].as_bool().unwrap_or(false) {
            "started"
        } else {
            "not started"
        },
        text(&value["definition"]),
        text(&value["registration"]),
    )
}

fn status(output: &Output, args: &Args) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let config = Config::load(&layout)?;
    let status = service::status(&layout, &config, &mut SystemRunner)?;
    output.emit(&serialized(&status)?, render_status);
    Ok(0)
}

fn render_status(value: &Value) -> String {
    let yes = |name: &str| {
        if value[name].as_bool().unwrap_or(false) {
            "yes"
        } else {
            "no"
        }
    };
    let mut rows = vec![
        vec!["label".to_owned(), text(&value["label"])],
        vec!["platform".to_owned(), text(&value["platform"])],
        vec!["registered".to_owned(), yes("registered").to_owned()],
        vec!["loaded".to_owned(), yes("loaded").to_owned()],
        vec![
            "running".to_owned(),
            match value["pid"].as_u64() {
                Some(pid) => format!("{} (pid {pid})", yes("running")),
                None => yes("running").to_owned(),
            },
        ],
        vec!["enabled".to_owned(), yes("enabled").to_owned()],
        vec!["starts at".to_owned(), text(&value["starts_at"])],
        vec![
            "survives logout".to_owned(),
            yes("survives_logout").to_owned(),
        ],
        vec!["committed".to_owned(), text(&value["committed"])],
    ];
    if value["pending_restart"].as_bool().unwrap_or(false) {
        let reasons: Vec<String> = value["pending_reasons"]
            .as_array()
            .into_iter()
            .flatten()
            .map(text)
            .collect();
        rows.push(vec!["pending restart".to_owned(), reasons.join("; ")]);
    }
    let descriptor = &value["descriptor"];
    if descriptor.is_object() {
        rows.push(vec!["host".to_owned(), host_line(descriptor)]);
        rows.push(vec![
            "update".to_owned(),
            update_line(&descriptor["update"]),
        ]);
    } else {
        rows.push(vec!["host".to_owned(), "no descriptor yet".to_owned()]);
    }
    out::table(&rows)
}

fn restart(output: &Output, args: &Args) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let config = Config::load(&layout)?;
    service::restart(&config, &mut SystemRunner)?;
    output.emit(
        &json!({ "result": "restarted", "label": config.label }),
        |value| format!("restarted {}", text(&value["label"])),
    );
    Ok(0)
}

fn uninstall(output: &Output, args: &Args) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let config = Config::load(&layout)?;
    let report = service::uninstall(&layout, &config, &mut SystemRunner)?;
    let mut value = serialized(&report)?;
    value["label"] = json!(config.label);
    output.emit(&value, |value| {
        let kept: Vec<String> = value["preserved"]
            .as_array()
            .into_iter()
            .flatten()
            .map(text)
            .collect();
        format!(
            "uninstalled {}{}\nkept {}",
            text(&value["label"]),
            if value["stopped"].as_bool().unwrap_or(false) {
                ""
            } else {
                " (the service manager does not report it stopped)"
            },
            kept.join(", ")
        )
    });
    Ok(0)
}

/// The result of an update request: `requested` when the command does
/// not wait, otherwise the trial's outcome or `waiting` when the wait ends
/// first.
fn update_result(state: Option<UpdateState>) -> Option<&'static str> {
    match state {
        Some(UpdateState::Committed) => Some("committed"),
        Some(UpdateState::RolledBack) => Some("rolled-back"),
        _ => None,
    }
}

fn update(output: &Output, args: &Args) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let target = args
        .option("to")
        .ok_or_else(|| Failure::Usage("update needs --to SHA256".into()))?;
    let wait = match args.option("wait") {
        Some(_) => Some(number(args, "wait", 0)?),
        None => None,
    };
    let request = launcher::request_update(&layout, target)?;
    let mut value = json!({
        "result": "requested",
        "request": request.id,
        "target": request.target,
        "descriptor": null,
    });
    if let Some(wait) = wait {
        value["result"] = json!("waiting");
        let deadline = Instant::now() + Duration::from_secs(wait);
        loop {
            let descriptor: Option<HostDescriptor> =
                launcher::read_descriptor(&layout)?.filter(|descriptor| {
                    descriptor.update.request.as_deref() == Some(request.id.as_str())
                });
            if let Some(result) = update_result(descriptor.as_ref().map(|d| d.update.state)) {
                value["result"] = json!(result);
                value["descriptor"] = serialized(&descriptor)?;
                break;
            }
            if Instant::now() >= deadline {
                value["descriptor"] = serialized(&descriptor)?;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    output.emit(&value, render_update);
    Ok(match value["result"].as_str() {
        Some("requested" | "committed") => 0,
        _ => crate::EXIT_FAILURE,
    })
}

fn render_update(value: &Value) -> String {
    let target = text(&value["target"]);
    let request = text(&value["request"]);
    match value["result"].as_str().unwrap_or("") {
        "requested" => format!("requested update to {target} (request {request})"),
        "committed" => format!("committed {target} (request {request})"),
        "rolled-back" => format!(
            "rolled back from {target}: {}",
            text(&value["descriptor"]["update"]["reason"])
        ),
        _ => format!(
            "update to {target} (request {request}) has not finished; `openagents service descriptor` shows its progress"
        ),
    }
}

fn descriptor(output: &Output, args: &Args) -> Result<u8, Failure> {
    let layout = layout(args)?;
    let Some(descriptor) = launcher::read_descriptor(&layout)? else {
        return Err(Failure::Refused(format!(
            "no descriptor under {}; the launcher has not run",
            layout.root().display()
        )));
    };
    output.emit(&serialized(&descriptor)?, |value| {
        format!(
            "{}\nupdate {}",
            host_line(value),
            update_line(&value["update"])
        )
    });
    Ok(0)
}

fn host_line(descriptor: &Value) -> String {
    format!(
        "{} {} generation {} version {} on {}",
        text(&descriptor["host_key"]),
        text(&descriptor["state"]),
        text(&descriptor["host_generation"]),
        text(&descriptor["version"]),
        text(&descriptor["listen"]),
    )
}

fn update_line(update: &Value) -> String {
    let mut line = text(&update["state"]);
    if let Some(target) = update["target"].as_str() {
        line.push_str(&format!(" to {target}"));
    }
    if let Some(from) = update["from"].as_str() {
        line.push_str(&format!(" from {from}"));
    }
    if let Some(reason) = update["reason"].as_str() {
        line.push_str(&format!(": {reason}"));
    }
    line
}

fn text(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_finished_trial_ends_the_wait() {
        assert_eq!(
            update_result(Some(UpdateState::Committed)),
            Some("committed")
        );
        assert_eq!(
            update_result(Some(UpdateState::RolledBack)),
            Some("rolled-back")
        );
        for state in [UpdateState::None, UpdateState::Prepared, UpdateState::Trial] {
            assert_eq!(update_result(Some(state)), None);
        }
        assert_eq!(update_result(None), None);
    }

    #[test]
    fn human_text_projects_the_descriptor() {
        let descriptor = json!({
            "host_key": "ab", "state": "ready", "host_generation": 3,
            "version": "v2", "listen": "127.0.0.1:47100",
            "update": { "state": "rolled-back", "target": "v3", "from": "v2", "reason": "not ready" },
        });
        assert_eq!(
            host_line(&descriptor),
            "ab ready generation 3 version v2 on 127.0.0.1:47100"
        );
        assert_eq!(
            update_line(&descriptor["update"]),
            "rolled-back to v3 from v2: not ready"
        );
        let update = json!({ "result": "rolled-back", "target": "v3", "request": "r", "descriptor": descriptor });
        assert_eq!(render_update(&update), "rolled back from v3: not ready");
    }

    #[test]
    fn a_missing_launcher_refuses() {
        assert!(matches!(
            launcher_source(Some("/no/such/coder-service")),
            Err(Failure::Refused(_))
        ));
    }
}
