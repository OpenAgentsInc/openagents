# Dhruv protocol conformance matrix

Role by role, which crate implements each contract in the
[implementation proposal](implementation-proposal.md), the exact Rust test
that exercises it, the hard cases that test covers, and whether anything
beyond fixtures has been shown. Every `crates/…::name` reference below is
checked by `cargo test -p dhruv-conformance --test matrix`: the file must
exist and define that test. Run each row's crate with `cargo test -p <crate>`.

**Fixture support is not qualification.** "Fixture" means in-process
adapters with isolated state, synthetic keys or roots, and fake funds. No
row here is qualified on live money or real hardware unless it says so;
the owner steps for that are in [`NEEDS_OWNER.md`](../../../NEEDS_OWNER.md).

Hard cases: **I** interrupted (crash, restart, lost reply), **D**
disconnected (lost message, unavailable party), **R** revoked, **C**
competing controller or conflicting record, **$** cost and unknown
accounting.

## Cross-contract runs

These runs compose the actual owners with isolated state. Each row names its
executable test. The cloud-vault reader verifies a retained hardware run at
its capture time; it does not recreate that machine's private keys.

| Run | Contracts joined | Fixture | Cases |
| --- | --- | --- | --- |
| Sealed attested job | ATT release + approval + rebuild, raw SEV-SNP, HPKE carriage, gift wrap, streamed output, PYLON sealed receipt | `crates/dhruv-conformance/tests/attested_job.rs::an_approved_attested_release_answers_a_sealed_request_with_a_public_receipt` | I (replay after restart refused), $ (receipt over ciphertext, fake regtest payment) |
| Sealed job failures | ATT approval, key book, streamed output | `crates/dhruv-conformance/tests/attested_job.rs::disconnection_revocation_and_competing_endpoints_refuse` | D (lost chunk), R (revoked approval), C (older and rival endpoint keys) |
| Funded labor and credit | LAB roles, rework, PYLON receipt, pay-ledger funding and payout, SOV income, X402 purchase, durable credit and metered use | `crates/dhruv-conformance/tests/funded_labor.rs::an_interrupted_rework_order_is_accepted_funded_once_and_costed_honestly` | I (crash before send, lost credit reply, treasury and credit restart, exact retry), $ (one wallet send, one income credit, distinct units, unknown fees stay null) |
| Labor without a buyer | LAB resolver, finals, pay-ledger | `crates/dhruv-conformance/tests/funded_labor.rs::a_disconnected_buyer_and_competing_finals_never_pay_by_default` | D (buyer unavailable), C (competing finals stop funding) |
| Managed execution | ENV allocator and local resource, COORD claims and call accounting, SESS native loop engine, CJ | `crates/session-host/src/tests/cj.rs::real_cj_open_and_turn_share_actual_environment_and_call_fences` | I (exact retries, one native turn, one charge per operation) |
| Uncertain native start | The same ENV, COORD, and SESS owners | `crates/session-host/src/tests/cj.rs::cj_lost_start_and_restart_keep_the_original_charge_and_unknown_outcome` | I, $ (restart retains unknown and its original charge; no second start) |
| Revoked execution | The same ENV, COORD, and SESS owners | `crates/session-host/src/tests/cj.rs::current_coord_revocation_and_environment_drain_block_native_continuations` | R, C (either authority stops native continuation without another call charge) |
| Synthetic cloud vault | ATT Google chain, image, signing key and HPKE binding; VAULT certificate, object, lease, and private answer | `crates/dhruv-conformance/tests/sealed_vault.rs::a_captured_attested_vault_run_verifies_its_endpoint_and_completion` | R (one use, revocation, expiry), C (substituted HPKE key); real TDX, same-process client, ephemeral LocalKms |
| Vault evidence reader | The same ATT and VAULT records and signed completion | `crates/dhruv-conformance/tests/sealed_vault.rs::changed_vault_records_certificates_ciphertexts_and_completion_are_refused` | C (changed records, certificate, ciphertexts, or result), R (expired endpoint); retained public evidence |

[Composition evidence from 2026-10-16](conformance-evidence-2026-10-16.md)
records the cloud image, source, synthetic limits, and instance cleanup.

