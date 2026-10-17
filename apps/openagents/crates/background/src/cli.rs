//! `openagents background ensure|status|stop`, hidden: the same steps the app
//! takes at launch, for a computer without a window and for checks.

use std::path::{Path, PathBuf};

use crate::{
    EngineMode, Ensure, Host, MODE_ENV, Outcome, START_WAIT, ServiceSpec, SystemManager,
    captured_env, engine_mode, ensure, is_source_build, port_listening, read_preference,
};

/// The subcommand that runs [`main`].
pub const SUBCOMMAND: &str = "background";

/// This app's service for `data_dir`, from the environment.
pub fn spec_from_env(data_dir: &Path, ipc_port: u16) -> Result<ServiceSpec, String> {
    let host = Host::from_env().ok_or("This computer has no background service manager.")?;
    let exe =
        std::env::current_exe().map_err(|error| format!("couldn't find this program: {error}"))?;
    Ok(ServiceSpec {
        host,
        exe,
        data_dir: data_dir.to_path_buf(),
        ipc_port,
        env: captured_env(|key| openagents_brand::env_var(key).ok()),
    })
}

/// The engine mode for this launch, with the reason.
pub fn mode_from_env(data_dir: &Path) -> (EngineMode, &'static str) {
    let exe = std::env::current_exe().unwrap_or_default();
    engine_mode(
        crate::Platform::current(),
        openagents_brand::env_var(MODE_ENV).ok().as_deref(),
        read_preference(data_dir),
        is_source_build(&exe),
        openagents_brand::env_var("OPENAGENTS_EDGE_TOKEN").is_ok(),
    )
}

/// Run the subcommand. `locked` says whether another process holds the data
/// folder's engine lock.
pub fn main(args: impl Iterator<Item = std::ffi::OsString>, locked: impl Fn(&Path) -> bool) -> i32 {
    let args: Vec<String> = args.map(|arg| arg.to_string_lossy().into_owned()).collect();
    let data_dir = openagents_brand::env_var_os("OPENAGENTS_DATA_DIR").map(PathBuf::from);
    let ipc_port = openagents_brand::env_var("OPENAGENTS_IPC_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(openagents_brand::DEFAULT_IPC_PORT);
    let Some(data_dir) = data_dir else {
        eprintln!("No data folder is set.");
        return 2;
    };
    let spec = match spec_from_env(&data_dir, ipc_port) {
        Ok(spec) => spec,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    match args.first().map(String::as_str) {
        Some("ensure") => {
            let outcome = ensure(Ensure {
                spec: &spec,
                // Asking for the service is the explicit choice.
                mode: EngineMode::Background,
                data_dir_locked: locked(&data_dir),
                manager: &SystemManager,
                listening: &|| port_listening(ipc_port),
                wait: START_WAIT,
            });
            match outcome {
                Outcome::Running { started, installed } => {
                    println!(
                        "The background engine is running on port {ipc_port}{}.",
                        match (installed, started) {
                            (true, _) => " (installed and started)",
                            (false, true) => " (started)",
                            (false, false) => "",
                        }
                    );
                    0
                }
                Outcome::Embed(reason) | Outcome::Failed(reason) => {
                    eprintln!("The background engine isn't running: {reason}.");
                    1
                }
            }
        }
        Some("status") => {
            let (mode, reason) = mode_from_env(&data_dir);
            println!("Data folder: {}", data_dir.display());
            println!("Mode:        {} ({reason})", mode.as_str());
            println!("Service:     {}", spec.file_path().display());
            println!("File:        {:?}", spec.file_state());
            println!(
                "Engine:      {}",
                if port_listening(ipc_port) {
                    "answering"
                } else {
                    "not answering"
                }
            );
            0
        }
        Some("stop") => match crate::manager::stop(&spec) {
            Ok(()) => {
                println!("Stopped the background engine.");
                0
            }
            Err(message) => {
                eprintln!("{message}");
                1
            }
        },
        _ => {
            eprintln!("Usage: openagents background ensure|status|stop");
            2
        }
    }
}
