---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.openagents.managed.agent.sandboxes"
assurance_revision: 2
title: "OpenAgents Managed Agent Sandboxes AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This revision designs one falsifiable obligation for every managed-sandbox
criterion. Contract, staged-runtime, and live GCP rungs remain separate:
designed proof is not executed proof, and no fixture can satisfy a live gate.

## Subject

The proposal is bound to the exact ProductSpec bytes, revision, path, and stable criterion identifiers below.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "MSB-AC-01",
      "MSB-AC-02",
      "MSB-AC-03",
      "MSB-AC-04",
      "MSB-AC-05",
      "MSB-AC-06",
      "MSB-AC-07",
      "MSB-AC-08",
      "MSB-AC-09",
      "MSB-AC-10",
      "MSB-AC-11",
      "MSB-AC-12",
      "MSB-AC-13",
      "MSB-AC-14",
      "MSB-AC-15",
      "MSB-AC-16",
      "MSB-AC-17",
      "MSB-AC-18"
    ],
    "document_digest": "sha256:0bef38178b696e0a3866f5206862af36fa80d31537d61bc15b90f2be3379665f",
    "path": "specs/openagents/managed-agent-sandboxes.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

No risk objects are inferred from ProductSpec prose. Reviewers must design the applicable risk model.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-MSB-01",
      "statement": "A provider or Box projection could be treated as the canonical sandbox identity and bypass owner, generation, lease, budget, or capability scope."
    },
    {
      "id": "RISK-MSB-02",
      "statement": "Concurrent or replayed lifecycle effects could leave two accepting generations, lose a durable filesystem, or report cleanup before residue is observed absent."
    },
    {
      "id": "RISK-MSB-03",
      "statement": "Fake, local, unhealthy, or weaker-isolation capacity could be rounded up to a live managed Google Cloud target."
    },
    {
      "id": "RISK-MSB-04",
      "statement": "Compatibility pressure could widen Box-v1 into ambient shell, credentials, ingress, snapshots, or generic cloud administration."
    },
    {
      "id": "RISK-MSB-05",
      "statement": "Quiet output or a compatibility status could be mistaken for structural runtime settlement, causing premature stop, deletion, or success."
    },
    {
      "id": "RISK-MSB-06",
      "statement": "Fixture, fake-server, or producer-authored evidence could be presented as live isolation, cost, zero-residue, or owner-observed acceptance."
    }
  ],
  "source_digest": "sha256:d8b52917ab9bea8eeba3548dbc5eee809783ce8946591bcc38e657da51800d54",
  "source_snapshot": "- A VM-shaped resource marketed as a container can obscure the actual\n  isolation and cost boundary. Product and receipts name the effective GCE VM\n  or Firecracker microVM and never claim OCI semantics unless implemented.\n- Compatibility pressure can weaken secret, network, desktop, or lifecycle\n  policy. Typed incompatibility is preferable to unsafe parity; account-wide\n  secret replication and public VNC remain unsupported.\n- Prewarming every message can become a cost leak. Admission decides whether\n  to prewarm, and idle stop waits for structural settlement under an exact\n  lease and budget.\n- A lossy compatibility event plane can be mistaken for evidence. Native\n  events, authority decisions, usage, and cleanup receipts remain canonical.\n- A convenient Sarah tool can become generic cloud-admin authority. Her broker\n  accepts only closed lifecycle and work-unit operations with exact bounded\n  inputs and independently enforced target policy.\n- Snapshot or resume language can imply process-memory continuity. The product\n  states filesystem checkpoint and service restart; hidden provider state,\n  processes, sockets, and memory do not move."
}
```

## Assurance Scope

All 18 executable ProductSpec criteria are in scope. SBX-00 supplies contract,
authority, provenance, and bounded-model candidates. Later SBX issues must land
the runtime, consumer, and live candidates named here. Snapshot, fork, and
desktop ingress are assured as unavailable until their future admission gates
are satisfied, rather than silently excluded.

## Environments

Four proposed profiles preserve the proof ladder: local contract/model tests,
staged runtime fault tests, owner-gated live GCP, and the cross-surface live
Desktop/Sarah journey. None is admitted by this proposal.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-SBX-LOCAL-CONTRACT",
      "status": "proposed"
    },
    {
      "id": "ENV-SBX-STAGED-RUNTIME",
      "status": "proposed"
    },
    {
      "id": "ENV-SBX-GCP-LIVE",
      "status": "proposed"
    },
    {
      "id": "ENV-SBX-CROSS-SURFACE-LIVE",
      "status": "proposed"
    }
  ],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "repository_not_supplied"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "not-supplied",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

Every obligation declares its domain, technique, environment, oracle, negative
control, evidence rung, independent verifier boundary, and activation gate.
The paths for later SBX issues are design targets, not claims that those files
or observations exist today.

```assurancespec-obligations
[
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-01"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-01-01",
    "source_claim_digest": "sha256:c305a6969b0c758c4d8746d674ea22e91c28d22a68275517320b198ef95f4d28",
    "source_claim_snapshot": "Every sandbox is bound before mutation to an authenticated\nowner, tenant, program/work unit, sandbox ref, attachment generation,\ntarget, image/profile, lease, budget, and capability set. A bearer or SDK\nbox ID alone grants no access, and cross-owner or stale-generation access\nfails before a runtime effect.",
    "title": "Assure MSB-AC-01",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["contract", "security", "regression"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT"],
    "evidence": {
      "proof_rung": "local_unit",
      "required_kinds": ["execution_trace", "negative_control_trace"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "cross_owner_or_stale_generation",
      "ref": "packages/managed-sandbox-contract/src/schemas.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/managed-sandbox-contract/src/schemas.test.ts",
      "statement": "Decode the exact resource and command boundary, then prove missing owner scope, mutable image identity, invalid lease, and stale generation are refused before effects."
    },
    "technique": "deterministic_test"
  },
  {
    "candidate_artifact_refs": [
      "packages/managed-sandbox-contract/src/lifecycle.test.ts",
      "packages/khala-sync-server/migrations/0080_managed_sandbox_authority.sql",
      "packages/khala-sync-server/src/managed-sandbox-store.test.ts"
    ],
    "criterion_refs": [
      "MSB-AC-02"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-02-01",
    "source_claim_digest": "sha256:056b7dfa9438df6ad5ac8d2d032c8f56a4dcc8983eaec27d676dad9aa00ff37b",
    "source_claim_snapshot": "Create, inspect, update, stop, resume, and delete serialize\nthrough durable lifecycle authority. Exact retries reconcile, conflicting\nrequest bytes refuse, and crash, disconnect, duplicate, and lost-ACK faults\nnever create two work-accepting generations or repeat a settled effect.",
    "title": "Assure MSB-AC-02",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["model", "resilience", "regression"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT"],
    "evidence": {
      "proof_rung": "bounded_model",
      "required_kinds": ["state_graph", "fault_trace", "negative_control_trace"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "duplicate_accepting_generation",
      "ref": "packages/khala-sync-server/src/managed-sandbox-store.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/khala-sync-server/src/managed-sandbox-store.test.ts",
      "statement": "Combine the bounded lifecycle model with real Postgres fault tests for gaps, retries, concurrency, generation fencing, crash reconciliation, and cleanup observation."
    },
    "technique": "bounded_state_model"
  },
  {
    "candidate_artifact_refs": [
      "crates/oa-codex-control/src/managed_sandbox_runtime.rs",
      "crates/oa-codex-control/tests/cloud_vm_contract.rs",
      "docs/sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json",
      "scripts/cloud/managed-sandbox-live-acceptance.ts"
    ],
    "criterion_refs": [
      "MSB-AC-03"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-03-01",
    "source_claim_digest": "sha256:ee067415d054a10c6bc36daeb16ee8650e31c77dc70d980db0b3e341c8844c01",
    "source_claim_snapshot": "A requested live managed target reports ready only from an\nobserved healthy admitted GCP provisioner and guest. Fake mode, missing KVM\nor images, unhealthy boot, quota/budget/capacity exhaustion, or unavailable\ncapacity returns a typed refusal; no local or fake execution can satisfy\nlive acceptance. Region/class/image, concurrency, TTL, least-privilege\nkeyless service identities, external-IP posture, and network policy bind\nbefore provisioning.",
    "title": "Assure MSB-AC-03",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["isolation", "resilience", "security"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME", "ENV-SBX-GCP-LIVE"],
    "evidence": {
      "proof_rung": "live_target_required",
      "required_kinds": ["provisioner_readiness_receipt", "negative_control_trace", "network_policy_receipt"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fake_or_unhealthy_target_admitted",
      "ref": "crates/oa-codex-control/src/managed_sandbox_runtime.rs"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "scripts/cloud/managed-sandbox-live-acceptance.ts",
      "statement": "A live request reaches ready only with observed GCP provisioner and guest health; fake, no-KVM/image, quota, budget, and capacity controls must refuse."
    },
    "technique": "target_readiness_fault_matrix"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "docs/sol/evidence/2026-07-19-sbx03-box-v1-conformance.json",
      "packages/managed-sandbox-contract/src/box-v1.test.ts",
      "packages/managed-sandbox-contract/src/provenance.ts"
    ],
    "criterion_refs": [
      "MSB-AC-04"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-04-01",
    "source_claim_digest": "sha256:51f45a5789ac87556918cd1bd96a3fe9ab87d8ad113d18709706fbb79a36f554",
    "source_claim_snapshot": "The unmodified `@asciidev/box-sdk@0.0.24` can select the\nOpenAgents `basePath` and pass the exact admitted method, status, envelope,\nquery, cursor, retry, and error corpus. The SDK and vendor types remain an\nisolated development dependency and cannot become canonical runtime types.\nThe receipt binds OpenAPI bytes, npm integrity, package digest, exact\nlockfile, SPDX/license notice, and translator version.",
    "title": "Assure MSB-AC-04",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["contract", "provenance", "regression"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT", "ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "pinned_dependency_conformance",
      "required_kinds": ["sdk_execution_trace", "provenance_receipt", "lockfile_audit"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "sdk_or_openapi_drift",
      "ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "statement": "Import the exact unmodified SDK, run the same admitted corpus against the in-process fake and loopback HTTP service, prove basePath, bearer, retry, fault, and cursor behavior, and bind package/OpenAPI/license/translator provenance."
    },
    "technique": "black_box_sdk_conformance"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "packages/managed-sandbox-contract/src/box-v1.test.ts"
    ],
    "criterion_refs": [
      "MSB-AC-05"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-05-01",
    "source_claim_digest": "sha256:b78265ae03fc991980dacae572c1f72432e73dda6fae71773a7215b20939e5bc",
    "source_claim_snapshot": "Phase 1 serves only the admitted account/limits, lifecycle,\nprompt/status/events/interrupt, file, command, and artifact operations.\nEvery unsupported Box operation returns stable typed `501\ncapability_not_implemented`; no empty result or fake success implies parity.",
    "title": "Assure MSB-AC-05",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["contract", "negative_capability", "regression"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT"],
    "evidence": {
      "proof_rung": "local_contract",
      "required_kinds": ["operation_corpus", "unsupported_operation_trace"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "unsupported_operation_succeeds",
      "ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "statement": "Partition every upstream SDK method into the 17 admitted operations or 14 typed 501 refusals, then exercise the admitted and refused HTTP paths with no omitted or optimistic method."
    },
    "technique": "exhaustive_api_partition"
  },
  {
    "candidate_artifact_refs": [
      "docs/sol/evidence/2026-07-19-sbx04-managed-sandbox-turns.json"
    ],
    "criterion_refs": [
      "MSB-AC-06"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-06-01",
    "source_claim_digest": "sha256:393ed2f2099e5d063ff64b17d98cda6a9381d535dbc4cafe8fb7c2bd04221434",
    "source_claim_snapshot": "A prompt creates one exact runtime turn with effective\nprovider, model, harness, work unit, and generation truth. Prompt status is\nfirst class; reconnectable ascending cursor pages preserve order; interrupt\ntargets exactly one turn and is idempotent.",
    "title": "Assure MSB-AC-06",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["contract", "ordering", "resilience"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "component_fault_test",
      "required_kinds": ["turn_trace", "cursor_reconnect_trace", "interrupt_replay_trace"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "duplicate_or_misordered_turn",
      "ref": "packages/khala-sync-server/src/managed-sandbox-store.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/khala-sync-server/src/managed-sandbox-store.test.ts",
      "statement": "One prompt binds one effective turn; ascending reconnect pages preserve native order and repeated interrupt settles the same exact turn."
    },
    "technique": "deterministic_component_fault_test"
  },
  {
    "candidate_artifact_refs": [
      "docs/sol/evidence/2026-07-19-sbx04-managed-sandbox-turns.json"
    ],
    "criterion_refs": [
      "MSB-AC-07"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-07-01",
    "source_claim_digest": "sha256:21697c8208126d669923fb2783f85f0909cdc5aa8c7a17a41b6b0c374adec813",
    "source_claim_snapshot": "A long-running turn has no silence-based completion and no\narbitrary production wall timeout. Structural runtime completion, explicit\nstop, a declared lease/budget guardrail, or a typed failure settles it; idle\nstop can arm only after that settlement and cannot race active hidden work.",
    "title": "Assure MSB-AC-07",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["liveness", "resilience", "regression"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "runtime_liveness_model",
      "required_kinds": ["long_running_trace", "quiet_process_negative_control", "guardrail_trace"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "silence_based_completion",
      "ref": "packages/managed-sandbox-contract/src/lifecycle.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/managed-sandbox-contract/src/lifecycle.test.ts",
      "statement": "Hold a quiet but active turn open and prove only structural settlement, explicit stop, declared guardrail, or typed failure can arm idle stop."
    },
    "technique": "liveness_fault_test"
  },
  {
    "candidate_artifact_refs": [
      "docs/sol/evidence/2026-07-19-sbx05-managed-sandbox-guest-io.json"
    ],
    "criterion_refs": [
      "MSB-AC-08"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-08-01",
    "source_claim_digest": "sha256:87427d75ec7681513f921a5e52600739807689265e163625272dd217bb725820",
    "source_claim_snapshot": "Provider, SCM, tool, network, ingress, and API capabilities\nare scoped, expiring, revocable broker leases redeemed only inside the exact\nsandbox generation. Raw credentials, auth homes, service-account material,\nprivate paths, and topology never enter payloads, checkpoints, prompts,\nlogs, public events, issues, or receipts.",
    "title": "Assure MSB-AC-08",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["security", "privacy", "capability"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME", "ENV-SBX-GCP-LIVE"],
    "evidence": {
      "proof_rung": "broker_redemption_fault_test",
      "required_kinds": ["scope_trace", "revocation_trace", "redaction_scan"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "cross_generation_or_secret_capability",
      "ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "statement": "Redeem only generation-bound, unexpired capability refs and scan all payload, event, checkpoint, issue, log, and receipt projections for forbidden material."
    },
    "technique": "security_fault_matrix"
  },
  {
    "candidate_artifact_refs": [
      "docs/sol/evidence/2026-07-19-sbx05-managed-sandbox-guest-io.json"
    ],
    "criterion_refs": [
      "MSB-AC-09"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-09-01",
    "source_claim_digest": "sha256:184746e3a1a397bd309ecbb6a827483c9446ce2c0ea3d7e626b5fc6063942e28",
    "source_claim_snapshot": "File, command, process, network, and artifact operations\nenforce root-relative path, symlink, secret, binary, byte, duration, output,\nconcurrency, egress, and quota policy below every client adapter. Artifacts\nbind exact content digest, size, source generation, retention, and evidence\nrefs.",
    "title": "Assure MSB-AC-09",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["security", "resource_policy", "regression"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "policy_property_test",
      "required_kinds": ["path_escape_trace", "quota_trace", "artifact_digest_receipt"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "adapter_bypasses_workspace_policy",
      "ref": "crates/oa-codex-control/src/managed_sandbox_guest_io.rs"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "crates/oa-codex-control/src/managed_sandbox_guest_io.rs",
      "statement": "Generate path, symlink, binary, size, duration, output, concurrency, egress, and quota violations below native and compatibility adapters and verify digest-bound artifacts."
    },
    "technique": "property_and_boundary_test"
  },
  {
    "candidate_artifact_refs": [
      "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "packages/khala-sync-server/src/managed-sandbox-store.test.ts",
      "packages/managed-sandbox-contract/src/box-v1.test.ts"
    ],
    "criterion_refs": [
      "MSB-AC-10"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-10-01",
    "source_claim_digest": "sha256:4c57ba06bdd9c129db288dce7553e7ed12e5824fb90e6a9b19dd50561c68d125",
    "source_claim_snapshot": "Native OpenAgents events and receipts remain the lossless\nrecord. Every Box-compatible event identifies its translator version and\ncursor; conformance proves projection ordering and omission, but a Box event\nor SDK terminal state can never replace native authority, private evidence,\nusage truth, or cleanup evidence.",
    "title": "Assure MSB-AC-10",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["contract", "projection", "ordering"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT", "ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "projection_conformance",
      "required_kinds": ["native_event_trace", "projection_trace", "omission_ledger"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "projection_claims_native_authority",
      "ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts",
      "statement": "Every compatibility page identifies translator, native sequence, generation-fenced cursor, and omissions while the Postgres native event and projection stores retain authority and private evidence, usage, and cleanup remain unprojected truth."
    },
    "technique": "projection_loss_conformance"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-11"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-11-01",
    "source_claim_digest": "sha256:c1d571f6b22661a27fc2266660c2236e00c090bf631dcf20c1b566b54b5896a7",
    "source_claim_snapshot": "Desktop creates and attaches a managed sandbox through the\nIDE-13 project-capability interface, and IDE-17 agents consume the same\nproject, file, document, proposal, work-unit, agent, and evidence refs as a\nlocal placement. Effective target, image/profile, custody, latency,\ngeneration, lease, budget, and capability state remain visible, with no\nrenderer credential, raw root, generic control-plane client, or silent\nmanaged fallback.",
    "title": "Assure MSB-AC-11",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["consumer_contract", "identity", "security"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME", "ENV-SBX-CROSS-SURFACE-LIVE"],
    "evidence": {
      "proof_rung": "desktop_integration",
      "required_kinds": ["identity_continuity_trace", "placement_trace", "renderer_boundary_scan"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "desktop_mints_parallel_identity_or_fallback",
      "ref": "apps/openagents-desktop/src/ide/managed-sandbox.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents-desktop/src/ide/managed-sandbox.test.ts",
      "statement": "IDE-13 placement and IDE-17 execution preserve exact project/work-unit/agent/evidence refs, expose effective custody and budgets, and reject silent local fallback or renderer credentials."
    },
    "technique": "deterministic_consumer_integration"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-12"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-12-01",
    "source_claim_digest": "sha256:fd7479ad231f642396d34d17df4c75061836ce77a6eb7d96a21bb137943095a1",
    "source_claim_snapshot": "After an exact authority-profile and broker admission,\n`principal.sarah` can create, list, inspect, dispatch into, interrupt, stop,\nresume, and delete only the authenticated owner's sandboxes. Every action\nrequires exact program/work-unit/target/profile/TTL/budget/capability refs,\nemits ordered activity and authority plus target receipts, and exposes no\ngeneric cloud, shell, database, topology, or credential tool.",
    "title": "Assure MSB-AC-12",
    "activation_gate": "GATE-SBX-CONTRACT",
    "domains": ["authority", "security", "negative_capability"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT", "ENV-SBX-CROSS-SURFACE-LIVE"],
    "evidence": {
      "proof_rung": "authority_contract_then_live_journey",
      "required_kinds": ["authority_decision_trace", "target_receipt", "negative_tool_inventory"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "sarah_generic_cloud_or_cross_owner_action",
      "ref": "packages/authority/src/managed-sandbox-authority.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/authority/src/managed-sandbox-authority.test.ts",
      "statement": "Admit only eight Sarah actions for the authenticated owner's sandbox resource when every scope, budget, broker, and target gate passes; generic cloud/container actions refuse."
    },
    "technique": "authority_decision_matrix"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-13"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-13-01",
    "source_claim_digest": "sha256:3eeb2e791d0b1a93c7109f0d62f5f9dca34fec35f01833a29e6b58a3cc9d72e0",
    "source_claim_snapshot": "Mobile and authenticated web decode bounded lifecycle,\neffective runtime, last structural event, attention, file/change/artifact,\nlease/budget, and cleanup projections from shared schemas and send only\ntyped commands through the durable outbox. They host no SDK, runtime, GCP\nclient, provider credential, raw filesystem, PTY, or generic shell.",
    "title": "Assure MSB-AC-13",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["consumer_contract", "security", "regression"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "mobile_web_component",
      "required_kinds": ["shared_schema_decode_trace", "outbox_replay_trace", "dependency_boundary_scan"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "client_hosts_runtime_or_generic_shell",
      "ref": "apps/openagents-mobile/tests/mobile-managed-sandbox.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "apps/openagents-mobile/tests/mobile-managed-sandbox.test.ts",
      "statement": "Mobile and web decode bounded shared projections and replay only typed outbox commands while dependency scans exclude SDK, runtime, GCP client, credentials, raw filesystem, PTY, and shell."
    },
    "technique": "consumer_boundary_test"
  },
  {
    "candidate_artifact_refs": [
      "crates/oa-codex-control/src/managed_sandbox_runtime.rs",
      "docs/sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json",
      "scripts/cloud/managed-sandbox-live-acceptance.ts"
    ],
    "criterion_refs": [
      "MSB-AC-14"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-14-01",
    "source_claim_digest": "sha256:6f4960fc6f810f7bb2f6ea948ea95a9a559ebf0046421f822e8e93c85e7b0ef9",
    "source_claim_snapshot": "Lease expiry, budget or quota exhaustion, revoke, guest crash,\nbroker outage, control-plane restart, cursor loss, stop/resume failure, and\nteardown failure remain distinct typed outcomes. Cleanup is complete only\nwhen receipts prove zero residual compute, firewall/ingress, scratch,\nprocess, and capability grants; otherwise the sandbox is recovery-required.",
    "title": "Assure MSB-AC-14",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["resilience", "cleanup", "cost"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME", "ENV-SBX-GCP-LIVE"],
    "evidence": {
      "proof_rung": "fault_injection_then_live_cleanup",
      "required_kinds": ["typed_fault_trace", "zero_residue_receipt", "cost_receipt"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "partial_cleanup_reported_complete",
      "ref": "crates/oa-codex-control/src/managed_sandbox_runtime.rs"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "crates/oa-codex-control/src/managed_sandbox_runtime.rs",
      "statement": "Inject every named lease, budget, quota, broker, guest, cursor, restart, stop/resume, and teardown fault; cleanup is green only after every residual class is observed absent."
    },
    "technique": "reconciliation_fault_matrix"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-15"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-15-01",
    "source_claim_digest": "sha256:33196235ae1fe20b76c59cadc93325a783ce11ad218b6dac34372666c94d6354",
    "source_claim_snapshot": "Snapshot and fork remain unavailable until an exact completed\ncheckpoint binds source sandbox/generation, image/toolchain, repository\npost-image, content digest, and retention. Fork creates a fresh sandbox and\nfresh capabilities and never clones credentials, memory, processes, sockets,\nports, network identity, or provider hidden state.",
    "title": "Assure MSB-AC-15",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["negative_capability", "checkpoint", "security"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "unsupported_until_admitted",
      "required_kinds": ["typed_refusal_trace", "future_checkpoint_conformance"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "snapshot_or_fork_available_without_gate",
      "ref": "packages/managed-sandbox-contract/src/box-v1.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/managed-sandbox-contract/src/box-v1.test.ts",
      "statement": "Snapshot and fork remain typed 501 until a later admitted checkpoint suite proves fresh identity/capabilities and excludes credentials, memory, process, socket, port, network, and hidden-provider state."
    },
    "technique": "negative_capability_gate"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-16"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-16-01",
    "source_claim_digest": "sha256:9ca1d703581010526f449b583c5adf1753b41b2318ccf2275a4c30614f62ec15",
    "source_claim_snapshot": "Private desktop or preview ingress remains unavailable until\nits short-lived owner/audience-scoped capability, revocation, redaction,\naudit, and cleanup tests pass. Public or ungated VNC is not admitted.",
    "title": "Assure MSB-AC-16",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["negative_capability", "ingress", "security"],
    "environment_refs": ["ENV-SBX-STAGED-RUNTIME"],
    "evidence": {
      "proof_rung": "unsupported_until_admitted",
      "required_kinds": ["typed_refusal_trace", "future_private_ingress_receipt"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "public_or_ungated_vnc_available",
      "ref": "packages/managed-sandbox-contract/src/box-v1.test.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "packages/managed-sandbox-contract/src/box-v1.test.ts",
      "statement": "Desktop/preview remains typed 501 until a later gate proves short-lived owner/audience scope, revoke, redaction, audit, and cleanup; public VNC always refutes."
    },
    "technique": "negative_capability_gate"
  },
  {
    "candidate_artifact_refs": [
      "crates/oa-codex-control/src/managed_sandbox_runtime.rs",
      "crates/oa-codex-control/tests/cloud_vm_contract.rs",
      "docs/sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json"
    ],
    "criterion_refs": [
      "MSB-AC-17"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-17-01",
    "source_claim_digest": "sha256:44189b1eb3d1fce10bcd6902abebdc5956f5f96d28e0f44807d76f883fb18295",
    "source_claim_snapshot": "The deterministic fault and isolation corpus covers\ncross-owner access, concurrent lifecycle calls, stale generations,\nreplay/conflict, partial provision, guest crash, event gaps, capability\nrevoke, secret markers, quota, cost cap, and partial teardown without false\nreadiness, duplicate execution, leaked private material, or residue.",
    "title": "Assure MSB-AC-17",
    "activation_gate": "GATE-SBX-RUNTIME",
    "domains": ["isolation", "resilience", "security", "cost"],
    "environment_refs": ["ENV-SBX-LOCAL-CONTRACT", "ENV-SBX-STAGED-RUNTIME", "ENV-SBX-GCP-LIVE"],
    "evidence": {
      "proof_rung": "deterministic_fault_corpus",
      "required_kinds": ["fault_matrix", "isolation_trace", "redaction_scan", "residue_scan"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fault_yields_false_green_or_residue",
      "ref": "crates/oa-codex-control/src/managed_sandbox_runtime.rs"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "crates/oa-codex-control/src/managed_sandbox_runtime.rs",
      "statement": "Execute the complete named fault/isolation matrix and require no false readiness, duplicate execution, leaked private material, budget escape, or residual resource."
    },
    "technique": "deterministic_fault_matrix"
  },
  {
    "candidate_artifact_refs": [],
    "criterion_refs": [
      "MSB-AC-18"
    ],
    "disposition": "required",
    "id": "AO-MSB-AC-18-01",
    "source_claim_digest": "sha256:71eff86400048183d3fd4552678b904d0502b0c01f83097729d763a8f056113a",
    "source_claim_snapshot": "Live acceptance independently proves the pinned SDK against\nstaging and owner-gated GCP plus one Desktop and one Sarah create-to-agent-\nturn-to-stop/resume/delete journey. Receipts bind source/deployed revisions,\nimage/provisioner/translator identities, measured incremental cost, zero\nresidue, rollback, limitations, and owner observation; fixture or fake proof\ncannot satisfy this criterion.",
    "title": "Assure MSB-AC-18",
    "activation_gate": "GATE-SBX-LIVE",
    "domains": ["live_acceptance", "isolation", "cleanup", "cost", "consumer_journey"],
    "environment_refs": ["ENV-SBX-GCP-LIVE", "ENV-SBX-CROSS-SURFACE-LIVE"],
    "evidence": {
      "proof_rung": "owner_gated_live",
      "required_kinds": ["staging_sdk_receipt", "gcp_lifecycle_receipt", "desktop_journey_receipt", "sarah_journey_receipt", "zero_residue_receipt", "rollback_receipt", "owner_observation"]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fixture_or_fake_claimed_live",
      "ref": "scripts/cloud/managed-sandbox-live-acceptance.ts"
    },
    "independence": { "producer_may_verify": false },
    "oracle": {
      "evaluator_ref": "scripts/cloud/managed-sandbox-live-acceptance.ts",
      "statement": "An independent identity reproduces pinned SDK staging plus owner-gated GCP, Desktop, and Sarah full lifecycle journeys and binds revisions, identities, measured cost, zero residue, rollback, limits, and owner observation."
    },
    "technique": "independent_live_acceptance"
  }
]
```

## Gates

The contract gate can become observable before the runtime exists. The runtime
gate additionally requires target, broker, fault, consumer, and cleanup proof.
The live gate is strictly higher and cannot accept fixture or fake evidence.

```assurancespec-gates
[
  {
    "expression": "Every obligation targeting GATE-SBX-CONTRACT has oracle and negative-control observations from ENV-SBX-LOCAL-CONTRACT, exact subject and dependency provenance are unchanged, and an independent reviewer records no blocking invariant failure.",
    "id": "GATE-SBX-CONTRACT"
  },
  {
    "expression": "GATE-SBX-CONTRACT is green and every GATE-SBX-RUNTIME obligation has staged target, broker, runtime, consumer, fault, cost, revoke, and cleanup observations with no false readiness, duplicate effect, private leak, or residue.",
    "id": "GATE-SBX-RUNTIME"
  },
  {
    "expression": "GATE-SBX-RUNTIME is green and AO-MSB-AC-18-01 is independently reproduced on owner-gated live GCP plus Desktop and Sarah using exact deployed revisions; fixture, fake, producer-only, stale, or partial evidence is ineligible.",
    "id": "GATE-SBX-LIVE"
  }
]
```

## Evidence Policy

Links and issue comments remain pointers, not verdicts. Each ready obligation
requires the exact environment binding, oracle observation, deliberately
failing negative control, and independent review named below. Missing, stale,
partial, producer-only, fixture-for-live, or unreviewed evidence is
`INCONCLUSIVE`. It never rounds green.

```assurancespec-evidence-policy
{
  "links_are_verdicts": false,
  "missing_evidence_verdict": "INCONCLUSIVE",
  "policy_state": "designed",
  "required_for_ready_obligation": [
    "oracle_observation",
    "falsifier_observation",
    "environment_binding",
    "independent_review"
  ]
}
```

## Authority Boundaries

This proposal cannot admit, execute, verify, waive, release, or change a public
promise.
Operating agents may produce scoped evidence under root authority revision 6.
Only a distinct independent reviewer may verify it.
A release operator remains bound to the existing product and release gates.

```assurancespec-authority
{
  "admitted_roles": [
    "operating_agent"
  ],
  "policy_state": "designed",
  "proposal_may_change_public_promises": false,
  "proposal_may_execute": false,
  "proposal_may_release": false,
  "proposal_may_self_admit": false,
  "proposal_may_verify": false,
  "release_roles": [
    "release_operator"
  ],
  "verifier_roles": [
    "independent_reviewer"
  ]
}
```