## Trajectories — ATIF

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Public sharer | `atif-nostr` | `crates/atif-nostr/src/tests.rs::public_sharing_is_explicit_and_binds_tags_author_and_pins` | | Fixture |
| Private recipient | `atif-nostr` | `crates/atif-nostr/src/tests.rs::private_sharing_encrypts_every_object_with_a_fresh_scope` | | Fixture |
| Reader | `atif-nostr` | `crates/atif-nostr/src/tests.rs::missing_duplicate_and_changed_chunks_cannot_be_rendered_as_complete` | D | Fixture |

## Plugins — REG and EXT

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Curator | `plugin-registry` | `crates/plugin-registry/src/tests.rs::multiple_curators_preserve_origins_and_conflicting_pins` | C | Fixture |
| Client resolving members | `plugin-registry` | `crates/plugin-registry/src/tests.rs::browse_is_inert_selection_verifies_independent_publishers_and_closure` | | Fixture |
| Revocation reader | `plugin-registry` | `crates/plugin-registry/src/tests.rs::revocations_are_independent_monotone_and_retained_on_partial_failure` | R, D | Fixture |

## Controlled, recoverable work — WORK, COORD, WS

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Work planner and writer | `work-host` | `crates/work-host/src/tests.rs::conditional_history_has_exact_retries_stale_conflicts_and_current_rights` | C | Fixture |
| Delegating worker | `work-host` | `crates/work-host/src/tests.rs::a_real_delegated_cj_job_uses_frozen_inputs_and_survives_restart` | I | Fixture |
| Admission after a crash | `work-host` | `crates/work-host/src/tests.rs::admission_crash_retains_unknown_without_replaying_or_reusing_identity` | I, $ | Fixture |
| Scoped reader | `work-host` | `crates/work-host/src/tests.rs::two_scoped_clients_converge_and_revoked_views_stop_reading` | R | Fixture |
| Claimant | `coordination-host` | `crates/coordination-host/src/tests.rs::source_alias_racing_claims_and_checked_conditional_integration` | C | Fixture |
| Controller | `coordination-host` | `crates/coordination-host/src/tests.rs::controller_cj_scope_pins_and_restart_exact_response` | I, C | Fixture |
| Child allowance | `coordination-host` | `crates/coordination-host/src/tests.rs::siblings_share_parent_holds_and_cancellation_does_not_free_unknown` | $ | Fixture |
| Checker | `coordination-host` | `crates/coordination-host/src/tests.rs::checker_is_separate_and_charged_once` | $ | Fixture |
| Workspace host | `workspace-host` | `crates/workspace-host/src/tests.rs::restart_reconciles_a_lost_response_without_replaying_dispatch` | I | Fixture |
| Workspace reader | `workspace-host` | `crates/workspace-host/src/tests.rs::revoked_access_and_approvals_refuse` | R | Fixture |

## Resources, sessions and media — ENV, SESS, LIVE

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Allocator | `env-host` | `crates/env-host/src/tests.rs::a_lost_create_is_reconciled_never_repeated` | I | Fixture |
| Attachment holder | `env-host` | `crates/env-host/src/tests.rs::two_attachments_race_for_one_slot` | C | Fixture |
| Cleanup | `env-host` | `crates/env-host/src/tests.rs::unknown_cleanup_is_not_released_and_keeps_its_hold` | $ | Fixture |
| Dispatcher | `env-host` | `crates/env-host/src/tests.rs::an_unreachable_dispatcher_keeps_the_slot_and_cleanup_pending` | D | Fixture |
| Session host | `session-host` | `crates/session-host/src/tests.rs::an_admitted_session_runs_a_turn_only_under_a_binding` | | Fixture |
| Session after restart | `session-host` | `crates/session-host/src/tests.rs::restart_turns_a_running_turn_into_unknown_and_never_reruns_it` | I | Fixture |
| Delivery of an answer | `session-host` | `crates/session-host/src/tests.rs::a_lost_delivery_stays_unknown_and_is_not_redelivered` | D | Fixture |
| Media controller | `live-host` | `crates/live-host/src/tests.rs::participants_need_controller_admission_and_their_own_consent` | | Fixture |
| Media stop | `live-host` | `crates/live-host/src/tests.rs::stop_with_an_unconfirmed_revocation_stays_unknown` | R, D | Fixture |
| Replacement controller | `live-host` | `crates/live-host/src/tests.rs::replacement_plans_advance_the_epoch_and_fence_old_admissions` | C | Fixture |
| Real text capture and approved source input | `live-host` | `crates/live-host/src/tests/document.rs::real_document_capture_and_approved_input_use_the_same_source_version` | C | Fixture |
| Resident session and scoped private clients | `live-host` | `crates/live-host/src/tests/resident.rs::resident_owns_one_broker_and_serves_only_signed_scoped_reads` | R | Fixture |
| Native text frame delivery | `live-host` | `crates/live-host/src/tests/frames.rs::real_text_delivery_enforces_signer_floor_sequence_epoch_and_private_retention` | R, D | Fixture |
| Interrupted delivery | `live-host` | `crates/live-host/src/tests/frames.rs::crash_after_mailbox_delivery_retains_unknown_without_duplicate_writes` | I | Fixture |
| Native write deadline | `live-host` | `crates/live-host/src/tests/frames.rs::native_mailbox_checks_expiry_at_the_actual_write_boundary` | D | Fixture |
| Expired media | `live-host` | `crates/live-host/src/tests.rs::media_expiry_removes_bytes_after_restart_and_keeps_observation_evidence` | R | Fixture |

