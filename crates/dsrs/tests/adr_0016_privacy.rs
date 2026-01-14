//! ADR-0016: Privacy Defaults for Swarm Dispatch
//!
//! Test coverage for normative rules defined in:
//! docs/adr/ADR-0016-privacy-defaults-swarm-dispatch.md
//!
//! Rules tested:
//! - ADR-0016.R1: Swarm dispatch MUST apply private_repo if no policy explicitly provided
//! - ADR-0016.R2: SwarmDispatcher MUST validate content against active policy before dispatch
//! - ADR-0016.R3: Policy violations MUST either reject or auto-redact (only if permitted)
//! - ADR-0016.R4: Callers MUST explicitly opt into open_source to disable redaction
//! - ADR-0016.R5: PolicyViolation enum variants are stable

use dsrs::privacy::{PolicyViolation, PrivacyPolicy, RedactionMode};

/// ADR-0016.R1: Swarm dispatch MUST apply `private_repo` if no policy is explicitly provided.
///
/// NOTE: This test documents the INTENDED behavior per the ADR. The current implementation
/// does NOT enforce this at the SwarmDispatcher level - it defaults to no policy (None).
/// This test verifies the private_repo preset exists and has the correct properties.
///
/// Implementation gap: SwarmDispatcher::generate() and ::new() should apply
/// private_repo by default, or dispatch_job should apply it when privacy_policy is None.
#[test]
fn test_adr_0016_r1_private_repo_preset_properties() {
    // Verify private_repo preset exists and has restrictive defaults
    let policy = PrivacyPolicy::private_repo();

    // private_repo MUST use path redaction (not None)
    assert!(
        !matches!(policy.redaction.mode, RedactionMode::None),
        "private_repo must apply redaction"
    );

    // private_repo MUST require verification
    assert!(
        policy.require_verification,
        "private_repo must require verification"
    );

    // private_repo MUST NOT allow file paths
    assert!(
        !policy.allow_file_paths,
        "private_repo must not allow file paths"
    );

    // private_repo MUST have a content size limit
    assert!(
        policy.max_content_size.is_some(),
        "private_repo must have a size limit"
    );
}

/// ADR-0016.R1 (negative case): Verify open_source is NOT the default.
/// The open_source preset has no restrictions - callers must explicitly choose it.
#[test]
fn test_adr_0016_r1_open_source_is_permissive() {
    let policy = PrivacyPolicy::open_source();

    // open_source has no redaction
    assert!(
        matches!(policy.redaction.mode, RedactionMode::None),
        "open_source must have no redaction"
    );

    // open_source allows everything
    assert!(!policy.require_verification);
    assert!(policy.allow_file_paths);
    assert!(policy.max_content_size.is_none());
}

/// ADR-0016.R2: SwarmDispatcher MUST validate content against the active policy before dispatch.
///
/// This tests that PrivacyPolicy.validate_content() correctly rejects violations.
#[test]
fn test_adr_0016_r2_content_validation_size_limit() {
    let policy = PrivacyPolicy::private_repo();
    let max_size = policy.max_content_size.unwrap();

    // Content within limit should pass
    let valid_content = "x".repeat(max_size - 1);
    assert!(policy.validate_content(&valid_content).is_ok());

    // Content exceeding limit should fail
    let large_content = "x".repeat(max_size + 1);
    let result = policy.validate_content(&large_content);
    assert!(matches!(
        result,
        Err(PolicyViolation::ContentTooLarge { .. })
    ));
}

/// ADR-0016.R2: File path validation.
#[test]
fn test_adr_0016_r2_content_validation_file_paths() {
    let policy = PrivacyPolicy::private_repo();
    assert!(!policy.allow_file_paths);

    // Content with file paths should fail for private_repo
    let path_content = "Load file from /Users/alice/secret/data.txt";
    let result = policy.validate_content(path_content);
    assert!(matches!(result, Err(PolicyViolation::FilePathsNotAllowed)));

    // Same content should pass for open_source
    let open_policy = PrivacyPolicy::open_source();
    assert!(open_policy.validate_content(path_content).is_ok());
}

/// ADR-0016.R2: Job type allowlist validation.
#[test]
fn test_adr_0016_r2_job_type_allowlist() {
    let policy = PrivacyPolicy::private_repo();

    // private_repo allows only sandbox_run by default
    assert!(policy.is_job_allowed("oa.sandbox_run.v1"));
    assert!(!policy.is_job_allowed("oa.code_chunk_analysis.v1"));
    assert!(!policy.is_job_allowed("arbitrary.job.type"));

    // open_source allows everything (empty allowlist = all allowed)
    let open_policy = PrivacyPolicy::open_source();
    assert!(open_policy.is_job_allowed("oa.sandbox_run.v1"));
    assert!(open_policy.is_job_allowed("oa.code_chunk_analysis.v1"));
    assert!(open_policy.is_job_allowed("arbitrary.job.type"));
}

