#![allow(dead_code)]
use code_index::recipes::*;
use std::path::Path;
use std::process::Command;

pub fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}

pub fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"),"pub enum Kind { A }\npub fn name(k: Kind) -> &'static str { match k { Kind::A => \"a\" } }\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "fixture"]);
    dir
}

pub fn artifact(context: &Context) -> Artifact {
    let r = context
        .reference("src/lib.rs", Some("demo::Kind"), 0, 0)
        .unwrap();
    Artifact {
        schema: "openagents.task-index.v1".into(),
        snapshot: context.snapshot.clone(),
        recipes: vec![Recipe {
            id: "add-kind".into(),
            capability: "registry".into(),
            operation: "add-kind".into(),
            description: "Add a registered kind and its name mapping.".into(),
            aliases: vec!["register a kind".into()],
            exclusions: vec!["inference model".into()],
            conditions: vec![],
            roles: vec![Role {
                id: "enum".into(),
                purpose: "Add the kind.".into(),
                required: true,
                when: None,
                references: vec![r.clone()],
            }],
            examples: vec![r.clone()],
            new_files: vec![NewFile {
                directory: "src".into(),
                purpose: "Optional implementation module.".into(),
                integration: r,
            }],
            checks: vec![Check {
                package: "demo".into(),
                manifest: "Cargo.toml".into(),
                filter: None,
                establishes: "All enum variants map to names.".into(),
            }],
            evidence: EvidenceClass::SourceReviewed,
            limitations: vec![],
        }],
    }
}
