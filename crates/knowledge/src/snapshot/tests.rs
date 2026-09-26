use super::*;

fn signer() -> RelaySigner {
    RelaySigner::from_secret_hex(&"21".repeat(32)).unwrap()
}
fn document() -> String {
    "---\nid: test.reference\nversion: 1\nkind: method\ntitle: Reference fixture\nsummary: A synthetic reference for a fixture.\ntags: [fixture]\napplies_when: Testing snapshot contracts.\nstatus: admitted\nauthor: Fixture author\nprovenance:\n  written_from: [fixture-source]\n  cites: [https://example.invalid/reference]\nevidence: []\n---\n\nKeep this exact synthetic reference.\n".into()
}
fn bundle() -> Bundle {
    create(&[document()], &signer(), "fixture", "1", "CC0-1.0", 100).unwrap()
}
fn resign(bundle: &mut Bundle) {
    let bytes = contracts::jcs(&bundle.manifest).unwrap();
    let mut record = ext::parse_record(&bundle.release).unwrap();
    record["manifest"] = artifact(&bytes, "application/json", "openagents.package.v1");
    bundle.release = signer().sign(
        100,
        ext::RELEASE_KIND,
        bundle.release.tags.clone(),
        record.to_string(),
    );
}
#[test]
fn signed_snapshot_loads_exact_provenance_without_admission() {
    let bundle = bundle();
    let verified = verify(&bundle).unwrap();
    assert_eq!(verified.base.entries.len(), 1);
    let entry = &verified.base.entries[0];
    assert_eq!(entry.digest, crate::digest(document().as_bytes()));
    assert_eq!(entry.written_from, ["fixture-source"]);
    assert_eq!(entry.status, Status::Candidate);
    assert_eq!(verified.release.id, bundle.release.id);
}
#[test]
fn signatures_files_paths_components_and_dependencies_are_not_advisory() {
    let mut changed = bundle();
    changed
        .documents
        .values_mut()
        .next()
        .unwrap()
        .push_str("changed");
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.release.content.push(' ');
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.documents.insert("extra.md".into(), document());
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.manifest["files"][0]["path"] = json!("../escape.md");
    resign(&mut changed);
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.manifest["components"][0]["kind"] = json!("operation");
    resign(&mut changed);
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.manifest["dependencies"] = json!(["aa".repeat(32)]);
    resign(&mut changed);
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.manifest["version"] = json!("2");
    resign(&mut changed);
    assert!(verify(&changed).is_err());
    let mut changed = bundle();
    changed.manifest["components"][0]["definition"]["size"] = json!(1);
    resign(&mut changed);
    assert!(verify(&changed).is_err());
}
#[test]
fn only_curated_unique_entries_can_be_packaged() {
    assert!(create(&[], &signer(), "fixture", "1", "CC0", 100).is_err());
    assert!(
        create(
            &[document(), document()],
            &signer(),
            "fixture",
            "1",
            "CC0",
            100
        )
        .is_err()
    );
    assert!(
        create(
            &[document().replace("status: admitted", "status: candidate")],
            &signer(),
            "fixture",
            "1",
            "CC0",
            100
        )
        .is_err()
    );
}
#[test]
fn installed_bundle_is_private_immutable_and_reverified() {
    let dir = std::env::temp_dir().join(format!(
        "kb-snapshot-{}-{}",
        std::process::id(),
        hex(&entropy().unwrap())
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("pin.json");
    let bundle = bundle();
    write_new(&path, &bundle).unwrap();
    assert_eq!(read(&path).unwrap().release.id, bundle.release.id);
    assert!(write_new(&path, &bundle).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen('{', "{\"v\":\"duplicate\",", 1)).unwrap();
    assert!(read(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
