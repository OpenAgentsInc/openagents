import type { NodeLaunchStatus } from "./rpc.js"

// AO-4 (EPIC #5441, issue #5445): the live first-run onboarding status surface.
//
// The visible "literally on Autopilot" chain: identity choice (AO-3) → agent
// registered (AO-1) → node online → wallet receive-ready → payout target
// registered → presence live → joined Tassadar → first work claimed → first
// sats earned. Each step shows pending/active/done/failed with an actionable
// message + retry on failure (offline → retry, never a dead/blank screen).
//
// Every step is driven by REAL observable state from the Phase 1 onboarding flow
// and the node's read-only control surface — never faked:
//   - identity choice  : the persisted AO-3 choice (or "needs choice")
//   - agent registered : a persisted `oa_agent_...` credential exists (AO-1)
//   - node online      : the honest node launch status + control token
//   - wallet           : wallet.status.receiveReady (read-only, CL-49)
//   - payout / presence: the node's onboarding env is on (token + base URL) and
//                        the node is online, so Pylon's heartbeat path
//                        (registerPylon + ensureSparkPayoutTargetRegistered) runs
//   - joined Tassadar  : the assignment worker is configured + node online
//   - first claimed    : assignments.poll returned ≥1 open assignment (CL-50)
//   - first sats earned: wallet.status.balanceSats > 0 (read-only)
//
// Public-safe: we surface npub / pylon refs / counts only — NEVER seeds, tokens,
// or raw payout addresses (the token never leaves the Bun host).

// The wizard step lifecycle the issue mandates.
//   - pending : not started / waiting on an earlier step
//   - active  : in progress / converging (e.g. node launching, presence
//               announcing) — the honest "working on it" state
//   - done    : observably complete
//   - failed  : a concrete failure the user can retry (offline, launch failed)
export type OnboardingStepStatus = "pending" | "active" | "done" | "failed"

export type OnboardingStep = {
  readonly id: string
  readonly label: string
  readonly status: OnboardingStepStatus
  // A short, actionable, public-safe message for the current status.
  readonly message: string
  // True when the failure is recoverable by retrying (offline / transient). The
  // UI shows a Retry affordance; the chain converges on the next bring-up.
  readonly retryable: boolean
}

export type OnboardingStatusInput = {
  readonly fetchedAt: string
  // AO-3: has the user made the first-run identity choice yet?
  readonly identityChoiceMade: boolean
  // AO-3: a public-safe label for the chosen identity, e.g. "new: Studio Mac" or
  // "existing pylon.ab12cd". Null until a choice is made.
  readonly identityLabel: string | null
  // AO-1: is an agent credential (oa_agent_ token) persisted for this home? Only
  // the boolean crosses into this projection — never the token itself.
  readonly agentRegistered: boolean
  // The honest node launch lifecycle (node-launcher), null before first status.
  readonly nodeLaunchStatus: NodeLaunchStatus | null
  // Local Pylon control reachable (home + control token present).
  readonly localPylonReady: boolean
  // AO-2: is the onboarding env configured so presence/payout/assignment run?
  // (token persisted + product base URL set). Drives presence/payout/Tassadar.
  readonly onboardingEnvConfigured: boolean
  // wallet.status (CL-49), read-only; null when the node has not reported it.
  readonly walletReceiveReady: boolean
  readonly walletBalanceSats: number | null
  // AF-2 (#5899): has the node claimed forum tip-recipient readiness (so its
  // forum posts can receive Spark tips)? Receive-only; derived from a persisted
  // tip-ready receipt in the Bun host — never wallet material.
  readonly forumTipReady: boolean
  // AF-3 (#5900): has the node posted its public forum self-introduction yet?
  // Derived from a persisted intro receipt in the Bun host (public-safe).
  readonly forumIntroPosted: boolean
  // AF-4 (#5901): has the node run a read-only work-search over the typed
  // `work-requests` lane at least once? Derived from a persisted receipt.
  readonly forumWorkSearched: boolean
  // AF-4 (#5901): count of open work items observed at the last search (0 when
  // none open or not yet searched). Discovery only — never a commitment.
  readonly forumWorkOpenCount: number
  // assignments.poll (CL-50): count of open work-lease assignments observed.
  readonly openAssignmentCount: number
}

