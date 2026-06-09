import { assertPublicProjectionSafe } from "./state"

export type LaunchClaimState = "allowed" | "blocked" | "planned"

export type LaunchClaimGate = {
  claimRef: string
  publicPhrase: string
  state: LaunchClaimState
  requiredEvidenceRefs: string[]
  blockerRefs: string[]
}

export const launchClaimGates: LaunchClaimGate[] = [
  {
    claimRef: "claim.pylon.v0_3.rc_package",
    publicPhrase: "@openagentsinc/pylon@0.3.0-rc1 is the v0.3 release candidate",
    state: "allowed",
    requiredEvidenceRefs: [
      "evidence.package.dry_run",
      "evidence.install.local",
      "evidence.dashboard.startup_smoke",
    ],
    blockerRefs: [],
  },
  {
    claimRef: "claim.pylon.v0_3.stable",
    publicPhrase: "Pylon v0.3.0 is stable",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.release.full_gate_matrix"],
    blockerRefs: ["blocker.copy.stable_release_not_cut"],
  },
  {
    claimRef: "claim.pylon.assignment_ready_network",
    publicPhrase: "Pylon v0.3 is assignment-ready across the network",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.live.assignment.multi_host", "evidence.fresh.heartbeat.inventory.wallet"],
    blockerRefs: ["blocker.copy.live_network_assignment_not_proven"],
  },
  {
    claimRef: "claim.pylon.paid_settlement",
    publicPhrase: "Paid Pylon work settles Bitcoin",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.mdk.send_ready", "evidence.paid.assignment.settlement"],
    blockerRefs: ["blocker.copy.paid_settlement_not_proven"],
  },
  {
    claimRef: "claim.pylon.qwen_training",
    publicPhrase: "Qwen is training on people's devices",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.psionic.training.gate"],
    blockerRefs: ["blocker.copy.qwen_training_postponed"],
  },
  {
    claimRef: "claim.pylon.optional_local_qwen_inference",
    publicPhrase: "Pylon can use optional local Qwen3.5 inference when the Psionic backend, model, and tool-call gates pass",
    state: "allowed",
    requiredEvidenceRefs: [
      "evidence.psionic.backend.doctor",
      "evidence.psionic.qwen35.model_admission",
      "evidence.psionic.qwen35.tool_call_smoke",
    ],
    blockerRefs: [],
  },
  {
    claimRef: "claim.pylon.paid_qwen_inference",
    publicPhrase: "Paid Qwen inference is live on Pylons",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.paid.assignment.settlement", "evidence.psionic.paid_inference_gate"],
    blockerRefs: ["blocker.copy.paid_qwen_inference_not_live"],
  },
  {
    claimRef: "claim.pylon.marketplace_capacity",
    publicPhrase: "Pylons sell compute capacity live",
    state: "blocked",
    requiredEvidenceRefs: ["evidence.capacity.market.live", "evidence.payment.market.settlement"],
    blockerRefs: ["blocker.copy.capacity_market_not_live"],
  },
]

const unsafePhrasePatterns = [
  /pylon v0\.3\.0 is stable/i,
  /assignment-ready across the network/i,
  /paid pylon work settles bitcoin/i,
  /qwen .*training on people/i,
  /paid qwen inference .*live/i,
  /qwen inference .*paid/i,
  /sell compute capacity live/i,
  /full live gepa network/i,
  /marketplace .* live/i,
  /referral payout/i,
  /data revenue .* live/i,
]

export function projectLaunchGateMatrix() {
  const projection = {
    schema: "openagents.pylon.launch_gate_matrix.v0.3",
    packageName: "@openagentsinc/pylon",
    version: "0.3.0-rc1",
    gates: launchClaimGates,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

export function assertLaunchCopyAllowed(copy: string) {
  for (const pattern of unsafePhrasePatterns) {
    if (pattern.test(copy)) {
      throw new Error(`launch copy contains blocked public claim: ${pattern}`)
    }
  }
  assertPublicProjectionSafe(copy)
}
