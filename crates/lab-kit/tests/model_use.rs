use lab_kit::{Artifact, ArtifactRole, AssetClass, AssetEntry, ModelUse, Version, check_model_use};
use serde_json::json;

fn version() -> Version {
    Version {
        document: "source".into(),
        number: 1,
        parent: None,
        author: "fixture".into(),
        summary: "Source".into(),
        created_unix_ms: 1,
        schema: "fixture.v1".into(),
        content: json!({"text":"public"}),
        content_sha256: "a".repeat(64),
        report: Default::default(),
        assets: vec![],
    }
}
fn artifact(v: &Version) -> Artifact {
    Artifact {
        id: "output".into(),
        version: v.reference(),
        role: ArtifactRole::Build,
        media_type: "application/json".into(),
        sha256: "b".repeat(64),
        size: 1,
        class: AssetClass::Public,
        no_training: false,
    }
}
#[test]
fn protected_versions_cannot_enter_any_model_use_even_when_old_outputs_have_no_flag() {
    for class in [AssetClass::PrivateLocal, AssetClass::NoTraining] {
        let mut v = version();
        let old_output = artifact(&v);
        v.assets.push(AssetEntry {
            location: "local/input".into(),
            sha256: "c".repeat(64),
            class,
        });
        for purpose in [ModelUse::Training, ModelUse::Evaluation, ModelUse::Trace] {
            assert_eq!(
                check_model_use(&v, std::slice::from_ref(&old_output), purpose)
                    .unwrap_err()
                    .code,
                "no_training"
            );
        }
    }
}
#[test]
fn protected_output_flags_and_classes_cannot_be_downgraded_by_public_inputs() {
    let v = version();
    for purpose in [ModelUse::Training, ModelUse::Evaluation, ModelUse::Trace] {
        for class in [AssetClass::PrivateLocal, AssetClass::NoTraining] {
            let mut output = artifact(&v);
            output.class = class;
            assert_eq!(
                check_model_use(&v, &[output], purpose).unwrap_err().code,
                "no_training"
            );
        }
        let mut output = artifact(&v);
        output.no_training = true;
        assert_eq!(
            check_model_use(&v, &[output], purpose).unwrap_err().code,
            "no_training"
        );
    }
}
#[test]
fn public_inputs_are_allowed_and_outputs_must_match_the_checked_version() {
    let v = version();
    for purpose in [ModelUse::Training, ModelUse::Evaluation, ModelUse::Trace] {
        check_model_use(&v, &[artifact(&v)], purpose).unwrap();
        let mut other = artifact(&v);
        other.version.version += 1;
        assert_eq!(
            check_model_use(&v, &[other], purpose).unwrap_err().code,
            "artifact_version"
        );
    }
}
