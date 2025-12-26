use std::fs;
use std::path::PathBuf;

use toml::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_wgpui_manifest() -> Value {
    let manifest_path = repo_root().join("crates/wgpui/Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read wgpui manifest");
    manifest.parse().expect("parse wgpui manifest")
}

fn feature_list<'a>(features: &'a toml::value::Table, name: &str) -> Vec<&'a str> {
    features
        .get(name)
        .and_then(Value::as_array)
        .expect("feature array")
        .iter()
        .filter_map(Value::as_str)
        .collect()
}

#[test]
fn test_wgpui_webgpu_support_configured() {
    let doc = load_wgpui_manifest();
    let features = doc
        .get("features")
        .and_then(Value::as_table)
        .expect("features table");
    let deps = doc
        .get("dependencies")
        .and_then(Value::as_table)
        .expect("dependencies table");

    let web_features = feature_list(features, "web");
    for dep in [
        "wasm-bindgen",
        "wasm-bindgen-futures",
        "web-sys",
        "js-sys",
        "console_error_panic_hook",
    ] {
        assert!(
            web_features.contains(&dep),
            "expected {} in wgpui web feature",
            dep
        );
    }

    let wgpu_dep = deps
        .get("wgpu")
        .and_then(Value::as_table)
        .expect("wgpu dependency table");
    let wgpu_features: Vec<&str> = wgpu_dep
        .get("features")
        .and_then(Value::as_array)
        .expect("wgpu features")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for feature in ["webgpu", "webgl"] {
        assert!(
            wgpu_features.contains(&feature),
            "expected wgpu feature {} for web support",
            feature
        );
    }

    let platform_src = fs::read_to_string(repo_root().join("crates/wgpui/src/platform.rs"))
        .expect("read platform.rs");
    assert!(
        platform_src.contains("Backends::BROWSER_WEBGPU"),
        "expected WebGPU backend selection in platform.rs"
    );
}

#[test]
fn test_wgpui_desktop_support_configured() {
    let doc = load_wgpui_manifest();
    let features = doc
        .get("features")
        .and_then(Value::as_table)
        .expect("features table");

    let desktop_features = feature_list(features, "desktop");
    for dep in ["winit", "pollster"] {
        assert!(
            desktop_features.contains(&dep),
            "expected {} in wgpui desktop feature",
            dep
        );
    }

    let example_src = fs::read_to_string(repo_root().join("crates/wgpui/examples/storybook.rs"))
        .expect("read storybook example");
    assert!(
        example_src.contains("Backends::all()"),
        "expected desktop example to request all wgpu backends"
    );
}
