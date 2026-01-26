use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Dev,
    Release,
}

fn parse_mode() -> RunMode {
    let mut release = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "release" | "--release" | "build" | "--build" => {
                release = true;
            }
            "dev" | "--dev" => {
                release = false;
            }
            _ => {}
        }
    }
    if release {
        RunMode::Release
    } else {
        RunMode::Dev
    }
}

fn release_bundle_path(app_dir: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(
            app_dir
                .join("src-tauri")
                .join("target")
                .join("release")
                .join("bundle")
                .join("macos")
                .join("Autopilot.app"),
        )
    } else {
        None
    }
}

fn open_release_bundle(path: &Path) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        let status = Command::new("open")
            .arg(path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| format!("Failed to spawn open: {err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("open exited non-zero".to_string())
        }
    } else {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve OpenAgents repo root"))?;
    let app_dir = repo_root.join("apps").join("autopilot-desktop");

    if !app_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "Autopilot desktop app not found at {}",
            app_dir.display()
        ));
    }

    let mode = parse_mode();
    let tauri_cmd = match mode {
        RunMode::Dev => "dev",
        RunMode::Release => "build",
    };

    let status = Command::new("bun")
        .arg("run")
        .arg("tauri")
        .arg(tauri_cmd)
        .current_dir(&app_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() && matches!(mode, RunMode::Release) {
        if let Some(path) = release_bundle_path(&app_dir) {
            if let Err(err) = open_release_bundle(&path) {
                eprintln!(
                    "Release build completed but failed to open app: {err}\nOpen manually: {}",
                    path.display()
                );
            }
        } else {
            eprintln!("Release build completed. Open the app bundle manually.");
        }
    }

    match status.code() {
        Some(code) => std::process::exit(code),
        None => std::process::exit(1),
    }
}
