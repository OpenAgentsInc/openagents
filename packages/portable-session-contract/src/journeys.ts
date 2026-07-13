export type PortableSessionJourneyStep = {
  readonly stepRef: string
  readonly actor: "desktop" | "mobile" | "local_pylon" | "managed_target" | "owner_managed_target" | "broker" | "authority"
  readonly action: string
  readonly requiredEvidence: ReadonlyArray<string>
}

export type PortableSessionJourney = {
  readonly journeyRef: string
  readonly hostClasses: ReadonlyArray<"owner_local" | "owner_managed" | "openagents_managed" | "managed_provider">
  readonly steps: ReadonlyArray<PortableSessionJourneyStep>
  readonly forbiddenOutcomes: ReadonlyArray<string>
}

export const PORTABLE_SESSION_REAL_HOST_JOURNEY: PortableSessionJourney = {
  journeyRef: "journey.portable_session.r7.v1",
  hostClasses: ["owner_local", "openagents_managed", "owner_managed"],
  steps: [
    { stepRef: "cold_open", actor: "mobile", action: "Paint shell and authorized top-level session metadata before detail hydration; inspect one causal child and its independent transcript.", requiredEvidence: ["first_paint_receipt_ref", "graph_digest", "child_transcript_cursor"] },
    { stepRef: "voice_follow_up", actor: "mobile", action: "Create an editable ASR transcript and submit one safe follow-up through the typed command registry.", requiredEvidence: ["typed_command_ref", "durable_outcome_ref", "audio_retention_none_ref"] },
    { stepRef: "quiesce", actor: "local_pylon", action: "Quiesce the complete root/descendant graph and seal a secret-free checkpoint.", requiredEvidence: ["checkpoint_digest", "graph_digest", "source_quiesce_receipt_ref"] },
    { stepRef: "revoke_source", actor: "broker", action: "Revoke source generation capability leases and prove source cleanup.", requiredEvidence: ["grant_revocation_receipt_ref", "source_cleanup_receipt_ref"] },
    { stepRef: "attach_managed", actor: "managed_target", action: "Verify compatibility/checkpoint integrity, redeem new target-scoped grants, and activate exactly one newer generation.", requiredEvidence: ["target_grant_receipt_ref", "attachment_receipt_ref", "checkpoint_readback_receipt_ref"] },
    { stepRef: "cross_client_control", actor: "desktop", action: "Open the same graph/refs, inspect exact diff and target history, and interrupt once through the shared command registry.", requiredEvidence: ["session_ref", "graph_digest", "diff_digest", "interrupt_outcome_ref"] },
    { stepRef: "faults", actor: "authority", action: "Revoke one secret, lose one acknowledgement, restart mobile, and reconcile without duplicate accepted work.", requiredEvidence: ["revocation_outcome_ref", "reconcile_outcome_ref", "duplicate_count_zero_ref"] },
    { stepRef: "owner_managed_move", actor: "owner_managed_target", action: "Move the same session to an enrolled owner-managed target or fail back with one live generation.", requiredEvidence: ["move_outcome_ref", "repository_post_image_digest", "attachment_history_ref"] },
    { stepRef: "stop_reclaim", actor: "authority", action: "Stop and reclaim all processes, scratch, ports, leases, and stale generation authority.", requiredEvidence: ["reclaim_receipt_ref", "scratch_wipe_receipt_ref", "stale_generation_denial_ref"] },
  ],
  forbiddenOutcomes: [
    "forked_session_identity",
    "duplicate_accepted_work",
    "duplicate_child_launch_card",
    "secret_projection",
    "silent_target_substitution",
    "orphaned_source_descendant",
    "false_completion_authority",
    "detail_blocked_first_paint",
    "click_tap_shortcut_divergence",
  ],
}
