---
assurance_spec_format_version: "0.1"
assurance_spec_id: "assurance.omega.full.auto.host"
assurance_revision: 5
title: "Omega Full Auto Host AssuranceSpec"
artifact_type: "product_assurance"
lifecycle_state: "proposed"
author: "OpenAgents"
---

## Assurance Objective

This proposal binds proof design for the Omega Full Auto host delta in
`specs/omega/full-auto.product-spec.md` revision 1. It preserves the Desktop
Full Auto lifecycle as upstream authority. The design tests that Omega does not
create a second lifecycle, weaken guardrails, leak private run data, or expand
first-port scope.

This proposal does not admit release or public claims. It does not replace
Desktop Full Auto AssuranceSpec revision 6. No observation, reviewer decision,
owner gate, or release verdict is present merely because this design validates.

## Subject

The subject binds the exact ProductSpec bytes, revision, path, and all eight
stable criteria. The Desktop ProductSpec, Desktop AssuranceSpec, and contract
freeze remain upstream authority, but they are not alternate subjects for this
Omega host-delta proposal.

```assurancespec-subject
{
  "product_spec": {
    "criterion_refs": [
      "OMEGA-FA-AC-01",
      "OMEGA-FA-AC-02",
      "OMEGA-FA-AC-03",
      "OMEGA-FA-AC-04",
      "OMEGA-FA-AC-05",
      "OMEGA-FA-AC-06",
      "OMEGA-FA-AC-07",
      "OMEGA-FA-AC-08"
    ],
    "document_digest": "sha256:09f8c2c2c14df6f5272737e26b85dbe3f20704ce66345a9377353710a8d6dddc",
    "path": "specs/omega/full-auto.product-spec.md",
    "profile": "openagents_executable_v0.1_exact_document",
    "spec_format_version": "0.1",
    "spec_revision": 1
  }
}
```

## Risk Model

The risk model covers authority forks, guardrail weakening, private-data
leakage, scope expansion, chat ambiguity, and false host state. These risks are
separate because each needs a distinct falsifier and evidence seam. A passing
unit suite cannot silently stand in for the packaged host journey.

```assurancespec-risks
{
  "risks": [
    {
      "id": "RISK-OMEGA-FA-SECOND-LIFECYCLE",
      "statement": "GPUI or another Omega surface can create a second durable lifecycle, active-run limit, lease model, or closeout authority."
    },
    {
      "id": "RISK-OMEGA-FA-GUARDRAIL-WEAKENING",
      "statement": "Configuration or UI can weaken workspace binding, own-capacity-only routing, or the prohibition on rate-limit reset triggering."
    },
    {
      "id": "RISK-OMEGA-FA-PRIVATE-DATA-LEAK",
      "statement": "A receipt, notification, or Sync projection can expose objective text, workspace paths, credentials, or transcript text."
    },
    {
      "id": "RISK-OMEGA-FA-SCOPE-EXPANSION",
      "statement": "The first port can introduce MemoHarness adaptation or initiative without a later admitted freeze revision."
    },
    {
      "id": "RISK-OMEGA-FA-CHAT-AMBIGUITY",
      "statement": "A composer toggle, ACP panel, ordinary chat path, or ambient preference can start Full Auto authority."
    },
    {
      "id": "RISK-OMEGA-FA-FABRICATED-HOST-STATE",
      "statement": "The service can accept a nonexistent thread, stale generation, unready lane, late reply, or fabricated workspace, turn, interruption, or evidence fact."
    }
  ],
  "source_digest": "sha256:e7fa4f0397f7e96f14836a434109120606cc0be704dc811cceea94ed3bcc1787",
  "source_snapshot": "The host can fork durable authority, weaken guardrails, leak private mission data, expand scope, disguise a run as chat, or accept fabricated host state."
}
```

## Assurance Scope

All eight ProductSpec criteria are in scope, and none is not applicable. The
source profile covers the frozen lifecycle, lease, guardrail, routing,
redaction, mutation-authority, and scope contracts in `omega-effectd`. The
packaged profile covers the real Omega host seam, dedicated launcher, ordinary
chat separation, process supervision, restart behavior, and owner-visible
journey.

The design excludes MemoHarness and initiative implementation. It also excludes
an inference that source tests, fixture tests, links, or historical Desktop
evidence prove the current packaged Omega candidate.

## Environments

