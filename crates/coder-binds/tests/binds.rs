//! The desktop's chords live in one table, `coder_binds::BINDS`, and its
//! window rules in another, `coder_binds::RULES`, and the copies that
//! describe them stay equal to them: `desktop.nix` holds the Hyprland text
//! the tables render, and the Coder compositor reads the tables themselves.
//!
//! A host's own launchers and window rules are not rows of either table.
//! `desktop.nix` renders them from `coderos.desktop.extraBinds` and
//! `extraWindowRules`, and the `extension-points` check in `os/flake.nix`
//! holds what they render.
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A bind line with the modifier field sorted, so `SUPER SHIFT ALT` and
/// `SUPER ALT SHIFT` compare equal. The word order is spelling, not
/// meaning.
fn normalize(line: &str) -> String {
    let (head, rest) = match line.split_once(", ") {
        Some((head, rest)) => (head, format!(", {rest}")),
        None => (line, String::new()),
    };
    let (kind, mods) = head.split_once(" = ").expect("bind line");
    let mut mods: Vec<&str> = mods.split_whitespace().collect();
    mods.sort_unstable();
    format!("{} = {}{}", kind, mods.join(" "), rest)
}

/// The byte offset of the next `bind`, `binde`, or `bindm` opening in
/// `line` at or after `from`. A bind starts a line (after indentation) or
/// follows the quote of a `lib.optionalString "..."` string.
fn find_bind(line: &str, from: usize) -> Option<usize> {
    let mut best = None;
    for pat in ["bindm = ", "binde = ", "bind = "] {
        let mut at = from;
        while let Some(i) = line[at..].find(pat) {
            at += i;
            let ok = line[..at]
                .chars()
                .next_back()
                .is_none_or(|c| c == ' ' || c == '\t' || c == '"');
            if ok {
                best = Some(best.map_or(at, |b: usize| b.min(at)));
                break;
            }
            at += 1;
        }
    }
    best
}

/// The `bind`, `binde`, and `bindm` lines `desktop.nix` can write, in file
/// order. Binds inside a `lib.optionalString "..."` run to the quote or a
/// literal `\n`; the rest run to the end of the line.
fn nix_bind_lines() -> Vec<String> {
    let text = std::fs::read_to_string(root().join("os/modules/coderos/desktop.nix"))
        .expect("desktop.nix");
    let mut out = Vec::new();
    for line in text.lines() {
        let mut from = 0;
        while let Some(at) = find_bind(line, from) {
            let end = line[at..]
                .find(['"', '\\'])
                .map(|i| at + i)
                .unwrap_or(line.len());
            out.push(normalize(line[at..end].trim_end()));
            from = end;
        }
    }
    out
}

/// The `windowrule` lines `desktop.nix` writes, in file order. Each one
/// starts a line, after indentation, and runs to the end of it.
fn nix_rule_lines() -> Vec<String> {
    let text = std::fs::read_to_string(root().join("os/modules/coderos/desktop.nix"))
        .expect("desktop.nix");
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("windowrule = "))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_table_renders_the_hyprland_window_rules() {
    assert_eq!(
        coder_binds::hyprland_rule_lines(),
        nix_rule_lines(),
        "crates/coder-binds and os/modules/coderos/desktop.nix disagree on the window rules"
    );
}

/// The launcher options `desktop.nix` grants the Coder compositor, in file
/// order: the quoted names in the list `compositorLaunchers` filters by
/// whether each option is on.
fn nix_launcher_options() -> Vec<String> {
    let text = std::fs::read_to_string(root().join("os/modules/coderos/desktop.nix"))
        .expect("desktop.nix");
    let from = text
        .find("compositorLaunchers = lib.filter")
        .expect("desktop.nix filters compositorLaunchers");
    let list = &text[from..];
    let open = list.find('[').expect("the list opens");
    let close = list.find("];").expect("the list closes");
    list[open + 1..close]
        .split_whitespace()
        .map(|name| name.trim_matches('"').to_string())
        .collect()
}

#[test]
fn the_grant_names_every_option_that_gates_a_launcher_row() {
    let gated: Vec<String> = coder_binds::BINDS
        .iter()
        .filter(|bind| bind.surfaces.contains(coder_binds::Surface::Compositor))
        .filter(|bind| {
            matches!(
                bind.action,
                coder_binds::Action::Exec { .. } | coder_binds::Action::ToggleHands
            )
        })
        .filter_map(|bind| bind.option.map(str::to_string))
        .collect();
    assert_eq!(
        nix_launcher_options(),
        gated,
        "the compositorLaunchers list in os/modules/coderos/desktop.nix and the launcher \
         rows of crates/coder-binds disagree"
    );
}

#[test]
fn the_table_renders_the_hyprland_binds() {
    let rendered: Vec<String> = coder_binds::hyprland_lines()
        .iter()
        .map(|line| normalize(line))
        .collect();
    assert_eq!(
        rendered,
        nix_bind_lines(),
        "crates/coder-binds and os/modules/coderos/desktop.nix disagree"
    );
}

/// Every chord CoderQuest reads on a Mac is a chord of its own, and none of
/// them is a Command chord macOS answers before the window does. The window
/// reads Control and Option for a row's Super, and Command for a row's Ctrl
/// on top of it, which `crates/coder-binds/src/modifier.rs` says why.
#[test]
fn the_mac_reading_gives_each_quest_row_a_chord_of_its_own() {
    use coder_binds::{Modifier, Surface};

    let mut seen: Vec<(coder_binds::Held, coder_binds::Key)> = Vec::new();
    for bind in coder_binds::BINDS
        .iter()
        .filter(|bind| bind.surfaces.contains(Surface::Quest))
    {
        let held = bind
            .mods
            .held(Modifier::ControlOption)
            .unwrap_or_else(|| panic!("{:?} {:?} has no Mac chord", bind.mods, bind.key));
        assert!(
            held.ctrl && held.alt,
            "{:?} {:?} reads as {held:?}, which macOS may answer first",
            bind.mods,
            bind.key
        );
        assert!(
            !seen.contains(&(held, bind.key)),
            "{:?} {:?} reads as {held:?}, which another row already reads",
            bind.mods,
            bind.key
        );
        seen.push((held, bind.key));
    }
    assert!(!seen.is_empty(), "CoderQuest binds nothing");
}

/// A row with no Mac chord is a compositor row. The Mac reading drops a row
/// that holds Alt, because Option is half of the desktop modifier there.
#[test]
fn a_row_without_a_mac_chord_belongs_to_the_compositor() {
    use coder_binds::{Modifier, Surface};

    for bind in coder_binds::BINDS
        .iter()
        .filter(|bind| bind.mods.held(Modifier::ControlOption).is_none())
    {
        assert!(
            !bind.surfaces.contains(Surface::Quest),
            "{:?} {:?} reaches CoderQuest with no Mac chord",
            bind.mods,
            bind.key
        );
    }
}
