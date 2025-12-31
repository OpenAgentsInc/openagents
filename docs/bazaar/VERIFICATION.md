# Verification Protocols

How buyers verify work products before paying providers.

---

## Overview

The marketplace uses **pay-after-verify** semantics. Buyers only pay Lightning invoices after verifying that work products meet acceptance criteria.

Every job produces:
1. **Primary artifact** - The work product (patch, review, index, etc.)
2. **Artifact hash** - SHA256 of the artifact for integrity verification
3. **Trajectory ID** - Link to execution log for auditing

---

## Verification Flow

```
┌──────────────────────────────────────────────────────────────────────┐
│                        VERIFICATION FLOW                              │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  1. RECEIVE RESULT                                                   │
│     └─► Parse NIP-90 result event (kind 6xxx)                        │
│                                                                       │
│  2. HASH VERIFICATION                                                │
│     └─► SHA256(content) == claimed hash?                            │
│                                                                       │
│  3. ARTIFACT VERIFICATION (job-type specific)                        │
│     ├─► PatchGen: Apply patch, run tests                            │
│     ├─► CodeReview: Validate schema, check references               │
│     ├─► RepoIndex: Validate schema, spot-check queries              │
│     └─► SandboxRun: Check exit code                                  │
│                                                                       │
│  4. TRAJECTORY VERIFICATION                                          │
│     └─► Fetch trajectory, verify signature, check completeness      │
│                                                                       │
│  5. DECISION                                                          │
│     ├─► All checks pass → PAY INVOICE                               │
│     └─► Any check fails → OPEN DISPUTE                              │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Verification by Job Type

### PatchGen Verification

```rust
/// Verify a PatchGen result
pub fn verify_patch(
    result: &PatchGenResult,
    repo_path: &Path,
    test_command: &str,
    verification_commands: &[String],
) -> VerificationResult {
    let mut checks = Vec::new();

    // 1. Hash verification
    let computed_hash = sha256(result.patch.as_bytes());
    checks.push(VerificationCheck {
        name: "hash_matches".to_string(),
        passed: computed_hash == result.patch_sha256,
        details: if computed_hash != result.patch_sha256 {
            Some(format!("Expected {}, got {}", result.patch_sha256, computed_hash))
        } else {
            None
        },
    });

    // 2. Patch applies cleanly
    let apply_result = Command::new("git")
        .args(["apply", "--check", "-"])
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .output();

    let applies_cleanly = match apply_result {
        Ok(output) => output.status.success(),
        Err(e) => {
            checks.push(VerificationCheck {
                name: "applies_cleanly".to_string(),
                passed: false,
                details: Some(format!("Failed to run git apply: {}", e)),
            });
            return VerificationResult::from_checks(checks);
        }
    };

    checks.push(VerificationCheck {
        name: "applies_cleanly".to_string(),
        passed: applies_cleanly,
        details: None,
    });

    if !applies_cleanly {
        return VerificationResult::from_checks(checks);
    }

    // 3. Apply patch and run tests
    let _ = Command::new("git")
        .args(["apply", "-"])
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .output();

    let test_result = Command::new("sh")
        .args(["-c", test_command])
        .current_dir(repo_path)
        .output();

    match test_result {
        Ok(output) => {
            checks.push(VerificationCheck {
                name: "tests_pass".to_string(),
                passed: output.status.success(),
                details: Some(format!("Exit code: {}", output.status.code().unwrap_or(-1))),
            });
        }
        Err(e) => {
            checks.push(VerificationCheck {
                name: "tests_pass".to_string(),
                passed: false,
                details: Some(format!("Failed to run tests: {}", e)),
            });
        }
    }

    // 4. Run additional verification commands
    for cmd in verification_commands {
        let result = Command::new("sh")
            .args(["-c", cmd])
            .current_dir(repo_path)
            .output();

        let (passed, details) = match result {
            Ok(output) => (output.status.success(), None),
            Err(e) => (false, Some(e.to_string())),
        };

        checks.push(VerificationCheck {
            name: format!("verification:{}", cmd.split_whitespace().next().unwrap_or("unknown")),
            passed,
            details,
        });
    }

    // Revert patch after verification
    let _ = Command::new("git")
        .args(["checkout", "."])
        .current_dir(repo_path)
        .output();

    VerificationResult::from_checks(checks)
}
```

**Acceptance criteria:**
- Hash matches claimed hash
- Patch applies cleanly to target ref
- Test command exits with code 0
- All verification commands pass

### CodeReview Verification

```rust
/// Verify a CodeReview result
pub fn verify_review(
    result: &CodeReviewResult,
    diff_content: &str,
) -> VerificationResult {
    let mut checks = Vec::new();

    // 1. Hash verification
    let review_json = serde_json::to_vec(&result.review).unwrap();
    let computed_hash = sha256(&review_json);
    checks.push(VerificationCheck {
        name: "hash_matches".to_string(),
        passed: computed_hash == result.review_sha256,
        details: None,
    });

    // 2. Schema validation
    let schema_valid = validate_review_schema(&result.review);
    checks.push(VerificationCheck {
        name: "schema_valid".to_string(),
        passed: schema_valid,
        details: None,
    });

    // 3. Non-trivial content
    let non_trivial = !result.review.summary.is_empty()
        && (result.review.issues.len() > 0
            || result.review.suggestions.len() > 0
            || result.review.highlights.len() > 0);
    checks.push(VerificationCheck {
        name: "non_trivial".to_string(),
        passed: non_trivial,
        details: None,
    });

    // 4. File references exist in diff
    let diff_files = extract_files_from_diff(diff_content);
    let mut all_refs_valid = true;

    for issue in &result.review.issues {
        if !diff_files.contains(&issue.file) {
            all_refs_valid = false;
            break;
        }
    }

    for suggestion in &result.review.suggestions {
        if !diff_files.contains(&suggestion.file) {
            all_refs_valid = false;
            break;
        }
    }

    checks.push(VerificationCheck {
        name: "references_valid".to_string(),
        passed: all_refs_valid,
        details: if !all_refs_valid {
            Some("Some file references not found in diff".to_string())
        } else {
            None
        },
    });

    VerificationResult::from_checks(checks)
}