All environment profiles remain proposed. Admission must bind the OpenAgents
source profile to an exact commit, tree, toolchain, lockfile, and commands. It
must bind the Omega source profile to an exact external commit and clean tree.
The packaged profile must bind to the same Omega source commit, signed package
digest, and embedded `omega-effectd` digest. It must also bind the operating
system, hardware, data roots, provider lanes, workspace, thread, and restart
conditions.

```assurancespec-environments
{
  "profiles": [
    {
      "id": "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "status": "proposed"
    },
    {
      "id": "ENV-OMEGA-FA-OMEGA-SOURCE",
      "status": "proposed"
    },
    {
      "id": "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64",
      "status": "proposed"
    }
  ],
  "repository_inventory": {
    "candidate_artifact_refs": [],
    "declared_scripts": [],
    "diagnostics": [
      "split_repository_subject"
    ],
    "inventory_digest": "sha256:13cef510a746daf9c1d6b2766fef971b7f66c7392a70709fd61ccd271f1b02e4",
    "repository_label": "OpenAgentsInc/openagents-plus-OpenAgentsInc/omega",
    "state": "absent",
    "tracked_file_count": 0,
    "truncated": false
  }
}
```

## Obligations

Each obligation preserves one original Full Auto law and its negative test.
Repository paths identify candidate seams, not verdicts. A `ready` projection
means that proof design is complete. Execution, evidence freshness, independent
review, owner observation, admission, and release remain separate decisions.

