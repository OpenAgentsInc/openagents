use super::*;

pub(super) fn fixture(
    command: &str,
    acceptance: Acceptance,
) -> (tempfile::TempDir, tempfile::TempDir, Plan) {
    let host = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("candidate.txt"), "immutable\n").unwrap();
    let manifest = host.path().join("check.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&capability::executor_document(
            "verification-fixture",
            "/bin/sh",
            vec!["/bin/sh".into(), "--version".into()],
            json!({
                "name": "Verification fixture",
                "invoke": ["/bin/sh"],
                "isolation": ["directory"]
            }),
        ))
        .unwrap(),
    )
    .unwrap();
    let entry = Entry::load(&manifest, capability::Source::Operator).unwrap();
    let plan = Plan {
        schema: SCHEMA.into(),
        input_digest: "candidate-digest".into(),
        seconds: 10,
        allow_unrestricted_reads: true,
        allow_network: true,
        checks: vec![Check {
            id: "fixture".into(),
            manifest,
            manifest_digest: entry.digest,
            arguments: vec!["-c".into(), command.into()],
            seconds: 3,
            output_bytes: 4096,
            acceptance,
        }],
    };
    (host, workspace, plan)
}

pub(super) fn supported() -> bool {
    if crate::delegate::boundary_supported() {
        true
    } else {
        eprintln!("skipping: verification needs an enforcing filesystem boundary");
        false
    }
}
