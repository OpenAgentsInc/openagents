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
            .and_then(|plugin| {
                let args = crate::Args::parse(rest, &[])?;
                match (args.option("version"), args.option("digest")) {
                    (None, None) => enable(plugin, command == "enable"),
                    (Some(version), Some(digest)) => enable_exact_in(
                        &layout()?,
                        plugin,
                        command == "enable",
                        Some((version, digest)),
                    ),
                    _ => Err("Exact enabling needs both --version and --digest.".into()),
                }
            }),
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
    // A plugin folder of one's own: its record states its files' digests
    // now, so nobody computes one by hand (#10306).
    let repinned = crate::plugin_new::repinned_note(dir)?;
    let mut value = install_into(&layout()?, dir)?;
    if let Some(note) = repinned {
        let text = value["text"].as_str().unwrap_or_default().to_owned();
        value["text"] = json!(format!("{note}\n{text}"));
    }
    Ok(value)
}

/// Installs the plugin in `dir` under `layout`, off.
pub(crate) fn install_into(layout: &Layout, dir: &Path) -> Result<serde_json::Value, String> {
    let _lock = plugins::mutation_lock(layout)?;
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
    // Installation is a separate choice from enabling, including when an
    // earlier version under this ID was on. The mutation lock remains held.
    let mut enabled = plugins::enabled(layout);
    enabled.remove(&format!("{key}:{}", package.slug));
    let bytes =
        serde_json::to_vec_pretty(&json!({"enabled": enabled})).map_err(|e| e.to_string())?;
    background::store::write_atomic(&layout.enabled_plugins(), &bytes)
        .map_err(|e| e.to_string())?;
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
    enable_in(&layout()?, name, on)
}

fn enable_in(layout: &Layout, name: &str, on: bool) -> Result<serde_json::Value, String> {
    enable_exact_in(layout, name, on, None)
}

pub(crate) fn enable_exact_in(
    layout: &Layout,
    name: &str,
    on: bool,
    pin: Option<(&str, &str)>,
) -> Result<serde_json::Value, String> {
    let layout = layout.clone();
    let plugin = plugins::set_enabled_exact(&layout, name, on, pin)?;
    let mut text = match (on, plugin.background.is_empty()) {
        (true, false) => format!(
            "{} is on. It runs in the background here; `openagents background list` shows it.",
            plugin.name
        ),
        (true, true) => format!("{} is on.", plugin.name),
        (false, _) => format!("{} is off.", plugin.name),
    };
    if on {
        for preview in start_packaged_rules(&layout, &plugin)? {
            text.push_str("\n\n");
            text.push_str(&preview);
        }
    }
    Ok(json!({"text": text, "plugin": row(&plugin)}))
}

/// Starts each of `plugin`'s background rules that still holds the pause
/// it was packaged with (a plugin's rule waits for a dry run before its
/// first real run), showing that dry run: turning the plugin on is when
/// its owner sees what it would do. A rule the person paused or turned off
/// here keeps that. The dry runs, one block per rule.
fn start_packaged_rules(layout: &Layout, plugin: &Installed) -> Result<Vec<String>, String> {
    let mut previews = Vec::new();
    for id in &plugin.background {
        let rule = background::store::load(layout, id)?;
        if rule.paused_until != Some(PACKAGED_PAUSE) {
            continue;
        }
        let mut lines = vec![format!(
            "{}, what it would do now (a dry run; nothing is deleted):",
            rule.name
        )];
        lines.extend(crate::background::dry_run_lines(layout, &rule)?);
        background::view::pause(layout, id, None, true)?;
        previews.push(lines.join("\n"));
    }
    Ok(previews)
}

/// The pause a plugin's rule is packaged with: until a dry run.
const PACKAGED_PAUSE: u64 = u64::MAX;

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

    #[test]
    fn installing_a_reviewed_release_does_not_inherit_enabled_state() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/disk-cleanup");
        let installed = install_into(&layout, &repo).unwrap();
        let id = installed["plugin"]["id"].as_str().unwrap();
        plugins::set_enabled(&layout, id, true).unwrap();
        assert!(plugins::find(&layout, id).unwrap().enabled);
        install_into(&layout, &repo).unwrap();
        assert!(!plugins::find(&layout, id).unwrap().enabled);
    }

    /// #10305: turning a background plugin on shows its rule's dry run and
    /// starts it, instead of leaving it paused for good.
    #[test]
    fn enable_starts_a_packaged_rule_after_showing_its_dry_run() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/disk-cleanup");
        install_into(&layout, &repo).unwrap();

        let value = enable_in(&layout, "disk-cleanup", true).unwrap();
        let text = value["text"].as_str().unwrap();
        assert!(text.contains("a dry run; nothing is deleted"), "{text}");
        let after = background::store::load(&layout, "disk-cleanup").unwrap();
        assert!(after.enabled);
        assert_eq!(after.paused_until, None);

        // Paused here by the person, it stays paused through off and on.
        background::view::pause(&layout, "disk-cleanup", None, false).unwrap();
        enable_in(&layout, "disk-cleanup", false).unwrap();
        let value = enable_in(&layout, "disk-cleanup", true).unwrap();
        assert!(!value["text"].as_str().unwrap().contains("dry run"));
        assert!(
            !background::store::load(&layout, "disk-cleanup")
                .unwrap()
                .enabled
        );
    }
}