fn validate_review_schema(review: &StructuredReview) -> bool {
    // Validate approval_status
    let valid_statuses = ["approve", "request_changes", "comment"];
    if !valid_statuses.contains(&review.approval_status.as_str()) {
        return false;
    }

    // Validate issue severities
    let valid_severities = ["critical", "high", "medium", "low", "info"];
    for issue in &review.issues {
        if !valid_severities.contains(&issue.severity.as_str()) {
            return false;
        }
    }

    true
}
```

**Acceptance criteria:**
- Hash matches claimed hash
- JSON conforms to StructuredReview schema
- Content is non-trivial (has summary + at least one issue/suggestion/highlight)
- All file references exist in the diff

### RepoIndex Verification

```rust
/// Verify a RepoIndex result
pub fn verify_index(
    result: &RepoIndexResult,
    sample_queries: &[IndexQuery],
) -> VerificationResult {
    let mut checks = Vec::new();

    // 1. Hash verification
    let computed_hash = sha256(&result.index_bundle);
    checks.push(VerificationCheck {
        name: "hash_matches".to_string(),
        passed: computed_hash == result.index_sha256,
        details: None,
    });

    // 2. Bundle decompression
    let bundle = match decompress_bundle(&result.index_bundle) {
        Ok(b) => b,
        Err(e) => {
            checks.push(VerificationCheck {
                name: "bundle_valid".to_string(),
                passed: false,
                details: Some(format!("Failed to decompress: {}", e)),
            });
            return VerificationResult::from_checks(checks);
        }
    };

    checks.push(VerificationCheck {
        name: "bundle_valid".to_string(),
        passed: true,
        details: None,
    });

    // 3. Schema validation
    let schema_valid = validate_index_schema(&bundle);
    checks.push(VerificationCheck {
        name: "schema_valid".to_string(),
        passed: schema_valid,
        details: None,
    });

    // 4. Spot-check queries (if embeddings included)
    if let Some(embeddings) = &bundle.embeddings {
        let mut queries_passed = 0;
        for query in sample_queries {
            if let Some(results) = query_embeddings(embeddings, &query.text, query.top_k) {
                if !results.is_empty() {
                    queries_passed += 1;
                }
            }
        }

        checks.push(VerificationCheck {
            name: "spot_check_queries".to_string(),
            passed: queries_passed >= sample_queries.len() / 2,
            details: Some(format!("{}/{} queries returned results", queries_passed, sample_queries.len())),
        });
    }

    VerificationResult::from_checks(checks)
}
```

**Acceptance criteria:**
- Hash matches claimed hash
- Bundle decompresses successfully
- Schema validates (correct structure for index type)
- Spot-check queries return plausible results

### SandboxRun Verification

```rust
/// Verify a SandboxRun result
pub fn verify_sandbox(result: &SandboxRunResult) -> VerificationResult {
    let mut checks = Vec::new();

    // 1. Hash verification
    let computed_hash = sha256(result.output.as_bytes());
    checks.push(VerificationCheck {
        name: "hash_matches".to_string(),
        passed: computed_hash == result.output_sha256,
        details: None,
    });

    // 2. Exit code check
    checks.push(VerificationCheck {
        name: "exit_code".to_string(),
        passed: result.exit_code == 0,
        details: Some(format!("Exit code: {}", result.exit_code)),
    });

    // 3. Resource limits respected
    checks.push(VerificationCheck {
        name: "within_time_limit".to_string(),
        passed: result.duration_ms <= result.requested_timeout_ms,
        details: None,
    });

    checks.push(VerificationCheck {
        name: "within_memory_limit".to_string(),
        passed: result.peak_memory_mb <= result.requested_memory_mb,
        details: None,
    });

    VerificationResult::from_checks(checks)
}
```

**Acceptance criteria:**
- Hash matches claimed hash
- Exit code is 0 (or expected value)
- Execution stayed within resource limits

---

## Trajectory Verification

Every job must produce a trajectory for auditing:

```rust
/// Verify job trajectory
pub fn verify_trajectory(
    trajectory_id: &str,
    provider_pubkey: &str,
    expected_job_id: &str,
) -> TrajectoryVerification {
    // 1. Fetch trajectory from provider or relay
    let trajectory = match fetch_trajectory(trajectory_id) {
        Ok(t) => t,
        Err(e) => {
            return TrajectoryVerification {
                valid: false,
                error: Some(format!("Failed to fetch trajectory: {}", e)),
                ..Default::default()
            };
        }
    };

    // 2. Verify header
    let header_valid = trajectory.header.provider_pubkey == provider_pubkey
        && trajectory.header.job_id.contains(expected_job_id);

    // 3. Verify signature chain
    let signatures_valid = verify_trajectory_signatures(&trajectory);

    // 4. Check completeness (has start and end markers)
    let complete = trajectory.entries.iter().any(|e| e.is_start())
        && trajectory.entries.iter().any(|e| e.is_end());

    // 5. Check timing consistency
    let timing_valid = verify_timing_consistency(&trajectory);

    TrajectoryVerification {
        valid: header_valid && signatures_valid && complete && timing_valid,
        header_valid,
        signatures_valid,
        complete,
        timing_valid,
        tool_calls: count_tool_calls(&trajectory),
        thinking_blocks: count_thinking_blocks(&trajectory),
        error: None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrajectoryVerification {
    pub valid: bool,
    pub header_valid: bool,
    pub signatures_valid: bool,
    pub complete: bool,
    pub timing_valid: bool,
    pub tool_calls: usize,
    pub thinking_blocks: usize,
    pub error: Option<String>,
}
```

---

## Dispute Protocol

When verification fails, buyers can open disputes:

### Dispute Creation

```rust
/// Create a dispute for a failed verification
pub fn create_dispute(
    job_id: &str,
    verification: &VerificationResult,
    buyer_keypair: &Keypair,
) -> DisputeEvent {
    let trigger = determine_trigger(&verification);

    DisputeEvent {
        kind: 39300, // Dispute event kind
        pubkey: buyer_keypair.public_key(),
        content: serde_json::json!({
            "job_id": job_id,
            "trigger": trigger,
            "evidence_hash": verification.evidence_hash,
            "failed_checks": verification.failed_checks(),
        }).to_string(),
        tags: vec![
            ["e", job_id, "", "dispute"],
            ["trigger", &trigger.to_string()],
        ],
    }
}

fn determine_trigger(verification: &VerificationResult) -> DisputeTrigger {
    for check in &verification.checks {
        if !check.passed {
            match check.name.as_str() {
                "hash_matches" => return DisputeTrigger::HashMismatch,
                "applies_cleanly" => return DisputeTrigger::PatchFailsToApply,
                "tests_pass" => return DisputeTrigger::TestsFailAfterPatch,
                "trajectory_valid" => return DisputeTrigger::TrajectoryInvalid,
                _ => {}
            }
        }
    }
    DisputeTrigger::OutputIrrelevant
}
```

### Dispute Resolution Timeline

| Stage | Duration | Action |
|-------|----------|--------|
| Open | 0h | Buyer submits dispute with evidence |
| Provider Response | 0-24h | Provider can submit counter-evidence |
| Auto-Resolution | 24h | Automated checks run on evidence |
| Arbitration | 24-72h | If auto-resolution fails, human arbitrators review |
| Final | 72h | Resolution published, reputation updated |

### Automated Resolution

```rust
/// Attempt automated dispute resolution
pub fn auto_resolve_dispute(dispute: &Dispute) -> Option<DisputeResolution> {
    match dispute.trigger {
        DisputeTrigger::HashMismatch => {
            // Definitive: hash either matches or doesn't
            let buyer_hash = dispute.buyer_evidence.claimed_hash;
            let computed = sha256(&dispute.artifact);
            if computed == buyer_hash {
                Some(DisputeResolution::buyer_wins("Hash mismatch confirmed"))
            } else if computed == dispute.provider_claimed_hash {
                Some(DisputeResolution::provider_wins("Hash matches provider claim"))
            } else {
                None // Inconclusive, escalate
            }
        }

        DisputeTrigger::TestsFailAfterPatch => {
            // Re-run tests in neutral environment
            let result = run_in_sandbox(
                &dispute.repo_url,
                &dispute.repo_ref,
                &dispute.patch,
                &dispute.test_command,
            );
            if result.exit_code == 0 {
                Some(DisputeResolution::provider_wins("Tests pass in neutral sandbox"))
            } else {
                Some(DisputeResolution::buyer_wins("Tests fail in neutral sandbox"))
            }
        }

        DisputeTrigger::TrajectoryInvalid => {
            // Verify trajectory exists and is signed
            let trajectory = fetch_trajectory(&dispute.trajectory_id);
            match trajectory {
                Ok(t) if verify_trajectory_signatures(&t) => {
                    Some(DisputeResolution::provider_wins("Trajectory valid"))
                }
                _ => Some(DisputeResolution::buyer_wins("Trajectory invalid or missing"))
            }
        }

        _ => None, // Other cases need human review
    }
}
```

### Reputation Impact

| Resolution | Provider Impact | Buyer Impact |
|------------|-----------------|--------------|
| Buyer wins | -10 to -50 points | None |
| Provider wins | +5 points | -5 points (frivolous) |
| Split | -5 points | -2 points |

---

## Verification Types

```rust
/// Complete verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Overall pass/fail
    pub passed: bool,
    /// Individual checks
    pub checks: Vec<VerificationCheck>,
    /// Trajectory verification
    pub trajectory: Option<TrajectoryVerification>,
    /// Hash of all evidence (for dispute reference)
    pub evidence_hash: String,
    /// Verification timestamp
    pub verified_at: DateTime<Utc>,
}

impl VerificationResult {
    pub fn from_checks(checks: Vec<VerificationCheck>) -> Self {
        let passed = checks.iter().all(|c| c.passed);
        let evidence_hash = sha256(&serde_json::to_vec(&checks).unwrap());
        Self {
            passed,
            checks,
            trajectory: None,
            evidence_hash,
            verified_at: Utc::now(),
        }
    }

    pub fn failed_checks(&self) -> Vec<&VerificationCheck> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    /// Check name (e.g., "hash_matches", "tests_pass")
    pub name: String,
    /// Whether check passed
    pub passed: bool,
    /// Additional details
    pub details: Option<String>,
}
```

---

## References

- [BAZAAR.md](BAZAAR.md) - Main specification
- [JOB-TYPES.md](JOB-TYPES.md) - Job type schemas
- [Recorder format](../../crates/recorder/docs/format.md) - Trajectory format
