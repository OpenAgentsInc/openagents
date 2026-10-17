use super::*;

fn complete(root: &Path) -> Packet {
    let bytes = b"{\"synthetic_unit_fixture\":true}";
    std::fs::write(root.join("raw.json"), bytes).unwrap();
    let source = Artifact {
        path: "raw.json".into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
    };
    let identity = Identity {
        web_commit: "a".repeat(40),
        computers_commit: "b".repeat(40),
        sdk_commit: "c".repeat(40),
        native_commit: "d".repeat(40),
        native_image_sha256: "a".repeat(64),
        template: format!("fixture/base@sha256:{}", "b".repeat(64)),
        runtime_sha256: "c".repeat(64),
        policy_sha256: "d".repeat(64),
        schema_sha256: "e".repeat(64),
        vcpus: 2,
        memory_mib: 2048,
    };
    let observed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let receipt = Receipt {
        schema: RECEIPT_SCHEMA.into(),
        identity: identity.clone(),
        observed_unix: observed,
        cases: Case::ALL.iter().map(|c| (*c, Status::Passed)).collect(),
        source,
    };
    let bytes = serde_json::to_vec(&receipt).unwrap();
    std::fs::write(root.join("proof.json"), &bytes).unwrap();
    let proof = Proof {
        status: Status::Passed,
        artifact: Some(Artifact {
            path: "proof.json".into(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        }),
    };
    Packet {
        schema: SCHEMA.into(),
        identity: Some(identity),
        observed_unix: Some(observed),
        advertised: Some(Advertised {
            fork: false,
            terminal: false,
            preview: false,
            cross_host_memory_restore: false,
        }),
        accounts: [format!("account:{}", "a".repeat(64))].into(),
        dedicated_test_accounts: true,
        private_management: true,
        rollback_ready: true,
        pricing_enabled: false,
        limits: Some(Limits {
            minimum_samples: 20,
            required_concurrency: 2,
            snapshot_p50_ms: 1999,
            snapshot_p90_ms: 2500,
            wake_p90_ms: 10000,
            maximum_error_permille: 0,
            maximum_service_cost_per_useful_task_micros: 1000,
            currency: "USD".into(),
            maximum_age_seconds: 86400,
        }),
        cases: Case::ALL.iter().map(|c| (*c, proof.clone())).collect(),
        trials: [Kind::Snapshot, Kind::Wake, Kind::Cold]
            .into_iter()
            .flat_map(|kind| {
                (0..20).map(move |i| Trial {
                    kind,
                    cache: if i % 2 == 0 { Cache::Cold } else { Cache::Warm },
                    concurrency: 2,
                    succeeded: true,
                    timings: Timings {
                        first_useful_ms: Some(if kind == Kind::Snapshot {
                            100 + i
                        } else {
                            900 + i
                        }),
                        ..Default::default()
                    },
                })
            })
            .collect(),
        economics: Some(economics::Window {
            duration_ms: 1000,
            worker_vcpus: 8,
            guest_vcpu_ms: Some(4000),
            command_wall_ms: Some(1500),
            task_attempts: 2,
            useful_tasks: 2,
            provider_cost_micros: Some(1000),
            currency: "USD".into(),
        }),
    }
}

#[test]
fn a_pending_packet_never_becomes_qualified_by_omitting_measurements() {
    let packet: Packet = serde_json::from_value(Packet::template()).unwrap();
    let assessment = packet.assess(Path::new(".")).unwrap();
    assert!(!assessment.ready_for_named_group && !assessment.ready_for_dedicated_tests);
    assert_eq!(assessment.incomplete_cases.len(), Case::ALL.len());
    assert_eq!(assessment.snapshot.p50_ms, None);
    assert_eq!(assessment.observed_error_permille, None);
    assert!(assessment.economics.is_none());
    let mut invalid = Packet::template();
    invalid["cases"]["public_sdk"]["status"] = serde_json::json!("skipped");
    assert!(serde_json::from_value::<Packet>(invalid).is_err());
}

#[test]
fn checked_artifacts_and_measured_budgets_only_recommend_a_finite_group() {
    let directory = tempfile::tempdir().unwrap();
    let packet = complete(directory.path());
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(assessment.ready_for_named_group && assessment.ready_for_dedicated_tests);
    assert!(!assessment.broad_admission && !assessment.pricing_enabled);
    assert_eq!(
        (assessment.snapshot.p50_ms, assessment.snapshot.p90_ms),
        (Some(109), Some(117))
    );
    assert_eq!(
        assessment
            .economics
            .unwrap()
            .service_cost_per_useful_task_micros,
        Some(500)
    );
    let mut pricing = packet;
    pricing.pricing_enabled = true;
    assert!(
        !pricing
            .assess(directory.path())
            .unwrap()
            .ready_for_named_group
    );
}

#[test]
fn failed_checks_changed_artifacts_and_slow_samples_block_admission() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet.cases.remove(&Case::DatabaseRestore);
    packet
        .cases
        .get_mut(&Case::CredentialRevocation)
        .unwrap()
        .status = Status::Failed;
    packet.trials[0].succeeded = false;
    packet.trials[0].timings.first_useful_ms = None;
    for trial in &mut packet.trials[1..20] {
        trial.timings.first_useful_ms = Some(9000);
    }
    packet.economics.as_mut().unwrap().provider_cost_micros = None;
    std::fs::write(directory.path().join("proof.json"), b"changed").unwrap();
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.incomplete_cases.contains(&Case::DatabaseRestore));
    assert!(
        assessment
            .failed_cases
            .contains(&Case::CredentialRevocation)
    );
    assert!(!assessment.invalid_artifacts.is_empty());
    for blocker in [
        "insufficient_samples",
        "latency_budget_exceeded",
        "error_budget_exceeded",
        "cost_or_capacity_measurement_missing_or_over_budget",
    ] {
        assert!(assessment.blockers.contains(&blocker));
    }
    assert_eq!(assessment.snapshot.observed_error_permille, Some(50));
}

