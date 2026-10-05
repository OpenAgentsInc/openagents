//! Executes the author's actual command workflow over the original licensed assets.
#![cfg(feature = "compiler")]
use std::{path::Path, process::Command};
fn cli(args: &[&std::ffi::OsStr]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_verse-content"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn author_a_second_playable_zone_without_renderer_changes() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("ritual");
    cli(&["ritual".as_ref(), source.as_os_str()]);
    let work = root.path().join("outpost");
    cli(&[
        "author".as_ref(),
        "init".as_ref(),
        source.as_os_str(),
        work.as_os_str(),
        "chamber-outpost".as_ref(),
    ]);
    let before = cli(&["author".as_ref(), "build".as_ref(), work.as_os_str()]);
    let edits = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/authoring/chamber-outpost.transaction.json");
    assert_eq!(
        cli(&[
            "author".as_ref(),
            "apply".as_ref(),
            work.as_os_str(),
            edits.as_os_str()
        ])["revision"],
        2
    );
    let report = cli(&[
        "author".as_ref(),
        "preview".as_ref(),
        work.as_os_str(),
        "1".as_ref(),
        "24".as_ref(),
    ]);
    assert!(
        report["actors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == 100 && a["name"] == "Outpost archivist")
    );
    assert!(
        report["actors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == 2 && a["health"] == 175)
    );
    assert_eq!(report["quests"][0]["giver"], 100);
    assert_eq!(report["quests"][0]["interactable"], true);
    assert!(report["navigation"]["stats"]["spans"].as_u64().unwrap() > 0);
    let after = cli(&["author".as_ref(), "build".as_ref(), work.as_os_str()]);
    assert_ne!(before["generation"], after["generation"]);
    assert_ne!(before["content"], after["content"]);
    assert_eq!(
        cli(&["author".as_ref(), "build".as_ref(), work.as_os_str()])["reused"],
        true
    );
    let current = std::fs::read(work.join("current.json")).unwrap();
    let invalid = root.path().join("invalid.json");
    std::fs::write(&invalid,br#"{"expected_revision":2,"label":"Broken quest giver","edits":[{"op":"remove_actor","id":100}]}"#).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_verse-content"))
        .args([
            "author".as_ref(),
            "apply".as_ref(),
            work.as_os_str(),
            invalid.as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("progression.quests[id=1].giver"));
    assert_eq!(std::fs::read(work.join("current.json")).unwrap(), current);
    cli(&[
        "author".as_ref(),
        "undo".as_ref(),
        work.as_os_str(),
        "2".as_ref(),
    ]);
    cli(&[
        "author".as_ref(),
        "redo".as_ref(),
        work.as_os_str(),
        "3".as_ref(),
    ]);
    assert_eq!(
        cli(&["author".as_ref(), "build".as_ref(), work.as_os_str()])["generation"],
        after["generation"]
    );
    println!(
        "{}",
        serde_json::json!({"schema":"verse.author.acceptance.v1","before":before,"after":after,"preview":report,"invalid_diagnostic":String::from_utf8_lossy(&refused.stderr)})
    );
    if let Some(destination) = std::env::var_os("VERSE_AUTHOR_EVIDENCE") {
        let destination = Path::new(&destination);
        std::fs::create_dir_all(destination).unwrap();
        let mut retained = serde_json::json!({"before":before,"after":after,"preview":report,"invalid_diagnostic":String::from_utf8_lossy(&refused.stderr)});
        retained["before"]["path"] = serde_json::json!("scratch/ritual-generation");
        retained["after"]["path"] = serde_json::json!("scratch/outpost-generation");
        std::fs::write(
            destination.join("acceptance.json"),
            serde_json::to_vec_pretty(&retained).unwrap(),
        )
        .unwrap();
        std::fs::copy(work.join("preview.svg"), destination.join("outpost.svg")).unwrap();
        let generation = Path::new(after["path"].as_str().unwrap());
        for name in ["document.json", "generation.json"] {
            std::fs::copy(generation.join(name), destination.join(name)).unwrap();
        }
    }
}
