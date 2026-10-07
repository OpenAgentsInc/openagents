use super::*;
use receipts::brainstorm_pilot::Basis;

fn package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/brainstorm")
}
fn private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if path.is_dir() { 0o700 } else { 0o600 }),
        )
        .unwrap();
    }
}
fn write(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    private(path);
}
fn fixture() -> (tempfile::TempDir, PathBuf, Pilot) {
    let work = tempfile::tempdir().unwrap();
    private(work.path());
    for entry in std::fs::read_dir(package().join("examples")).unwrap() {
        let entry = entry.unwrap();
        let target = work.path().join(entry.file_name());
        std::fs::copy(entry.path(), &target).unwrap();
        private(&target);
    }
    let input = work.path().join("pilot-fixture.json");
    let record = serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    (work, input, record)
}

#[test]
fn exact_key_fixture_check_preserves_absent_zero_and_source_without_activation() {
    let (work, input, _) = fixture();
    let value = verify(&input, work.path(), 1000).unwrap();
    assert_eq!(value["verified_source_files"], 3);
    assert_eq!(value["summary"]["lookup_coverage"][0]["coverage"], "absent");
    assert_eq!(
        value["summary"]["lookup_coverage"][1]["coverage"],
        "unknown"
    );
    assert_eq!(value["summary"]["settled_payment_claims"], 0);
    assert_eq!(
        value["summary"]["independently_verified_paid_conversion"],
        false
    );
    assert_eq!(value["summary"]["authority_granted"], false);
    let output = value.to_string();
    assert!(!output.contains("synthetic publisher"));
    assert!(!output.contains("79be667e"));
    assert!(!output.contains("input_ref"));
    let expired = verify(&input, work.path(), 1_000_000).unwrap();
    assert_eq!(expired["summary"]["lookup_coverage"][1]["expired"], true);
    assert!(verify(&input, work.path(), 2_524_608_000_000).is_err());
}

#[test]
fn copied_scores_recipient_times_and_house_cannot_replace_pinned_projection() {
    let (work, input, record) = fixture();
    for field in ["influence", "origin", "house_pubkey", "input_digest"] {
        let mut value = serde_json::to_value(&record).unwrap();
        value["lookups"][1][field] = match field {
            "influence" => json!(1.0),
            "origin" => json!("https://other.example"),
            _ => json!("a".repeat(64)),
        };
        if field == "influence" {
            value["lookups"][1]["coverage"] = json!("reported");
        }
        write(&input, &value);
        assert!(verify(&input, work.path(), 1000).is_err(), "{field}");
    }
}

#[test]
fn source_tampering_missing_files_bounds_and_secret_fields_refuse() {
    let (work, input, mut record) = fixture();
    std::fs::write(work.path().join("rank-zero.json"), b"{}").unwrap();
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("digest")
    );
    record.lookups[1].observation.path = "missing.json".into();
    write(&input, &record);
    assert!(verify(&input, work.path(), 1000).is_err());
    let mut value = serde_json::to_value(&record).unwrap();
    value["query"] = json!("private workspace content");
    write(&input, &value);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("JSON")
    );
    std::fs::write(&input, vec![b' '; pilot::MAX_RECORD_BYTES + 1]).unwrap();
    assert!(verify(&input, work.path(), 1000).is_err());
}

#[test]
fn context_projections_cannot_be_pinned_as_complete_lookup_sources() {
    let (work, input, record) = fixture();
    let path = work.path().join("search-absent.json");
    let observation: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut flattened = observation.clone();
    flattened["context_truncated"] = json!(true);
    flattened["omitted_subjects"] = json!(1);
    for value in [
        json!({"observation":observation,"context_truncated":true,"omitted_subjects":1}),
        flattened,
    ] {
        write(&path, &value);
        let mut updated = record.clone();
        updated.lookups[0].observation.sha256 = sha256(&std::fs::read(&path).unwrap());
        write(&input, &updated);
        assert!(
            verify(&input, work.path(), 1000)
                .unwrap_err()
                .contains("complete normalized")
        );
    }
}

