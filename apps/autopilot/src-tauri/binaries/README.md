# Autopilot Sidecar Binary Drop Point

Release packaging may place signed `pylon` and `oa` sidecar binaries here or
in the Tauri resource directory next to the packaged executable.

Do not check compiled binaries into this repository by default. The Rust
resolver in `../src/pylon.rs` already looks for:

- binaries next to the app executable
- `binaries/` next to the app executable
- `../Resources/`
- `../Resources/binaries/`
- workspace builds under `target/{debug,fast-release,release}`
- app-managed cache under `~/.openagents/autopilot/bin`
- Pylon bootstrap cache under `~/.openagents/pylon/bootstrap`
- `PATH`

The release job or installer is responsible for copying or hydrating the
approved sidecar binaries before packaged-app validation. Source-checkout
development can use workspace binaries.
