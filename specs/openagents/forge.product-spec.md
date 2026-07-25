---
spec_format_version: "0.1"
title: "OpenAgents Invite-Only Forge"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-25T00:00:00Z"
updated_at: "2026-07-25T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-writable-owner-matrix"
    label: "Writable Owner Matrix"
    after: "scope"
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "9242"
  openagents_lane: "FORGE-01 through FORGE-11 (#9243-#9253)"
  openagents_design: "docs/fable/2026-07-25-nostr-git-forge-invite-only-analysis.md"
  openagents_strategy_audit: "docs/forge/2026-07-25-buzz-ngit-openagents-forge-strategy-audit.md"
  openagents_sol_reconciliation: "docs/sol/MASTER_ROADMAP.md revision 136"
  openagents_assurance_level: "repository authority, signed collaboration, and invite-gated Git transport"
---

## Problem

OpenAgents uses GitHub for repository objects, changes, issues, reviews,
checks, and merge state. This adds a remote coordination dependency to each
agent work loop. It also separates repository state from OpenAgents identity,
work, and verification receipts.

OpenAgents already has a typed NIP-34 vocabulary, an owned relay, authenticated
Git receive-pack intake, scoped Git tokens, ref locks, packfile evidence, and
invite records. It does not yet offer one admitted Forge route or one complete
record authority contract. A product surface without those contracts can
create two writers for one field or can treat a projection as authority.

## Hypothesis

If OpenAgents gives invited users a Forge that hosts Git objects and uses
signed NIP-34 collaboration data, then invited humans and agents can complete
repository work without GitHub coordination on the critical path. A live
read-only GitHub mirror keeps the first migration reversible. One writer for
each record field keeps imported, native, and portable data distinct.

## Scope

```productspec-scope
in:
  - the canonical /forge route and all descendant document routes in the single openagents.com app
  - invite-only access for repository creation, Git transport, collaboration writes, review, checks, and merge
  - OpenAgents-hosted Git objects and refs for invited repositories
  - NIP-34 repository announcements, signed ref state, patches, proposals, issues, status events, and NIP-22 comments on the owned relay
  - openagents_git_authoritative for invited and migrated repositories
  - github_authoritative for repositories that are not migrated
  - a read-only GitHub mirror for each migrated repository
  - one canonical record vocabulary for project, repository, ref, change, work, actor binding, invite binding, revocation, NIP-98 replay consumption, membership reconciliation, review, check, receipts, action intents, and projection state
  - one writable owner for every canonical field
  - read-all and write-one conformance for admitted NIP-34 proposal forms
  - object-before-projection admission and purgatory for unresolved Git objects
  - server-side review and verification gates before a signed merge state exists
  - exact receipts for Git transport, admission, review, checks, merge, mirror, and recovery
out:
  - a separate Forge deployment or a second OpenAgents web app
  - a TypeScript packfile engine
  - public anonymous repository creation, Git reads, Git writes, or collaboration writes
  - GitHub write authority for a migrated repository
  - claim-ledger cutover, public GRASP reads, settlement, or a public marketplace
  - a new agent identity, credential home, memory store, or tool authority
cut:
  - general tenant onboarding before invite-only recovery and operations proof
  - a public GitHub replacement claim before governance and recovery receipts pass
  - an owned git-remote-nostr implementation before conformance evidence requires it
```

## Writable Owner Matrix

Each field below has one writable owner. A writer can append a new observed
version, but it cannot change a field that another writer owns. A projection
can copy a value for a read model. It cannot write the source field.

The source class has these values:

- `openagents_native` is state that an OpenAgents authority creates.
- `github_imported` is an immutable observation from GitHub.
- `nip34_portable` is decoded data from a verified Nostr event.
- `derived_projection` is a rebuildable view with source references.

The field lists below are complete for revision 1. A later issue must revise
this ProductSpec before it adds a canonical field. It must name one writer in
the same revision.