export type OnboardingStatusResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: "desktop:onboarding-status"
  // True once the whole chain is observably complete (sats earned).
  readonly complete: boolean
  // The first step that is currently `active` or `pending` after the last done
  // step — the "you are here" pointer for the wizard. Null when complete.
  readonly currentStepId: string | null
  // True when any step is in a retryable `failed` state (so the UI shows Retry).
  readonly hasRetryableFailure: boolean
  // Public-safe read-only wallet balance for the local Pylon. This is separate
  // from the earned-step prose so compact surfaces can render the number.
  readonly walletBalanceSats: number | null
  readonly steps: readonly OnboardingStep[]
}

// A node that has crashed past its restart budget reports `failed`. We treat
// that as a retryable failure for the node-dependent steps (the user can retry
// bring-up; an offline-only failure also converges on retry).
const nodeFailed = (status: NodeLaunchStatus | null): boolean =>
  status === "failed"

// --- per-step projections ---------------------------------------------------

const identityStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.identityChoiceMade) {
    return {
      id: "identity",
      label: "Identity",
      status: "done",
      message:
        input.identityLabel !== null
          ? `Using ${input.identityLabel}.`
          : "Identity chosen.",
      retryable: false,
    }
  }
  return {
    id: "identity",
    label: "Identity",
    status: "active",
    message: "Choose to use your existing Pylon or create a new named identity.",
    retryable: false,
  }
}

const registeredStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.agentRegistered) {
    return {
      id: "registered",
      label: "Agent registered",
      status: "done",
      message: "Your agent is registered with OpenAgents.",
      retryable: false,
    }
  }
  if (!input.identityChoiceMade) {
    return {
      id: "registered",
      label: "Agent registered",
      status: "pending",
      message: "Waiting on your identity choice.",
      retryable: false,
    }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id: "registered",
      label: "Agent registered",
      status: "failed",
      message: "The local node is not up yet, so registration is paused. Retry.",
      retryable: true,
    }
  }
  return {
    id: "registered",
    label: "Agent registered",
    status: "active",
    message: "Registering your agent (this happens automatically once online).",
    retryable: false,
  }
}

const nodeOnlineStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.localPylonReady) {
    return {
      id: "node-online",
      label: "Node online",
      status: "done",
      message: "The local Pylon node is online.",
      retryable: false,
    }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id: "node-online",
      label: "Node online",
      status: "failed",
      message: "The local node did not come online. Retry to relaunch it.",
      retryable: true,
    }
  }
  return {
    id: "node-online",
    label: "Node online",
    status: "active",
    message: "Bringing the bundled Pylon node online.",
    retryable: false,
  }
}

const walletStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.walletReceiveReady) {
    return {
      id: "wallet",
      label: "Wallet receive-ready",
      status: "done",
      message: "Your wallet can receive Bitcoin.",
      retryable: false,
    }
  }
  if (!input.localPylonReady) {
    return {
      id: "wallet",
      label: "Wallet receive-ready",
      status: nodeFailed(input.nodeLaunchStatus) ? "failed" : "pending",
      message: "Waiting for the node before provisioning the wallet.",
      retryable: nodeFailed(input.nodeLaunchStatus),
    }
  }
  return {
    id: "wallet",
    label: "Wallet receive-ready",
    status: "active",
    message: "Provisioning a receive-ready wallet.",
    retryable: false,
  }
}

// Presence + payout-target both run inside Pylon's heartbeat path, which is
// un-gated once the onboarding env (token + base URL) is set and the node is
// online (audit §3 d/e). We project them from those observable signals: done
// when registered + online + env configured; active while converging; pending
// until the agent is registered; failed (retryable) when the node failed.
const heartbeatDerivedStep = (
  id: string,
  label: string,
  doneMessage: string,
  activeMessage: string,
  input: OnboardingStatusInput,
): OnboardingStep => {
  if (!input.agentRegistered || !input.onboardingEnvConfigured) {
    return {
      id,
      label,
      status: "pending",
      message: "Waiting on agent registration.",
      retryable: false,
    }
  }
  if (input.localPylonReady) {
    return { id, label, status: "done", message: doneMessage, retryable: false }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id,
      label,
      status: "failed",
      message: `${label} paused — the node is offline. Retry.`,
      retryable: true,
    }
  }
  return { id, label, status: "active", message: activeMessage, retryable: false }
}

