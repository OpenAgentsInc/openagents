use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_files_with_extension(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries {
        let entry = entry.expect("read entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            out.push(path);
        }
    }
}

fn assert_exists(root: &Path, rel: &str) {
    let path = root.join(rel);
    assert!(path.exists(), "expected {} to exist", rel);
}

fn assert_missing(root: &Path, rel: &str) {
    let path = root.join(rel);
    assert!(!path.exists(), "expected {} to be archived", rel);
}

#[test]
fn test_html_maud_stack_archived() {
    let root = repo_root();
    let gui_dir = root.join("src/gui");
    assert!(gui_dir.is_dir(), "expected src/gui directory");

    let mut rs_files = Vec::new();
    collect_files_with_extension(&gui_dir, "rs", &mut rs_files);
    assert!(
        rs_files.is_empty(),
        "expected no Rust sources in src/gui, found {:?}",
        rs_files
    );

    for rel in [
        "src/gui/README.md",
        "src/gui/assets/fonts/VeraMono.ttf",
        "src/gui/assets/fonts/VeraMono-Italic.ttf",
        "src/gui/assets/fonts/VeraMono-Bold.ttf",
        "src/gui/assets/fonts/VeraMono-Bold-Italic.ttf",
        "src/gui/assets/fonts/Bitstream Vera License.txt",
    ] {
        assert_exists(&root, rel);
    }

    for rel in [
        "src/gui/app.rs",
        "src/gui/server.rs",
        "src/gui/mod.rs",
        "src/gui/routes",
        "src/gui/views",
        "src/gui/middleware",
        "src/gui/assets/htmx.min.js",
        "src/gui/assets/htmx-ws.js",
    ] {
        assert_missing(&root, rel);
    }

    for rel in [
        "docs/legacy/gui-web/README.md",
        "docs/legacy/gui-web/app.rs",
        "docs/legacy/gui-web/server.rs",
        "docs/legacy/gui-web/state.rs",
        "docs/legacy/gui-web/ws.rs",
        "docs/legacy/gui-web/mod.rs",
        "docs/legacy/gui-web/routes/mod.rs",
        "docs/legacy/gui-web/views/layout.rs",
        "docs/legacy/gui-web/middleware/mod.rs",
        "docs/legacy/gui-web/assets/htmx.min.js",
        "docs/legacy/gui-web/assets/htmx-ws.js",
    ] {
        assert_exists(&root, rel);
    }
}
