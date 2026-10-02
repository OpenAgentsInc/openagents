//! `openagents plugin install|enable|disable` and `openagents background
//! list` with a plugin that runs in the background, under a temporary
//! home (docs/background/2026-10-02-disk-cleanup-plugin.md).
#![cfg(unix)]

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

fn openagents(home: &Path, args: &[&str]) -> (bool, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .arg("--json")
        .args(args)
        .env("HOME", home)
        .env_remove("OPENAGENTS_HOME")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let value = serde_json::from_str(text.trim()).unwrap_or_else(
        |_| json!({"stdout": text, "stderr": String::from_utf8_lossy(&out.stderr)}),
    );
    (out.status.success(), value)
}

/// A plugin directory whose one background rule is the built-in disk rule
/// under the id `disk-cleanup`, with `change` applied to it.
fn plugin(dir: &Path, change: impl FnOnce(&mut Value)) {
    let mut rule = serde_json::to_value(background::rule::disk()).unwrap();
    rule["id"] = json!("disk-cleanup");
    rule["needs"] = json!({
        "delete": ["ended_targets", "stale_targets", "worktrees", "gate_pools", "incremental", "trash"],
        "tasks": true,
        "notify": true,
    });
    change(&mut rule);
    let text = serde_json::to_string_pretty(&rule).unwrap();
    std::fs::create_dir_all(dir.join("background")).unwrap();
    std::fs::write(dir.join("background/disk-cleanup.json"), &text).unwrap();
    let record = json!({
        "v": 1, "slug": "disk-cleanup", "name": "Disk cleanup",
        "summary": "Keeps the disk from filling up.", "version": "0.1.0",
        "background": [{"name": "disk-cleanup", "digest": coder::package::digest(&text)}],
    });
    std::fs::write(
        dir.join("package.json"),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();
}

fn rules(home: &Path) -> Vec<String> {
    let (ok, list) = openagents(home, &["background", "list"]);
    assert!(ok, "{list}");
    list.as_array()
        .or_else(|| list["rules"].as_array())
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .filter(|row| row["error"].is_null())
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_background_plugin_is_off_after_install_and_runs_only_while_on() {
    let home = tempfile::tempdir().unwrap();
    let source = home.path().join("src/disk-cleanup");
    plugin(&source, |_| {});
    let (ok, installed) = openagents(
        home.path(),
        &["plugin", "install", source.to_str().unwrap()],
    );
    assert!(ok, "{installed}");
    assert_eq!(installed["plugin"]["enabled"], json!(false));
    assert_eq!(rules(home.path()), vec!["disk"]);

    let (ok, on) = openagents(home.path(), &["plugin", "enable", "disk-cleanup"]);
    assert!(ok, "{on}");
    assert_eq!(rules(home.path()), vec!["disk", "disk-cleanup"]);
    let (ok, dry) = openagents(
        home.path(),
        &["background", "run", "disk-cleanup", "--dry-run"],
    );
    assert!(ok, "{dry}");
    let (_, here) = openagents(home.path(), &["plugin", "installed"]);
    assert_eq!(here["plugins"][0]["enabled"], json!(true));

    let (ok, off) = openagents(home.path(), &["plugin", "disable", "disk-cleanup"]);
    assert!(ok, "{off}");
    assert_eq!(rules(home.path()), vec!["disk"]);
    let (ok, _) = openagents(
        home.path(),
        &["background", "run", "disk-cleanup", "--dry-run"],
    );
    assert!(!ok);
}

#[test]
fn a_plugin_that_asks_for_more_than_the_host_grants_is_never_on() {
    let home = tempfile::tempdir().unwrap();
    let source = home.path().join("src/greedy");
    plugin(&source, |rule| {
        rule["safety"]["allow"]
            .as_array_mut()
            .unwrap()
            .push(json!("~/Documents"));
    });
    let (ok, installed) = openagents(
        home.path(),
        &["plugin", "install", source.to_str().unwrap()],
    );
    assert!(ok, "{installed}");
    let (ok, refused) = openagents(home.path(), &["plugin", "enable", "disk-cleanup"]);
    assert!(!ok);
    assert!(
        refused
            .to_string()
            .contains("outside the places the host cleans"),
        "{refused}"
    );
    assert_eq!(rules(home.path()), vec!["disk"]);
}
