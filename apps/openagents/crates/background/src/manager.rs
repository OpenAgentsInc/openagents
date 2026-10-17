//! Starting a service through launchd or systemd.

use std::process::Command;

#[cfg_attr(test, allow(unused_imports))]
use crate::service::{Platform, ServiceSpec};

/// Loads and starts a service. Tests pass a recorder.
pub trait ServiceManager {
    /// Start `spec`'s service. `reload` is true when its file was just
    /// written, so the service manager must read it again.
    fn start(&self, spec: &ServiceSpec, reload: bool) -> Result<(), String>;
}

/// `launchctl` on macOS, `systemctl --user` on Linux.
pub struct SystemManager;

impl ServiceManager for SystemManager {
    fn start(&self, spec: &ServiceSpec, reload: bool) -> Result<(), String> {
        #[cfg(test)]
        {
            let _ = (spec, reload);
            panic!("a test reached the real service manager");
        }
        #[cfg(not(test))]
        match spec.host.platform {
            Platform::MacOs => {
                let domain = format!("gui/{}", uid()?);
                let target = format!("{domain}/{}", spec.label());
                let plist = spec.file_path();
                if reload {
                    // Not running (the caller checked); drop the old definition.
                    let _ = run("launchctl", &["bootout", &target]);
                }
                // Fails when already loaded, which is fine: kickstart starts it.
                let _ = run(
                    "launchctl",
                    &["bootstrap", &domain, &plist.to_string_lossy()],
                );
                run("launchctl", &["kickstart", &target])
            }
            Platform::Linux => {
                let unit = spec.unit_name();
                if reload {
                    run("systemctl", &["--user", "daemon-reload"])?;
                }
                // Enabled units start again at the next login.
                let _ = run("systemctl", &["--user", "enable", &unit]);
                run("systemctl", &["--user", "start", &unit])
            }
        }
    }
}

#[cfg(not(test))]
fn uid() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|error| format!("couldn't run id: {error}"))?;
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uid.is_empty() {
        return Err("couldn't find the user id".into());
    }
    Ok(uid)
}

#[cfg_attr(test, allow(dead_code))]
fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("couldn't run {program}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Stop `spec`'s service.
pub fn stop(spec: &ServiceSpec) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = spec;
        panic!("a test reached the real service manager");
    }
    #[cfg(not(test))]
    match spec.host.platform {
        Platform::MacOs => run(
            "launchctl",
            &["bootout", &format!("gui/{}/{}", uid()?, spec.label())],
        ),
        Platform::Linux => run("systemctl", &["--user", "stop", &spec.unit_name()]),
    }
}