/// ADR-0016.R3: Policy violations MUST either reject or auto-redact.
///
/// Current implementation: violations always reject (no auto-redact implemented).
/// This test verifies rejection works correctly.
#[test]
fn test_adr_0016_r3_violations_reject() {
    let policy = PrivacyPolicy::private_repo();

    // Size violation -> rejection
    let large_content = "x".repeat(100_000);
    let result = policy.validate_content(&large_content);
    assert!(result.is_err(), "Size violation must be rejected");

    // Path violation -> rejection
    let path_content = "File at /home/user/secret.txt";
    let result = policy.validate_content(path_content);
    assert!(result.is_err(), "Path violation must be rejected");
}

/// ADR-0016.R4: Callers MUST explicitly opt into `open_source` to disable redaction.
///
/// This is a design constraint - verified by checking that:
/// 1. Default policy has redaction enabled
/// 2. Only open_source() explicitly disables redaction
#[test]
fn test_adr_0016_r4_explicit_opt_in_for_no_redaction() {
    // Default struct has permissive settings (this is intentional per ADR)
    // But swarm dispatch layer should apply private_repo

    // private_repo has redaction
    let private = PrivacyPolicy::private_repo();
    assert!(!matches!(private.redaction.mode, RedactionMode::None));

    // paranoid has full redaction
    let paranoid = PrivacyPolicy::paranoid();
    assert!(matches!(paranoid.redaction.mode, RedactionMode::Full));

    // Only open_source disables redaction - must be explicit choice
    let open = PrivacyPolicy::open_source();
    assert!(matches!(open.redaction.mode, RedactionMode::None));
}

/// ADR-0016.R5: PolicyViolation enum variants are stable.
///
/// This test uses exhaustive matching to catch any changes to the enum.
/// If a new variant is added, this test will fail to compile, signaling
/// that the ADR needs review.
#[test]
fn test_adr_0016_r5_policy_violation_variants_stable() {
    // Test each variant exists and can be constructed
    let violations = vec![
        PolicyViolation::JobTypeNotAllowed("test".to_string()),
        PolicyViolation::UntrustedProvider("test".to_string()),
        PolicyViolation::ContentTooLarge { size: 100, max: 50 },
        PolicyViolation::FilePathsNotAllowed,
        PolicyViolation::VerificationRequired,
    ];

    // Exhaustive match to catch any new variants at compile time
    for v in violations {
        match v {
            PolicyViolation::JobTypeNotAllowed(t) => {
                assert!(!t.is_empty());
            }
            PolicyViolation::UntrustedProvider(p) => {
                assert!(!p.is_empty());
            }
            PolicyViolation::ContentTooLarge { size, max } => {
                assert!(size > max);
            }
            PolicyViolation::FilePathsNotAllowed => {}
            PolicyViolation::VerificationRequired => {}
            // If a new variant is added, this match will be non-exhaustive
            // and the test will fail to compile
        }
    }
}

/// ADR-0016.R5: PolicyViolation Display implementation.
#[test]
fn test_adr_0016_r5_policy_violation_display() {
    // Verify all variants have meaningful display strings
    let violations = vec![
        PolicyViolation::JobTypeNotAllowed("test.job".to_string()),
        PolicyViolation::UntrustedProvider("npub123".to_string()),
        PolicyViolation::ContentTooLarge {
            size: 100000,
            max: 50000,
        },
        PolicyViolation::FilePathsNotAllowed,
        PolicyViolation::VerificationRequired,
    ];

    for v in violations {
        let msg = format!("{}", v);
        assert!(!msg.is_empty(), "Display should not be empty");
        assert!(
            msg.len() > 10,
            "Display should be descriptive: {}",
            msg
        );
    }
}

/// Verify preset escalation path: open_source < private_repo < paranoid
#[test]
fn test_preset_escalation_path() {
    let open = PrivacyPolicy::open_source();
    let private = PrivacyPolicy::private_repo();
    let paranoid = PrivacyPolicy::paranoid();

    // Redaction escalates
    assert!(matches!(open.redaction.mode, RedactionMode::None));
    assert!(matches!(private.redaction.mode, RedactionMode::PathsOnly));
    assert!(matches!(paranoid.redaction.mode, RedactionMode::Full));

    // Size limits get stricter
    assert!(open.max_content_size.is_none());
    let private_limit = private.max_content_size.unwrap();
    let paranoid_limit = paranoid.max_content_size.unwrap();
    assert!(
        paranoid_limit < private_limit,
        "paranoid should have stricter size limit"
    );

    // Verification requirements escalate
    assert!(!open.require_verification);
    assert!(private.require_verification);
    assert!(paranoid.require_verification);
}
