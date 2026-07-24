---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.omega.identity.first.onboarding"
assurance_revision: 1
title: "Omega Identity-First Onboarding AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposal defines the proof for the Omega identity-first journey. It
separates source automation from installed-candidate checks. It requires owner
observation and an independent verifier before candidate admission.

The current Omega source has identity and onboarding automation. That fact
does not confirm an installed candidate. This proposal records no execution,
admission, owner observation, independent verdict, release, or public claim.

## Subject

This proposal binds the exact ProductSpec bytes, revision, path, and criterion
identifiers. A ProductSpec change makes this proposal stale until review and
an exact rebind.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "OMEGA-AC-01",
      "OMEGA-AC-02",
      "OMEGA-AC-03",
      "OMEGA-AC-04",
      "OMEGA-AC-05",
      "OMEGA-AC-06",
      "OMEGA-AC-07",
      "OMEGA-AC-08",
      "OMEGA-AC-09",
      "OMEGA-AC-10",
      "OMEGA-AC-11",
      "OMEGA-AC-12"
    ],
    "document_digest": "sha256:fdbf66ee5c9c89a357be14d19aa21197313ab39b2c95bdc330743bda37a91c12",
    "path": "specs/omega/identity-first-onboarding.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

The highest risks are secret disclosure, silent identity change, startup
bypass, inherited UI regressions, and a false green from source-only tests.
The candidate gate must test the real operating-system and package seams.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-OMEGA-SECRET-DISCLOSURE",
      "statement": "A normal application path can expose the person signing secret in memory, output, storage, logs, telemetry, clipboard data, diagnostics, or crash data."
    },
    {
      "id": "RISK-OMEGA-IDENTITY-ROTATION",
      "statement": "A restart, race, partial write, recovery error, or reset error can create a second identity or report completion for mismatched durable facts."
    },
    {
      "id": "RISK-OMEGA-STARTUP-BYPASS",
      "statement": "A restored workspace, path request, remote request, wait request, or deep link can bypass identity inspection or run more than once."
    },
    {
      "id": "RISK-OMEGA-ONBOARDING-REGRESSION",
      "statement": "The identity section can change Theme or Agent Setup behavior, hide a blocked state, fail assistive use, or show unsupported Zed product text."
    },
    {
      "id": "RISK-OMEGA-SOURCE-ONLY-FALSE-GREEN",
      "statement": "Hermetic source tests can pass while the signed package uses a wrong locator, cannot use the operating-system secret store, leaks a secret, or changes identity across package lifecycle events."
    }
  ],
  "source_digest": "sha256:146b79f4a68944cb0e44e0f533dc29f90464c73ac362ec15a57a922993281542",
  "source_snapshot": "The ProductSpec requires secret isolation, durable identity continuity, one startup coordinator, preserved editor controls, and an exact installed-candidate lifecycle. Source tests cannot prove the real package, operating-system custody, assistive interface, or owner-visible journey."
}
```

## Assurance Scope

All twelve ProductSpec criteria are in scope. No criterion is not applicable.
The source gate covers deterministic contracts and state transitions. The
candidate gate adds exact package identity, real custody, offline operation,
assistive use, lifecycle continuity, Zed isolation, privacy checks, owner
observation, and independent review.

## Environments

Both profiles are proposed. Admission must replace each proposed profile with
an exact profile document. The source profile must bind the Omega commit,
Rust toolchain, dependency lock, commands, and clean source state.

The installed profile must bind the same source commit to one package digest.
It must also bind the signature, bundle ID, channel, hardware, operating-system
version, data roots, keyring locator, network condition, permissions, Zed
baseline, update source, and rollback source.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-OMEGA-IDENTITY-SOURCE",
      "status": "proposed"
    },
    {
      "id": "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64",
      "status": "proposed"
    }
  ],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "external_subject_repository"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "OpenAgentsInc/omega",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

The obligations use the Omega repository paths. A path is a candidate seam,
not a verdict. Source automation can satisfy only the source gate. The
installed gate requires new candidate-bound observations.

```assurancespec-obligations
[
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-SOURCE",
    "candidate_artifact_refs": [
      "crates/omega_identity/src/contract.rs",
      "crates/omega_identity/src/secret.rs",
      "crates/app_identity/src/app_identity.rs"
    ],
    "criterion_refs": [
      "OMEGA-AC-01",
      "OMEGA-AC-02",
      "OMEGA-AC-03"
    ],
    "disposition": "required",
    "domains": [
      "identity_authority",
      "secret_custody",
      "channel_isolation"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-SOURCE",
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "deterministic_contract_plus_real_keyring",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "keyring_observation",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "secret_return_wrong_locator_or_implicit_generation",
      "ref": "crates/omega_identity/src/custody.rs"
    },
    "id": "AO-OMEGA-IDENTITY-CUSTODY-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "crates/omega_identity/src/custody.rs",
      "statement": "The exact contract keeps all identity roles separate, creates only after owner action, uses the channel locator, and keeps secret values behind the custody port. The installed package confirms the real keyring service and account without secret extraction."
    },
    "source_claim_digest": "sha256:86ef12fb520dacf021aacd8894a08509326ed4f3b8243f91a3bf8995d4ab225c",
    "source_claim_snapshot": "Omega keeps three identity roles separate, creates a Nostr-only identity only after an explicit action, and keeps secret material behind the exact operating-system custody locator.",
    "technique": "contract_matrix_with_real_keyring",
    "title": "Assure identity authority and custody"
  },
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-SOURCE",
    "candidate_artifact_refs": [
      "crates/omega_identity/src/custody.rs",
      "crates/omega_identity/src/mutation_lock.rs",
      "crates/omega_identity/src/recovery.rs",
      "crates/omega_identity/src/recovery_artifact.rs"
    ],
    "criterion_refs": [
      "OMEGA-AC-04",
      "OMEGA-AC-05",
      "OMEGA-AC-06",
      "OMEGA-AC-07"
    ],
    "disposition": "required",
    "domains": [
      "transaction_durability",
      "identity_recovery",
      "identity_reset"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-SOURCE",
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "fault_matrix_plus_process_kill",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "process_kill_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "identity_rotation_false_completion_or_signing_while_blocked",
      "ref": "crates/omega_identity/src/custody.rs"
    },
    "id": "AO-OMEGA-IDENTITY-DURABILITY-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "crates/omega_identity/src/custody.rs",
      "statement": "Faults and races at each durable boundary preserve one identity, one receipt result, and honest custody state. Real process termination resumes the same transaction without key generation, and reset requires a later process before creation."
    },
    "source_claim_digest": "sha256:1196c627aaf6b4c3364fb293853acff1e24b3eeb6e5615f7d6f911e33d7d5de3",
    "source_claim_snapshot": "Create, recovery, and reset must survive races, partial writes, errors, and process termination without identity rotation, false completion, or signing from a blocked state.",
    "technique": "deterministic_fault_injection_and_process_kill",
    "title": "Assure durable create, recovery, and reset"
  },
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-CANDIDATE",
    "candidate_artifact_refs": [
      "crates/onboarding/src/onboarding.rs",
      "crates/onboarding/src/basics_page.rs",
      "crates/onboarding/src/identity_section.rs"
    ],
    "criterion_refs": [
      "OMEGA-AC-08",
      "OMEGA-AC-10"
    ],
    "disposition": "required",
    "domains": [
      "onboarding_ui",
      "editor_setup",
      "accessibility"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "source_preservation_plus_installed_owner_journey",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "accessibility_receipt",
        "owner_observation",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "theme_agent_replay_or_accessibility_regression",
      "ref": "crates/onboarding/src/basics_page.rs"
    },
    "id": "AO-OMEGA-IDENTITY-ONBOARDING-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "crates/onboarding/src/identity_section.rs",
      "statement": "The installed journey puts identity first, preserves Theme and registry Agent Setup behavior, and keeps Editor Onboarding replay separate. Keyboard, focus, screen reader, larger text, reduced motion, light and dark themes, and a 360-pixel width remain usable."
    },
    "source_claim_digest": "sha256:2ca21ba6167df8b8f3a79c55b84e924d8ad2486b18744187d76d6c38a3c2edeb",
    "source_claim_snapshot": "Identity is the first section in the preserved native onboarding structure. Theme and registry Agent Setup behavior stay unchanged, and Editor Onboarding has independent completion and replay.",
    "technique": "preservation_tests_and_installed_accessibility_journey",
    "title": "Assure the native onboarding journey"
  },
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-SOURCE",
    "candidate_artifact_refs": [
      "crates/onboarding/src/identity_startup.rs",
      "crates/zed/src/zed.rs",
      "crates/zed/src/zed/open_listener.rs"
    ],
    "criterion_refs": [
      "OMEGA-AC-09"
    ],
    "disposition": "required",
    "domains": [
      "startup_routing",
      "intent_continuity"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-SOURCE",
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "intent_matrix_plus_installed_launch",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "installed_launch_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "startup_bypass_duplicate_onboarding_or_duplicate_intent",
      "ref": "crates/onboarding/src/identity_startup.rs"
    },
    "id": "AO-OMEGA-IDENTITY-STARTUP-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "crates/onboarding/src/identity_startup.rs",
      "statement": "Each supported launch intent waits for one terminal identity inspection, opens at most one onboarding surface, and resumes exactly once with its original data after durable completion."
    },
    "source_claim_digest": "sha256:39916460e33a3ee5e03465e83a26068ad8f82a1a68433b28c55a5e3fb2c2dbde",
    "source_claim_snapshot": "One startup coordinator gates every supported launch intent, opens no duplicate onboarding surface, and releases the preserved intent exactly once after durable completion.",
    "technique": "concurrent_intent_matrix_and_installed_launch",
    "title": "Assure identity-first startup routing"
  },
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-CANDIDATE",
    "candidate_artifact_refs": [
      "crates/app_identity/src/app_identity.rs",
      "crates/paths/src/paths.rs",
      "script/prove-omega-rc-install"
    ],
    "criterion_refs": [
      "OMEGA-AC-11"
    ],
    "disposition": "required",
    "domains": [
      "application_isolation",
      "privacy",
      "brand_integrity"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "installed_isolation_and_secret_tripwire",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "zed_baseline_receipt",
        "privacy_tripwire_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "zed_state_change_secret_disclosure_or_public_zed_label",
      "ref": "script/prove-omega-rc-install"
    },
    "id": "AO-OMEGA-IDENTITY-ISOLATION-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "script/prove-omega-rc-install",
      "statement": "An installed Omega candidate uses only its bound application and custody roots. Before-and-after Zed facts are equal, public surfaces show no unsupported Zed label, and secret tripwires stay clear across UI, clipboard, logs, telemetry, diagnostics, and crash output."
    },
    "source_claim_digest": "sha256:f1cdb3ecf750901b5b3c02a16ac2b96041548511af9ace2be91d4fb0d03b918a",
    "source_claim_snapshot": "The installed identity journey must use isolated Omega roots, preserve all measured Zed state, show no unsupported Zed label, and disclose no secret through public or diagnostic channels.",
    "technique": "installed_baseline_diff_and_secret_tripwire",
    "title": "Assure Omega isolation and privacy"
  },
  {
    "activation_gate": "GATE-OMEGA-IDENTITY-CANDIDATE",
    "candidate_artifact_refs": [
      "script/bundle-omega-rc",
      "script/prove-omega-rc-install",
      "docs/src/development/omega-rc-installed-proof.md"
    ],
    "criterion_refs": [
      "OMEGA-AC-12"
    ],
    "disposition": "required",
    "domains": [
      "package_binding",
      "offline_operation",
      "lifecycle_continuity",
      "independent_verification"
    ],
    "environment_refs": [
      "ENV-OMEGA-IDENTITY-INSTALLED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "exact_candidate_manual_lifecycle",
      "required_kinds": [
        "package_identity_receipt",
        "offline_receipt",
        "lifecycle_receipt",
        "owner_observation",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "unbound_candidate_network_dependency_or_identity_change",
      "ref": "script/prove-omega-rc-install"
    },
    "id": "AO-OMEGA-IDENTITY-CANDIDATE-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/src/development/omega-rc-installed-proof.md",
      "statement": "One exact package digest completes create, restart, recovery, reset, physical offline start, update, downgrade, rollback, uninstall, and reinstall. The expected public identity remains stable unless the explicit verified reset contract requires absence."
    },
    "source_claim_digest": "sha256:322d7caf2a2975697b6ca0baf929b643adaad13edb556561c374dd600add8fad",
    "source_claim_snapshot": "One exact installed candidate must pass the full identity, offline, accessibility, privacy, update, rollback, uninstall, and reinstall matrix with owner observation and independent verification.",
    "technique": "bound_installed_candidate_lifecycle",
    "title": "Assure the exact installed candidate"
  }
]
```

## Gates

The source gate confirms only source behavior. The candidate gate requires the
same source identity and one exact installed artifact. It also requires all
manual observations. No gate can admit or release itself.

```assurancespec-gates
[
  {
    "expression": "product_spec=exact && omega_source=exact && dependency_lock=exact && source_commands=PASS && source_falsifiers=REFUTED && secret_tripwire_fixture=PASS && exception=none",
    "id": "GATE-OMEGA-IDENTITY-SOURCE"
  },
  {
    "expression": "source_gate=PASS && package_digest=exact && signature=valid && environment=bound && real_keyring=CONFIRMED && physical_offline=CONFIRMED && process_kill_matrix=PASS && launch_intent_matrix=PASS && accessibility_matrix=PASS && zed_baseline=UNCHANGED && privacy_tripwire=PASS && update_rollback_uninstall_reinstall=PASS && owner_observation=accepted && independent_review=accepted && exception=none",
    "id": "GATE-OMEGA-IDENTITY-CANDIDATE"
  }
]
```

## Evidence Policy

Links and source tests are not verdicts for an installed candidate. Missing,
stale, self-produced, unbound, or unreviewed evidence is INCONCLUSIVE. Each
manual receipt must name the package digest, environment profile, observer,
time, method, oracle result, and falsifier result.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "designed",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "exact_candidate_binding",
    "owner_observation_when_required",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit its proof design or candidate. The implementation
producer cannot verify an obligation that they produced. Owner observation is
required for the journey, but it is not independent verification. Candidate
admission does not grant release or a public claim.

```assurancespec-authority
{
  "admitted_roles": [],
  "policy_state": "designed",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [
    "openagents.owner"
  ],
  "verifier_roles": [
    "openagents.assurance_reviewer"
  ]
}
```