#[cfg(unix)]
#[test]
fn public_permissions_links_and_traversal_refuse_private_capture() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (work, input, mut record) = fixture();
    std::fs::set_permissions(&input, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(verify(&input, work.path(), 1000).is_err());
    private(&input);
    std::fs::remove_file(work.path().join("rank-zero.json")).unwrap();
    symlink(
        work.path().join("search-absent.json"),
        work.path().join("rank-zero.json"),
    )
    .unwrap();
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("links")
    );
    record.lookups[1].observation.path = "../outside.json".into();
    write(&input, &record);
    assert!(verify(&input, work.path(), 1000).is_err());
}

#[test]
fn profile_signature_approval_and_exact_target_are_checked_without_publication() {
    let (work, input, mut record) = fixture();
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let link = "https://openagents.example/public-release";
    let event = signer.sign(1, 0, vec![], json!({"about":link}).to_string());
    write(&work.path().join("profile.json"), &event);
    let evidence = Reference {
        path: "profile.json".into(),
        sha256: sha256(&std::fs::read(work.path().join("profile.json")).unwrap()),
    };
    record.profile_event = Some(evidence.clone());
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("approval")
    );
    record.profile_approval = Some(record.consent.evidence.clone());
    record.approved_public_links = vec![link.into()];
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("exact pilot target")
    );
    record.target_pubkey = signer.pubkey().into();
    record.lookups.clear();
    record.basis = Basis::OperatorRecorded;
    write(&input, &record);
    assert!(verify(&input, work.path(), 1000).is_ok());
    record.approved_public_links = vec!["https://unapproved.example".into()];
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("approved public link")
    );
    record.approved_public_links.clear();
    let mut tampered = serde_json::to_value(event).unwrap();
    tampered["content"] = json!("{}");
    write(&work.path().join("profile.json"), &tampered);
    record.profile_event.as_mut().unwrap().sha256 =
        sha256(&std::fs::read(work.path().join("profile.json")).unwrap());
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("signature")
    );
}

#[test]
fn checker_accepts_only_explicit_files_and_no_network_or_side_effect_options() {
    for words in [
        vec!["check"],
        vec!["publish"],
        vec![
            "check",
            "--input",
            "file",
            "--sources",
            "dir",
            "--origin",
            "https://example.com",
        ],
    ] {
        assert!(check(&words.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
    }
}

#[test]
fn canonical_hex_that_is_not_a_curve_point_refuses_before_observation_projection() {
    let (work, input, mut record) = fixture();
    record.target_pubkey = "ff".repeat(32);
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("valid public key")
    );
}

#[test]
fn public_profile_link_match_is_exact_and_decodes_json_strings() {
    let link = "https://openagents.example/release";
    assert!(has_exact_link(&json!({"website":link}), link));
    assert!(has_exact_link(
        &json!({"about":format!("Public release {link}")}),
        link
    ));
    assert!(!has_exact_link(
        &json!({"website":format!("{link}-unapproved")}),
        link
    ));
}

#[test]
fn an_empty_pinned_consent_file_is_missing_evidence() {
    let (work, input, mut record) = fixture();
    std::fs::write(work.path().join("consent-fixture.txt"), []).unwrap();
    record.consent.evidence.sha256 = sha256(&[]);
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("empty")
    );
}

#[test]
fn a_valid_signature_does_not_make_an_invalid_wire_profile_admissible() {
    let (work, input, mut record) = fixture();
    let signer = nostr::domain::RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let event = signer.sign(1, 0, vec![nostr::domain::Tag::new(vec![])], "{}".into());
    event.validate_crypto().unwrap();
    write(&work.path().join("profile.json"), &event);
    record.target_pubkey = signer.pubkey().into();
    record.lookups.clear();
    record.profile_approval = Some(record.consent.evidence.clone());
    record.profile_event = Some(Reference {
        path: "profile.json".into(),
        sha256: sha256(&std::fs::read(work.path().join("profile.json")).unwrap()),
    });
    write(&input, &record);
    assert!(
        verify(&input, work.path(), 1000)
            .unwrap_err()
            .contains("wire structure")
    );
}