| Record                               | Source class         | Fields                                                                                                                                                                                                                                                              | Only writable owner                       |
| ------------------------------------ | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| `ForgeProject`                       | `openagents_native`  | `project_ref`, `tenant_ref`, `slug`, `title`, `description`, `visibility`, `membership_policy_ref`, `created_at`, `updated_at`                                                                                                                                      | Forge project registry                    |
| `ForgeRepository`                    | `openagents_native`  | `repository_ref`, `project_ref`, `slug`, `authority_mode`, `authority_generation`, `default_branch`, `object_format`, `canonical_clone_url`, `public_web_read`, `created_at`, `updated_at`                                                                          | Forge repository registry                 |
| `ForgeRepository`                    | `github_imported`    | `github_repository_id`, `github_full_name`, `github_default_branch`, `github_observed_at`, `github_source_ref`                                                                                                                                                      | GitHub import adapter                     |
| `ForgeRepository`                    | `nip34_portable`     | `announcement_event_id`, `announcement_coordinate`, `announcement_author_pubkey`, `announcement_created_at`                                                                                                                                                         | NIP-34 repository projector               |
| `ForgeRepository`                    | `derived_projection` | `mirror_health`, `mirror_head`, `mirror_checked_at`, `migration_state`, `projection_freshness`                                                                                                                                                                      | Forge repository projection service       |
| `ForgeRefSnapshot`                   | `openagents_native`  | `snapshot_ref`, `repository_ref`, `ref_name`, `object_id`, `object_format`, `authority_generation`, `committed_at`, `commit_receipt_ref`                                                                                                                            | OpenAgents Git ref authority              |
| `ForgeRefSnapshot`                   | `github_imported`    | `github_ref_name`, `github_object_id`, `github_observed_at`, `github_source_ref`                                                                                                                                                                                    | GitHub ref import adapter                 |
| `ForgeRefSnapshot`                   | `nip34_portable`     | `state_event_id`, `state_author_pubkey`, `state_created_at`, `state_signature_valid`, `state_object_id`                                                                                                                                                             | NIP-34 ref-state projector                |
| `ForgeRefSnapshot`                   | `derived_projection` | `agreement_state`, `object_available`, `purgatory_state`, `last_compared_at`                                                                                                                                                                                        | Forge ref reconciliation service          |
| `ForgeChange`                        | `openagents_native`  | `change_ref`, `repository_ref`, `author_binding_ref`, `base_ref`, `base_object_id`, `head_ref`, `head_object_id`, `state`, `created_at`, `updated_at`, `merge_decision_ref`                                                                                         | Forge change authority                    |
| `ForgeChange`                        | `github_imported`    | `github_pull_number`, `github_state`, `github_base_object_id`, `github_head_object_id`, `github_merged_object_id`, `github_observed_at`, `github_source_ref`                                                                                                        | GitHub change import adapter              |
| `ForgeChange`                        | `nip34_portable`     | `proposal_event_id`, `proposal_kind`, `proposal_author_pubkey`, `proposal_created_at`, `proposal_subject`, `proposal_content`, `proposal_object_refs`                                                                                                               | NIP-34 change projector                   |
| `ForgeChange`                        | `derived_projection` | `object_resolution_state`, `review_summary`, `check_summary`, `attention_state`, `projection_freshness`                                                                                                                                                             | Forge change projection service           |
| `ForgeWork`                          | `openagents_native`  | `work_ref`, `project_ref`, `repository_ref`, `title`, `scope`, `state`, `priority_ref`, `assignee_binding_refs`, `claim_ref`, `target_change_ref`, `blocker_refs`, `verification_refs`, `created_at`, `updated_at`                                                  | OpenAgents work authority                 |
| `ForgeWork`                          | `github_imported`    | `github_issue_number`, `github_issue_state`, `github_issue_title`, `github_labels`, `github_assignees`, `github_observed_at`, `github_source_ref`                                                                                                                   | GitHub work import adapter                |
| `ForgeWork`                          | `nip34_portable`     | `work_event_id`, `work_event_kind`, `work_author_pubkey`, `work_created_at`, `work_subject`, `work_content`                                                                                                                                                         | NIP-34 work projector                     |
| `ForgeWork`                          | `derived_projection` | `claim_health`, `agent_progress`, `verification_summary`, `attention_state`, `projection_freshness`                                                                                                                                                                 | Forge work projection service             |
| `ForgeActorBinding`                  | `openagents_native`  | `binding_ref`, `tenant_ref`, `account_ref`, `actor_kind`, `display_name`, `owner_binding_ref`, `role_refs`, `membership_state`, `binding_generation`, `created_at`, `revoked_at`                                                                                    | Forge identity binding authority          |
| `ForgeActorBinding`                  | `github_imported`    | `github_actor_id`, `github_login`, `github_observed_at`, `github_source_ref`                                                                                                                                                                                        | GitHub actor import adapter               |
| `ForgeActorBinding`                  | `nip34_portable`     | `nostr_pubkey`, `nostr_binding_event_id`, `nostr_binding_created_at`, `nostr_binding_signature_valid`                                                                                                                                                               | NIP-34 identity projector                 |
| `ForgeActorBinding`                  | `derived_projection` | `effective_permissions`, `credential_health`, `last_activity_at`                                                                                                                                                                                                    | Forge identity projection service         |
| `ForgeInviteBinding`                 | `openagents_native`  | `invite_binding_ref`, `tenant_ref`, `team_ref`, `invite_ref`, `invite_digest`, `invite_kind`, `inviter_binding_ref`, `invited_subject_ref`, `role_refs`, `issued_at`, `expires_at`, `accepted_at`, `accepted_binding_ref`, `provenance_source_refs`                 | Forge invitation authority                |
| `ForgeBurnedKeyFact`                 | `openagents_native`  | `burned_key_fact_ref`, `tenant_ref`, `key_kind`, `public_key`, `binding_ref`, `burn_reason_ref`, `burned_at`, `burn_sequence`, `source_refs`                                                                                                                        | Forge revocation authority                |
| `ForgeNip98ReplayConsumption`        | `openagents_native`  | `consumption_ref`, `tenant_ref`, `request_digest`, `event_id`, `actor_pubkey`, `http_method`, `canonical_path`, `body_digest`, `event_created_at`, `consumed_at`, `expires_at`, `authority_generation`, `result`                                                    | Forge NIP-98 replay guard                 |
| `ForgeMembershipReconciliationState` | `openagents_native`  | `reconciliation_ref`, `tenant_ref`, `team_ref`, `binding_ref`, `source_membership_generation`, `reconciliation_generation`, `observed_present`, `absence_first_observed_at`, `absence_confirmed_at`, `hysteresis_deadline`, `state`, `reconciled_at`, `source_refs` | Forge membership reconciliation authority |
| `ForgeReview`                        | `openagents_native`  | `review_ref`, `change_ref`, `reviewer_binding_ref`, `revision_object_id`, `verdict`, `body`, `submitted_at`, `supersedes_review_ref`                                                                                                                                | Forge review authority                    |
| `ForgeReview`                        | `github_imported`    | `github_review_id`, `github_review_state`, `github_review_commit_id`, `github_observed_at`, `github_source_ref`                                                                                                                                                     | GitHub review import adapter              |
| `ForgeReview`                        | `nip34_portable`     | `review_event_id`, `review_event_kind`, `review_author_pubkey`, `review_created_at`, `review_content`, `review_target_event_id`                                                                                                                                     | NIP-34 review projector                   |
| `ForgeReview`                        | `derived_projection` | `current_for_revision`, `gate_effect`, `projection_freshness`                                                                                                                                                                                                       | Forge review projection service           |
| `ForgeCheck`                         | `openagents_native`  | `check_ref`, `change_ref`, `revision_object_id`, `check_name`, `state`, `verdict`, `started_at`, `completed_at`, `executor_binding_ref`, `evidence_receipt_ref`                                                                                                     | OpenAgents verification authority         |
| `ForgeCheck`                         | `github_imported`    | `github_check_run_id`, `github_check_suite_id`, `github_check_status`, `github_check_conclusion`, `github_observed_at`, `github_source_ref`                                                                                                                         | GitHub check import adapter               |
| `ForgeCheck`                         | `nip34_portable`     | `status_event_id`, `status_event_kind`, `status_author_pubkey`, `status_created_at`, `status_content`                                                                                                                                                               | NIP-34 status projector                   |
| `ForgeCheck`                         | `derived_projection` | `required_for_merge`, `stale_for_revision`, `attention_state`                                                                                                                                                                                                       | Forge check projection service            |
| `ForgeEvidenceReceipt`               | `openagents_native`  | `receipt_ref`, `receipt_kind`, `subject_ref`, `subject_generation`, `actor_binding_ref`, `started_at`, `completed_at`, `verdict`, `artifact_refs`, `source_refs`, `redacted`                                                                                        | OpenAgents evidence receipt service       |
| `ForgeEvidenceReceipt`               | `github_imported`    | `github_delivery_id`, `github_request_id`, `github_observed_at`, `github_source_ref`                                                                                                                                                                                | GitHub receipt import adapter             |
| `ForgeEvidenceReceipt`               | `nip34_portable`     | `portable_receipt_event_id`, `portable_receipt_author_pubkey`, `portable_receipt_created_at`                                                                                                                                                                        | NIP-34 receipt projector                  |
| `ForgeDecisionReceipt`               | `openagents_native`  | `decision_ref`, `decision_kind`, `subject_ref`, `authority_generation`, `policy_ref`, `gate_refs`, `gate_results`, `blocker_refs`, `decided_by_binding_ref`, `decided_at`, `old_object_id`, `new_object_id`, `source_refs`, `redacted`                              | Forge decision gate                       |
| `ForgeDecisionReceipt`               | `derived_projection` | `mirror_receipt_ref`, `recovery_receipt_ref`, `receipt_chain_complete`                                                                                                                                                                                              | Forge receipt projection service          |
| `ForgeActionIntent`                  | `openagents_native`  | `intent_ref`, `actor_binding_ref`, `repository_ref`, `action_kind`, `target_ref`, `expected_generation`, `idempotency_key_hash`, `requested_at`, `expires_at`, `authority_decision_ref`, `state`, `result_receipt_ref`                                              | Forge action-intent authority             |
| `ForgeActionIntent`                  | `github_imported`    | `github_operation`, `github_target_ref`, `github_response_ref`, `github_observed_at`                                                                                                                                                                                | GitHub action adapter                     |
| `ForgeActionIntent`                  | `nip34_portable`     | `request_event_id`, `request_author_pubkey`, `request_created_at`                                                                                                                                                                                                   | NIP-34 action projector                   |
| `ForgeProjectionState`               | `derived_projection` | `projection_ref`, `projection_kind`, `source_class`, `source_coordinate`, `source_version`, `source_observed_at`, `projected_at`, `freshness_state`, `disagreement_state`, `recovery_state`, `cursor`, `source_refs`                                                | Forge projection supervisor               |