const payoutStep = (input: OnboardingStatusInput): OnboardingStep =>
  heartbeatDerivedStep(
    "payout",
    "Payout target registered",
    "Your payout target is registered.",
    "Registering your payout target.",
    input,
  )

// AF-2 (#5899): forum tip-recipient readiness. Receive-only — once the wallet is
// receive-ready and the agent is registered, the Bun host claims tip readiness so
// the agent's forum posts can receive Spark tips. Done when the receipt exists;
// active while the wallet is ready and the claim is converging; pending until the
// wallet can receive. Non-blocking: a stuck claim never dead-ends the wizard.
const tipReadyStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.forumTipReady) {
    return {
      id: "tip-ready",
      label: "Forum tips enabled",
      status: "done",
      message: "Your agent's forum posts can receive Bitcoin tips.",
      retryable: false,
    }
  }
  if (!input.agentRegistered) {
    return {
      id: "tip-ready",
      label: "Forum tips enabled",
      status: "pending",
      message: "Waiting on agent registration.",
      retryable: false,
    }
  }
  if (!input.walletReceiveReady) {
    return {
      id: "tip-ready",
      label: "Forum tips enabled",
      status: "pending",
      message: "Waiting for the wallet to become receive-ready.",
      retryable: false,
    }
  }
  return {
    id: "tip-ready",
    label: "Forum tips enabled",
    status: "active",
    message: "Enabling tips on your agent's forum posts.",
    retryable: false,
  }
}

const presenceStep = (input: OnboardingStatusInput): OnboardingStep =>
  heartbeatDerivedStep(
    "presence",
    "Presence live",
    "Your node is announcing presence to OpenAgents.",
    "Announcing presence to OpenAgents.",
    input,
  )

// AF-3 (#5900): automated forum self-introduction. Once the agent is registered
// and presence is live, the Bun host posts one public-safe introduction to the
// resolved intro lane. Done when the receipt exists; active while registered and
// converging; pending until registration. Non-blocking: a stuck post never
// dead-ends the wizard (the rest of the chain proceeds).
const forumIntroStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.forumIntroPosted) {
    return {
      id: "forum-intro",
      label: "Forum introduction posted",
      status: "done",
      message: "Your agent introduced itself on the OpenAgents Forum.",
      retryable: false,
    }
  }
  if (!input.agentRegistered) {
    return {
      id: "forum-intro",
      label: "Forum introduction posted",
      status: "pending",
      message: "Waiting on agent registration.",
      retryable: false,
    }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id: "forum-intro",
      label: "Forum introduction posted",
      status: "failed",
      message: "The node is offline, so the introduction is paused. Retry.",
      retryable: true,
    }
  }
  return {
    id: "forum-intro",
    label: "Forum introduction posted",
    status: "active",
    message: "Posting your agent's introduction to the Forum.",
    retryable: false,
  }
}

// AF-4 (#5901): automated read-only work-search over the typed work-requests
// lane. Discovery only — never a bid/quote/accept/spend. Done once a search has
// completed (honest empty state is acceptable, so the wizard advances); the
// message surfaces the observed open-item count. Active while the first search
// runs; pending until registration.
const workSearchStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.forumWorkSearched) {
    const n = input.forumWorkOpenCount
    return {
      id: "work-search",
      label: "Work search active",
      status: "done",
      message:
        n > 0
          ? `Watching the work market — ${n} open item${n === 1 ? "" : "s"} right now.`
          : "Watching the work market — none open right now.",
      retryable: false,
    }
  }
  if (!input.agentRegistered) {
    return {
      id: "work-search",
      label: "Work search active",
      status: "pending",
      message: "Waiting on agent registration.",
      retryable: false,
    }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id: "work-search",
      label: "Work search active",
      status: "failed",
      message: "The node is offline, so work search is paused. Retry.",
      retryable: true,
    }
  }
  return {
    id: "work-search",
    label: "Work search active",
    status: "active",
    message: "Searching the Forum work market for relevant tasks.",
    retryable: false,
  }
}

const tassadarStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.openAssignmentCount > 0) {
    return {
      id: "tassadar",
      label: "Joined Tassadar",
      status: "done",
      message: "Joined the Tassadar run and polling for work.",
      retryable: false,
    }
  }
  if (!input.agentRegistered || !input.onboardingEnvConfigured) {
    return {
      id: "tassadar",
      label: "Joined Tassadar",
      status: "pending",
      message: "Waiting on agent registration + presence.",
      retryable: false,
    }
  }
  if (nodeFailed(input.nodeLaunchStatus)) {
    return {
      id: "tassadar",
      label: "Joined Tassadar",
      status: "failed",
      message: "The node is offline, so it cannot join the run. Retry.",
      retryable: true,
    }
  }
  return {
    id: "tassadar",
    label: "Joined Tassadar",
    status: input.localPylonReady ? "active" : "pending",
    message: "Joining the Tassadar run and waiting for work.",
    retryable: false,
  }
}

const claimedStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.openAssignmentCount > 0) {
    return {
      id: "claimed",
      label: "First work claimed",
      status: "done",
      message: `Claimed ${input.openAssignmentCount} work assignment${input.openAssignmentCount === 1 ? "" : "s"}.`,
      retryable: false,
    }
  }
  if (!input.agentRegistered || !input.onboardingEnvConfigured) {
    return {
      id: "claimed",
      label: "First work claimed",
      status: "pending",
      message: "Waiting to join the run before claiming work.",
      retryable: false,
    }
  }
  return {
    id: "claimed",
    label: "First work claimed",
    status: input.localPylonReady ? "active" : "pending",
    message: "Waiting for the first claimable work item.",
    retryable: false,
  }
}

const earnedStep = (input: OnboardingStatusInput): OnboardingStep => {
  if (input.walletBalanceSats !== null && input.walletBalanceSats > 0) {
    return {
      id: "earned",
      label: "First sats earned",
      status: "done",
      message: `Earned ${input.walletBalanceSats} sats.`,
      retryable: false,
    }
  }
  if (input.openAssignmentCount > 0) {
    return {
      id: "earned",
      label: "First sats earned",
      status: "active",
      message: "Work claimed — waiting for the first settled payout.",
      retryable: false,
    }
  }
  return {
    id: "earned",
    label: "First sats earned",
    status: "pending",
    message: "Earn Bitcoin once work is claimed and settled.",
    retryable: false,
  }
}

/**
 * AO-4: project the live onboarding chain for the wizard. Pure; never throws.
 * Each step reflects real observable state. `currentStepId` points at the first
 * not-yet-done step (the "you are here"); `complete` is true once sats are
 * earned; `hasRetryableFailure` flags an offline/failed step so the UI offers
 * Retry rather than dead-ending.
 */
export const projectOnboardingStatus = (
  input: OnboardingStatusInput,
): OnboardingStatusResponse => {
  const steps: readonly OnboardingStep[] = [
    identityStep(input),
    registeredStep(input),
    nodeOnlineStep(input),
    walletStep(input),
    payoutStep(input),
    tipReadyStep(input),
    presenceStep(input),
    forumIntroStep(input),
    workSearchStep(input),
    tassadarStep(input),
    claimedStep(input),
    earnedStep(input),
  ]

  const firstUnfinished = steps.find(step => step.status !== "done")
  const earned = steps.find(step => step.id === "earned")
  const complete = earned?.status === "done"

  return {
    ok: true,
    fetchedAt: input.fetchedAt,
    sourceUrl: "desktop:onboarding-status",
    complete,
    currentStepId: complete ? null : (firstUnfinished?.id ?? null),
    hasRetryableFailure: steps.some(
      step => step.status === "failed" && step.retryable,
    ),
    walletBalanceSats: input.walletBalanceSats,
    steps,
  }
}
