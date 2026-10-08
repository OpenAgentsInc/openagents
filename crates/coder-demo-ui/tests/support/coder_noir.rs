//! Color-only expectations derived from the independent original native renderer.

use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn expected(original: &Value) -> Value {
    let expected: Value =
        serde_json::from_str(include_str!("../fixtures/coder-noir-d2fb95d33d.json")).unwrap();
    assert_eq!(expected["source"], original["source"]);
    assert_eq!(
        expected["original_fixture_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("../fixtures/native-d2fb95d33d.json"))
        )
    );
    // Three formerly identical cyan roles were marked in the archived renderer.
    // Restoring the markers reproduced every original cell hash before recoloring.
    for (role, color) in [
        ("ACCENT_MODEL", coder_ui::source_theme::ACCENT_MODEL),
        ("ACCENT_SKILL", coder_ui::source_theme::ACCENT_SKILL),
        ("MD_CODE", coder_ui::source_theme::MD_CODE),
    ] {
        assert_eq!(
            expected["role_markers"][role]["coder_noir"],
            format!("Rgb({}, {}, {})", color.red, color.green, color.blue)
        );
        assert_eq!(
            expected["role_markers"][role]["original"],
            "Rgb(0, 255, 255)"
        );
    }
    let frames = expected["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 93);
    for (mapped, retained) in frames.iter().zip(original["frames"].as_array().unwrap()) {
        for key in ["name", "width", "height"] {
            assert_eq!(mapped[key], retained[key]);
        }
        assert_eq!(mapped["original_cell_sha256"], retained["cell_sha256"]);
    }
    expected
}