`authority_mode` can be `openagents_git_authoritative` or
`github_authoritative`. Only the Forge repository registry can change this
field. It changes the field only through an admitted transition that increments
`authority_generation`.

In `openagents_git_authoritative` mode, OpenAgents Git ref authority is the
only ref writer. The GitHub adapter can write mirror observations, but it
cannot write canonical refs. In `github_authoritative` mode, the GitHub import
adapter records immutable observations. The Forge ref reconciliation service
selects the applicable observed version for a read projection. It cannot
change the GitHub source or an OpenAgents ref.

Portable NIP-34 data keeps its event ID, author public key, created time, and
signature result. A portable event does not overwrite an OpenAgents-native
field. Imported GitHub data keeps its provider IDs, observation time, and
source reference. It does not become native state by copying.

`ForgeInviteBinding` is the durable link from one team invite to one accepted
actor binding. The record keeps the invite digest and provenance refs. It does
not contain the raw invite secret. `ForgeBurnedKeyFact` is append-only. A
higher `burn_sequence` can add evidence, but no writer can delete the fact or
make the key active again.

The Forge NIP-98 replay guard writes one `ForgeNip98ReplayConsumption` before a
protected effect. The unique event and request digests make a replay refuse.
`ForgeMembershipReconciliationState` records both presence and absence. An
absence does not revoke membership until the configured hysteresis deadline
passes and the reconciliation authority confirms it in a newer generation.
These operational records do not add fields to `ForgeActorBinding`.

