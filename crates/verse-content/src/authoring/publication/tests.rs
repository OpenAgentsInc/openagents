use super::*;
use crate::authoring::{Workspace, tests::input};
use verse_engine::{
    assets::Pack,
    inventory::{Asset, AssetId, Binding, Inventory, License, Origin, fingerprint},
};
fn key(n: u8) -> Keypair {
    Keypair::from_secret_key(
        &Secp256k1::new(),
        &secp256k1::SecretKey::from_byte_array([n; 32]).unwrap(),
    )
}
fn fixture(root: &Path, license: License) -> Workspace {
    let source = root.join("input");
    input(&source);
    let mut pack: Pack =
        serde_json::from_slice(&std::fs::read(source.join("pack.json")).unwrap()).unwrap();
    let id = |s: &str| AssetId::new(s).unwrap();
    let origin = id("fixture:source");
    let mut assets = vec![Asset {
        id: origin.clone(),
        binding: Binding::Source {
            origin: Origin {
                creator: "OpenAgents procedural acceptance fixture".into(),
                license,
                revision: workspace::hash(b"procedural-fixture-v1"),
                format: "rust".into(),
            },
        },
        sha256: workspace::hash(b"procedural-fixture-v1"),
        bytes: 21,
        dependencies: vec![],
    }];
    for (name, model) in &pack.models {
        let (sha256, bytes) = fingerprint(model).unwrap();
        assets.push(Asset {
            id: id(&format!("fixture:model/{name}")),
            binding: Binding::Model { key: name.clone() },
            sha256,
            bytes,
            dependencies: vec![origin.clone()],
        });
    }
    for (slot, texture) in pack.textures.iter().enumerate() {
        assets.push(Asset {
            id: id(&format!("fixture:texture/{slot}")),
            binding: Binding::Texture { slot },
            sha256: texture.sha256.clone(),
            bytes: std::fs::metadata(source.join(&texture.file)).unwrap().len(),
            dependencies: vec![origin.clone()],
        });
    }
    pack.inventory = Some(Inventory {
        version: 1,
        compiler: id("fixture:compiler"),
        compiler_revision: workspace::hash(b"fixture-compiler-v1"),
        assets,
    });
    std::fs::write(source.join("pack.json"), serde_json::to_vec(&pack).unwrap()).unwrap();
    Workspace::init(&source, &root.join("workspace"), "fixture".into()).unwrap()
}
#[test]
fn signed_publication_requires_separate_review_and_live_revocation_checks() {
    let root = tempfile::tempdir().unwrap();
    let work = fixture(root.path(), License::Apache2);
    let path = root.path().join("release");
    let release = release::build(&work, &path).unwrap();
    assert!(!path.join("host-template.json").exists());
    assert!(!path.join("preview.json").exists());
    assert!(!path.join("author.lock").exists());
    let publisher = key(11);
    let public = publisher.x_only_public_key().0.serialize();
    let submission = Submission::sign([7; 32], release.id(), &publisher).unwrap();
    let book_path = root.path().join("operator");
    let mut book = Book::open(&book_path, [7; 32]).unwrap();
    assert!(Book::open(&book_path, [7; 32]).is_err());
    assert!(book.submit(&submission, &release).is_err());
    book.publisher(public, true, "Approved test publisher enrollment")
        .unwrap();
    assert!(book.resolve(public, release.id(), &path).is_err());
    let pending = book.submit(&submission, &release).unwrap();
    assert_eq!(pending.status, Status::Pending);
    assert!(book.resolve(public, release.id(), &path).is_err());
    let mut forged = submission.clone();
    forged.signature[0] ^= 1;
    assert!(book.submit(&forged, &release).is_err());
    let other = Submission::sign([8; 32], release.id(), &publisher).unwrap();
    assert!(book.submit(&other, &release).is_err());
    assert!(
        book.review(public, &release, 2, Status::Approved, "Review")
            .is_err()
    );
    let approved = book
        .review(
            public,
            &release,
            1,
            Status::Approved,
            "Distribution approved for fixture",
        )
        .unwrap();
    assert_eq!(book.submit(&submission, &release).unwrap(), approved);
    let served = book.resolve(public, release.id(), &path).unwrap();
    drop(book);
    let mut book = Book::open(&book_path, [7; 32]).unwrap();
    assert!(book.resolve(public, release.id(), &path).is_ok());
    book.publisher(public, false, "Publisher suspended")
        .unwrap();
    assert!(book.resolve(public, release.id(), &path).is_err());
    assert!(
        book.review(public, &release, 2, Status::Approved, "Review")
            .is_err()
    );
    book.publisher(public, true, "Publisher reinstated")
        .unwrap();
    book.review(public, &release, 2, Status::Revoked, "Artifact revoked")
        .unwrap();
    assert!(book.resolve(public, release.id(), &path).is_err());
    assert!(
        book.review(public, &release, 3, Status::Approved, "Review")
            .is_err()
    );
    assert_eq!(
        book.submit(&submission, &release).unwrap().status,
        Status::Revoked
    );
    // A serving snapshot cannot change when the original directory changes.
    let original = served.file("fixture.png").unwrap().to_vec();
    std::fs::write(path.join("fixture.png"), b"substituted").unwrap();
    assert_eq!(served.file("fixture.png").unwrap(), original);
    assert!(release::verify(&path).is_err());
    drop(book);
    assert!(Book::open(&book_path, [8; 32]).is_err());
    let book = Book::open(&book_path, [7; 32]).unwrap();
    assert!(book.resolve(public, release.id(), &path).is_err());
}
#[test]
fn local_builds_do_not_grant_distribution_and_unused_research_is_refused() {
    for license in [License::Research, License::OwnerSuppliedLocal] {
        let root = tempfile::tempdir().unwrap();
        let work = fixture(root.path(), license);
        assert_eq!(work.build().is_ok(), license == License::OwnerSuppliedLocal);
        assert!(release::build(&work, &root.path().join("release")).is_err());
        assert!(!root.path().join("release").exists());
    }
    let root = tempfile::tempdir().unwrap();
    let work = fixture(root.path(), License::Apache2);
    let mut pack = work.base.clone();
    pack.inventory.as_mut().unwrap().assets.push(Asset {
        id: AssetId::new("fixture:unused-research").unwrap(),
        binding: Binding::Source {
            origin: Origin {
                creator: "Retained reference".into(),
                license: License::Research,
                revision: workspace::hash(b"reference"),
                format: "reference".into(),
            },
        },
        sha256: workspace::hash(b"reference"),
        bytes: 9,
        dependencies: vec![],
    });
    // Shipping admission covers unused retained declarations as well as runtime roots.
    assert!(release::admission(&pack).is_err());
}
#[test]
fn release_refuses_unsealed_files_symlinks_and_incompatible_profiles() {
    let root = tempfile::tempdir().unwrap();
    let work = fixture(root.path(), License::Apache2);
    let path = root.path().join("release");
    release::build(&work, &path).unwrap();
    std::fs::write(path.join("private-studio.json"), b"secret").unwrap();
    assert!(release::verify(&path).is_err());
    std::fs::remove_file(path.join("private-studio.json")).unwrap();
    let file = path.join("release.json");
    let original = std::fs::read(&file).unwrap();
    let pack_file = path.join("pack.json");
    let original_pack = std::fs::read(&pack_file).unwrap();
    let mut pack: serde_json::Value = serde_json::from_slice(&original_pack).unwrap();
    pack["private_studio"] = serde_json::json!({"log":"must not be silently preserved"});
    let altered_pack = serde_json::to_vec(&pack).unwrap();
    let mut resealed: release::Manifest = serde_json::from_slice(&original).unwrap();
    resealed.files.insert(
        "pack.json".into(),
        release::File {
            sha256: workspace::hash(&altered_pack),
            bytes: altered_pack.len() as u64,
        },
    );
    std::fs::write(&pack_file, altered_pack).unwrap();
    std::fs::write(&file, serde_json::to_vec(&resealed).unwrap()).unwrap();
    assert!(release::verify(&path).is_err());
    let mut hidden_model = pack["models"]["adventurer"].clone();
    hidden_model["private_studio"] = serde_json::json!({"log":"hidden in a discarded duplicate"});
    let duplicated = String::from_utf8(original_pack.clone())
        .unwrap()
        .replacen(
            "\"models\": {",
            &format!(
                "\"models\": {{\"adventurer\":{},",
                serde_json::to_string(&hidden_model).unwrap()
            ),
            1,
        )
        .into_bytes();
    resealed.files.insert(
        "pack.json".into(),
        release::File {
            sha256: workspace::hash(&duplicated),
            bytes: duplicated.len() as u64,
        },
    );
    std::fs::write(&pack_file, duplicated).unwrap();
    std::fs::write(&file, serde_json::to_vec(&resealed).unwrap()).unwrap();
    assert!(release::verify(&path).is_err());
    std::fs::write(&pack_file, original_pack).unwrap();
    std::fs::write(&file, &original).unwrap();
    let mut manifest: release::Manifest = serde_json::from_slice(&original).unwrap();
    manifest.profile.wire += 1;
    std::fs::write(&file, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(release::verify(&path).is_err());
    std::fs::write(&file, &original).unwrap();
    #[cfg(unix)]
    {
        let texture = std::fs::read(path.join("fixture.png")).unwrap();
        std::fs::write(root.path().join("external.png"), texture).unwrap();
        std::fs::remove_file(path.join("fixture.png")).unwrap();
        std::os::unix::fs::symlink(root.path().join("external.png"), path.join("fixture.png"))
            .unwrap();
        assert!(release::verify(&path).is_err());
    }
}

#[test]
fn withdrawal_does_not_require_the_old_files_and_uncertain_review_fails_closed() {
    let root = tempfile::tempdir().unwrap();
    let work = fixture(root.path(), License::Apache2);
    let path = root.path().join("release");
    let release = release::build(&work, &path).unwrap();
    let pair = key(11);
    let public = pair.x_only_public_key().0.serialize();
    let book_path = root.path().join("operator");
    let mut book = Book::open(&book_path, [7; 32]).unwrap();
    book.publisher(public, true, "Fixture enrollment").unwrap();
    let submission = Submission::sign([7; 32], release.id(), &pair).unwrap();
    book.submit(&submission, &release).unwrap();
    std::fs::create_dir(book_path.join("publication.pending")).unwrap();
    assert!(
        book.review(public, &release, 1, Status::Approved, "Review")
            .is_err()
    );
    assert!(book.resolve(public, release.id(), &path).is_err());
    assert!(book.publisher(public, true, "Retry").is_err());
    drop(book);
    std::fs::remove_dir(book_path.join("publication.pending")).unwrap();
    let mut book = Book::open(&book_path, [7; 32]).unwrap();
    assert!(book.resolve(public, release.id(), &path).is_err());
    book.review(
        public,
        &release,
        1,
        Status::Approved,
        "Distribution approved",
    )
    .unwrap();
    assert!(book.resolve(public, release.id(), &path).is_ok());
    std::fs::create_dir(book_path.join("publication.pending")).unwrap();
    assert!(
        book.revoke(public, release.id(), 2, "Withdrawal with storage failure")
            .is_err()
    );
    assert!(book.resolve(public, release.id(), &path).is_err());
    drop(book);
    std::fs::remove_dir(book_path.join("publication.pending")).unwrap();
    let mut book = Book::open(&book_path, [7; 32]).unwrap();
    std::fs::remove_dir_all(&path).unwrap();
    book.revoke(public, release.id(), 2, "Withdraw missing artifact")
        .unwrap();
    assert!(
        book.review(public, &release, 3, Status::Approved, "Cannot resurrect")
            .is_err()
    );
    assert!(book.resolve(public, release.id(), &path).is_err());
}
