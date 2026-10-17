use agent_lab::components::{ModelContextSource, ReadTool, ToolBinding};
use lab_kit::{Artifact, ArtifactRole, AssetClass, AssetEntry, Version};
use serde_json::json;
use std::cell::Cell;

fn fixture() -> (ToolBinding, Version, Artifact) {
    let binding = ToolBinding {
        id: "part".into(),
        name: "Part".into(),
        account: "alice".into(),
        project: "project".into(),
        origin: "http://127.0.0.1:9000/".into(),
        tool: ReadTool::Cad,
        arguments: json!({"doc":"part","version":3}),
        training_allowed: true,
        private_local: false,
    };
    let content = json!({"part":"public input"});
    let version = Version {
        document: "part".into(),
        number: 3,
        parent: Some(2),
        author: "alice".into(),
        summary: "Saved part".into(),
        created_unix_ms: 0,
        schema: "part.v1".into(),
        content_sha256: lab_kit::digest(&content),
        content,
        report: Default::default(),
        assets: vec![],
    };
    let artifact = Artifact {
        id: "source".into(),
        version: version.reference(),
        role: ArtifactRole::Source,
        media_type: "text/plain".into(),
        sha256: "a".repeat(64),
        size: 10,
        class: AssetClass::Public,
        no_training: false,
    };
    (binding, version, artifact)
}

fn format_after_admission(
    binding: &ToolBinding,
    source: Option<ModelContextSource<'_>>,
    formatted: &Cell<usize>,
) -> Result<String, lab_kit::Refusal> {
    let source = binding.admit_model_context("alice", "project", source)?;
    formatted.set(formatted.get() + 1);
    Ok(source.version.content.to_string())
}

#[test]
fn source_policy_overrides_permissive_operator_flags_before_formatting() {
    let (binding, mut version, artifact) = fixture();
    assert!(binding.validate("alice", "project"));
    let formatted = Cell::new(0);
    for class in [AssetClass::PrivateLocal, AssetClass::NoTraining] {
        version.assets = vec![AssetEntry {
            location: "input".into(),
            sha256: "b".repeat(64),
            class,
        }];
        let err = format_after_admission(
            &binding,
            Some(ModelContextSource {
                version: &version,
                artifacts: &[],
            }),
            &formatted,
        )
        .unwrap_err();
        assert_eq!(err.code, "no_training");
    }
    version.assets.clear();
    for (class, no_training) in [
        (AssetClass::PrivateLocal, false),
        (AssetClass::NoTraining, false),
        (AssetClass::Public, true),
    ] {
        let protected = Artifact {
            class,
            no_training,
            ..artifact.clone()
        };
        assert!(
            format_after_admission(
                &binding,
                Some(ModelContextSource {
                    version: &version,
                    artifacts: &[protected]
                }),
                &formatted,
            )
            .is_err()
        );
    }
    assert_eq!(formatted.get(), 0);
}

#[test]
fn absent_unreadable_or_mismatched_metadata_never_reaches_formatter() {
    let (binding, version, artifact) = fixture();
    let formatted = Cell::new(0);
    assert!(format_after_admission(&binding, None, &formatted).is_err());
    // Parsing arbitrary read-tool JSON is insufficient to construct source metadata.
    for raw in ["not json", r#"{"passed":true,"training_allowed":true}"#] {
        let unreadable = serde_json::from_str::<Version>(raw).ok();
        assert!(unreadable.is_none());
        assert!(
            format_after_admission(
                &binding,
                unreadable.as_ref().map(|version| ModelContextSource {
                    version,
                    artifacts: &[]
                }),
                &formatted,
            )
            .is_err()
        );
    }
    let mut wrong_document = version.clone();
    wrong_document.document = "other".into();
    let mut wrong_number = version.clone();
    wrong_number.number += 1;
    let mut wrong_hash = version.clone();
    wrong_hash.content["part"] = json!("changed");
    for source in [&wrong_document, &wrong_number, &wrong_hash] {
        assert!(
            format_after_admission(
                &binding,
                Some(ModelContextSource {
                    version: source,
                    artifacts: &[]
                }),
                &formatted,
            )
            .is_err()
        );
    }
    let mut wrong_artifact = artifact;
    wrong_artifact.version.version += 1;
    assert!(
        format_after_admission(
            &binding,
            Some(ModelContextSource {
                version: &version,
                artifacts: &[wrong_artifact]
            }),
            &formatted,
        )
        .is_err()
    );
    let mut robot = binding;
    robot.tool = ReadTool::Robot;
    robot.arguments = json!({"document":"part","version":version.content_sha256,"frame":0});
    assert!(
        format_after_admission(
            &robot,
            Some(ModelContextSource {
                version: &version,
                artifacts: &[]
            }),
            &formatted,
        )
        .is_err()
    );
    assert_eq!(formatted.get(), 0);
}

#[test]
fn trusted_public_metadata_still_requires_local_permission_and_scope() {
    let (mut binding, version, artifact) = fixture();
    for (account, project) in [("bob", "project"), ("alice", "other")] {
        assert!(
            binding
                .admit_model_context(
                    account,
                    project,
                    Some(ModelContextSource {
                        version: &version,
                        artifacts: &[],
                    })
                )
                .is_err()
        );
    }
    let formatted = Cell::new(0);
    for (training_allowed, private_local) in [(false, false), (true, true)] {
        binding.training_allowed = training_allowed;
        binding.private_local = private_local;
        assert!(
            format_after_admission(
                &binding,
                Some(ModelContextSource {
                    version: &version,
                    artifacts: &[],
                }),
                &formatted
            )
            .is_err()
        );
    }
    assert_eq!(formatted.get(), 0);
    binding.training_allowed = true;
    binding.private_local = false;
    assert!(
        format_after_admission(
            &binding,
            Some(ModelContextSource {
                version: &version,
                artifacts: &[artifact],
            }),
            &formatted
        )
        .is_ok()
    );
    binding.tool = ReadTool::Video;
    binding.arguments = json!({"document":"part","version":3,"render":false});
    assert!(
        format_after_admission(
            &binding,
            Some(ModelContextSource {
                version: &version,
                artifacts: &[],
            }),
            &formatted
        )
        .is_ok()
    );
    assert_eq!(formatted.get(), 2);
}