## User Experience

An invited user opens `/forge` and sees an attention queue, projects, and
repositories. The user can open a repository, inspect its README and tree,
review branches and commits, and open a change or work item. Each view shows
the authority mode, freshness, source, disagreement, and recovery state.

The user can clone and fetch from `openagents.com`. An invited maintainer can
push under scoped Git authority. A human or agent can propose a change with
NIP-34, review it, attach checks and receipts, and request merge. Server-side
gates run before the maintainer signer creates new signed ref state.

For a migrated repository, GitHub is visible as a read-only mirror. A mirror
delay does not change the OpenAgents ref. For an unmigrated repository,
GitHub remains authoritative and the Forge shows imported observations without
claiming native merge authority.

## Acceptance Criteria

- **FORGE-AC-01:** The shared Start document route table admits `/forge`,
  `/forge/`, and all descendants. The Worker uses the same pattern list. The
  Start and Worker route-agreement test passes.
- **FORGE-AC-02:** Every canonical field in the revision-1 record vocabulary
  appears exactly once in the writable owner matrix. A source adapter cannot
  write a field that belongs to another owner.
- **FORGE-AC-03:** Each repository names one current `authority_mode` and one
  `authority_generation`. A transition serializes through the Forge repository
  registry and increments the generation.
- **FORGE-AC-04:** An invited and migrated repository uses
  `openagents_git_authoritative`. OpenAgents Git ref authority is its only
  canonical ref writer. GitHub receives a read-only mirror after the
  authoritative change.
