//! Installed plugins' hooks in Coder's sessions (#11499): every plugin on
//! this computer that pins hooks (`coder_sdk::plugin_hooks`), registered
//! into a session's hook set while the person has it on.
//!
//! Turning a plugin on admits what its listing shows each hook disclosing
//! (to its own Wasm guest, and to the Decisions API for a hook that asks
//! for decisions). A hook that asks to change commands runs without that
//! until the person turns it on (`openagents plugin allow-rewrite PLUGIN
//! HOOK`, kept in `~/.openagents/extensions/hook-grants.json`). Each
//! registration note and each firing (with its replayable receipt) is one
//! JSON line under `~/.openagents/plugin-hooks/`.

use std::path::PathBuf;
use std::sync::Arc;

use background::Layout;
use coder_sdk::plugin_hooks::{self, Grants, Note, Plugin};
use serde_json::json;

/// Where the person's rewrite grants live.
#[must_use]
pub fn grants_path(layout: &Layout) -> PathBuf {
    layout.extensions().join("hook-grants.json")
}

/// The rewrites the person turned on.
#[must_use]
pub fn grants(layout: &Layout) -> Grants {
    std::fs::read(grants_path(layout))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The installed plugins that pin hooks, as their hooks see them. A plugin
/// whose hooks don't check out is left out, with why.
#[must_use]
pub fn installed(layout: &Layout) -> (Vec<Plugin>, Vec<String>) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    for plugin in background::plugin_list::installed(layout) {
        let pins_hooks = std::fs::read_to_string(plugin.dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|record| {
                record["hooks"]
                    .as_array()
                    .is_some_and(|hooks| !hooks.is_empty())
            });
        if !pins_hooks {
            continue;
        }
        match plugin_hooks::load(&plugin.id, &plugin.name, plugin.enabled, &plugin.dir) {
            Ok(loaded) => found.push(loaded),
            Err(why) => problems.push(why),
        }
    }
    (found, problems)
}

/// Registers every installed plugin's hooks into `registry`, recording
/// what was done under `layout`. Returns the notes.
pub fn register_from(
    layout: &Layout,
    registry: &mut coder_sdk::Registry,
    decider: Option<Arc<dyn coder_sdk::decide::Decider>>,
) -> Vec<Note> {
    let (plugins, problems) = installed(layout);
    let log = log_dir(layout);
    let firings = plugin_hooks::Firings::logging(log.join("firings.jsonl"));
    let grants = grants(layout);
    let mut notes = Vec::new();
    for plugin in &plugins {
        notes.extend(plugin_hooks::register(
            registry,
            plugin,
            &grants,
            &plugin_hooks::admitted_by(plugin),
            decider.clone(),
            &firings,
        ));
    }
    notes.extend(problems.into_iter().map(|message| Note::Invalid {
        hook: String::new(),
        message,
    }));
    let worth_keeping: Vec<&Note> = notes
        .iter()
        .filter(|note| !matches!(note, Note::PluginOff { .. }))
        .collect();
    if !worth_keeping.is_empty() {
        let lines: Vec<serde_json::Value> = worth_keeping
            .iter()
            .map(|note| json!({"ts": now_ms(), "note": note}))
            .collect();
        append(&log.join("registrations.jsonl"), &lines);
    }
    notes
}

