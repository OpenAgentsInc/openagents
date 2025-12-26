use std::fs;
use std::path::PathBuf;

use toml::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_autopilot_gui_is_wgpui_only() {
    let manifest_path = repo_root().join("crates/autopilot-gui/Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read autopilot-gui manifest");
    let doc: Value = manifest.parse().expect("parse autopilot-gui manifest");
    let deps = doc
        .get("dependencies")
        .and_then(Value::as_table)
        .expect("dependencies table");

    for required in ["wgpui", "wgpu", "winit"] {
        assert!(
            deps.contains_key(required),
            "expected {} dependency in autopilot-gui",
            required
        );
    }

    for forbidden in ["actix-web", "actix-ws", "maud", "wry", "tao"] {
        assert!(
            !deps.contains_key(forbidden),
            "unexpected legacy web dependency {} in autopilot-gui",
            forbidden
        );
    }
}