```assurancespec-obligations
[
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-run-registry.ts",
      "packages/omega-effectd/src/protocol/server.fa07-proof.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-01"
    ],
    "disposition": "required",
    "domains": [
      "lifecycle_authority",
      "transition_safety"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "frozen_contract_plus_packaged_host",
      "required_kinds": [
        "freeze_digest_observation",
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "illegal_transition_or_second_lifecycle",
      "ref": "packages/omega-effectd/src/protocol/server.fa07-proof.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-01-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/engine/full-auto-run-registry.ts",
      "statement": "The frozen Desktop ten-state graph is the only durable lifecycle, every legal transition matches the freeze, and every illegal transition returns a typed refusal."
    },
    "source_claim_digest": "sha256:085d39813a9db32dcff3a6a684bc41c0c30e9ea7df0d44cb2c389287eb25b2fa",
    "source_claim_snapshot": "Omega Full Auto keeps the Desktop ten-state lifecycle and\nthe exact legal transition graph from `full-auto-run-registry.ts` at the\nfreeze digests. An illegal transition refuses with a typed error.",
    "technique": "contract_matrix_and_installed_authority_review",
    "title": "Assure OMEGA-FA-AC-01"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-capacity.ts",
      "packages/omega-effectd/src/engine/full-auto-capacity.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-02"
    ],
    "disposition": "required",
    "domains": [
      "capacity",
      "lease_exclusivity"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "deterministic_capacity_plus_concurrent_host",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "concurrency_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "ninth_active_run_or_duplicate_thread_lease",
      "ref": "packages/omega-effectd/src/engine/full-auto-capacity.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-02-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/engine/full-auto-capacity.test.ts",
      "statement": "Eight distinct eligible threads can hold active runs, a ninth refuses, and a second active lease for one thread refuses before durable mutation."
    },
    "source_claim_digest": "sha256:1367131068d6451d5b3641ea797bfd6728cbaf69af7a580c867ca473520492d4",
    "source_claim_snapshot": "The active run limit is exactly 8. Each thread holds at\nmost one active lease.",
    "technique": "deterministic_concurrency_matrix",
    "title": "Assure OMEGA-FA-AC-02"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-reconcile.ts",
      "packages/omega-effectd/src/omega-effectd.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-03"
    ],
    "disposition": "required",
    "domains": [
      "guardrail_authority",
      "configuration_immunity"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "mutation_matrix_plus_installed_ui_review",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "ui_control_inventory",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "config_or_ui_guardrail_override",
      "ref": "packages/omega-effectd/src/omega-effectd.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-03-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/engine/full-auto-reconcile.ts",
      "statement": "The three frozen guardrails remain mandatory after every configuration decode, control request, retry decision, and installed UI action."
    },
    "source_claim_digest": "sha256:85437249c07a38a7fb2d8a6a6f260f28cac4cc272fee6c3a023987951ab77ca7",
    "source_claim_snapshot": "The non-overridable guardrail set is exactly\n`workspace_binding`, `own_capacity_only`, and\n`no_rate_limit_reset_triggering`. No config or UI control may weaken it.",
    "technique": "negative_configuration_and_ui_inventory",
    "title": "Assure OMEGA-FA-AC-03"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-routing.ts",
      "packages/omega-effectd/src/engine/full-auto-routing.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-04"
    ],
    "disposition": "required",
    "domains": [
      "provider_routing",
      "lane_admission"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "routing_matrix_plus_live_lane_readiness",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "lane_readiness_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "wrong_default_order_or_unready_lane_dispatch",
      "ref": "packages/omega-effectd/src/engine/full-auto-routing.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-04-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/engine/full-auto-routing.test.ts",
      "statement": "Without Advanced policy the admitted set is codex-local and claude-local in that order, and dispatch requires fresh readiness for the selected lane."
    },
    "source_claim_digest": "sha256:257a272ec6882de23806e26e30f646c750a79eaa28f106b267efe8ee62af8122",
    "source_claim_snapshot": "The first admitted action-lane set includes\n`codex-local` and `claude-local`. Default routing order is `codex-local`\nthen `claude-local` when Advanced policy is absent.",
    "technique": "routing_and_readiness_matrix",
    "title": "Assure OMEGA-FA-AC-04"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-run-report.ts",
      "packages/omega-effectd/src/engine/full-auto-run-report.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-05"
    ],
    "disposition": "required",
    "domains": [
      "receipt_redaction",
      "sync_projection",
      "notification_privacy"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "forbidden_field_matrix_plus_packaged_projection",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "redaction_receipt",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "forbidden_private_field_in_public_projection",
      "ref": "packages/omega-effectd/src/engine/full-auto-run-report.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-05-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/engine/full-auto-run-report.test.ts",
      "statement": "Receipts, notifications, and Sync projections contain only the frozen public-safe fields, while every forbidden objective, path, credential, and transcript sentinel remains absent."
    },
    "source_claim_digest": "sha256:9738a19c7360b267191ff3f94620c15fb8c6a71f58d5cfd4b1c5f40a13f8fb16",
    "source_claim_snapshot": "Receipts, notifications, and Sync projections carry only\npublic-safe receipt fields for schema\n`openagents.desktop.full_auto_run_receipt.v1`. Raw objective text, workspace\npaths, credentials, and transcript text are forbidden.",
    "technique": "forbidden_field_and_projection_matrix",
    "title": "Assure OMEGA-FA-AC-05"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-PACKAGED",
    "candidate_artifact_refs": [
      "packages/omega-effectd/src/engine/full-auto-run-actions.ts",
      "packages/omega-effectd/src/protocol/host-bridge.ts",
      "packages/omega-effectd/src/protocol/server.host-bridge.test.ts"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-06"
    ],
    "disposition": "required",
    "domains": [
      "mutation_authority",
      "host_generation_fence",
      "process_supervision"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-OMEGA-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "generation_fenced_host_integration_plus_restart",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "process_restart_receipt",
        "architecture_review",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "fabricated_or_stale_host_fact_or_second_store",
      "ref": "packages/omega-effectd/src/protocol/server.host-bridge.test.ts"
    },
    "id": "AO-OMEGA-FA-AC-06-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "packages/omega-effectd/src/protocol/server.host-bridge.test.ts",
      "statement": "All durable mutations pass through full-auto-run-actions in supervised omega-effectd, host facts are generation fenced, restart restores the same durable run, and GPUI contains no durable run store."
    },
    "source_claim_digest": "sha256:fb2cf45b202f2f988cb60fedd0ef66f7d7ff061dc305f7d9af0eda7e8a81767a",
    "source_claim_snapshot": "Durable run mutation occurs only through\n`full-auto-run-actions` (or its released successor) inside supervised\n`omega-effectd`. GPUI, ACP panels, and ordinary chat are not run authority.",
    "technique": "cross_repository_authority_and_restart_review",
    "title": "Assure OMEGA-FA-AC-06"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-SOURCE",
    "candidate_artifact_refs": [
      "docs/omega/2026-07-24-full-auto-contract-freeze.md",
      "docs/omega/2026-07-24-omega-full-auto-proof-matrix.md"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-07"
    ],
    "disposition": "required",
    "domains": [
      "scope_authority",
      "deferred_capabilities"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OPENAGENTS-SOURCE",
      "ENV-OMEGA-FA-OMEGA-SOURCE"
    ],
    "evidence": {
      "proof_rung": "packet_inventory_and_repository_scan",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "scope_review",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "first_port_memoharness_or_initiative_path",
      "ref": "docs/omega/2026-07-24-full-auto-contract-freeze.md"
    },
    "id": "AO-OMEGA-FA-AC-07-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/omega/2026-07-24-omega-full-auto-proof-matrix.md",
      "statement": "The complete first-port packet and both repositories contain no MemoHarness adaptation or initiative path beyond deferred documentation."
    },
    "source_claim_digest": "sha256:e869e22d5ea41d78617525a140e39ef252a4e6872f6a0019cc2e275f79af99fd",
    "source_claim_snapshot": "MemoHarness and initiative remain deferred for\n`OMEGA-FA-01` through `OMEGA-FA-07` unless a later freeze revision admits\nthem.",
    "technique": "bounded_scope_inventory",
    "title": "Assure OMEGA-FA-AC-07"
  },
  {
    "activation_gate": "GATE-OMEGA-FA-PACKAGED",
    "candidate_artifact_refs": [
      "docs/omega/2026-07-24-omega-full-auto-gpui-launcher.md",
      "docs/omega/2026-07-24-omega-full-auto-proof-matrix.md"
    ],
    "criterion_refs": [
      "OMEGA-FA-AC-08"
    ],
    "disposition": "required",
    "domains": [
      "dedicated_run_ui",
      "chat_authority_separation"
    ],
    "environment_refs": [
      "ENV-OMEGA-FA-OMEGA-SOURCE",
      "ENV-OMEGA-FA-PACKAGED-MACOS-ARM64"
    ],
    "evidence": {
      "proof_rung": "source_inventory_plus_owner_visible_ui",
      "required_kinds": [
        "oracle_observation",
        "falsifier_observation",
        "environment_binding",
        "accessibility_observation",
        "owner_observation",
        "independent_review"
      ]
    },
    "falsifier": {
      "expected_verdict": "REFUTED",
      "kind": "ordinary_chat_or_composer_starts_full_auto",
      "ref": "docs/omega/2026-07-24-omega-full-auto-proof-matrix.md"
    },
    "id": "AO-OMEGA-FA-AC-08-01",
    "independence": {
      "producer_may_verify": false
    },
    "oracle": {
      "evaluator_ref": "docs/omega/2026-07-24-omega-full-auto-gpui-launcher.md",
      "statement": "Only the dedicated Full Auto launcher starts a run, while ordinary chat, ACP panels, composer controls, ambient preferences, and restored workspaces do not start one."
    },
    "source_claim_digest": "sha256:baae88de8646dd2daeb213536b33948991f244e0ee3ca29802bdf4e123b7b53b",
    "source_claim_snapshot": "Full Auto remains a dedicated run. It is never a\ncomposer toggle or ambient chat preference.",
    "technique": "negative_entrypoint_inventory_and_installed_ui",
    "title": "Assure OMEGA-FA-AC-08"
  }
]
```

