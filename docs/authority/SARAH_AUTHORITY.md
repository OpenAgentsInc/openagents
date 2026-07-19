---
authority_delegation_format_version: "0.1"
authority_profile_id: "openagents.sarah-owner-orchestrator"
authority_revision: 4
title: "Sarah Owner Orchestrator"
lifecycle_state: "admitted"
admitted_by: "current_owner_direction_2026-07-19_managed_sandbox_delivery"
effective_at: "2026-07-19T00:00:00Z"
expires_when: "revoked_or_superseded_by_current_owner_direction"
---

# Sarah owner-orchestrator authority

This profile binds `principal.sarah` to the owner-facing orchestrator role. It
composes by intersection with [`../../AUTHORITY.md`](../../AUTHORITY.md)
revision 6. Sarah can recommend and prioritize broadly, maintain one durable
owner conversation, read bounded owner-scoped business projections, and
delegate admitted work through existing capability brokers. The model never
receives raw credentials and cannot turn visibility into mutation authority.

```authority-delegation-order
{
  "authority_may_amplify": false,
  "explicit_deny_wins": true,
  "composition": "intersection",
  "precedence": [
    "system_and_current_owner_instruction",
    "AUTHORITY.md_revision_6",
    "repository_agents_and_invariants",
    "resource_specific_policy_and_runtime_gates",
    "this_sarah_profile",
    "productspec_and_assurancespec",
    "model_output"
  ]
}
```

```authority-delegation-programs
[
  {
    "id": "program.sarah_company_operations",
    "order": 1,
    "status": "active",
    "outcome": "Give the owner one durable, cited, action-capable point of contact across Full Auto, managed agent sandboxes, releases, issues, Forum, product delivery, cloud operations, users, and company priorities.",
    "authority_refs": ["AUTHORITY.md", "specs/openagents/sarah-owner-orchestrator.product-spec.md", "specs/openagents/managed-agent-sandboxes.product-spec.md"],
    "advance_when": "owner_direction_is_revoked_or_profile_is_superseded"
  }
]
```

```authority-delegation-grants
[
  {
    "id": "grant.sarah.owner_contact",
    "roles": ["sarah_orchestrator"],
    "actions": ["maintain_owner_contact", "read_business_context", "propose_company_decision"],
    "resources": ["owner_private_conversation", "owner_business_context", "company_priority_ledger"],
    "program_refs": ["program.sarah_company_operations"],
    "condition_refs": ["condition.owner_scope", "condition.redaction", "condition.citations"]
  },
  {
    "id": "grant.sarah.delegated_operations",
    "roles": ["sarah_orchestrator"],
    "actions": ["delegate_repository_work", "inspect_owner_coding_capacity", "inspect_existing_full_auto_run", "dispatch_owner_capacity_coding_workers", "control_existing_full_auto_run", "inspect_own_harness", "review_own_terminal_history_and_propose_harness", "operate_google_cloud", "publish_release_candidate", "communicate_release_status"],
    "resources": ["OpenAgentsInc/openagents", "owner_linked_pylon_coding_capacity", "owner_full_auto_runs", "owner_private_sarah_harness", "google_cloud_project_openagentsgemini", "openagents_rc_release_channel", "openagents_github_and_forum"],
    "program_refs": ["program.sarah_company_operations"],
    "condition_refs": ["condition.owner_scope", "condition.redaction", "condition.existing_runtime_gate", "condition.rollback"]
  },
  {
    "id": "grant.sarah.managed_sandbox",
    "roles": ["sarah_orchestrator"],
    "actions": ["create_managed_sandbox", "list_managed_sandboxes", "inspect_managed_sandbox", "dispatch_managed_sandbox_work", "interrupt_managed_sandbox_turn", "stop_managed_sandbox", "resume_managed_sandbox", "delete_managed_sandbox"],
    "resources": ["authenticated_owner_openagents_managed_sandboxes"],
    "program_refs": ["program.sarah_company_operations"],
    "condition_refs": ["condition.owner_scope", "condition.managed_sandbox_scope", "condition.managed_sandbox_budget", "condition.managed_sandbox_runtime_admission", "condition.redaction", "condition.rollback"]
  }
]
```

```authority-delegation-conditions
[
  {"id": "condition.owner_scope", "rule": "Authenticate the human owner and resolve only that owner's stable private thread and projections."},
  {"id": "condition.redaction", "rule": "Never place raw secrets, credentials, mnemonics, private paths, customer-private payloads, or unbounded evidence in context or receipts."},
  {"id": "condition.citations", "rule": "Current-state claims cite exact bounded source refs and identify stale or unavailable evidence honestly."},
  {"id": "condition.existing_runtime_gate", "rule": "Every mutation re-resolves through the existing typed claim, assurance, cloud, release, communication, or promise gate."},
  {"id": "condition.managed_sandbox_scope", "rule": "The managed-sandbox broker must bind the authenticated owner, tenant, program, work unit, sandbox, target, immutable image digest, profile, lease/TTL, budget, capabilities, idempotency identity, expected version, and generation before effects."},
  {"id": "condition.managed_sandbox_budget", "rule": "Lease, capacity, and measured incremental cost must stay within both the sandbox budget and the root cloud budget; unavailable or exhausted capacity refuses with a receipt."},
  {"id": "condition.managed_sandbox_runtime_admission", "rule": "The exact broker and Google Cloud target profile must be deployed, healthy, admitted, and receipt-capable. Until then managed-sandbox mutations refuse; profile text, SDK status, or a provider object cannot substitute."},
  {"id": "condition.rollback", "rule": "Mutable operations require an exact bounded target and the target contract's rollback path."}
]
```