/// Where plugin hooks' notes and firings are kept:
/// `~/.openagents/plugin-hooks/`.
#[must_use]
pub fn log_dir(layout: &Layout) -> PathBuf {
    layout
        .extensions()
        .parent()
        .map_or_else(|| layout.extensions(), std::path::Path::to_path_buf)
        .join("plugin-hooks")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// [`register_from`] on this person's plugins, deciding through the
/// Decisions API. Tests stay off the person's own plugins.
pub fn register(registry: &mut coder_sdk::Registry) {
    if cfg!(test) {
        return;
    }
    if let Ok(layout) = Layout::from_env() {
        let _ = register_from(&layout, registry, Some(Arc::new(coder_sdk::decide::Api)));
    }
}

fn append(path: &std::path::Path, lines: &[serde_json::Value]) {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        for line in lines {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// The listing lines of the installed plugin `id`'s hooks; none when it
/// pins none.
#[must_use]
pub fn listing(layout: &Layout, id: &str) -> Vec<String> {
    let (plugins, _) = installed(layout);
    plugins
        .iter()
        .find(|plugin| plugin.id == id)
        .map(|plugin| plugin_hooks::listing(plugin, &grants(layout)))
        .unwrap_or_default()
}

/// Turns `hook`'s rewrite on or off for the installed plugin `name`
/// (`KEY:SLUG` or a slug). The sentence to show.
///
/// # Errors
///
/// No such plugin, no such hook, a hook that doesn't ask to rewrite, or
/// the file can't be written.
pub fn set_rewrite(layout: &Layout, name: &str, hook: &str, on: bool) -> Result<String, String> {
    let installed = background::plugin_list::find(layout, name)?;
    let plugin = plugin_hooks::load(
        &installed.id,
        &installed.name,
        installed.enabled,
        &installed.dir,
    )?;
    let found = plugin
        .hooks
        .iter()
        .find(|candidate| candidate.doc.id == hook)
        .ok_or_else(|| format!("{} has no hook called {hook}.", plugin.name))?;
    if !found
        .doc
        .authority
        .contains(&plugin_hooks::Authority::Rewrite)
    {
        return Err(format!(
            "{}'s hook {hook} doesn't ask to change commands.",
            plugin.name
        ));
    }
    let mut grants = grants(layout);
    let key = Grants::key(&plugin.id, hook);
    if on {
        grants.rewrite.insert(key);
    } else {
        grants.rewrite.remove(&key);
    }
    let bytes = serde_json::to_vec_pretty(&grants).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(layout.extensions()).map_err(|error| error.to_string())?;
    background::layout::write_atomic(&grants_path(layout), &bytes)
        .map_err(|error| error.to_string())?;
    Ok(if on {
        format!(
            "{}'s hook {hook} can now change commands, from the next session.",
            plugin.name
        )
    } else {
        format!("{}'s hook {hook} no longer changes commands.", plugin.name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example plugin installed off, then on: its hooks register only
    /// once it is on, the rewrite waits for the person, and turning that on
    /// is kept.
    #[test]
    #[cfg(unix)]
    fn an_installed_plugins_hooks_register_once_it_is_on() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::new(home.path(), None).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-hook-guard");
        let into = layout
            .extensions()
            .join(background::plugin_list::LOCAL_KEY)
            .join("hook-guard")
            .join("0.1.0");
        for part in [
            "package.json",
            "hooks/guard.json",
            "hooks/remind.json",
            "hooks/dry-run.json",
            "hooks/risky.json",
            "wasm/hook-guard.wasm",
        ] {
            std::fs::create_dir_all(into.join(part).parent().unwrap()).unwrap();
            std::fs::copy(source.join(part), into.join(part)).unwrap();
        }
        let mut registry = coder_sdk::Registry::new();
        let notes = register_from(&layout, &mut registry, None);
        assert!(matches!(notes[..], [Note::PluginOff { .. }]), "{notes:?}");
        assert!(registry.is_empty());

        background::plugins::set_enabled(&layout, "hook-guard", true).unwrap();
        let mut registry = coder_sdk::Registry::new();
        let notes = register_from(&layout, &mut registry, None);
        assert_eq!(registry.hooks().count(), 4, "{notes:?}");
        assert!(
            notes
                .iter()
                .any(|note| matches!(note, Note::RewriteWithheld { .. }))
        );
        let recorded = std::fs::read_to_string(
            home.path()
                .join(".openagents/plugin-hooks/registrations.jsonl"),
        )
        .unwrap();
        assert!(recorded.contains("rewrite_withheld"), "{recorded}");
        let lines = listing(
            &layout,
            &format!("{}:hook-guard", background::plugins::LOCAL_KEY),
        );
        assert_eq!(lines.len(), 4);
        assert!(lines[2].contains("off until you turn it on"), "{lines:?}");

        assert!(set_rewrite(&layout, "hook-guard", "guard", true).is_err());
        set_rewrite(&layout, "hook-guard", "dry-run", true).unwrap();
        let mut registry = coder_sdk::Registry::new();
        let notes = register_from(&layout, &mut registry, None);
        assert!(
            !notes
                .iter()
                .any(|note| matches!(note, Note::RewriteWithheld { .. }))
        );
        let applied = coder_sdk::Runner::new(registry, coder_sdk::Budget::default()).fire(
            &coder_sdk::Call::tool(
                coder_sdk::Event::PreToolUse,
                "Bash",
                json!({"command": "git push --force"}),
            ),
        );
        assert!(matches!(
            applied.permission,
            coder_sdk::Permission::Deny { .. }
        ));
    }
}
