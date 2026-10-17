//! Fails when code in the desktop workspace builds the real GPUI platform
//! directly (#11973). Tests use `openagents_gpui_test::headless_app` or
//! `#[gpui::test]`; fixture binaries that show a window use
//! `openagents_gpui_test::real_application`, which is off unless
//! `OPENAGENTS_GPUI_REAL_PLATFORM=1`.

use std::path::{Path, PathBuf};

/// Calls that build the real platform (or reach it through a helper that
/// does).
const FORBIDDEN: &[&str] = &[
    "gpui_platform::application",
    "gpui_platform::headless",
    "gpui_platform::current_platform",
    "gpui_platform::background_executor",
    "gpui_platform::current_headless_renderer",
    "use gpui_platform::{",
    "MacPlatform",
    "gpui_macos::",
    "Application::new(",
    "VisualTestAppContext",
];

/// Files allowed to build the platform: this crate, and the desktop app's
/// own entry point.
const ALLOWED: &[&str] = &["crates/gpui-test/", "crates/ui/src/lib.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != "ios" && name != "node_modules" {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn offenders(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("apps"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED.iter().any(|allowed| relative.starts_with(allowed)) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for pattern in FORBIDDEN {
                if line.contains(pattern) {
                    found.push(format!("{relative}:{}: {pattern}", number + 1));
                }
            }
        }
    }
    found
}

#[test]
fn no_code_builds_the_real_platform_outside_the_helper() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap();
    assert!(
        root.join("crates/ui").is_dir(),
        "desktop workspace not found"
    );
    let found = offenders(&root);
    assert!(
        found.is_empty(),
        "These lines build the real GPUI platform. Use openagents_gpui_test::headless_app \
         or #[gpui::test] in tests, and openagents_gpui_test::real_application in fixtures \
         that show a window (#11973):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_guard_catches_a_new_offender() {
    let dir = std::env::temp_dir().join(format!("gpui-guard-{}", std::process::id()));
    let src = dir.join("crates/example/src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("lib.rs"),
        "#[test]\nfn t() {\n    // gpui_platform::application() in a comment is fine\n    \
         gpui_platform::application().run(|_| {});\n}\n",
    )
    .unwrap();
    let found = offenders(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        found,
        ["crates/example/src/lib.rs:4: gpui_platform::application"]
    );
}
