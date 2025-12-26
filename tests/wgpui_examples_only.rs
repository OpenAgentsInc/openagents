use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries {
        let entry = entry.expect("read entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn example_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let crates_dir = repo_root().join("crates");
    let entries = fs::read_dir(&crates_dir).expect("read crates dir");
    for entry in entries {
        let entry = entry.expect("read crate entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let examples_dir = path.join("examples");
        if examples_dir.is_dir() {
            collect_rs_files(&examples_dir, &mut files);
        }
    }
    files
}

#[test]
fn test_examples_do_not_reference_legacy_web_stack() {
    let examples = example_files();
    assert!(!examples.is_empty(), "expected example files in crates");

    let forbidden = ["actix_web", "actix_ws", "maud::", "wry::", "tao::", "htmx"];
    for path in examples {
        let contents = fs::read_to_string(&path).expect("read example");
        for token in forbidden {
            assert!(
                !contents.contains(token),
                "legacy web stack reference {} in {}",
                token,
                path.display()
            );
        }
    }
}
