//! Which backend the compositor starts on, and the arguments that say so.
//!
//! Two backends share the state, the layout, and the protocols. The nested
//! backend, `winit`, opens a window in the session you are in. The hardware
//! backend, `udev`, takes a seat through `libseat`, drives the monitors
//! through DRM and KMS, and reads the keyboard and the pointer through
//! `libinput`, which is what a TTY needs.
//!
//! `--backend udev` or `--backend winit` names one. With neither, a process
//! that finds a Wayland or X11 display in its environment is inside a
//! session and opens nested, and a process that finds neither is on a TTY
//! and takes the hardware.

/// The backend the compositor draws through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// DRM, KMS, `libinput`, and `libseat`, for a TTY.
    Udev,
    /// A window in the session this process runs in.
    Winit,
}

impl Backend {
    /// The name `--backend` takes.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Udev => "udev",
            Backend::Winit => "winit",
        }
    }
}

/// What the command line asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Run the compositor, on the backend named or on the one the
    /// environment picks.
    Run(Option<Backend>),
    /// Print the usage.
    Help,
    /// Print the version.
    Version,
}

/// The command one list of arguments makes, less the program's name.
pub fn parse(arguments: &[String]) -> Result<Command, String> {
    let mut backend = None;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--backend" => {
                let Some(name) = rest.next() else {
                    return Err("--backend takes udev or winit".to_string());
                };
                backend = Some(named(name)?);
            }
            other => match other.strip_prefix("--backend=") {
                Some(name) => backend = Some(named(name)?),
                None => return Err(format!("{other} is not an argument it takes")),
            },
        }
    }
    Ok(Command::Run(backend))
}

fn named(name: &str) -> Result<Backend, String> {
    match name {
        "udev" => Ok(Backend::Udev),
        "winit" => Ok(Backend::Winit),
        other => Err(format!(
            "--backend takes udev or winit, and {other} is neither"
        )),
    }
}

/// The backend a run starts on: the one the command line named, or the one
/// the environment picks. `value` reads one environment variable.
pub fn choose(named: Option<Backend>, value: impl Fn(&str) -> Option<String>) -> Backend {
    if let Some(backend) = named {
        return backend;
    }
    let set = |name: &str| value(name).is_some_and(|held| !held.is_empty());
    if set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET") || set("DISPLAY") {
        Backend::Winit
    } else {
        Backend::Udev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    fn environment<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn no_arguments_run_on_the_backend_the_environment_picks() {
        assert_eq!(parse(&args(&[])), Ok(Command::Run(None)));
    }

    #[test]
    fn the_backend_flag_names_either_backend_in_either_spelling() {
        assert_eq!(
            parse(&args(&["--backend", "udev"])),
            Ok(Command::Run(Some(Backend::Udev)))
        );
        assert_eq!(
            parse(&args(&["--backend=winit"])),
            Ok(Command::Run(Some(Backend::Winit)))
        );
    }

    #[test]
    fn a_backend_the_compositor_does_not_have_is_refused() {
        assert!(parse(&args(&["--backend", "x11"])).is_err());
        assert!(parse(&args(&["--backend"])).is_err());
        assert!(parse(&args(&["--frobnicate"])).is_err());
    }

    #[test]
    fn help_and_version_answer_before_anything_runs() {
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(
            parse(&args(&["--backend", "udev", "-V"])),
            Ok(Command::Version)
        );
    }

    #[test]
    fn a_session_in_the_environment_opens_nested() {
        assert_eq!(
            choose(None, environment(&[("WAYLAND_DISPLAY", "wayland-1")])),
            Backend::Winit
        );
        assert_eq!(
            choose(None, environment(&[("DISPLAY", ":0")])),
            Backend::Winit
        );
    }

    #[test]
    fn a_tty_with_no_session_takes_the_hardware() {
        assert_eq!(choose(None, environment(&[])), Backend::Udev);
        assert_eq!(
            choose(
                None,
                environment(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", "")])
            ),
            Backend::Udev
        );
    }

    #[test]
    fn a_named_backend_wins_over_the_environment() {
        assert_eq!(
            choose(
                Some(Backend::Udev),
                environment(&[("WAYLAND_DISPLAY", "wayland-1")])
            ),
            Backend::Udev
        );
        assert_eq!(
            choose(Some(Backend::Winit), environment(&[])),
            Backend::Winit
        );
        assert_eq!(Backend::Udev.name(), "udev");
    }
}