- **FORGE-AC-05:** An unmigrated repository uses `github_authoritative`.
  GitHub observations remain immutable imported data. No Forge action claims
  native Git or merge authority for that repository.
- **FORGE-AC-06:** Portable NIP-34 and NIP-22 data keeps event provenance and
  signature results. The Forge reads all admitted proposal forms and writes
  one declared form. An event cannot overwrite native or imported source
  fields.
- **FORGE-AC-07:** A proposal that references unavailable Git objects stays in
  purgatory. No change, review, check, or merge projection can report it as
  actionable until the objects resolve.
- **FORGE-AC-08:** Repository creation, Git reads and writes, collaboration
  writes, review, checks, and merge require current invite membership and
  exact actor binding. Revocation survives replay and blocks a stale
  credential.
- **FORGE-AC-09:** Review, verification, generation, and policy gates pass
  before a signer creates the merge state. The Git ref commits before the
  relay projection. A relay or mirror failure cannot split canonical truth.
- **FORGE-AC-10:** The first-stage close journey has one invited human and one
  invited agent. They clone from `openagents.com`, do work, propose with
  NIP-34, review under server gates, merge through signed ref state, resolve
  the receipt chain, and update the GitHub mirror.
- **FORGE-AC-11:** The close journey uses no GitHub coordination read on its
  critical path. Its receipts bind the repository, authority generation,
  actors, old and new object IDs, proposal, reviews, checks, merge decision,
  signed state, and mirror outcome.
