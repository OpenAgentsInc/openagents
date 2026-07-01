// CL-53: pure-helper + reducer tests for the Foldkit desktop rewrite.
//
// The view itself needs a DOM/runtime to assert against, but the logic that
// used to live in the deleted hand-DOM panes/cards now lives in pure helpers
// (helpers.ts) and the pure reducer (update.ts). These cover the same behavior
// the deleted cl-47..cl-58 tests covered, without a DOM.

import { describe, expect, test } from "bun:test"
import { LAUNCH_RECOGNITION_REPLAY_SLUG } from "@openagentsinc/proof-replay"
import type {
  AssignmentRow,
  NodeStateMessage,
  OnboardingStatusResponse,
  TrainingRunsResponse,
  WalletStatusRow,
} from "../src/shared/rpc"
import {
  approvalLabel,
  artifactLineText,
  assignmentMeta,
  commandErrorText,
  connectionSummary,
  coordinatorToggleLabel,
  nodeStatusLine,
  parseVerifyLines,
  pylonFleetSummary,
  shipStatusLine,
  stateBreakdown,
  trainingProjectionMeta,
  verifyLineText,
  walletSummary,
} from "../src/ui/helpers"
import { initialRuntimeState } from "../src/ui/initial-state"
// #5466: live Blueprint chat refs now live in the runtime-step module, and the
// signature is chosen by the semantic router.
import {
  CHAT_TASSADAR_TOOL_REF,
  CHAT_REPLAY_TOOL_REF,
} from "../src/ui/blueprint-chat-runtime"
import { selectSignatureForMessage } from "../src/ui/blueprint-chat-routing"
import {
  initialModel,
  Model,
  modelAppleFmReadiness,
  modelBuiltInAgentReadiness,
  modelInstallReadiness,
  modelNode,
  modelProofReplay,
  modelPromiseSurfacingReadiness,
  modelPromiseSurfacingResult,
  modelPublicActivityTimeline,
  modelTrainingBootstrap,
  modelTrainingDashboard,
  modelTrainingEvidenceAdmission,
  modelTrainingEvidencePacketBuild,
  modelTrainingEvidencePacketSummary,
  modelTrainingOperatorReadiness,
  modelTrainingPromiseGates,
} from "../src/ui/model"
import {
  ChangedProofReplayGeneratedActorRef,
  ChangedProofReplayGeneratedFrom,
  ChangedProofReplayGeneratedKind,
  ChangedProofReplayGeneratedLimit,
  ChangedProofReplayGeneratedPairRef,
  ChangedProofReplayGeneratedRunRef,
  ChangedProofReplayGeneratedSince,
  ChangedProofReplayGeneratedSource,
  ChangedProofReplayGeneratedTo,
  ChangedProofReplayGeneratedWindowRef,
  ChangedAskTitle,
  ClickedActivateTrainingWindow,
  ClickedAdmitTrainingEvidence,
  ClickedBuildTrainingEvidencePacket,
  ClickedClaimTrainingLease,
  ClickedLoadGeneratedProofReplay,
  ClickedPlanTrainingWindow,
  ClickedQueueTrainingCloseout,
  ClickedRefreshPublicActivity,
  ClickedRefreshAppleFm,
  ClickedRefreshBuiltInAgent,
  ClickedRefreshInstallReadiness,
  ClickedRefreshPromiseSurfacing,
  ClickedRefreshProofReplay,
  ClickedRefreshTrainingRuns,
  ClickedSurfacePromiseGap,
  ChangedPromiseSurfacingPromiseId,
  ChangedPromiseSurfacingClaimText,
  ChangedPromiseSurfacingExpectedBehavior,
  ChangedPromiseSurfacingObservedBehavior,
  ChangedPromiseSurfacingEvidenceOrSteps,
  ChangedPromiseSurfacingImpact,
  ChangedVersePresenceZone,
  ClickedReconcileTrainingWindow,
  ClickedResolveApproval,
  ClickedRequestTrainingBootstrap,
  ClickedStartAppleFm,
  ClickedStartBuiltInAgent,
  ClickedSubmitIntent,
  GotAppleFmReadiness,
  GotBuiltInAgentReadiness,
  ClickedBlueprintChatSubmit,
  ChangedChatInput,
  ClickedChatSubmit,
  SucceededVerseTurn,
  GotInstallReadiness,
  GotPromiseSurfacingReadiness,
  GotPromiseSurfacingResult,
  GotProofReplayBundle,
  GotPublicActivityTimeline,
  GotTrainingDashboard,
  GotTrainingEvidencePacketSummary,
  GotTrainingOperatorReadiness,
  GotTrainingPromiseGates,
  GotTrainingRuns,
  GotNodeState,
  GotNodeLaunchStatus,
  GotOnboardingStatus,
  NavigatedTo,
  SelectedAgentMode,
  SelectedChatWorldNode,
  SelectedProofReplay,
  SelectedSession,
  SelectedTrainingSceneNode,
  SettledActivateTrainingWindow,
  SettledAdmitTrainingEvidence,
  SettledBuildTrainingEvidencePacket,
  SettledClaimTrainingLease,
  SettledPlanTrainingWindow,
  SettledQueueTrainingCloseout,
  SettledReconcileTrainingWindow,
  SettledRequestTrainingBootstrap,
  SettledResolveApproval,
  SettledSubmitIntent,
  SucceededAppleFmSession,
  SucceededBuiltInAgent,
  SucceededChatTurn,
  FailedAppleFmSession,
  ToggledEvent,
} from "../src/ui/message"
import { degradedOnboardingProjection } from "../src/ui/commands"
import { update } from "../src/ui/update"

const session = (sessionRef: string, state: string) =>
  ({
    sessionRef,
    adapter: "codex",
    state,
    accountRefHash: null,
    updatedAt: "2026-06-13T00:00:00.000Z",
  }) as never

const onboardingProjection = (
  complete: boolean,
): OnboardingStatusResponse => ({
  ok: true,
  fetchedAt: "2026-06-19T00:00:00.000Z",
  sourceUrl: "desktop:onboarding-status",
  complete,
  currentStepId: complete ? null : "identity",
  hasRetryableFailure: false,
  walletBalanceSats: complete ? 1 : null,
  steps: [
    {
      id: complete ? "earned" : "identity",
      label: complete ? "First sats earned" : "Identity",
      status: complete ? "done" : "active",
      message: complete
        ? "Your first earned sats are visible."
        : "Choose an identity.",
      retryable: false,
    },
  ],
})

