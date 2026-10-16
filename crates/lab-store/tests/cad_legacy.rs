use lab_store::{
    Error,
    backend::FsBackend,
    cad_legacy::{CadLegacy, DirectorySource},
};
use serde_json::json;
use std::sync::Arc;

fn fixture(root: &std::path::Path) {
    let prefix = root.join("owner/cad/me");
    std::fs::create_dir_all(prefix.join("part/v1/out")).unwrap();
    std::fs::write(prefix.join("index.json"), serde_json::to_vec(&json!({"schema":"openagents.web.cad-index.v1","docs":{"part":{"title":"Synthetic block","kind":"part","versions":1,"history":[{"n":1,"summary":"Created","author":"test","created":1}]}}})).unwrap()).unwrap();
    std::fs::write(
        prefix.join("part/v1/doc.json"),
        br#"{"schema":"cad.part.v1","features":[]}"#,
    )
    .unwrap();
    std::fs::write(
        prefix.join("part/v1/version.json"),
        br#"{"n":1,"summary":"Created","author":"test","created":1}"#,
    )
    .unwrap();
    std::fs::write(prefix.join("part/v1/out/example.stl"), b"synthetic export").unwrap();
}
#[tokio::test]
async fn exact_dry_run_checked_copy_and_unchanged_source() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let source = Arc::new(DirectorySource::open(root.path()).unwrap());
    let legacy = CadLegacy::personal(source.clone(), "owner").unwrap();
    let report = legacy.dry_run().await.unwrap();
    assert_eq!(report.documents, 1);
    assert_eq!(report.versions, 1);
    assert_eq!(report.files.len(), 4);
    assert_eq!(
        report.total_bytes,
        report.files.iter().map(|f| f.bytes).sum::<u64>()
    );
    assert!(report.files.iter().all(|f| f.sha256.len() == 64));
    assert_eq!(
        legacy.version("part", 1).await.unwrap().0["schema"],
        "cad.part.v1"
    );
    assert!(legacy.version("part", 2).await.is_err());
    assert!(legacy.version("../part", 1).await.is_err());
    assert!(CadLegacy::personal(source.clone(), "../owner").is_err());
    let target_root = tempfile::tempdir().unwrap();
    let target = FsBackend::open(target_root.path()).unwrap();
    assert!(matches!(
        legacy.copy(&target, &"0".repeat(64)).await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        legacy
            .copy(&target, &report.checksum().unwrap())
            .await
            .unwrap(),
        report
    );
    assert_eq!(
        legacy
            .copy(&target, &report.checksum().unwrap())
            .await
            .unwrap(),
        report
    );
    assert_eq!(legacy.dry_run().await.unwrap(), report);
    // A changed source needs another reviewed dry-run, even if its layout is valid.
    std::fs::write(
        root.path().join("owner/cad/me/part/v1/doc.json"),
        br#"{"schema":"cad.part.v1","features":[1]}"#,
    )
    .unwrap();
    assert!(matches!(
        legacy.copy(&target, &report.checksum().unwrap()).await,
        Err(Error::Conflict)
    ));
    assert!(
        CadLegacy::project(source, "owner", "elsewhere")
            .unwrap()
            .index()
            .await
            .is_err()
    );
}
#[cfg(unix)]
#[tokio::test]
async fn shortcuts_cannot_escape_the_legacy_root() {
    let root = tempfile::tempdir().unwrap();
    fixture(root.path());
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("private"), b"not a CAD file").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("owner/cad/me/escape")).unwrap();
    let legacy = CadLegacy::personal(
        Arc::new(DirectorySource::open(root.path()).unwrap()),
        "owner",
    )
    .unwrap();
    assert!(legacy.dry_run().await.is_err());
}
