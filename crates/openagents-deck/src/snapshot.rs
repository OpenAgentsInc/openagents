//! The golden-snapshot check for the deck's slides.
//!
//! Every slide renders to a grid, and the grid's `snapshot()` is checked in
//! under `snapshots/<deck>/<id>.txt`. A change to the copy or to a layout
//! shows up as a diff a reviewer reads. The check is this crate's own because the
//! files live beside this crate; the format is
//! [`Grid::snapshot`]'s.
//!
//! To record a change on purpose:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p openagents-deck
//! git diff crates/openagents-deck/snapshots
//! ```

use crate::grid::Grid;
use std::path::PathBuf;

/// The directory the snapshots of `deck` live in.
fn directory(deck: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join(deck)
}

/// The file for `name` in `deck`.
pub fn path(deck: &str, name: &str) -> PathBuf {
    directory(deck).join(format!("{name}.txt"))
}

/// The checked-in snapshot for `name` in `deck`, when there is one.
pub fn read(deck: &str, name: &str) -> Option<String> {
    std::fs::read_to_string(path(deck, name)).ok()
}

/// Every snapshot name under `snapshots/<deck>/`, sorted.
pub fn names(deck: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory(deck))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    name.strip_suffix(".txt").map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Compares `grid` with the snapshot for `name` in `deck`, or writes it
/// when `UPDATE_SNAPSHOTS` is set. Panics on a difference.
pub fn check(deck: &str, name: &str, grid: &Grid) {
    let actual = grid.snapshot();
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(directory(deck)).expect("the snapshots directory");
        std::fs::write(path(deck, name), &actual).expect("the snapshot file");
        return;
    }
    match read(deck, name) {
        Some(expected) if expected == actual => {}
        Some(expected) => panic!(
            "the {deck}/{name} snapshot differs.\nexpected:\n{expected}\nactual:\n{actual}\n\
             Run the tests with UPDATE_SNAPSHOTS=1 to record the change."
        ),
        None => panic!(
            "no snapshot for {deck}/{name}. Run the tests with UPDATE_SNAPSHOTS=1 to record \
             it; the grid is:\n{actual}"
        ),
    }
}
