import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import {
  BUNDLE_TAG,
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  mobileHeaderProps,
  renderContentView,
  renderDrawerView,
  renderHomeView,
  syncStatusCopy,
  surfaceModeOptions,
} from "../src/screens/home-core"

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise((resolve) => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.persona_neutral_home.v1", () => {
  test("starts on the persona-neutral Khala surface and contains no Sarah mode", () => {
    expect(initialHomeState.surfaceMode).toBe("khala")
    expect(surfaceModeOptions.map((option) => option.id)).toEqual(["openagents", "khala"])
    expect(JSON.stringify(surfaceModeOptions)).not.toContain("Sarah")
    expect(chromeProps(initialHomeState).glassComposerVisible).toBe(true)
    expect(mobileHeaderProps(initialHomeState)).toEqual({ title: "Khala", subtitle: null })
  })

  test("projects T3-style thread identity into compact header chrome", () => {
    const state = {
      ...initialHomeState,
      conversationAuthority: "sync" as const,
      activeThreadRef: "thread.header.1",
      conversationThreads: [{
        threadRef: "thread.header.1",
        title: "Header parity",
        status: "active" as const,
        messageCount: 2,
        lastMessageAt: "2026-07-17T12:00:00.000Z",
        updatedAt: "2026-07-17T12:00:00.000Z",
        version: 1,
      }],
    }
    const serialized = JSON.stringify(renderHomeView(state))
    expect(mobileHeaderProps(state)).toEqual({ title: "Header parity", subtitle: null })
    expect(serialized).toContain('"key":"home-header-title"')
    expect(serialized).toContain('"key":"home-header-actions"')
    expect(serialized).toContain('"icon":"Menu"')
    expect(serialized).toContain('"icon":"Ellipsis"')
    expect(serialized).not.toContain('"key":"home-surface-mode"')
  })

  test("the content has exactly one Effect Native composer", () => {
    const serialized = JSON.stringify(renderContentView(initialHomeState))
    expect(serialized).toContain('"_tag":"Transcript"')
    expect(serialized.match(/"_tag":"Composer"/g)?.length).toBe(1)
    expect(serialized).toContain("khala-composer")
    expect(serialized).toContain('"name":"KhalaDraftChanged"')
    expect(serialized).toContain('"name":"KhalaTurnSubmitted"')
  })

  test("the neutral OpenAgents surface reports its unconfigured Sync state without inventing work", () => {
    const content = JSON.stringify(renderContentView({ ...initialHomeState, surfaceMode: "openagents" }))
    expect(syncStatusCopy("unconfigured")).toEqual({
      title: "Sync not configured",
      detail: "Connect an OpenAgents session to view shared work, repositories, and Fleet state.",
    })
    expect(content).toContain("Sync not configured")
    expect(content).toContain("Connect an OpenAgents session")
    expect(content).not.toContain("Active FleetRun")
    expect(content).not.toContain("Repository:")
  })

  test("reports local durability without claiming authenticated network Sync", async () => {
    const program = buildHomeProgram()
    program.sync.setPhase("local_ready")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const content = JSON.stringify(renderContentView({ ...state, surfaceMode: "openagents" }))
    expect(syncStatusCopy("local_ready")).toEqual({title:"Local device ready",detail:"Coding, conversations, and fleets work without an account. Link OpenAgents only for cross-device Sync and network features."})
    expect(content).toContain("Local device ready")
    expect(content).toContain("Link OpenAgents")
    expect(content).not.toContain("Sync live")
  })

  test("hides shared work while a recovered credential awaits server verification", async () => {
    const program = buildHomeProgram()
    program.sync.setPhase("credential_present_unverified")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const content = JSON.stringify(renderContentView({ ...state, surfaceMode: "openagents" }))
    expect(content).toContain("Session verification required")
    expect(content).toContain("Shared work stays hidden")
    expect(content).not.toContain("Sync live")
    expect(content).not.toContain("accessToken")
    expect(content).not.toContain("refreshToken")
    expect(content).not.toContain("ownerUserId")
  })

  test("distinguishes a verified session from live Sync", async () => {
    const program = buildHomeProgram()
    program.sync.setPhase("session_ready")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const content = JSON.stringify(renderContentView({ ...state, surfaceMode: "openagents" }))
    expect(content).toContain("Session verified")
    expect(content).toContain("Shared work is ready to connect")
    expect(content).not.toContain("Sync live")
    expect(content).toContain("Sign out")
  })

  // Oracle for openagents_mobile.session.pkce_sign_in_sign_out.v1.
  test("routes typed sign-in and sign-out intents through host session actions", async () => {
    const calls: Array<string> = []
    const program = buildHomeProgram({
      sessionActions: {
        signIn: async () => { calls.push("sign-in") },
        signOut: async () => { calls.push("sign-out") },
      },
    })
    const signedOutView = JSON.stringify(renderContentView({
      ...initialHomeState,
      surfaceMode: "openagents",
      syncPhase: "local_ready",
    }))
    expect(signedOutView).toContain("Link OpenAgents account")
    program.session.signIn()
    program.session.signOut()
    await Effect.runPromise(settle)
    expect(calls).toEqual(["sign-in", "sign-out"])
  })

  test("Effect Native composer dispatchers own the Khala draft and New chat clears it", async () => {
    const program = buildHomeProgram()
    program.khala.draftChanged("Plan the mobile handoff")
    await Effect.runPromise(settle)
    expect((await Effect.runPromise(lastState(program))).khala.draft).toBe("Plan the mobile handoff")

    program.chrome.pressNewChat()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.surfaceMode).toBe("khala")
    expect(state.khala.draft).toBe("")
    expect(state.khala.entries).toEqual([])
  })

  test("drawer keeps the owner-only Sarah entry visible without inventing local recents", () => {
    const serialized = JSON.stringify(renderDrawerView(initialHomeState))
    expect(serialized).toContain('"label":"New chat"')
    expect(serialized).toContain('"label":"Khala"')
    expect(serialized).toContain('"label":"Sarah · Sign in as owner"')
    expect(serialized).toContain("OpenAgentsSignInPressed")
    expect(serialized).toContain(`Bundle ${BUNDLE_TAG}`)
    expect(serialized).not.toContain("Recents")
  })

  test("drawer summarizes authenticated FleetRun activity without exposing raw refs", () => {
    const serialized = JSON.stringify(renderDrawerView({
      ...initialHomeState,
      fleetRuns: {
        schema: "openagents.fleet_run_client_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: "2026-07-13T10:55:20.179Z",
        runs: [{
          runRef: "fleet_run.sarah.f566771758bbe0ab5fc5",
          authorityStatus: "claimed_by_pylon",
          executionState: "completed",
          lastSequence: 96,
          attempts: [{
            workUnitRef: "unit.fc4.owner_local.acceptance.202607131047",
            workClaimRef: "claim.unit.owner-local",
            intakeClaimRef: "claim.sarah_fleet_run.0123456789abcdef01234567",
            assignmentRef: "assignment.public.khala_coding.chatcmpl_c9db1507f52a44468b43545317e10c8e",
            accountRefHash: "account.pylon.codex.f88a4773edd26cae162ceb2f",
            requestedTarget: "owner_local",
            selectedTarget: "owner_local",
            fallback: { truth: "not_applicable" },
            outcome: "accepted",
            closeoutRef: "assignment.closeout.summary.e2d06ebe9e9eaf48dd9e8d74",
            artifactRefs: [], proofRefs: [], authorityReceiptRefs: [],
            usageTruth: "exact", usageEvidenceRef: "receipt.usage.exact", tokenUsageRefs: ["receipt.usage.exact"], usageCaveatRefs: [], blockerRefs: [],
            terminalAt: "2026-07-13T10:55:20.179Z", updatedAt: "2026-07-13T10:55:20.179Z",
          }],
          createdAt: "2026-07-13T10:55:20.179Z",
          updatedAt: "2026-07-13T10:55:20.179Z",
        }],
      },
    }))
    expect(serialized).toContain("Fleet activity · 1 run · 1 completed")
    expect(serialized).not.toContain("fleet_run.sarah.f566771758bbe0ab5fc5")
    expect(serialized).not.toContain("assignment.closeout.summary.e2d06ebe9e9eaf48dd9e8d74")
  })

  test("drawer Settings opens the first-class settings hierarchy", async () => {
    const program = buildHomeProgram()
    program.chrome.toggleDrawer()
    program.chrome.pressSettings()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state).toMatchObject({ drawerOpen: false, workbenchRoute: "settings" })
    expect(JSON.stringify(renderContentView({ ...state, syncPhase: "local_ready" })))
      .toContain("Local identity and verified OpenAgents Sync")
  })
})
