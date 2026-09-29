//! The `migrate` helper process.
//!
//! Detecting and adopting an old-style setup read the host's key file, so
//! they run here, in a child the window starts and waits for, never in
//! the window itself. The child prints one line of JSON without a secret
//! ([`Report`]) and exits.

use openagents_desktop::migrate::{self, Report};
use std::path::PathBuf;
use std::process::Command;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// `openagents-desktop migrate detect|adopt`: runs in the helper process.
pub fn main(args: &[String]) -> i32 {
    let Some(home) = home() else {
        eprintln!("HOME is not set");
        return 2;
    };
    let report = match args.first().map(String::as_str) {
        Some("detect") => migrate::detect(&home, unix_now()),
        Some("adopt") => {
            migrate::adopt_here(
                &home,
                unix_now(),
                &mut || match crate::mac::register_agent() {
                    openagents_desktop::model::Agent::Enabled
                    | openagents_desktop::model::Agent::NeedsApproval => Ok(()),
                    openagents_desktop::model::Agent::NotRegistered => {
                        Err("this is not the OpenAgents app bundle".into())
                    }
                    openagents_desktop::model::Agent::Failed(message) => Err(message),
                },
            )
        }
        _ => {
            eprintln!("usage: openagents-desktop migrate detect|adopt");
            return 2;
        }
    };
    match serde_json::to_string(&report) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(_) => 1,
    }
}

/// Runs this executable as the helper and reads its report.
fn helper(command: &str) -> Report {
    let Ok(exe) = std::env::current_exe() else {
        return Report::Failed {
            message: "OpenAgents can't find itself".into(),
        };
    };
    match Command::new(exe).args(["migrate", command]).output() {
        Ok(output) => {
            let line = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(line.trim()).unwrap_or(Report::Failed {
                message: "the setup check gave no answer".into(),
            })
        }
        Err(_) => Report::Failed {
            message: "the setup check did not start".into(),
        },
    }
}

/// Detects an old-style setup in the helper process.
pub fn detect_in_helper() -> Report {
    helper("detect")
}

/// Adopts it in the helper process. The message on failure is for the
/// person.
pub fn adopt_in_helper() -> Result<(), String> {
    match helper("adopt") {
        Report::Adopted { .. } => Ok(()),
        Report::Failed { .. } | Report::None | Report::Found { .. } => {
            Err("Couldn't move your setup. Coder keeps running as it was. Try again later.".into())
        }
    }
}
