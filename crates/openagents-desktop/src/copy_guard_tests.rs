//! The machine-talk guard (#11031): no copy literal in the desktop's sources
//! and no text in its rendered-scene snapshots may narrate internals.

use std::path::Path;

/// Files with no user-facing copy.
const SKIP: &[&str] = &[
    // The release acceptance drivers: their strings name test steps and
    // failures for the gate's log, never a screen.
    "acceptance",
];

/// Reviewed exceptions, each with a reason. Keep this empty if you can.
const ALLOW: &[&str] = &[];

#[test]
fn desktop_copy_has_no_machine_talk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = oa_copy::scan_dir(&root.join("src"), SKIP, ALLOW);
    let snapshots = root.join("snapshots");
    let mut files: Vec<_> = std::fs::read_dir(&snapshots)
        .expect("the snapshots folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a snapshot");
        for (index, line) in text.lines().enumerate() {
            for v in oa_copy::violations(line, ALLOW) {
                hits.push(format!(
                    "{}:{}: {:?} in {:?}",
                    file.display(),
                    index + 1,
                    v.term,
                    v.context
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "machine talk in desktop copy; rewrite it in plain words (see AGENTS.md, #11031):\n{}",
        hits.join("\n")
    );
}