## Sovereign agents — SOV

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Remote signer | `sov-custody` | `crates/sov-custody/src/tests.rs::remote_signing_binds_the_exact_request_and_never_repeats_it` | I | Fixture |
| Guardians | `sov-custody` | `crates/sov-custody/src/tests.rs::a_guardian_quorum_is_exact_and_single_use` | | Fixture |
| Signer outage | `sov-custody` | `crates/sov-custody/src/tests.rs::signer_outages_refuse_or_stay_unknown_without_a_second_call` | D | Fixture |
| Controller transfer | `sov-lifecycle` | `crates/sov-lifecycle/src/tests.rs::a_two_host_transfer_needs_authority_acceptance_and_complete_fencing` | C | Fixture |
| Transfer under outage | `sov-lifecycle` | `crates/sov-lifecycle/src/tests.rs::unreachable_dispatchers_and_signer_outages_hold_the_transfer` | D | Fixture |
| Recovery | `sov-lifecycle` | `crates/sov-lifecycle/src/tests.rs::recovery_needs_a_complete_chain_from_a_stopped_unrevoked_lifecycle` | R | Fixture |
| Treasury: earn, receive, buy, use credit | `sov-treasury` | `crates/sov-treasury/src/tests.rs::an_agent_earns_receives_buys_credit_and_uses_it` | $ | Fixture |
| Wallet reply lost | `sov-treasury` | `crates/sov-treasury/src/tests.rs::a_lost_wallet_reply_is_never_paid_again_and_survives_rollover` | I, $ | Fixture |
| Credit fulfillment | `sov-treasury` | `crates/sov-treasury/src/tests.rs::a_lost_callback_credits_once_across_retries_and_restored_hosts` | I, D | Fixture |
| Account recovery | `sov-treasury` | `crates/sov-treasury/src/tests.rs::recovery_needs_account_control_and_revocation_stops_credit` | R | Fixture |
| Economic statement | `sov-treasury` | `crates/sov-treasury/src/tests.rs::the_economic_statement_joins_every_step_by_its_original_identity` | $ | Fixture |
| Incomplete earnings view | `sov-treasury` | `crates/sov-treasury/src/tests.rs::economic_coverage_requires_explicit_complete_source_pages` | $ (missing pages or fees cannot claim complete coverage) | Fixture |
| Economic statement reader | `nostr` | `crates/nostr/src/sovereign_treasury.rs::statement_v2_preserves_unknown_costs_and_reads_legacy_v1` | $ (v2 null fees, explicit measured zero, legacy v1 reading) | Fixture |

## Funded labor — MKT and LAB

| Role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Buyer and provider agreement | `coder-labor` | `crates/coder-labor/src/tests.rs::both_roles_confirm_the_exact_order_without_creating_execution` | | Fixture |
| Execution worker | `coder-labor` | `crates/coder-labor/src/tests.rs::worker_requires_durable_linkage_and_generic_workers_reject_labor_feature` | | Fixture |
| Unknown execution | `coder-labor` | `crates/coder-labor/src/tests.rs::unknown_execution_cannot_be_delivered_and_dispute_blocks_acceptance` | I | Fixture |
| Native execution, repair, independent checker and funding | `coder-labor`, `labor-roles` | `crates/coder-labor/src/tests/roundtrip.rs::native_rework_runs_exact_buyer_context_and_independent_checker_before_acceptance` | I, R, D, $ | Real local commands; synthetic source and fake funds |
| Worker, reviewer, buyer with rework | `labor-roles` | `crates/labor-roles/src/tests.rs::rework_then_acceptance_records_an_obligation_that_only_funding_settles` | I, $ | Fixture |
| Resolver for an unavailable buyer | `labor-roles` | `crates/labor-roles/src/tests.rs::an_unavailable_buyer_does_not_forfeit_timely_conforming_work` | D | Fixture |
| Cancellation | `labor-roles` | `crates/labor-roles/src/tests.rs::cancellation_before_during_and_after_delivery` | R | Fixture |
| Supersession and refunds owed | `labor-roles` | `crates/labor-roles/src/tests.rs::superseding_a_funded_acceptance_owes_a_refund_and_competing_finals_conflict` | C, $ | Fixture |

