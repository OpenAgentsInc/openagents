use aethyrforge_host::*;
use base64::Engine;
use sha2::Digest;
#[test]
fn capture_preview_checks_bytes_hash_dimensions_and_limits() {
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aF1sAAAAASUVORK5CYII=";
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png)
        .unwrap();
    let mut outcome = Outcome {
        state: ResultState::Applied,
        message: "Preview captured.".into(),
        status: None,
        detail: Some(Detail::CapturePreview {
            png: png.into(),
            sha256: format!("{:x}", sha2::Sha256::digest(bytes)),
            width: 1,
            height: 1,
        }),
    };
    Action::Capture.validate_outcome(&outcome).unwrap();
    if let Some(Detail::CapturePreview { width, .. }) = &mut outcome.detail {
        *width = 2;
    }
    assert!(outcome.validate().is_err());
    if let Some(Detail::CapturePreview { width, sha256, .. }) = &mut outcome.detail {
        *width = 1;
        *sha256 = "0".repeat(64);
    }
    assert!(outcome.validate().is_err());
}
#[test]
fn private_engine_fields_do_not_fit_the_public_authoring_view() {
    let view = serde_json::json!({"schema":AUTHORING_SCHEMA,"revision":0,"effect_scope":"world","can_undo":false,"can_redo":false,"draft":DEFAULT_EFFECT,"active_source":DEFAULT_EFFECT,"diagnostics":[],"materials":[],"environment":{},"renderer_source":"private source"});
    assert!(serde_json::from_value::<AuthoringView>(view).is_err());
}

#[test]
fn compile_permission_binds_the_exact_source_draft() {
    let action = Action::Compile {
        asset: Some(EFFECT_ASSET.into()),
        source_sha256: Some("a".repeat(64)),
    };
    let other = Action::Compile {
        asset: Some(EFFECT_ASSET.into()),
        source_sha256: Some("b".repeat(64)),
    };
    assert_ne!(
        action_digest(&action).unwrap(),
        action_digest(&other).unwrap()
    );
    assert!(
        Action::Compile {
            asset: Some(EFFECT_ASSET.into()),
            source_sha256: Some("wrong".into())
        }
        .validate()
        .is_err()
    );
}
