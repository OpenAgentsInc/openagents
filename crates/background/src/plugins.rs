//! Background rules that plugins bring
//! (docs/background/2026-10-02-disk-cleanup-plugin.md).
//!
//! A plugin installed on this computer
//! (`~/.openagents/extensions/<key>/<slug>/<version>/`) may pin background
//! rules in its package record (`"background": [{"name", "digest"}]`), each
//! a rule document under its `background/` folder. A plugin's rules run
//! only while the plugin is enabled on this computer
//! (`~/.openagents/extensions/enabled.json`); a new install is off.
//!
//! The plugin describes; the host executes. [`admit`] takes a plugin's rule
//! only within what the host grants: actions only over the classes its
//! `needs.delete` names, roots and patterns only under the host's own
//! roots, the task store only when it asks, and never the id of a built-in
//! rule. Every safety check (the deny list, links, other volumes, locks,
//! open files, unsaved work, dry runs, the log) stays in this crate's code
//! and applies to a plugin's rule exactly as to the built-in one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::paths::Layout;
use crate::rule::{self, Class, Origin, Rule};
use crate::store::write_atomic;

/// The places a plugin's rule may name: the built-in rule's allow roots.
/// A plugin narrows them; it never adds one.
#[must_use]
pub fn host_roots() -> Vec<String> {
    rule::disk().safety.allow
}

/// The key a plugin installs under when its record names no 64-hex
/// publisher: no signer yet.
pub const LOCAL_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One plugin installed on this computer, its newest version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    /// `KEY:SLUG`, what enabling names.
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub dir: PathBuf,
    /// The rule ids its package record pins.
    pub background: Vec<String>,
    /// Folders it proposes as caches (`"classes": [{"path", "kind"}]`):
    /// the person confirms each, as for Jev's proposals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<(String, String)>,
    pub enabled: bool,
}

/// Every plugin installed under `~/.openagents/extensions`, newest version
/// of each, in key and slug order.
#[must_use]
pub fn installed(layout: &Layout) -> Vec<Installed> {
    let enabled = enabled(layout);
    let mut found = Vec::new();
    for key in sorted(&layout.extensions()) {
        let Some(key_name) = name_of(&key) else {
            continue;
        };
        for slug in sorted(&key) {
            let Some(dir) = sorted(&slug)
                .into_iter()
                .filter(|version| version.join("package.json").is_file())
                .next_back()
            else {
                continue;
            };
            let Ok(record) = read_record(&dir) else {
                continue;
            };
            let slug_name = text(&record, "slug");
            if slug_name.is_empty() {
                continue;
            }
            let id = format!("{key_name}:{slug_name}");
            found.push(Installed {
                enabled: enabled.contains(&id),
                name: Some(text(&record, "name"))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| slug_name.clone()),
                summary: text(&record, "summary"),
                version: text(&record, "version"),
                background: references(&record)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect(),
                classes: proposed(&record),
                slug: slug_name,
                dir,
                id,
            });
        }
    }
    found
}

/// The installed plugin `name` names: `KEY:SLUG`, or a slug only one
/// installed plugin has.
///
/// # Errors
/// None or several match, in words for the person.
pub fn find(layout: &Layout, name: &str) -> Result<Installed, String> {
    let all = installed(layout);
    let matches: Vec<Installed> = all
        .into_iter()
        .filter(|plugin| plugin.id == name || plugin.slug == name)
        .collect();
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap_or_else(|| unreachable!())),
        0 => Err(format!(
            "No plugin `{name}` is installed on this computer. Install it with `openagents plugin install DIR`."
        )),
        _ => Err(format!(
            "Several installed plugins are called `{name}`; name one as KEY:SLUG."
        )),
    }
}

/// Turn the installed plugin `name` on or off on this computer. Turning
/// one on admits each of its background rules first, so a plugin the host
/// would not run is never on.
///
/// # Errors
/// No such plugin, a rule the host refuses, or the file cannot be written.
pub fn set_enabled(layout: &Layout, name: &str, on: bool) -> Result<Installed, String> {
    let plugin = find(layout, name)?;
    if on {
        for id in &plugin.background {
            load_rule(&plugin, id)?;
        }
    }
    if on && !plugin.classes.is_empty() {
        crate::judged::from_plugin(layout, &plugin.id, &plugin.classes, crate::paths::now())?;
    }
    let mut set = enabled(layout);
    if on {
        set.insert(plugin.id.clone());
    } else {
        set.remove(&plugin.id);
    }
    let body = json!({"enabled": set.iter().collect::<Vec<_>>()});
    let bytes = serde_json::to_vec_pretty(&body).map_err(|error| error.to_string())?;
    write_atomic(&layout.enabled_plugins(), &bytes).map_err(|error| error.to_string())?;
    Ok(Installed {
        enabled: on,
        ..plugin
    })
}