describe("helpers (CL-47..CL-58 parity, pure)", () => {
  test("desktop startup lands on the Verse chat home and warms public training context", () => {
    // VERSE HOME (owner directive, 2026-06-20): the app now launches to the
    // Pylon/Tassadar Verse surface. It warms local identity/readiness plus
    // lightweight public training projections so the default scene can render
    // the user's Pylon base and the Tassadar run core without opening Training
    // Live; the older multi-loader startup remains gone.
    const [model, commands] = initialRuntimeState()

    expect(model.pane).toBe("chat")
    expect(commands.map(command => command.name)).toEqual([
      "LoadIdentityChoiceState",
      "LoadOnboardingStatus",
      "LoadPromiseSurfacingReadiness",
      "LoadTrainingRuns",
      "LoadTrainingPromiseGates",
      "LoadTrainingOperatorReadiness",
    ])
    expect(model.identityChoicePending).toBe(true)
    expect(model.onboardingPending).toBe(true)
    expect(model.promiseSurfacingReadinessPending).toBe(true)
    expect(model.trainingOperatorReadinessPending).toBe(true)
    expect(model.shellTurns).toHaveLength(0)
    expect(model.shellInput).toBe("")
  })

  test("commandErrorText unwraps Effect.tryPromise causes", () => {
    const wrapped = Object.assign(
      new Error("An error occurred in Effect.tryPromise"),
      { cause: new Error("desktop RPC bridge unavailable") },
    )
    expect(commandErrorText(wrapped)).toBe("desktop RPC bridge unavailable")
  })

  test("trainingProjectionMeta hides internal Effect.tryPromise wrappers", () => {
    const projection: TrainingRunsResponse = {
      ok: false,
      fetchedAt: "2026-06-14T00:00:00.000Z",
      sourceUrl: "desktop:training-runs",
      runs: [],
      summaries: [],
      error: "An error occurred in Effect.tryPromise",
    }
    expect(trainingProjectionMeta(projection)).toBe(
      "waiting for Worker projection",
    )
  })

  test("nodeStatusLine summarizes connection + breakdown", () => {
    expect(nodeStatusLine({ ok: true, sessions: [] })).toBe("connected · 0 sessions")
    expect(
      nodeStatusLine({
        ok: true,
        sessions: [session("a", "running"), session("b", "failed")],
      }),
    ).toBe("connected · 2 sessions · 1 running · 1 failed")
    expect(nodeStatusLine({ ok: false, sessions: [session("a", "running")] })).toBe(
      "offline · 1 session · 1 running",
    )
  })

  test("stateBreakdown groups by state", () => {
    expect(stateBreakdown([])).toBe("")
    expect(
      stateBreakdown([{ state: "running" }, { state: "running" }, { state: "failed" }]),
    ).toBe("2 running · 1 failed")
  })

  test("approvalLabel falls back to kind when prompt empty", () => {
    expect(approvalLabel({ prompt: "Approve deploy?", kind: "deploy" })).toBe(
      "Approve deploy?",
    )
    expect(approvalLabel({ prompt: "   ", kind: "deploy" })).toBe("deploy")
  })

  test("shipStatusLine maps intent status to label + terminal flag", () => {
    expect(shipStatusLine("shipped")).toEqual({
      text: "✓ shipped",
      terminal: true,
      dotColor: "#3fb950",
    })
    expect(shipStatusLine("planning").terminal).toBe(false)
    expect(shipStatusLine("mystery").text).toBe("mystery")
  })

  test("walletSummary derives value + summary", () => {
    const wallet: WalletStatusRow = {
      configured: true,
      daemonOnline: true,
      balanceSats: 12345,
      receiveReady: true,
      sendReady: false,
      readiness: "ready",
    }
    expect(walletSummary(wallet)).toEqual({
      value: "12,345 sats",
      summary: "wallet online · ready · receive ✓",
    })
    expect(walletSummary({ ...wallet, balanceSats: null, daemonOnline: false, receiveReady: false }).value).toBe(
      "—",
    )
  })

  test("assignmentMeta derives goal + meta", () => {
    const row: AssignmentRow = {
      assignmentRef: "assign-abcdef123456",
      leaseRef: "lease-1",
      goal: "Fix the bug",
      paymentMode: "fixed",
      expiresAt: "2026-07-01T00:00:00.000Z",
    }
    const { goal, meta } = assignmentMeta(row)
    expect(goal).toBe("Fix the bug")
    expect(meta).toContain("fixed")
    expect(meta).toContain("expires 2026-07-01")
  })

  test("pylonFleetSummary distinguishes verified capacity from stale work", () => {
    const summary = pylonFleetSummary({
      assignments: [],
      capacity: {
        ageSeconds: 30,
        availableCodexSlots: 2,
        blockerRefs: [],
        lastHeartbeatAt: "2026-06-29T15:00:00.000Z",
        sourceRefs: ["source.local.pylon.presence_state"],
        state: "verified",
      },
      counts: {
        accepted: 3,
        assigned: 2,
        executing: 2,
        khalaRequestWrappers: 1,
        pylons: 1,
        rejected: 1,
        stale: 0,
        tokenFailures: 0,
      },
      fetchedAt: "2026-06-29T15:00:30.000Z",
      pylonRefs: ["pylon.local"],
    })
    expect(summary.tone).toBe("ready")
    expect(summary.capacityLine).toContain("capacity verified")
    expect(summary.capacityLine).toContain("2 slots available")
    expect(summary.line).toContain("2 executing")
  })

  test("connectionSummary + coordinatorToggleLabel", () => {
    expect(connectionSummary(null)).toBe("connecting…")
    expect(connectionSummary({ ok: true })).toBe("online")
    expect(connectionSummary({ ok: false })).toBe("offline")
    expect(coordinatorToggleLabel(true)).toBe("▶ Resume")
    expect(coordinatorToggleLabel(false)).toBe("⏸ Pause")
  })

  test("parseVerifyLines trims + drops empties", () => {
    expect(parseVerifyLines("bun test\n\n  bun run typecheck  \n")).toEqual([
      "bun test",
      "bun run typecheck",
    ])
  })

  test("verifyLineText + artifactLineText", () => {
    expect(verifyLineText(session("a", "completed")).toneClass).toBe("verify-completed")
    expect(verifyLineText(session("a", "failed")).text).toContain("✗")
    expect(artifactLineText(null)).toBe("")
    expect(
      artifactLineText({
        kind: "diff",
        outcome: "applied",
        editedFileCount: 3,
        commandCount: 2,
        totalTokens: 100,
      }),
    ).toBe("artifact: applied · 3 files · 2 cmds · 100 tok")
  })
})