## Gates

The source gate requires fresh oracle and falsifier observations for every
OpenAgents source obligation. The packaged gate additionally requires exact
cross-repository source and package bindings, real host facts, process restart,
the dedicated launcher, accessibility, and owner-visible observations. The
release gate requires both lower gates plus an independent admission receipt.

No gate admits this proposal or releases software by itself. Fixture-only and
harness-only evidence cannot satisfy the packaged gate.

```assurancespec-gates
[
  {
    "expression": "all source obligations have fresh oracle, falsifier, environment, and independent-review evidence bound to exact OpenAgents and Omega commits",
    "id": "GATE-OMEGA-FA-SOURCE"
  },
  {
    "expression": "the source gate passes and the exact signed package has fresh host, restart, provider-lane, accessibility, owner-observation, and independent-review evidence",
    "id": "GATE-OMEGA-FA-PACKAGED"
  },
  {
    "expression": "the packaged gate passes and an authorized independent reviewer admits this exact AssuranceSpec and candidate digest with a separate receipt",
    "id": "GATE-OMEGA-FA-RELEASE"
  }
]
```

## Evidence Policy

Links, source paths, historical receipts, and test names are pointers, not
verdicts. Missing, stale, producer-only, fixture-only, or candidate-mismatched
evidence remains INCONCLUSIVE. Every ready obligation needs both its positive
oracle and negative falsifier, an exact environment binding, and independent
review. Packaged obligations also need the additional kinds named above.

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

This proposal cannot admit itself, execute tests, verify its own producer,
waive an obligation, release software, or change a public promise. The owner
has designated an independent reviewer role for admission, the OpenAgents
assurance reviewer role for verification, and the OpenAgents owner role for
release. Those designations do not admit this proposal or authorize its
producer to review or admit the revision.

```assurancespec-authority
{
  "admitted_roles": [
    "owner_designated_independent_reviewer"
  ],
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