/// The plugins enabled on this computer, as `KEY:SLUG`.
#[must_use]
pub fn enabled(layout: &Layout) -> BTreeSet<String> {
    std::fs::read(layout.enabled_plugins())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value["enabled"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Every background rule of every enabled plugin, admitted, or why not
/// (`(rule id, reason)`).
#[must_use]
pub fn rules(layout: &Layout) -> Vec<Result<Rule, (String, String)>> {
    installed(layout)
        .into_iter()
        .filter(|plugin| plugin.enabled)
        .flat_map(|plugin| {
            plugin
                .background
                .iter()
                .map(|id| load_rule(&plugin, id).map_err(|why| (id.clone(), why)))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The enabled plugin that brings rule `id`, and its admitted rule.
#[must_use]
pub fn rule(layout: &Layout, id: &str) -> Option<Result<(Installed, Rule), String>> {
    let plugin = installed(layout)
        .into_iter()
        .filter(|plugin| plugin.enabled)
        .find(|plugin| plugin.background.iter().any(|rule| rule == id))?;
    Some(load_rule(&plugin, id).map(|rule| (plugin, rule)))
}

/// Read, check the digest of, and admit the rule `id` of `plugin`.
///
/// # Errors
/// The record does not pin it, its bytes moved, or the host refuses it.
pub fn load_rule(plugin: &Installed, id: &str) -> Result<Rule, String> {
    let record = read_record(&plugin.dir)?;
    let (_, stated) = references(&record)
        .into_iter()
        .find(|(name, _)| name == id)
        .ok_or_else(|| format!("{} does not pin a background rule `{id}`", plugin.name))?;
    let path = rule_file(&plugin.dir.join("background"), id)
        .ok_or_else(|| format!("{} has no background rule `{id}`", plugin.name))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let found = digest(&text);
    if found != stated {
        return Err(format!(
            "{}'s rule `{id}` changed since it was pinned (digest {found}, pinned {stated})",
            plugin.name
        ));
    }
    let asked: Rule =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    // Confirmed caches are this computer's, never a package's.
    if !asked.classes.judged.is_empty()
        || asked
            .actions
            .iter()
            .any(|action| action.classes().contains(&Class::Judged))
    {
        return Err(format!(
            "{}: a plugin cannot bring confirmed caches; it proposes folders in its record's classes",
            plugin.name
        ));
    }
    admit(asked, plugin)
}

/// The host's admission of a plugin's rule: the rule as the host will run
/// it, or why it will not. The host sets the origin; the plugin's own
/// `enabled` and pause state are the computer's, not the plugin's.
///
/// # Errors
/// A plain sentence naming what the rule asked for beyond the host's
/// grant.
pub fn admit(mut asked: Rule, plugin: &Installed) -> Result<Rule, String> {
    let named = |what: String| format!("{}: {what}", plugin.name);
    asked.origin = Origin::Plugin {
        plugin: plugin.id.clone(),
        version: plugin.version.clone(),
    };
    asked.enabled = true;
    // Keep a packaged pause so activation can require a preview first.
    if rule::built_in(&asked.id).is_some() {
        return Err(named(format!(
            "a plugin cannot replace the built-in rule `{}`",
            asked.id
        )));
    }
    if !is_slug(&asked.id) {
        return Err(named(format!("`{}` is not a rule id", asked.id)));
    }
    asked.validate().map_err(named)?;
    let granted: BTreeSet<Class> = asked.needs.delete.iter().copied().collect();
    for action in &asked.actions {
        // Folders the person confirmed on this computer (class 7) need
        // no grant: the confirmation is the grant.
        for class in action.classes().into_iter().filter(|c| *c != Class::Judged) {
            if !granted.contains(&class) {
                return Err(named(format!(
                    "it deletes {} without asking for them in needs.delete",
                    class.noun(2)
                )));
            }
        }
    }
    for action in &asked.actions {
        match action {
            rule::Action::Notify { .. } if !asked.needs.notify => {
                return Err(named("it notifies without asking for needs.notify".into()));
            }
            rule::Action::GitFastForward { .. } => {
                return Err(named(
                    "a plugin's rule cannot update a Git checkout; that is a rule made here".into(),
                ));
            }
            rule::Action::StartCoderRun { .. } if !asked.needs.coder => {
                return Err(named(
                    "it starts Coder runs without asking for needs.coder".into(),
                ));
            }
            rule::Action::RunPlugin { plugin: other, .. }
                if *other != plugin.id && *other != plugin.slug =>
            {
                return Err(named(format!(
                    "it runs {other}; a plugin's rule runs only its own plugin"
                )));
            }
            rule::Action::ReleaseStaleClaims { .. }
            | rule::Action::HealthWatch { .. }
            | rule::Action::FlakeWatch
            | rule::Action::UsageSummary
            | rule::Action::Recalibrate
            | rule::Action::RotateLogs { .. } => {
                return Err(named(
                    "that action belongs to the host's built-in rules".into(),
                ));
            }
            _ => {}
        }
    }
    if asked.escalate.is_some() && !asked.needs.coder {
        return Err(named(
            "it escalates to Coder without asking for needs.coder".into(),
        ));
    }
    let task_classes = [Class::EndedTargets, Class::Worktrees];
    if !asked.needs.tasks && granted.iter().any(|class| task_classes.contains(class)) {
        return Err(named(
            "ended tasks' builds and worktrees need the task store; add needs.tasks".into(),
        ));
    }
    let roots = host_roots();
    let under_host = |pattern: &str| {
        let pattern = pattern.trim_end_matches('/');
        !pattern.contains("..")
            && roots.iter().any(|root| {
                pattern == root
                    || pattern
                        .strip_prefix(root.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
    };
    for root in &asked.safety.allow {
        if !under_host(root) {
            return Err(named(format!(
                "`{root}` is outside the places the host cleans ({})",
                roots.join(", ")
            )));
        }
    }
    for pattern in asked
        .classes
        .agent_targets
        .iter()
        .chain(&asked.classes.checkouts)
    {
        if !under_host(pattern) {
            return Err(named(format!(
                "`{pattern}` is outside the places the host cleans ({})",
                roots.join(", ")
            )));
        }
    }
    Ok(asked)
}

/// The digest a package record states for a component's text, as
/// `coder::package::digest` computes it.
#[must_use]
pub fn digest(text: &str) -> String {
    atif::digest(&json!(text))
}

fn read_record(dir: &Path) -> Result<Value, String> {
    let path = dir.join("package.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The folders a record proposes as caches, kept only when each is a
/// folder a rule may name and a disposable kind.
fn proposed(record: &Value) -> Vec<(String, String)> {
    record["classes"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|item| (text(item, "path"), text(item, "kind")))
                .filter(|(path, kind)| {
                    path.starts_with("~/")
                        && path.len() > 2
                        && !path.contains("..")
                        && crate::judged::KINDS.contains(&kind.as_str())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Package rule `id` as a plugin in `dir`: its record pins the rule by
/// digest and asks for exactly what the rule's actions need. The rule
/// starts paused, so its first real run waits for a dry run. Confirmed
/// caches stay on this computer. `openagents plugin publish DIR` then
/// publishes it.
///
/// # Errors
/// No such rule, an action a plugin cannot take, or the files cannot be
/// written.
pub fn package(layout: &Layout, id: &str, dir: &Path) -> Result<PathBuf, String> {
    let mut rule = crate::store::load(layout, id)?;
    rule.classes.judged.clear();
    for action in &mut rule.actions {
        if let rule::Action::DeleteCaches { classes } = action {
            classes.retain(|class| *class != Class::Judged);
        }
    }
    rule.actions.retain(
        |action| !matches!(action, rule::Action::DeleteCaches { classes } if classes.is_empty()),
    );
    let slug = if rule::built_in(id).is_some() {
        format!("{id}-rule")
    } else {
        id.to_owned()
    };
    rule.id.clone_from(&slug);
    let mut needs = rule::Needs::default();
    for action in &rule.actions {
        for class in action.classes() {
            if !needs.delete.contains(&class) {
                needs.delete.push(class);
            }
        }
        match action {
            rule::Action::Notify { .. } => needs.notify = true,
            rule::Action::StartCoderRun { .. } => needs.coder = true,
            rule::Action::GitFastForward { .. }
            | rule::Action::ReleaseStaleClaims { .. }
            | rule::Action::HealthWatch { .. }
            | rule::Action::FlakeWatch
            | rule::Action::UsageSummary
            | rule::Action::Recalibrate
            | rule::Action::RotateLogs { .. } => {
                return Err(format!(
                    "{} uses an action only this computer's own rules take; it cannot be a plugin",
                    rule.name
                ));
            }
            _ => {}
        }
    }
    needs.delete.sort();
    needs.tasks = needs
        .delete
        .iter()
        .any(|class| matches!(class, Class::EndedTargets | Class::Worktrees));
    needs.coder |= rule.escalate.is_some();
    rule.needs = needs;
    rule.origin = Origin::File {
        path: format!("background/{slug}.json"),
    };
    rule.enabled = false;
    rule.paused_until = Some(u64::MAX);
    rule.version = 1;
    let text = serde_json::to_string_pretty(&rule).map_err(|e| e.to_string())?;
    let record = json!({
        "v": 1,
        "slug": slug,
        "name": rule.name,
        "summary": format!("The background rule {} from this computer.", rule.name),
        "version": "0.1.0",
        "publisher": "",
        "provenance": "local file",
        "background": [{"name": slug, "digest": digest(&text)}],
    });
    let background = dir.join("background");
    std::fs::create_dir_all(&background).map_err(|e| e.to_string())?;
    write_atomic(&background.join(format!("{slug}.json")), text.as_bytes())
        .map_err(|e| e.to_string())?;
    let record = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    write_atomic(&dir.join("package.json"), &record).map_err(|e| e.to_string())?;
    Ok(dir.to_owned())
}

fn references(record: &Value) -> Vec<(String, String)> {
    record["background"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|item| (text(item, "name"), text(item, "digest")))
                .filter(|(name, _)| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The rule file in `dir` whose `id` is `id`.
fn rule_file(dir: &Path, id: &str) -> Option<PathBuf> {
    sorted(dir)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .find(|path| {
            std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|value| value["id"].as_str() == Some(id))
        })
}

fn text(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_owned()
}

fn name_of(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| !path.is_symlink())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !text.starts_with('-')
}