describe("update reducer (CL-53)", () => {
  test("GotNodeState stores the projection", () => {
    const node: NodeStateMessage = { ok: true, schema: "x", sessions: [] }
    const [model] = update(initialModel, GotNodeState({ node }))
    expect(modelNode(model)?.ok).toBe(true)
  })

  test("GotNodeLaunchStatus stores the launch lifecycle status (#5025)", () => {
    expect(initialModel.nodeLaunchStatus).toBe(null)
    const [launching, launchCommands] = update(
      initialModel,
      GotNodeLaunchStatus({ status: "launching" }),
    )
    expect(launching.nodeLaunchStatus).toBe("launching")
    expect(launchCommands).toHaveLength(1)
    const [failed, failedCommands] = update(
      launching,
      GotNodeLaunchStatus({ status: "failed" }),
    )
    expect(failed.nodeLaunchStatus).toBe("failed")
    expect(failedCommands).toHaveLength(1)
  })

  test("NavigatedTo switches pane and resets expanded events", () => {
    const start = Model.make({ ...initialModel, expandedEvents: [1, 2] })
    const [model, commands] = update(start, NavigatedTo({ pane: "settings" }))
    expect(model.pane).toBe("settings")
    expect(model.expandedEvents).toEqual([])
    expect(model.installReadinessPending).toBe(true)
    // CS-A1: Settings also surfaces accounts, so it loads install readiness,
    // managed-account registry/status, and #5485 inference-gateway readiness.
    expect(model.managedAccountsPending).toBe(true)
    expect(commands.map(command => command.name)).toEqual([
      "LoadInstallReadiness",
      "LoadManagedAccounts",
      "LoadAccountStatus",
      "LoadInferenceGatewayReadiness",
    ])
  })

  test("NavigatedTo fullscreen training pane refreshes training projections", () => {
    const start = Model.make({ ...initialModel, expandedEvents: [1, 2] })
    const [model, commands] = update(
      start,
      NavigatedTo({ pane: "training-fullscreen" }),
    )
    expect(model.pane).toBe("training-fullscreen")
    expect(model.expandedEvents).toEqual([])
    expect(model.trainingRunsPending).toBe(true)
    expect(model.trainingDashboardPending).toBe(true)
    expect(model.trainingPromiseGatesPending).toBe(true)
    expect(model.trainingOperatorReadinessPending).toBe(true)
    expect(model.trainingEvidencePacketSummaryPending).toBe(true)
    expect(model.publicActivityTimelinePending).toBe(true)
    expect(commands).toHaveLength(7)
    expect(commands.map(command => command.name)).toContain(
      "LoadPublicActivityTimeline",
    )
  })

  test("NavigatedTo chat pane refreshes Pylon base readiness", () => {
    const [model, commands] = update(
      Model.make({ ...initialModel, expandedEvents: [1] }),
      NavigatedTo({ pane: "chat" }),
    )
    expect(model.pane).toBe("chat")
    expect(model.expandedEvents).toEqual([])
    expect(commands.map(command => command.name)).toEqual([
      "LoadIdentityChoiceState",
      "LoadOnboardingStatus",
      "LoadTrainingOperatorReadiness",
    ])
  })

  test("incomplete onboarding status keeps the onboarding pane", () => {
    // The auto-advance contract is scoped to the onboarding pane (the user must
    // have opened it). Verse launch lands on `chat`, so start the user on the
    // onboarding pane explicitly to exercise this guard.
    const start = Model.make({ ...initialModel, pane: "onboarding" })
    const [model, commands] = update(
      start,
      GotOnboardingStatus({ projection: onboardingProjection(false) }),
    )
    expect(model.pane).toBe("onboarding")
    expect(model.onboardingPending).toBe(false)
    expect(model.onboardingStatusLine.tone).toBe("info")
    expect(commands).toHaveLength(0)
  })

  test("degraded onboarding status keeps the full chain without faking node failure", () => {
    const [start] = initialRuntimeState()
    const projection = degradedOnboardingProjection("timeout")
    const [model, commands] = update(
      start,
      GotOnboardingStatus({ projection }),
    )
    expect(projection.steps).toHaveLength(9)
    expect(projection.steps.find(step => step.id === "node-online")?.status).toBe(
      "active",
    )
    expect(model.onboardingStatusLine).toEqual({
      text: "status refresh needs a retry",
      tone: "error",
    })
    expect(commands).toHaveLength(0)
  })

  test("complete onboarding status auto-navigates the onboarding pane to chat", () => {
    // From the onboarding pane, a completed chain auto-advances to chat. Verse
    // launch already lands on chat, so start on onboarding to exercise the
    // auto-advance the way a returning user reaches it.
    const start = Model.make({ ...initialModel, pane: "onboarding" })
    const [model, commands] = update(
      start,
      GotOnboardingStatus({ projection: onboardingProjection(true) }),
    )
    expect(model.pane).toBe("chat")
    expect(model.onboardingPending).toBe(false)
    expect(model.onboardingStatusLine.tone).toBe("success")
    expect(commands).toHaveLength(0)
  })

  test("complete onboarding status does not steal focus outside onboarding", () => {
    const [model] = update(
      Model.make({ ...initialModel, pane: "training" }),
      GotOnboardingStatus({ projection: onboardingProjection(true) }),
    )
    expect(model.pane).toBe("training")
    expect(model.onboardingStatusLine.tone).toBe("success")
  })

  test("SelectedTrainingSceneNode stores the selected scene node id", () => {
    const [model, commands] = update(
      initialModel,
      SelectedTrainingSceneNode({ nodeId: "freivalds" }),
    )
    expect(model.selectedTrainingSceneNodeId).toBe("freivalds")
    expect(commands).toHaveLength(0)
  })

  test("SelectedChatWorldNode keeps payment labels short while inspecting full receipt refs", () => {
    const [model, commands] = update(
      initialModel,
      SelectedChatWorldNode({
        id: "pay:tip.forum.post_1:to",
        label: "receipt.forum.post_1.bitcoin.21 · 21 sats · Forum Author · avatar",
      }),
    )
    expect(model.chatWorldInspectedRef).toBe("receipt.forum.post_1.bitcoin.21")
    expect(commands).toHaveLength(0)
  })

  test("SelectedChatWorldNode inspects Khala inference receipt refs", () => {
    const [model, commands] = update(
      initialModel,
      SelectedChatWorldNode({
        id: "world:inference:event.public.khala_inference_served.receipt.inference.charge.chatcmpl_1:to",
        label: "https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_1 · openagents/khala-code · public_activity_timeline · gateway",
      }),
    )
    expect(model.chatWorldInspectedRef).toBe(
      "https://openagents.com/api/public/inference/receipts/receipt.inference.charge.chatcmpl_1",
    )
    expect(commands).toHaveLength(0)
  })

  test("ChangedVersePresenceZone stores the local avatar HUD zone", () => {
    const [inside, enterCommands] = update(
      initialModel,
      ChangedVersePresenceZone({ zone: "tassadar_area" }),
    )
    expect(inside.versePresenceZone).toBe("tassadar_area")
    expect(enterCommands).toHaveLength(0)

    const [outside, leaveCommands] = update(
      inside,
      ChangedVersePresenceZone({ zone: null }),
    )
    expect(outside.versePresenceZone).toBeNull()
    expect(leaveCommands).toHaveLength(0)
  })

  test("SelectedSession focuses the detail pane", () => {
    const [model] = update(initialModel, SelectedSession({ sessionRef: "sess-1" }))
    expect(model.pane).toBe("session-detail")
    expect(model.selectedSessionRef).toBe("sess-1")
  })

  test("ToggledEvent toggles membership", () => {
    const [a] = update(initialModel, ToggledEvent({ eventIndex: 3 }))
    expect(a.expandedEvents).toEqual([3])
    const [b] = update(a, ToggledEvent({ eventIndex: 3 }))
    expect(b.expandedEvents).toEqual([])
  })

  test("approval resolve optimistically hides, un-hides on rejection", () => {
    const [hidden, commands] = update(
      initialModel,
      ClickedResolveApproval({ approvalRef: "ap-1", decision: "approve" }),
    )
    expect(hidden.resolvedApprovals).toEqual(["ap-1"])
    expect(commands).toHaveLength(1)

    // Confirmed → stays hidden.
    const [ok] = update(hidden, SettledResolveApproval({ approvalRef: "ap-1", ok: true }))
    expect(ok.resolvedApprovals).toEqual(["ap-1"])

    // Rejected → un-hidden.
    const [back] = update(hidden, SettledResolveApproval({ approvalRef: "ap-1", ok: false }))
    expect(back.resolvedApprovals).toEqual([])
  })

  test("ask submit validation blocks empty title (no command)", () => {
    const [model, commands] = update(initialModel, ClickedSubmitIntent())
    expect(model.askStatus.tone).toBe("error")
    expect(commands).toHaveLength(0)
  })

  test("ask submit dispatches a command with a valid title", () => {
    const withTitle = update(initialModel, ChangedAskTitle({ value: "Ship it" }))[0]
    const [model, commands] = update(withTitle, ClickedSubmitIntent())
    expect(model.askPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("SettledSubmitIntent clears the form on success", () => {
    const withTitle = update(initialModel, ChangedAskTitle({ value: "Ship it" }))[0]
    const [model] = update(withTitle, SettledSubmitIntent({ ok: true, text: "sent · received" }))
    expect(model.askTitle).toBe("")
    expect(model.askStatus.tone).toBe("success")
  })

  // #5821: the visible Verse Send button talks to Tassadar/OpenAgents through
  // the model command. It must not spawn a Codex/Claude coding session.
  test("default Verse chat submit dispatches a Tassadar model turn, not session spawn", () => {
    const prompt = "What is my Pylon doing right now?"
    const typed = update(initialModel, ChangedChatInput({ value: prompt }))[0]
    const [pending, commands] = update(typed, ClickedChatSubmit())

    expect(pending.chatPending).toBe(true)
    expect(pending.chatInput).toBe("")
    expect(pending.chatStatus.text).toContain("Tassadar")
    expect(commands.map(command => command.name)).toEqual(["RespondToVerseInput"])
    expect(commands[0]?.args).toEqual({ prompt })

    const userMessage = pending.chatMessages[pending.chatMessages.length - 1]
    expect(userMessage?.body).toBe(prompt)
    expect(userMessage?.linkedSessionRef).toBeNull()
    expect(userMessage?.steps).toEqual([])

    const [answered] = update(
      pending,
      SucceededVerseTurn({
        ok: true,
        text: "Your Pylon is visible in the public Verse context.",
        sourceRefs: ["https://openagents.com/api/public/pylon-stats"],
        blockerRefs: [],
      }),
    )
    expect(answered.chatPending).toBe(false)
    expect(answered.chatStatus.tone).toBe("success")
    expect(answered.chatSessionRef).toBeNull()
    const assistantMessage =
      answered.chatMessages[answered.chatMessages.length - 1]
    expect(assistantMessage?.body).toContain("public Verse context")
    expect(assistantMessage?.linkedSessionRef).toBeNull()
    expect(assistantMessage?.steps).toEqual([])
  })

  test("default Verse chat surfaces one clean model blocker without session jargon", () => {
    const prompt = "Talk to Tassadar"
    const typed = update(initialModel, ChangedChatInput({ value: prompt }))[0]
    const [pending] = update(typed, ClickedChatSubmit())
    const [blocked] = update(
      pending,
      SucceededVerseTurn({
        ok: false,
        text: "I can't reach Tassadar yet: this desktop does not have an OpenAgents account token configured.",
        sourceRefs: [],
        blockerRefs: ["verse.auth.token_missing"],
      }),
    )

    expect(blocked.chatPending).toBe(false)
    expect(blocked.chatStatus.tone).toBe("error")
    expect(blocked.chatStatus.text).toContain("verse.auth.token_missing")
    const body = blocked.chatMessages[blocked.chatMessages.length - 1]?.body ?? ""
    expect(body).toContain("OpenAgents account token")
    expect(body).not.toContain("Codex")
    expect(body).not.toContain("Claude")
    expect(body).not.toContain("session")
  })

  // #5466 (EPIC #5461): the advanced Blueprint command still routes through
  // SEMANTIC signature selection and derives its program steps from REAL session
  // events. It is no longer the default Verse chat mechanic. The objective
  // carries the ROUTED signature ref (not a hardcoded one), and no step is
  // "verified" until the live session reaches a real terminal phase.
  test("explicit Blueprint chat routes semantically and stays unverified until live evidence", () => {
    const prompt = "Continue the C1 Tassadar module proof."
    const selection = selectSignatureForMessage(prompt)
    const typed = update(initialModel, ChangedChatInput({ value: prompt }))[0]
    const [pending, commands] = update(typed, ClickedBlueprintChatSubmit())

    expect(pending.chatPending).toBe(true)
    expect(pending.chatInput).toBe("")
    expect(pending.chatStatus.tone).toBe("info")
    expect(commands.map(command => command.name)).toEqual(["SpawnChatTurn"])
    expect(commands[0]?.args).toMatchObject({
      adapter: "codex",
      accountRef: null,
      lane: "auto",
      verify: [],
    })
    // The objective embeds the SEMANTICALLY-selected signature ref, NOT a
    // hardcoded one, and never asserts a digest/verdict up front.
    expect(String(commands[0]?.args.objective)).toContain(selection.signatureRef)
    expect(String(commands[0]?.args.objective)).toContain(CHAT_REPLAY_TOOL_REF)
    expect(String(commands[0]?.args.objective)).not.toContain("sha256:")

    const userMessage = pending.chatMessages[pending.chatMessages.length - 1]
    expect(userMessage?.body).toBe(prompt)
    // The user message renders the program shape but with NO verdict yet.
    expect(
      userMessage?.steps.some(step => step.toolRef === CHAT_TASSADAR_TOOL_REF),
    ).toBe(true)
    const userTassadar = userMessage?.steps.find(
      step => step.toolRef === CHAT_TASSADAR_TOOL_REF,
    )
    expect(userTassadar?.verdict).toBe("pending")
    expect(userTassadar?.digestRef).toBeNull()

    // On spawn success the assistant message links to the live session and is
    // STILL not verified — only the real node-state poll can flip the verdict.
    const sessionRef = "session.blueprint.chat.1"
    const [settled] = update(pending, SucceededChatTurn({ sessionRef }))
    expect(settled.chatPending).toBe(false)
    expect(settled.chatSessionRef).toBe(sessionRef)
    const assistantMessage =
      settled.chatMessages[settled.chatMessages.length - 1]
    expect(assistantMessage?.linkedSessionRef).toBe(sessionRef)
    const assistantTassadar = assistantMessage?.steps.find(
      step => step.toolRef === CHAT_TASSADAR_TOOL_REF,
    )
    expect(assistantTassadar?.verdict).toBe("pending")
    expect(assistantTassadar?.digestRef).toBeNull()

    // A node-state poll carrying a RUNNING event keeps the verdict pending.
    const runningNode: NodeStateMessage = {
      ok: true,
      schema: "x",
      sessions: [],
      events: {
        [sessionRef]: [
          { eventIndex: 0, phase: "started", state: "running", observedAt: "2026-06-19T00:00:00Z", detail: "agent turn 1" },
        ],
      },
    }
    const [runningModel] = update(settled, GotNodeState({ node: runningNode }))
    const runningStep = runningModel.chatMessages
      .find(m => m.linkedSessionRef === sessionRef)
      ?.steps.find(step => step.toolRef === CHAT_TASSADAR_TOOL_REF)
    expect(runningStep?.verdict).toBe("pending")
    expect(runningStep?.status).toBe("running")

    // A terminal COMPLETED event carrying a real digest flips it to verified
    // and surfaces the REAL digest — honest verification from live evidence.
    const digest = `sha256:${"a".repeat(64)}`
    const doneNode: NodeStateMessage = {
      ok: true,
      schema: "x",
      sessions: [],
      events: {
        [sessionRef]: [
          { eventIndex: 0, phase: "started", state: "running", observedAt: "2026-06-19T00:00:00Z", detail: "agent turn 1" },
          { eventIndex: 1, phase: "completed", state: "completed", observedAt: "2026-06-19T00:00:05Z", detail: `exact replay ${digest}` },
        ],
      },
    }
    const [doneModel] = update(runningModel, GotNodeState({ node: doneNode }))
    const doneStep = doneModel.chatMessages
      .find(m => m.linkedSessionRef === sessionRef)
      ?.steps.find(step => step.toolRef === CHAT_TASSADAR_TOOL_REF)
    expect(doneStep?.verdict).toBe("verified")
    expect(doneStep?.status).toBe("verified")
    expect(doneStep?.digestRef).toBe(digest)
  })

  test("built-in agent pane loads readiness", () => {
    const [model, commands] = update(
      initialModel,
      NavigatedTo({ pane: "builtin-agent" }),
    )
    expect(model.pane).toBe("builtin-agent")
    expect(model.builtInAgentStatus.tone).toBe("info")
    expect(model.appleFmStatus.tone).toBe("info")
    expect(commands).toHaveLength(3)
  })

  test("built-in agent readiness stores bounded hosted-compute status", () => {
    const [model] = update(
      initialModel,
      GotBuiltInAgentReadiness({
        projection: {
          ok: true,
          fetchedAt: "2026-06-15T00:00:00.000Z",
          sourceUrl: "desktop:builtin-agent-readiness",
          enabled: true,
          localPylonReady: true,
          hostedComputeConfigured: true,
          userApiKeyRequired: false,
          lane: "cloud-gcp",
          modelSet: "openagents-hosted-gemini",
          maxSessionSeconds: 600,
          dailySessionCap: 3,
          dailySessionsUsed: 1,
          meteringLabel: "3 sessions/day · 600s/session · openagents-hosted-gemini",
          worktreePathPresent: true,
          blockerRefs: [],
        },
      }),
    )
    expect(modelBuiltInAgentReadiness(model)?.ok).toBe(true)
    expect(model.builtInAgentStatus.tone).toBe("success")
    expect(model.builtInAgentStatus.text).toContain("3 sessions/day")
  })

  test("built-in agent go-online starts a session and focuses detail", () => {
    const [pending, commands] = update(initialModel, ClickedStartBuiltInAgent())
    expect(pending.pane).toBe("builtin-agent")
    expect(pending.builtInAgentPending).toBe(true)
    expect(commands).toHaveLength(1)

    const [online] = update(
      pending,
      SucceededBuiltInAgent({ sessionRef: "session.pylon.control.test" }),
    )
    expect(online.builtInAgentPending).toBe(false)
    expect(online.pane).toBe("session-detail")
    expect(online.selectedSessionRef).toBe("session.pylon.control.test")
  })

  test("built-in agent refresh dispatches readiness check", () => {
    const [model, commands] = update(initialModel, ClickedRefreshBuiltInAgent())
    expect(model.builtInAgentStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)
  })

  test("local Apple FM mode stores blockers and can be selected", () => {
    const [selected] = update(
      initialModel,
      SelectedAgentMode({ mode: "local-apple-fm" }),
    )
    expect(selected.agentMode).toBe("local-apple-fm")

    const [model] = update(
      selected,
      GotAppleFmReadiness({
        projection: {
          ok: false,
          fetchedAt: "2026-06-15T00:00:00.000Z",
          sourceUrl: "desktop:apple-fm-readiness",
          localPylonReady: true,
          available: false,
          status: "unavailable",
          backendKind: "apple_fm_bridge",
          profileId: "apple-fm-local",
          model: "apple-foundation-model",
          capability: "probe.backend.apple_fm_bridge",
          advertisedCapabilities: [],
          baseUrl: "http://127.0.0.1:11435",
          platform: "darwin-arm64",
          version: "fake-bridge",
          unavailableReason: "apple_intelligence_disabled",
          message: "Apple Intelligence is disabled.",
          blockerRefs: ["blocker.pylon.apple_fm.apple_intelligence_disabled"],
        },
      }),
    )

    expect(modelAppleFmReadiness(model)?.ok).toBe(false)
    expect(model.appleFmStatus.tone).toBe("info")
    expect(model.appleFmStatus.text).toContain("Apple Intelligence is disabled")
  })

  test("local Apple FM refresh dispatches readiness check", () => {
    const [model, commands] = update(initialModel, ClickedRefreshAppleFm())
    expect(model.appleFmPending).toBe(true)
    expect(model.appleFmStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)
  })

  test("local Apple FM start focuses the normal session detail timeline", () => {
    const [pending, commands] = update(
      { ...initialModel, agentMode: "local-apple-fm" },
      ClickedStartAppleFm(),
    )
    expect(pending.pane).toBe("builtin-agent")
    expect(pending.appleFmPending).toBe(true)
    expect(commands).toHaveLength(1)

    const [online] = update(
      pending,
      SucceededAppleFmSession({ sessionRef: "session.pylon.apple_fm.local" }),
    )
    expect(online.appleFmPending).toBe(false)
    expect(online.pane).toBe("session-detail")
    expect(online.selectedSessionRef).toBe("session.pylon.apple_fm.local")

    const [failed] = update(online, FailedAppleFmSession({ error: "not ready" }))
    expect(failed.pane).toBe("builtin-agent")
    expect(failed.appleFmStatus.tone).toBe("error")
    expect(failed.appleFmStatus.text).toBe("not ready")
  })

  test("install readiness stores first-run health projection", () => {
    const [model] = update(
      initialModel,
      GotInstallReadiness({
        projection: {
          ok: false,
          fetchedAt: "2026-06-15T00:00:00.000Z",
          sourceUrl: "desktop:install-readiness",
          platform: "darwin",
          arch: "arm64",
          runtime: "packaged",
          nodeLaunchStatus: "failed",
          pylonHomePresent: false,
          controlTokenPresent: false,
          localPylonReady: false,
          builtInAgentReady: false,
          appleFmReady: false,
          userApiKeyRequired: false,
          autoUpdateEnabled: true,
          highestRoiAction: "Restart Autopilot or install a newer build",
          blockerRefs: ["blocker.autopilot.install.local_pylon_failed"],
          items: [
            {
              id: "local-pylon",
              label: "Local node",
              status: "blocked",
              detail: "The local Pylon node did not become reachable.",
              blockerRef: "blocker.autopilot.install.local_pylon_failed",
            },
          ],
        },
      }),
    )
    expect(modelInstallReadiness(model)?.ok).toBe(false)
    expect(model.installReadinessStatus.text).toContain("Restart Autopilot")
    expect(model.installReadinessStatus.tone).toBe("info")
  })

  test("install readiness refresh dispatches a check", () => {
    const [model, commands] = update(
      initialModel,
      ClickedRefreshInstallReadiness(),
    )
    expect(model.installReadinessPending).toBe(true)
    expect(model.installReadinessStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)
  })

  test("public activity refresh and result stay read-only", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedRefreshPublicActivity(),
    )
    expect(pending.publicActivityTimelinePending).toBe(true)
    expect(commands.map(command => command.name)).toEqual([
      "LoadPublicActivityTimeline",
    ])

    const [model] = update(
      pending,
      GotPublicActivityTimeline({
        projection: {
          ok: true,
          fetchedAt: "2026-06-18T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/public/activity-timeline?limit=20",
          envelope: {
            generatedAt: "2026-06-18T00:00:00.000Z",
            nextCursor: null,
            events: [
              {
                eventRef: "activity.training.settlement.1",
                cursor:
                  "2026-06-18T00:00:01.000Z:settlement_receipt:activity.training.settlement.1",
                ts: "2026-06-18T00:00:01.000Z",
                kind: "real_bitcoin_moved",
                sourceKind: "settlement_receipt",
                runRef: "run.cs336.a1.demo",
                refs: ["receipt.public.real.1"],
                sourceRefs: ["receipt.public.real.1"],
                blockerRefs: [],
                caveatRefs: [],
                amountSats: 2100,
                realBitcoinMoved: true,
                state: "settled",
                text: "Receipt-backed real Bitcoin movement confirmed.",
              },
            ],
            sourceLag: [
              {
                sourceKind: "forum",
                status: "stale",
                latestSourceEventAt: null,
                observedAt: "2026-06-18T00:00:00.000Z",
                lagSeconds: null,
                maxStalenessSeconds: 30,
                sourceRefs: ["forum.activity.public.1"],
                blockerRefs: [],
                caveatRefs: ["caveat.public.activity_timeline.source_lag"],
              },
            ],
          },
        },
      }),
    )

    expect(model.publicActivityTimelinePending).toBe(false)
    expect(modelPublicActivityTimeline(model)?.ok).toBe(true)
    expect(model.publicActivityTimelineStatus.text).toContain("1 event")
    expect(model.publicActivityTimelineStatus.text).toContain("1 source warning")
    expect(model.publicActivityTimelineStatus.tone).toBe("info")
  })

  test("promise surfacing readiness stores Forum token state", () => {
    const [model] = update(
      initialModel,
      GotPromiseSurfacingReadiness({
        projection: {
          ok: true,
          fetchedAt: "2026-06-15T00:00:00.000Z",
          sourceUrl: "desktop:promise-surfacing-readiness",
          forumSlug: "product-promises",
          baseUrl: "https://openagents.test",
          productPromisesUrl: "https://openagents.test/api/public/product-promises",
          forumTopicsUrl:
            "https://openagents.test/api/forum/forums/product-promises/topics",
          agentTokenPresent: true,
          blockerRefs: [],
        },
      }),
    )
    expect(modelPromiseSurfacingReadiness(model)?.agentTokenPresent).toBe(true)
    expect(model.promiseSurfacingStatus.tone).toBe("success")
  })

  test("promise surfacing submit validates and dispatches report command", () => {
    const [invalid, invalidCommands] = update(
      initialModel,
      ClickedSurfacePromiseGap(),
    )
    expect(invalid.promiseSurfacingStatus.tone).toBe("error")
    expect(invalidCommands).toHaveLength(0)

    const filled = [
      ChangedPromiseSurfacingPromiseId({
        value: "autopilot.builtin_compute_agent.v1",
      }),
      ChangedPromiseSurfacingClaimText({
        value: "Click Go online without a user API key.",
      }),
      ChangedPromiseSurfacingExpectedBehavior({
        value: "A hosted agent starts.",
      }),
      ChangedPromiseSurfacingObservedBehavior({
        value: "Hosted compute is unavailable.",
      }),
      ChangedPromiseSurfacingEvidenceOrSteps({
        value: "Open Agent pane, click Go online, copy blocker ref.",
      }),
      ChangedPromiseSurfacingImpact({
        value: "Normal user cannot get an agent.",
      }),
    ].reduce((model, message) => update(model, message)[0], initialModel)
    const [pending, commands] = update(filled, ClickedSurfacePromiseGap())
    expect(pending.promiseSurfacingSubmitPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("promise surfacing result stores posted or drafted outcome", () => {
    const [model] = update(
      initialModel,
      GotPromiseSurfacingResult({
        projection: {
          ok: true,
          mode: "posted",
          draft: {
            title: "[Promise Report] autopilot.builtin_compute_agent.v1",
            requestedSlug:
              "promise-report-autopilot-builtin-compute-agent-v1",
            bodyText: "Surface only. Do not ship code.",
            ledgerVerdict: "ledger_claims_fixed_report_new_mismatch",
            registryVersion: "2026-06-15.4",
            promiseState: "green",
            relatedTopicRefs: [],
          },
          topicId: "topic.promise.created",
          topicUrl: "https://openagents.test/forum/t/topic.promise.created",
          blockerRefs: [],
        },
      }),
    )
    expect(modelPromiseSurfacingResult(model)?.mode).toBe("posted")
    expect(model.promiseSurfacingStatus.tone).toBe("success")
  })

  test("promise surfacing refresh dispatches readiness check", () => {
    const [model, commands] = update(
      initialModel,
      ClickedRefreshPromiseSurfacing(),
    )
    expect(model.promiseSurfacingReadinessPending).toBe(true)
    expect(commands).toHaveLength(1)
  })

  test("training plan action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(initialModel, ClickedPlanTrainingWindow())
    expect(pending.trainingPlanPending).toBe(true)
    expect(pending.trainingPlanStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledPlanTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/windows/plan",
          trainingRunRef: "training.run.desktop.r1.test",
          windowRef: "training.window.desktop.r1.test",
          run: null,
          window: null,
          runPlanned: true,
          windowPlanned: true,
          reason: "planned",
          message: "planned training.run.desktop.r1.test / training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingPlanPending).toBe(false)
    expect(settled.trainingPlanStatus.tone).toBe("success")
    expect(settled.trainingPlan).toMatchObject({
      trainingRunRef: "training.run.desktop.r1.test",
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(7)
    expect(followups.map(command => command.name)).toContain(
      "LoadPublicActivityTimeline",
    )
  })

  test("training run projection records first observation after planning", () => {
    const [, planFollowups] = update(initialModel, ClickedPlanTrainingWindow())
    expect(planFollowups).toHaveLength(1)

    const [planned] = update(
      initialModel,
      SettledPlanTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/windows/plan",
          trainingRunRef: "training.run.desktop.r1.test",
          windowRef: "training.window.desktop.r1.test",
          run: null,
          window: null,
          runPlanned: true,
          windowPlanned: true,
          reason: "planned",
          message: "planned training.run.desktop.r1.test / training.window.desktop.r1.test",
        },
      }),
    )

    const [observed] = update(
      planned,
      GotTrainingRuns({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:01:00.000Z",
          sourceUrl: "https://openagents.test/api/training/runs",
          runs: [
            {
              createdAtDisplay: "today",
              maxAllowedStale: 5,
              promiseRef: "pylon.first_real_model_training_run.v1",
              receiptRefs: [],
              sealInFlight: false,
              sealPublicationCadenceWindows: 1,
              sourceRefs: [],
              state: "planned",
              trainingRunRef: "training.run.desktop.r1.test",
              updatedAtDisplay: "today",
            },
          ],
          summaries: [],
        },
      }),
    )
    expect(observed.trainingPlanFirstObservedAt).toBe(
      "2026-06-14T00:01:00.000Z",
    )

    const [refreshed] = update(
      observed,
      GotTrainingRuns({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:02:00.000Z",
          sourceUrl: "https://openagents.test/api/training/runs",
          runs: [
            {
              createdAtDisplay: "today",
              maxAllowedStale: 5,
              promiseRef: "pylon.first_real_model_training_run.v1",
              receiptRefs: [],
              sealInFlight: false,
              sealPublicationCadenceWindows: 1,
              sourceRefs: [],
              state: "active",
              trainingRunRef: "training.run.desktop.r1.test",
              updatedAtDisplay: "today",
            },
          ],
          summaries: [],
        },
      }),
    )
    expect(refreshed.trainingPlanFirstObservedAt).toBe(
      "2026-06-14T00:01:00.000Z",
    )
  })

  test("training refresh loads run, dashboard, promise, readiness, packet, proof replay, and activity projections", () => {
    const [pending, commands] = update(initialModel, ClickedRefreshTrainingRuns())
    expect(pending.trainingRunsPending).toBe(true)
    expect(pending.trainingDashboardPending).toBe(true)
    expect(pending.trainingPromiseGatesPending).toBe(true)
    expect(pending.trainingOperatorReadinessPending).toBe(true)
    expect(pending.trainingEvidencePacketSummaryPending).toBe(true)
    expect(pending.publicActivityTimelinePending).toBe(true)
    expect(pending.proofReplayPending).toBe(true)
    expect(commands).toHaveLength(7)
    expect(commands.map(command => command.name)).toContain(
      "LoadPublicActivityTimeline",
    )
  })

  test("proof replay selection and refresh dispatch public bundle loads", () => {
    const [selected, selectedCommands] = update(
      initialModel,
      SelectedProofReplay({ slug: LAUNCH_RECOGNITION_REPLAY_SLUG }),
    )
    expect(selected.selectedProofReplaySlug).toBe(LAUNCH_RECOGNITION_REPLAY_SLUG)
    expect(selected.selectedProofReplayMode).toBe("catalog")
    expect(selected.proofReplayPending).toBe(true)
    expect(selected.proofReplayStatus.tone).toBe("info")
    expect(selectedCommands).toHaveLength(1)
    expect(selectedCommands[0]?.args).toEqual({
      request: {
        mode: "catalog",
        slug: LAUNCH_RECOGNITION_REPLAY_SLUG,
      },
    })

    const [refreshing, refreshCommands] = update(
      selected,
      ClickedRefreshProofReplay(),
    )
    expect(refreshing.proofReplayPending).toBe(true)
    expect(refreshCommands).toHaveLength(1)
  })

  test("generated proof replay filters dispatch bounded activity replay loads", () => {
    const [withFrom] = update(
      initialModel,
      ChangedProofReplayGeneratedFrom({ value: "2026-06-18T12:00:00.000Z" }),
    )
    const [withTo] = update(
      withFrom,
      ChangedProofReplayGeneratedTo({ value: "2026-06-18T12:05:00.000Z" }),
    )
    const [withRun] = update(
      withTo,
      ChangedProofReplayGeneratedRunRef({ value: "run.tassadar.executor.20260615" }),
    )
    const [withWindow] = update(
      withRun,
      ChangedProofReplayGeneratedWindowRef({
        value: "training.window.tassadar.executor.20260615.w1",
      }),
    )
    const [withActor] = update(
      withWindow,
      ChangedProofReplayGeneratedActorRef({
        value: "pylon.448ba824b5fc879f3a59",
      }),
    )
    const [withKind] = update(
      withActor,
      ChangedProofReplayGeneratedKind({ value: "real_bitcoin_moved" }),
    )
    const [withPair] = update(
      withKind,
      ChangedProofReplayGeneratedPairRef({
        value: "pylon.448ba824b5fc879f3a59+pylon.treasury",
      }),
    )
    const [withSource] = update(
      withPair,
      ChangedProofReplayGeneratedSource({ value: "settlement_receipt" }),
    )
    const [withSince] = update(
      withSource,
      ChangedProofReplayGeneratedSince({
        value: "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
      }),
    )
    const [withLimit] = update(
      withSince,
      ChangedProofReplayGeneratedLimit({ value: "10" }),
    )
    const [loading, commands] = update(
      withLimit,
      ClickedLoadGeneratedProofReplay(),
    )

    expect(loading.selectedProofReplayMode).toBe("generated")
    expect(loading.proofReplayPending).toBe(true)
    expect(commands).toHaveLength(1)
    expect(commands[0]?.args).toEqual({
      request: {
        actorRef: "pylon.448ba824b5fc879f3a59",
        from: "2026-06-18T12:00:00.000Z",
        kind: "real_bitcoin_moved",
        limit: "10",
        mode: "generated",
        pairRef: "pylon.448ba824b5fc879f3a59+pylon.treasury",
        runRef: "run.tassadar.executor.20260615",
        since: "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
        source: "settlement_receipt",
        to: "2026-06-18T12:05:00.000Z",
        windowRef: "training.window.tassadar.executor.20260615.w1",
      },
    })
  })

  test("proof replay bundle projection is stored with replay status", () => {
    const [settled] = update(
      initialModel,
      GotProofReplayBundle({
        projection: {
          ok: true,
          fetchedAt: "2026-06-18T02:38:00.000Z",
          sourceUrl:
            "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
          entry: {
            bundleEndpoint:
              "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
            primarySourceRefs: ["receipt.real"],
            slug: "first-real-settlement",
            summary: "receipt-backed replay",
            title: "Tassadar Run 1: First Real Bitcoin Settlement",
            websitePath:
              "https://openagents.com/tassadar/replay/first-real-settlement",
          },
          bundle: null,
          summary: {
            actorCount: 4,
            confirmedZapSats: 1000,
            durationSecond: 60,
            eventCount: 12,
            gapCount: 1,
            sourceRefCount: 6,
          },
          blockerRefs: [],
          cacheState: "live_https",
          cacheLabel: "live HTTPS read from openagents.com; no offline snapshot",
        },
      }),
    )

    expect(settled.proofReplayPending).toBe(false)
    expect(settled.proofReplayStatus.tone).toBe("success")
    expect(settled.proofReplayStatus.text).toContain("1,000 sats")
    expect(modelProofReplay(settled)?.summary?.eventCount).toBe(12)
  })

  test("training dashboard projection is stored with a lane summary", () => {
    const [settled] = update(
      initialModel,
      GotTrainingDashboard({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/leaderboards",
          leaderboards: {
            blockerRefs: [],
            lanes: [
              {
                blockerRefs: [],
                lane: "a1_loss",
                rowCount: 1,
                title: "A1 Loss Under Budget",
                topRow: {
                  contributorRef: "pylon.training.1",
                  rank: 1,
                  score: 3.1,
                  scoreLabel: "validation_loss=3.1",
                  settledPayoutSats: 21,
                  trainingRunRef: "training.run.1",
                },
              },
            ],
          },
          a2: {
            blockerRefs: [],
            observedDeviceClassCount: 2,
            observedMeasurementCount: 3,
            verifiedMeasurementCount: 2,
          },
          a3: {
            blockerRefs: [],
            cellCount: 2,
            fitArtifactCount: 1,
            verifiedCellCount: 1,
          },
          a4: {
            blockerRefs: [],
            evalDeltaBonusBlockerRefs: [],
            observedVerifiedStages: ["pii_masking"],
            requiredVerifiedStageCount: 3,
            shardCount: 1,
          },
          a5: {
            blockerRefs: [],
            evalSuiteCount: 1,
            updateBoundaryRef: "issue.github.openagents.4669",
            verifiedSuiteCount: 1,
          },
        },
      }),
    )

    expect(settled.trainingDashboardPending).toBe(false)
    expect(settled.trainingDashboardStatus.tone).toBe("success")
    expect(modelTrainingDashboard(settled)?.leaderboards.lanes[0]?.rowCount).toBe(1)
  })

  test("training promise gates projection is stored with blocker status", () => {
    const [settled] = update(
      initialModel,
      GotTrainingPromiseGates({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          registryVersion: "2026-06-12.8",
          sourceUrl: "https://openagents.test/api/public/product-promises",
          blockerRefs: [
            "blocker.product_promises.public_distributed_training_run_receipts_missing",
          ],
          promises: [
            {
              blockerRefs: [
                "blocker.product_promises.public_distributed_training_run_receipts_missing",
              ],
              claim: "Pylons participate in public distributed model-training runs.",
              evidenceRefCount: 3,
              productArea: "training",
              promiseId: "training.public_distributed_training_run.v1",
              safeCopy: "Not green yet.",
              state: "red",
              verification: "Requires run, work, validation, and settlement refs.",
            },
          ],
          stateCounts: {
            degraded: 0,
            green: 0,
            planned: 0,
            red: 1,
            withdrawn: 0,
            yellow: 0,
            unknown: 0,
          },
        },
      }),
    )

    expect(settled.trainingPromiseGatesPending).toBe(false)
    expect(settled.trainingPromiseGatesStatus.tone).toBe("info")
    expect(modelTrainingPromiseGates(settled)?.promises[0]?.state).toBe("red")
  })

  test("training operator readiness projection is stored with blocker status", () => {
    const [settled] = update(
      initialModel,
      GotTrainingOperatorReadiness({
        projection: {
          ok: false,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "desktop:training-operator-readiness",
          trainingBaseUrl: "https://openagents.test",
          adminEnabled: false,
          adminTokenPresent: false,
          adminReady: false,
          leaseEnabled: true,
          leaseReady: true,
          pylonRefPresent: true,
          pylonRefSource: "identity",
          pylonRef: "pylon.training.1",
          pylonHomePresent: true,
          controlTokenPresent: true,
          localPylonReady: true,
          evidenceEnabled: true,
          evidencePacketPathPresent: true,
          evidenceReady: true,
          blockerRefs: [
            "env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE",
            "env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN",
          ],
        },
      }),
    )

    expect(settled.trainingOperatorReadinessPending).toBe(false)
    expect(settled.trainingOperatorReadinessStatus).toEqual({
      text: "2 operator blockers · https://openagents.test",
      tone: "info",
    })
    expect(modelTrainingOperatorReadiness(settled)?.pylonRefSource).toBe(
      "identity",
    )
  })

  test("training evidence packet summary is stored with blocker status", () => {
    const [settled] = update(
      initialModel,
      GotTrainingEvidencePacketSummary({
        projection: {
          ok: false,
          configured: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "desktop:training-evidence-packet",
          packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
          budgetLabel: "desktop tiny loss budget",
          budgetRefPresent: true,
          evalRefPresent: true,
          mergeRefPresent: true,
          finalValidationLoss: 3.4,
          maxValidationLoss: 3,
          lossPointCount: 2,
          freivaldsCommitmentRefCount: 1,
          gradientCloseoutRefCount: 1,
          evidenceRefCount: 10,
          receiptRefCount: 3,
          shardContributionCount: 1,
          distinctPylonCount: 1,
          blockerRefs: [
            "training.evidence_packet.loss_exceeds_budget",
            "training.evidence_packet.requires_two_distinct_pylons",
          ],
        },
      }),
    )

    expect(settled.trainingEvidencePacketSummaryPending).toBe(false)
    expect(settled.trainingEvidencePacketSummaryStatus).toEqual({
      text: "packet blocked · 2 blockers",
      tone: "info",
    })
    expect(
      modelTrainingEvidencePacketSummary(settled)?.distinctPylonCount,
    ).toBe(1)
  })

  test("training activation action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedActivateTrainingWindow({
        windowRef: "training.window.desktop.r1.test",
      }),
    )
    expect(pending.trainingActivationPending).toBe(true)
    expect(pending.trainingActivationStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledActivateTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/windows/training.window.desktop.r1.test/activate",
          windowRef: "training.window.desktop.r1.test",
          window: null,
          reason: "activated",
          message: "activated training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingActivationPending).toBe(false)
    expect(settled.trainingActivationStatus.tone).toBe("success")
    expect(settled.trainingActivation).toMatchObject({
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(7)
  })

  test("training lease claim action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(initialModel, ClickedClaimTrainingLease())
    expect(pending.trainingLeasePending).toBe(true)
    expect(pending.trainingLeaseStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledClaimTrainingLease({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "https://openagents.test/api/training/leases/claim",
          pylonRef: "pylon.training.1",
          lease: {
            claimedAtDisplay: "now",
            leaseExpiresInSeconds: 900,
            leaseRef: "training.lease.1",
            pylonRef: "pylon.training.1",
            receiptRefs: ["receipt.training.lease"],
            state: "active",
            trainingRunRef: "training.run.desktop.r1.test",
            windowRef: "training.window.desktop.r1.test",
          },
          reason: "claimed",
          message: "claimed training.lease.1 for training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingLeasePending).toBe(false)
    expect(settled.trainingLeaseStatus.tone).toBe("success")
    expect(settled.trainingLease).toMatchObject({
      lease: { leaseRef: "training.lease.1" },
    })
    expect(followups).toHaveLength(7)
  })

  test("training bootstrap action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedRequestTrainingBootstrap({
        trainingRunRef: "training.run.4850",
      }),
    )
    expect(pending.trainingBootstrapPending).toBe(true)
    expect(pending.trainingBootstrapStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledRequestTrainingBootstrap({
        projection: {
          ok: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/runs/training.run.4850/bootstrap-grant",
          pylonRef: "pylon.training.1",
          trainingRunRef: "training.run.4850",
          outcome: {
            grant: {
              checkpointDigestRef: "checkpoint.digest.1",
              grantRef: "training.bootstrap.grant.1",
              joinerReceiptRefs: ["receipt.bootstrap.1"],
              joinerRef: "pylon.training.1",
              sealReceiptRefs: ["receipt.seal.1"],
              sealedAtDisplay: "now",
              sealedWindowRef: "training.window.sealed.1",
              trainingRunRef: "training.run.4850",
            },
            kind: "granted",
          },
          reason: "granted",
          message: "bootstrap grant training.bootstrap.grant.1",
        },
      }),
    )

    expect(settled.trainingBootstrapPending).toBe(false)
    expect(settled.trainingBootstrapStatus.tone).toBe("success")
    expect(modelTrainingBootstrap(settled)?.outcome?.kind).toBe("granted")
    expect(followups).toHaveLength(7)
  })

  test("training bootstrap queue feedback refreshes public projections", () => {
    const [settled, followups] = update(
      initialModel,
      SettledRequestTrainingBootstrap({
        projection: {
          ok: false,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/runs/training.run.4850/bootstrap-grant",
          pylonRef: "pylon.training.1",
          trainingRunRef: "training.run.4850",
          outcome: {
            joinerRef: "pylon.training.1",
            kind: "queued",
            reasonCode: "seal_in_flight",
            trainingRunRef: "training.run.4850",
          },
          reason: "queued",
          message: "bootstrap queued: seal_in_flight",
        },
      }),
    )

    expect(settled.trainingBootstrapPending).toBe(false)
    expect(settled.trainingBootstrapStatus).toEqual({
      text: "bootstrap queued: seal_in_flight",
      tone: "info",
    })
    expect(modelTrainingBootstrap(settled)?.outcome?.kind).toBe("queued")
    expect(followups).toHaveLength(7)
  })

  test("training closeout packet action dispatches and stores local queue feedback", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedQueueTrainingCloseout({
        trainingRunRef: "training.run.4850",
        windowRef: "training.window.4850.active",
        leaseRef: "training.lease.4850",
        bootstrapGrantRef: "training.bootstrap.grant.4850",
      }),
    )
    expect(pending.trainingCloseoutPending).toBe(true)
    expect(pending.trainingCloseoutStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledQueueTrainingCloseout({
        ok: true,
        text: "queued · accepted",
      }),
    )

    expect(settled.trainingCloseoutPending).toBe(false)
    expect(settled.trainingCloseoutStatus).toEqual({
      text: "queued · accepted",
      tone: "success",
    })
    expect(followups).toHaveLength(7)
  })

  test("training evidence packet build action dispatches and refreshes packet summary", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedBuildTrainingEvidencePacket({
        trainingRunRef: "training.run.4855",
      }),
    )
    expect(pending.trainingEvidencePacketBuildPending).toBe(true)
    expect(pending.trainingEvidencePacketBuildStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledBuildTrainingEvidencePacket({
        projection: {
          ok: false,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl: "desktop:training-evidence-packet-build",
          trainingRunRef: "training.run.4855",
          inputSource: "local.training_worker_receipts",
          packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
          reason: "packet_blocked",
          message: "wrote evidence packet candidate · 1 blockers",
          summary: null,
          blockerRefs: ["training.evidence_packet.requires_two_distinct_pylons"],
        },
      }),
    )

    expect(settled.trainingEvidencePacketBuildPending).toBe(false)
    expect(settled.trainingEvidencePacketBuildStatus).toEqual({
      text: "wrote evidence packet candidate · 1 blockers",
      tone: "info",
    })
    expect(modelTrainingEvidencePacketBuild(settled)?.reason).toBe(
      "packet_blocked",
    )
    expect(followups).toHaveLength(7)
  })

  test("training evidence admission action dispatches and refreshes projections on success", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedAdmitTrainingEvidence({
        trainingRunRef: "training.run.4855",
      }),
    )
    expect(pending.trainingEvidenceAdmissionPending).toBe(true)
    expect(pending.trainingEvidenceAdmissionStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledAdmitTrainingEvidence({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/runs/training.run.4855/real-gradient-evidence",
          trainingRunRef: "training.run.4855",
          packetSource: "env.OPENAGENTS_TRAINING_EVIDENCE_PACKET_PATH",
          run: null,
          realGradient: null,
          reason: "admitted",
          message: "admitted A1 real-gradient evidence for training.run.4855 · 3 receipts",
          evidenceRefCount: 6,
          receiptRefCount: 3,
          shardContributionCount: 2,
          distinctPylonCount: 2,
        },
      }),
    )

    expect(settled.trainingEvidenceAdmissionPending).toBe(false)
    expect(settled.trainingEvidenceAdmissionStatus.tone).toBe("success")
    expect(modelTrainingEvidenceAdmission(settled)?.receiptRefCount).toBe(3)
    expect(followups).toHaveLength(7)
  })

  test("training reconcile action dispatches and stores the public-safe result", () => {
    const [pending, commands] = update(
      initialModel,
      ClickedReconcileTrainingWindow({
        windowRef: "training.window.desktop.r1.test",
      }),
    )
    expect(pending.trainingReconcilePending).toBe(true)
    expect(pending.trainingReconcileStatus.tone).toBe("info")
    expect(commands).toHaveLength(1)

    const [settled, followups] = update(
      pending,
      SettledReconcileTrainingWindow({
        projection: {
          ok: true,
          enabled: true,
          fetchedAt: "2026-06-14T00:00:00.000Z",
          sourceUrl:
            "https://openagents.test/api/training/windows/training.window.desktop.r1.test/reconcile",
          windowRef: "training.window.desktop.r1.test",
          window: null,
          reason: "reconciled",
          message: "reconciled training.window.desktop.r1.test",
        },
      }),
    )
    expect(settled.trainingReconcilePending).toBe(false)
    expect(settled.trainingReconcileStatus.tone).toBe("success")
    expect(settled.trainingReconcile).toMatchObject({
      windowRef: "training.window.desktop.r1.test",
    })
    expect(followups).toHaveLength(7)
  })
})