#[test]
fn paths_and_sample_buckets_cannot_hide_missing_coverage() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet
        .cases
        .get_mut(&Case::PublicSdk)
        .unwrap()
        .artifact
        .as_mut()
        .unwrap()
        .path = "../proof.json".into();
    for trial in &mut packet.trials {
        trial.cache = Cache::Warm;
        trial.concurrency = 1;
    }
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.invalid_artifacts.contains(&Case::PublicSdk));
    assert!(
        assessment
            .blockers
            .contains(&"missing_cache_or_concurrency_coverage")
    );
    packet.accounts.insert("*".into());
    assert!(packet.assess(directory.path()).is_err());
}

#[test]
fn a_single_writer_check_does_not_establish_cross_host_memory_restore() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet
        .advertised
        .as_mut()
        .unwrap()
        .cross_host_memory_restore = true;
    packet.cases.remove(&Case::CrossHostMemoryRestore);
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(
        assessment
            .incomplete_cases
            .contains(&Case::CrossHostMemoryRestore)
    );
    packet.advertised.as_mut().unwrap().preview = true;
    assert!(
        packet
            .assess(directory.path())
            .unwrap()
            .blockers
            .contains(&"unsupported_controls_advertised")
    );
}

#[test]
fn a_valid_hash_cannot_relabel_a_receipt_from_another_deployment() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet.identity.as_mut().unwrap().computers_commit = "f".repeat(40);
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.invalid_artifacts.contains(&Case::PublicSdk));
}

#[test]
fn an_old_report_cannot_authorize_new_admission() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet.observed_unix = Some(packet.observed_unix.unwrap() - 86401);
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(
        assessment
            .blockers
            .contains(&"observation_time_missing_or_expired")
    );
}

#[test]
fn failed_wakes_cannot_be_hidden_by_successful_cold_trials() {
    let directory = tempfile::tempdir().unwrap();
    let mut packet = complete(directory.path());
    packet.limits.as_mut().unwrap().maximum_error_permille = 10;
    let mut failed = packet.trials[20].clone();
    failed.succeeded = false;
    failed.timings.first_useful_ms = None;
    packet.trials.push(failed);
    let mut cold = packet.trials[0].clone();
    cold.kind = Kind::Cold;
    packet.trials.extend(std::iter::repeat_n(cold, 1000));
    let assessment = packet.assess(directory.path()).unwrap();
    assert_eq!(assessment.observed_error_permille, Some(1));
    assert_eq!(assessment.wake.observed_error_permille, Some(48));
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.blockers.contains(&"error_budget_exceeded"));
    for trial in packet.trials.iter_mut().filter(|t| t.kind == Kind::Wake) {
        trial.cache = Cache::Warm;
    }
    assert!(
        packet
            .assess(directory.path())
            .unwrap()
            .blockers
            .contains(&"missing_cache_or_concurrency_coverage")
    );
}

#[test]
fn changed_raw_evidence_cannot_reuse_an_unchanged_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let packet = complete(directory.path());
    std::fs::write(directory.path().join("raw.json"), b"changed").unwrap();
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.invalid_artifacts.contains(&Case::PublicSdk));
}

#[cfg(unix)]
#[test]
fn symlinked_raw_evidence_is_not_followed() {
    let directory = tempfile::tempdir().unwrap();
    let packet = complete(directory.path());
    std::fs::rename(
        directory.path().join("raw.json"),
        directory.path().join("other.json"),
    )
    .unwrap();
    std::os::unix::fs::symlink("other.json", directory.path().join("raw.json")).unwrap();
    let assessment = packet.assess(directory.path()).unwrap();
    assert!(!assessment.ready_for_named_group);
    assert!(assessment.invalid_artifacts.contains(&Case::PublicSdk));
}
