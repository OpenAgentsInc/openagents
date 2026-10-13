//! #11541: live scripts and test helpers run Coder in their own HOME and
//! `--state`, and never carry the person's sync setting there: a copied
//! `sync.json` would send their test chats to the person's account. (The
//! setting itself also stays off in a folder that didn't turn it on, see
//! `coder_sync`'s `sync_on_stays_with_the_folder_that_turned_it_on`.)

use std::path::Path;

#[test]
fn live_scripts_and_test_helpers_never_copy_the_sync_setting() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for dir in ["scripts/coder", "crates/coder-new/tests"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let script = dir == "scripts/coder" && name.ends_with("-live.sh");
            let helper =
                dir != "scripts/coder" && name.ends_with(".rs") && name != "live_scripts_sync.rs";
            if !script && !helper {
                continue;
            }
            checked += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                !text.contains("sync.json"),
                "{} names sync.json: a test run must not carry the person's sync setting",
                path.display()
            );
        }
    }
    assert!(
        checked > 1,
        "found no live scripts or test helpers to check"
    );
}