## Vault — VAULT

| Tier and role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| `user` device vault | `oa-vault` | `crates/oa-vault/src/tests.rs::every_slot_method_opens_with_its_secret_only` | | Fixture |
| `sealed` client and workload | `vault-sealed` | `crates/vault-sealed/src/tests.rs::a_sealed_object_opens_only_inside_an_admitted_session` | | Fixture |
| `sealed` leases | `vault-sealed` | `crates/vault-sealed/src/tests.rs::leases_are_exact_single_use_revocable_and_signed_by_an_enrolled_slot` | R | Fixture |
| `operator` key service | `vault-operator` | `crates/vault-operator/src/tests.rs::an_unknown_key_service_outcome_reconciles_without_spending_twice` | I | Fixture |
| `operator` grants | `vault-operator` | `crates/vault-operator/src/tests.rs::revocation_expiry_and_audit_tampering_stop_use` | R | Fixture |

## Attestation — ATT

| Profile and role | Owner | Fixture | Cases | Shown live |
| --- | --- | --- | --- | --- |
| Confidential Space client | `oa-att` | `crates/oa-att/src/token.rs::a_chain_not_ending_in_the_root_is_refused` | | Live demo at `/att` (Confidential Space on TDX) |
| Confidential Space GPU | `oa-att` | `crates/oa-att/src/lib.rs::a_gpu_release_needs_an_h100_in_cc_mode` | | Fixture |
| Raw TDX | `att-platforms` | `crates/att-platforms/src/tests.rs::a_tdx_quote_verifies_to_the_pinned_root_with_current_collateral` | R (revoked PCK, revoked TCB) | Fixture (synthetic roots) |
| Raw SEV-SNP | `att-platforms` | `crates/att-platforms/src/tests.rs::an_snp_report_verifies_up_the_amd_chain_for_its_chip_and_tcb` | R | Fixture (synthetic roots) |
| NVIDIA via NRAS | `att-platforms` | `crates/att-platforms/src/tests.rs::nras_gpu_evidence_is_bound_current_and_passing` | | Fixture (synthetic roots) |
| HPKE keys and rotation | `att-seal` | `crates/att-seal/src/tests.rs::key_admission_and_rotation_follow_the_attested_records` | C | Fixture |
| Sealed request and output | `att-seal` | `crates/att-seal/src/tests.rs::chunks_are_verified_one_by_one_and_in_order` | D | Fixture |
| Transparency | `att-release` | `crates/att-release/src/tests.rs::missing_stale_forked_and_mismatched_transparency_refuses` | C, D | A real Rekor entry verifies offline: `crates/att-release/src/tests.rs::a_real_rekor_entry_verifies_under_the_pinned_sigstore_key` |
| Personal approvals | `att-release` | `crates/att-release/src/tests.rs::approval_never_shortens_notice_and_emergencies_are_flagged` | R | Fixture |
| Sealed-job receipts | `att-release`, `nostr` | `crates/att-release/src/tests.rs::a_sealed_receipt_names_ciphertext_and_the_attested_endpoint_only` | $ | Fixture |
| Receipt validation | `nostr` | `crates/nostr/src/pylon/tests.rs::a_sealed_job_receipt_carries_exactly_its_attested_block` | | Fixture |

## What remains for qualification

Each needs an owner step, recorded in `NEEDS_OWNER.md`, and none holds the
code open: raw TDX, SEV-SNP and H100 hardware runs (#11745); the sealed
vault with a remote client and attestation-bound Cloud KMS (#11743); the agent treasury loop with
real money (#11741); a paid labor order between separately run parties
(#11748). Cost rows record what was metered; unmetered costs stay null and
no all-in cost is claimed.
