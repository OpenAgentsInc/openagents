//! `openagents plugin install|installed|enable|disable`: the plugins on this
//! computer (docs/background/2026-10-02-disk-cleanup-plugin.md).
//!
//! `install` copies a plugin directory whose package record resolves into
//! `~/.openagents/extensions/<key>/<slug>/<version>/`, off. `enable` turns it
//! on for this computer, after the host admits each background rule it
//! brings; `disable` turns it off. The host runs a plugin's background
//! rules only while it is on, and checks `enabled.json` at every check, so
//! the change needs no restart.

use std::path::{Path, PathBuf};

use background::Layout;
use background::plugins::{self, Installed, LOCAL_KEY};
use coder::package::Package;
use serde_json::json;

use crate::Output;

/// The directories a copy leaves out: test results, build output, and
/// version control.
pub(crate) const SKIP: &[&str] = &[".git", "target", "results", "node_modules", ".openagents"];

/// `openagents plugin COMMAND ...` for the local commands; `None` for any
/// other command.
pub fn run(output: &Output, words: &[String]) -> Option<u8> {
    let (command, rest) = words.split_first()?;
    let name = format!("plugin {command}");
    let first = rest.iter().find(|word| !word.starts_with('-'));
    let result = match command.as_str() {
        "install" => first
            .ok_or_else(|| "install needs the plugin's directory".to_owned())
            .and_then(|dir| install(Path::new(dir))),
        "installed" => list(),
        "enable" | "disable" => first
            .ok_or_else(|| format!("{command} needs the plugin's name"))
            .and_then(|plugin| enable(plugin, command == "enable")),
        _ => return None,
    };
    Some(match result {
        Ok(value) => {
            output.emit(&value, |value| {
                value["text"].as_str().unwrap_or_default().to_owned()
            });
            0
        }
        Err(message) => output.fail(&name, &message),
    })
}

/// This user's layout, the one the host reads.
fn layout() -> Result<Layout, String> {
    Layout::from_env().map_err(|error| error.to_string())
}

fn install(dir: &Path) -> Result<serde_json::Value, String> {
    install_into(&layout()?, dir)
}

/// Installs the plugin in `dir` under `layout`, off.
pub(crate) fn install_into(layout: &Layout, dir: &Path) -> Result<serde_json::Value, String> {
    let dir = dir
        .canonicalize()
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    let package = Package::load(&dir.join("package.json"))
        .map_err(|why| format!("{} is not a plugin: {why}", dir.display()))?;
    Package::resolve(&dir, &package)
        .map_err(|refusal| format!("{} does not resolve: {refusal}", dir.display()))?;
    let key = if is_hex64(&package.publisher) {
        package.publisher.clone()
    } else {
        LOCAL_KEY.to_owned()
    };
    let version = if package.version.is_empty() {
        "0.0.0".to_owned()
    } else {
        package.version.clone()
    };
    if version.contains('/') || version.contains("..") {
        return Err(format!("version {version:?} cannot name a folder"));
    }
    let into = layout
        .extensions()
        .join(&key)
        .join(&package.slug)
        .join(&version);
    let staging = into.with_extension("installing");
    let _ = std::fs::remove_dir_all(&staging);
    copy(&dir, &staging).map_err(|error| format!("copying {}: {error}", dir.display()))?;
    let _ = std::fs::remove_dir_all(&into);
    std::fs::rename(&staging, &into).map_err(|error| error.to_string())?;
    let plugin = plugins::find(layout, &format!("{key}:{}", package.slug))?;
    let text = if plugin.enabled {
        format!("Installed {} {version}. It is on.", plugin.name)
    } else {
        format!(
            "Installed {} {version}. It is off; turn it on with `openagents plugin enable {}`.",
            plugin.name, plugin.slug
        )
    };
    Ok(json!({"text": text, "plugin": row(&plugin)}))
}

fn list() -> Result<serde_json::Value, String> {
    let layout = layout()?;
    let all = plugins::installed(&layout);
    let text = if all.is_empty() {
        "No plugins are installed on this computer.".to_owned()
    } else {
        all.iter().map(line).collect::<Vec<_>>().join("\n")
    };
    Ok(json!({
        "text": text,
        "plugins": all.iter().map(row).collect::<Vec<_>>(),
    }))
}

fn enable(name: &str, on: bool) -> Result<serde_json::Value, String> {
    let layout = layout()?;
    let plugin = plugins::set_enabled(&layout, name, on)?;
    let text = match (on, plugin.background.is_empty()) {
        (true, false) => format!(
            "{} is on. It runs in the background here; `openagents background list` shows it.",
            plugin.name
        ),
        (true, true) => format!("{} is on.", plugin.name),
        (false, _) => format!("{} is off.", plugin.name),
    };
    Ok(json!({"text": text, "plugin": row(&plugin)}))
}

/// One plugin as the terminal and `installed` show it.
pub(crate) fn line(plugin: &Installed) -> String {
    let state = if plugin.enabled { "on" } else { "off" };
    let mut line = format!("{} · {state} · {}", plugin.slug, plugin.version);
    if !plugin.background.is_empty() {
        line.push_str(" · runs in the background");
    }
    line
}

fn row(plugin: &Installed) -> serde_json::Value {
    json!({
        "id": plugin.id,
        "slug": plugin.slug,
        "name": plugin.name,
        "version": plugin.version,
        "enabled": plugin.enabled,
        "background": plugin.background,
        "dir": plugin.dir.display().to_string(),
    })
}

/// The plugins installed here, for the terminal's `/plugins`.
pub(crate) fn installed_here() -> Vec<Installed> {
    layout()
        .map(|layout| plugins::installed(&layout))
        .unwrap_or_default()
}

/// Turn `id` on or off, for the terminal's `/plugins`: the words to show.
pub(crate) fn turn(id: &str, on: bool) -> Result<String, String> {
    enable(id, on).map(|value| value["text"].as_str().unwrap_or_default().to_owned())
}

fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIP.iter().any(|skip| name == *skip) {
            continue;
        }
        let kind = entry.file_type()?;
        let target: PathBuf = to.join(&name);
        if kind.is_dir() {
            copy(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
        // Symbolic links are not copied: a plugin is its own files.
    }
    Ok(())
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_skips_results_builds_and_links() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("p");
        std::fs::create_dir_all(from.join("evals/results/x")).unwrap();
        std::fs::create_dir_all(from.join("background")).unwrap();
        std::fs::create_dir_all(from.join("target")).unwrap();
        std::fs::write(from.join("background/r.json"), "{}").unwrap();
        std::fs::write(from.join("evals/results/x/report.json"), "{}").unwrap();
        std::os::unix::fs::symlink("/etc", from.join("link")).unwrap();
        let to = dir.path().join("q");
        copy(&from, &to).unwrap();
        assert!(to.join("background/r.json").is_file());
        assert!(!to.join("evals/results").exists());
        assert!(!to.join("target").exists());
        assert!(!to.join("link").exists());
    }
}