- **FORGE-AC-12:** A failed backup, restore, garbage collection, mirror, relay,
  or projection step has a typed result and recovery state. No projection,
  request acceptance, or UI success state is a completion receipt.
- **FORGE-AC-13:** Invite acceptance keeps a durable team, invite, actor,
  digest, inviter, role, time, and provenance binding without storing the raw
  invite secret. A burned key fact is monotonic and survives replay,
  reconciliation, and restart.
- **FORGE-AC-14:** Each protected NIP-98 request records one replay consumption
  before its effect. A reused event or request digest refuses. Membership
  reconciliation records source and reconciliation generations, present or
  absent observation, hysteresis, and confirmed absence without changing
  `ForgeActorBinding`.

## Success Metrics

```productspec-success-metrics
- id: forge_invited_change_completion
  metric: invited_human_and_agent_changes_completed_without_github_coordination_reads_on_the_critical_path
  target: "100% for the first dogfood close journey; >= 95% after invite alpha admission"
  window: each release candidate and rolling 30-day invite alpha
  segment: openagents_git_authoritative invited repositories
  source: forge_action_git_review_check_merge_and_mirror_receipts
- id: forge_field_authority_integrity
  metric: canonical_field_writes_with_exactly_one_admitted_owner
  target: "100%; zero dual-writer events"
  window: every write and every authority transition
  segment: all Forge canonical records
  source: forge_authority_decision_and_projection_reconciliation_receipts
- id: forge_mirror_reversibility
  metric: authoritative_merges_with_a_terminal_github_mirror_or_typed_recovery_outcome
  target: "100%"
  window: every openagents_git_authoritative merge
  segment: migrated repositories with a configured GitHub mirror
  source: forge_merge_and_github_mirror_receipts
- id: forge_projection_truth
  metric: projected_records_with_source_version_freshness_disagreement_and_recovery_state
  target: "100%"
  window: every Forge read model build
  segment: imported, native, portable, and derived Forge data
  source: forge_projection_state_audit
```

## Risks

- OpenAgents becomes a Git host for the invited set. Backup, restore, garbage
  collection, and recovery are release gates.
- Membership reduces anonymous abuse, but it does not remove authorization,
  key custody, moderation, or revocation risk.
- NIP-34 clients use more than one proposal form. The reader can lose data if
  it treats one form as the full protocol.
- A mirror can look authoritative to an operator. Each migrated repository
  must show that the GitHub copy is read-only.
- The existing Forge control-plane records predate this product vocabulary.
  Later issues must adapt them through explicit migrations. They must not
  infer field ownership from a similar name.

## Owner Gates

- The owner direction recorded on 2026-07-25 admits this invite-only web Forge
  and the issue program in epic #9242.
- Public anonymous Git access, general tenant onboarding, a public replacement
  claim, claim-ledger cutover, settlement, or a new infrastructure authority
  needs a separate admitted contract and its existing release gates.
- A production repository authority transition requires the exact repository,
  current generation, backup and restore receipts, mirror target, rollback,
  and owner-visible migration result.

## Receipts

- Start and Worker route-agreement test result for `/forge` descendants.
- ProductSpec validation result for this exact revision.
- Field-owner matrix audit with zero absent or duplicate owners.
- Invite, actor binding, Git transport, object admission, and purgatory
  receipts.
- Invite provenance, burned-key, NIP-98 replay-consumption, and membership
  reconciliation receipts.
- Review, verification, signed-state merge, projection, and GitHub mirror
  receipts.
- Backup, restore, garbage collection, recovery, and rollback receipts.
- The FORGE-10 dogfood close receipt binds the exact journey in
  `FORGE-AC-10` and `FORGE-AC-11`.

## Promise Links

- This ProductSpec authorizes no public promise transition by itself.
- A public “GitHub replacement” claim remains blocked until the promise
  registry names and verifies governance, recovery, public-read, and migration
  gates.
- Invite-alpha copy must state the membership limit and the effective
  repository authority mode.
