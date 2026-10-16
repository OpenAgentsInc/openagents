//! A public research release, two machine rebuilds, and real Rekor inclusion.
use att_release::{Evidence, Policy, approvals::Decision, check_release, rekor};
use nostr::domain::Event;

#[test]
fn the_reproduced_signet_probe_release_verifies_offline() {
    let release: Event =
        serde_json::from_str(include_str!("fixtures/layertree-signet/release.json")).unwrap();
    let head = serde_json::from_str(include_str!("fixtures/layertree-signet/head.json")).unwrap();
    let entry =
        serde_json::from_str(include_str!("fixtures/layertree-signet/rekor-entry.json")).unwrap();
    let rebuilds = [
        serde_json::from_str(include_str!("fixtures/layertree-signet/local-rebuild.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/layertree-signet/gcp-rebuild.json")).unwrap(),
    ];
    let key = rekor::LogKey::sigstore().unwrap();
    let evidence = Evidence {
        release: &release,
        head: &head,
        entry: Some(&entry),
        consistency: None,
        rebuilds: &rebuilds,
        approvals: None,
    };
    let policy = Policy {
        publisher: "fc4d6cd08891e19627bca90ae667097d261b8290e54ada0bb7e959f06a390f8d",
        workload: "layertree-signet-probe",
        log: Some(&key),
        min_rebuilds: 2,
        seen_generation: None,
    };
    assert_eq!(
        check_release(
            &evidence,
            &policy,
            &mut rekor::Witness::default(),
            1_792_137_325
        )
        .unwrap(),
        Decision::Admitted
    );
    let body = nostr::att::parse_release(&release).unwrap();
    assert_eq!(
        body.source.commit,
        "2bc0a8f11e79da4aaf3b70e19f43152a9b429108"
    );
    assert_eq!(
        body.image.digest,
        "sha256:97ebee658897e3e655c18e01c043b6ab9394ff63ab06b056aea6a6db0127671f"
    );
    assert_eq!(entry.log_index, 3_275_843_513);
    assert!(
        check_release(
            &evidence,
            &policy,
            &mut rekor::Witness::default(),
            1_792_136_800
        )
        .is_err()
    );
}