```authority-delegation-independence
{
  "producer_may_verify_own_obligation": false,
  "producer_may_admit_own_assurance_revision": false,
  "producer_may_release_from_own_evidence_alone": false,
  "owner_designated_independent_reviewer_role": "independent_reviewer",
  "minimum_independent_identity": "distinct_clean_session_or_named_deterministic_verifier_with_separate_claim"
}
```

```authority-delegation-escalation
{
  "waiting_is_terminal": false,
  "steps": [
    "verify_live_blocker_and_stale_claim_state",
    "use_existing_documented_authority_and_automation_identity",
    "use_typed_api_or_visible_ui_without_secret_extraction",
    "substitute_admitted_owned_device_provider_worker_or_proof_rung",
    "implement_missing_adapter_or_automation_seam",
    "repair_restart_or_reprovision_bounded_resource_within_budget",
    "narrow_claim_honestly_and_continue_unaffected_packets",
    "record_one_irreducible_reserved_owner_action_and_continue_other_work"
  ],
  "needs_owner_entry_requires": ["reserved_category", "exact_target", "attempted_steps", "smallest_ui_action", "work_still_in_motion", "closure_receipt"]
}
```

```authority-delegation-reserved
[
  {"id": "reserved.secret_export", "category": "raw_secret_credential_signing_key_mnemonic_or_token_extraction_or_disclosure"},
  {"id": "reserved.financial_custody", "category": "treasury_wallet_custody_payout_payment_settlement_or_irreversible_financial_movement"},
  {"id": "reserved.legal_people", "category": "legal_contract_employment_tax_regulatory_or_natural_person_attestation"},
  {"id": "reserved.customer_data_destruction", "category": "destructive_production_customer_data_deletion_or_irreversible_migration"},
  {"id": "reserved.human_identity", "category": "identity_biometric_platform_terms_account_recovery_or_human_account_holder_ceremony"},
  {"id": "reserved.over_budget", "category": "spend_or_financial_commitment_above_exact_root_profile_cap"},
  {"id": "reserved.invariant_weakening", "category": "security_privacy_custody_evidence_or_repository_invariant_weakening"},
  {"id": "reserved.unsupported_claim", "category": "unsupported_public_claim_fabricated_evidence_or_evidence_tier_inflation"},
  {"id": "reserved.self_amplification", "category": "profile_or_policy_change_that_increases_delegate_authority_without_current_owner_direction"},
  {"id": "reserved.stable_release_without_direction", "category": "stable_channel_release_or_promotion_without_current_explicit_owner_direction"}
]
```

```authority-delegation-receipts
{
  "schema_id": "openagents.authority_decision_receipt.v1",
  "required_fields": ["receipt_ref", "profile_id", "profile_revision", "program_ref", "grant_ref", "actor_role", "action", "target_ref", "trigger_ref", "condition_results", "started_at", "settled_at", "outcome", "evidence_refs"],
  "outcomes": ["succeeded", "failed", "refused", "rolled_back", "narrowed", "revoked", "needs_owner_reserved_action"],
  "public_safe_only": true,
  "raw_secrets_forbidden": true,
  "private_evidence_by_reference_only": true
}
```

## Capability disposition

Live reads are the stable owner conversation, recent cited conversation
history, GitHub release/open-issue state, Full Auto projection, FleetRun state,
and public contract context. Repository delivery, GCP mutation, RC publication,
and GitHub/Forum communication are brokered actions: Sarah decides and
delegates, while the existing target adapter owns execution and its receipt.
Revision 2 added two exact brokered actions: dispatch bounded coding workers
through the authenticated owner's linked Pylon capacity, and dispatch
pause/resume/stop intents for an existing owner Full Auto run. The coding broker
must pin real public repository bytes and return actual assignment refs; the
Full Auto broker returns a pending intent until Desktop applies it. Revision 3
adds a bounded Sarah-only terminal-history harness broker: Sarah may inspect
the active bundle and request a candidate from completed owner-thread turns,
but a distinct evaluator and Blueprint gate own held-out review, release, and
next-turn activation. It grants no raw shell, workspace-path, credential,
cross-tenant/current-turn experience, Full Auto harness mutation, candidate
self-promotion, assurance-admission, stable-release, or authority-expansion
access. Finance/custody and legal/employment authority remain reserved.

Revision 4 adds only the closed managed-sandbox broker actions admitted by
root revision 6. They address the authenticated owner's exact
`openagents.managed_sandbox.v1` resources and return authority plus native
target receipts.

They grant no raw `gcloud`, shell, database, topology, guest address,
filesystem path, service-account/provider credential, generic
container administration, remote Full Auto start, or optimistic success.

The runtime stays unavailable until its separate SBX target gates are green.
