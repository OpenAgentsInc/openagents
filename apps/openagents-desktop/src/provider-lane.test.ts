/**
 * Provider lane SPI (L1 #8899): a FIXTURE lane — a provider that has never
 * been hand-wired anywhere in main.ts — implements the typed adapter and runs
 * through the SAME shared dispatch engine as codex-local and fable-local.
 *
 * What this suite proves:
 * - the durable local-turn journal lifecycle (accept → streaming → terminal)
 *   works for a never-hand-wired lane ref;
 * - every envelope the dispatcher forwards to the renderer decodes against
 *   the FROZEN fable-local event envelope schema (no third vocabulary);
 * - the renderer's shared projection path renders the fixture lane's stream
 *   through the exact same transcript notes as the built-in lanes;
 * - interrupt, exact usage attribution, capability reporting, duplicate-turn
 *   refusal, and fail-closed restart recovery all hold with zero
 *   lane-specific wiring.
 */
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import type { DesktopThread } from "./chat-contract.ts"
import {
  FableLocalEventEnvelopeSchema,
  decodeFableLocalEventEnvelope,
  type FableLocalEvent,
  type FableLocalStartRequest,
} from "./fable-local-contract.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { reconcileLocalTurns } from "./local-turn-recovery.ts"
import {
  makeProviderLaneDispatcher,
  userNoteText,
  turnPromptText,
  type ProviderLane,
  type ProviderLaneDispatcherDeps,
} from "./provider-lane.ts"
import { makeLocalHarnessChatHost, type FableLocalRendererBridge } from "./renderer/local-harness.ts"
import { makeThreadStore } from "./thread-store.ts"

const FIXTURE_LANE_REF = "fixture-acp"
const FIXTURE_CHANNEL = "openagents:fixture-lane:event"

const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

type FixtureHarness = Readonly<{
  lane: ProviderLane<null>
  deps: ProviderLaneDispatcherDeps
  store: ReturnType<typeof makeThreadStore>
  journal: ReturnType<typeof openLocalTurnJournal>
  forwarded: Array<{ channel: string; payload: unknown }>
  ledgerRecords: Array<unknown>
  graphBegins: Array<unknown>
  graphEvents: Array<unknown>
  checkpoints: Array<string>
  runMessages: Array<string>
  interruptRequested: () => boolean
}>

/** A scripted fixture lane: emits a full typed stream, honors interrupt. */
const makeFixtureHarness = (root: string, options?: Readonly<{
  hangUntilInterrupt?: boolean
}>): FixtureHarness => {
  const store = makeThreadStore(path.join(root, "threads.json"))
  const journal = openLocalTurnJournal(path.join(root, "turns.json"))
  const forwarded: Array<{ channel: string; payload: unknown }> = []
  const ledgerRecords: Array<unknown> = []
  const graphBegins: Array<unknown> = []
  const graphEvents: Array<unknown> = []
  const checkpoints: Array<string> = []
  const runMessages: string[] = []
  let interrupted = false
  let releaseInterrupt: (() => void) | null = null

  const lane: ProviderLane<null> = {
    laneRef: FIXTURE_LANE_REF,
    graphLaneRef: "fixture_lane",
    eventChannel: FIXTURE_CHANNEL,
    usageProvider: "fixture_provider",
    capabilities: () => ({
      laneRef: FIXTURE_LANE_REF,
      provider: "fixture_provider",
      models: ["fixture-model-1"],
      features: {
        skills: false,
        planOnly: false,
        reasoningEffort: false,
        images: false,
        fullAuto: false,
        interrupt: true,
        queueFollowup: false,
        steerTurn: false,
        steerChild: false,
        answerQuestion: false,
      },
      composer: { displayName: "Fixture", reasoningEfforts: [], permissionModes: ["owner_full"], approvals: "none", extensions: [] },
      policy: {
        source: "native-static-declaration",
        profileRef: "native:fixture:v1",
        evidence: "conformant",
        allowedModels: ["fixture-model-1"],
        allowedFeatures: ["interrupt"],
        allowedExtensions: [],
      },
      recovery: "interrupt_on_restart",
    }),
    admit: request => {
      if (request.target !== undefined) {
        return { ok: false, error: "That provider target is not available on the fixture lane." }
      }
      return { ok: true, model: "fixture-model-1", context: null }
    },
    streamMeta: ctx => ({
      lane: FIXTURE_LANE_REF,
      turnRef: ctx.request.turnRef,
      ...(ctx.effectiveModel() === null ? {} : { model: "fixture-model-1" }),
    }),
    modelNoteText: model => `Fixture · ${model}`,
    runTurn: async ({ request, message, emit }) => {
      runMessages.push(message)
      if (options?.hangUntilInterrupt === true) {
        await new Promise<void>(resolve => {
          releaseInterrupt = resolve
        })
        return { ok: false, reason: "interrupted", detail: "turn interrupted" }
      }
      const events: ReadonlyArray<FableLocalEvent> = [
        { kind: "turn_started" },
        { kind: "model_effective", model: "fixture-model-1" },
        { kind: "text_delta", text: "Hello from " },
        // Header-only accounting may arrive between arbitrary text deltas. It
        // must not split one assistant sentence into multiple transcript rows.
        { kind: "meter_updated", inputTokens: 12, outputTokens: 3, totalTokens: 15 },
        { kind: "text_delta", text: "the fixture lane." },
        { kind: "tool_use", toolName: "Read", summary: "notes.md", itemRef: "item-1" },
        { kind: "tool_result", toolName: "Read", ok: true, summary: "12 lines", itemRef: "item-1" },
        { kind: "reasoning", text: "planning the reply" },
        {
          kind: "turn_completed",
          totalTokens: 42,
          accountRef: "fixture-account-1",
          usage: {
            inputTokens: 20,
            cachedInputTokens: 5,
            outputTokens: 17,
            reasoningTokens: 5,
            totalTokens: 42,
          },
        },
      ]
      for (const event of events) emit(event)
      void request
      return {
        ok: true,
        text: "Hello from the fixture lane.",
        totalTokens: 42,
        accountRef: "fixture-account-1",
        providerSessionRef: "fixture-session-1",
      }
    },
    interrupt: () => {
      interrupted = true
      releaseInterrupt?.()
      return releaseInterrupt !== null
    },
    finalMeta: ctx => ({
      lane: FIXTURE_LANE_REF,
      turnRef: ctx.request.turnRef,
      model: "fixture-model-1",
      ...(ctx.result.accountRef === undefined ? {} : { accountRef: ctx.result.accountRef }),
      ...(ctx.result.providerSessionRef === undefined || ctx.result.providerSessionRef === null
        ? {}
        : { requestId: ctx.result.providerSessionRef }),
      totalTokens: ctx.result.totalTokens,
      durationMs: ctx.durationMs,
    }),
    failureMessage: (reason, detail) =>
      `The fixture lane turn failed (${reason}${detail === "" ? "" : ` · ${detail}`}).`,
  }

  const deps: ProviderLaneDispatcherDeps = {
    threads: () => store,
    journal,
    liveAgentGraph: {
      beginTurn: input => graphBegins.push(input),
      applyEvent: (threadRef, envelope) => graphEvents.push({ threadRef, envelope }),
    },
    usageLedger: { record: input => ledgerRecords.push(input) },
    captureTurnCheckpoint: async (_threadRef, _turnRef, phase) => {
      checkpoints.push(phase)
    },
    localTurnFlushers: new Set(),
    isQuitting: () => false,
  }

  return {
    lane,
    deps,
    store,
    journal,
    forwarded,
    ledgerRecords,
    graphBegins,
    graphEvents,
    checkpoints,
    runMessages,
    interruptRequested: () => interrupted,
  }
}

const fakeSender = (forwarded: Array<{ channel: string; payload: unknown }>) => ({
  isDestroyed: () => false,
  send: (channel: string, payload: unknown) => forwarded.push({ channel, payload }),
})

const startRequest = (threadRef: string, turnRef = "turn-fixture-1"): FableLocalStartRequest => ({
  turnRef,
  threadRef,
  message: "run the fixture",
})

describe("provider lane SPI with a never-hand-wired fixture lane", () => {
  test("projects and revalidates the same bounded spec context across two lane refs", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-spec-lanes-"))
    try {
      const harness = makeFixtureHarness(root)
      const revalidated: string[] = []
      const projection = {
        snapshot: {
          productSpecs: [],
          assuranceSpecs: [{ path: "specs/work.assurance-spec.md", assuranceSpecId: "assurance.work", revision: 1, lifecycleState: "proposed" }],
          obligations: [{
            assuranceSpecPath: "specs/work.assurance-spec.md",
            obligationId: "AO-1",
            title: "Prove the failing case",
            criterionRefs: ["AC-1"],
            state: "unmet" as const,
            reason: "no schema-valid evidence index",
          }],
          diagnostics: [],
          truncated: false,
        },
        promptContext: "SPEC WORK CONTEXT (bounded)\nUNMET AO-1: Prove the failing case",
      }
      const dispatcher = makeProviderLaneDispatcher({
        ...harness.deps,
        specWorkflow: {
          beforeTurn: () => projection,
          afterTurn: laneRef => revalidated.push(laneRef),
        },
      })
      for (const [index, laneRef] of ["codex-local", "fable-local"].entries()) {
        const thread = harness.store.newThread()
        const lane = {
          ...harness.lane,
          laneRef,
          graphLaneRef: laneRef,
        }
        const result = await dispatcher.dispatchTurn(
          lane,
          startRequest(thread.id, `turn-spec-${index}`),
          fakeSender(harness.forwarded),
        )
        expect(result.ok).toBe(true)
      }
      expect(harness.runMessages).toEqual([
        "SPEC WORK CONTEXT (bounded)\nUNMET AO-1: Prove the failing case\n\nOWNER TURN INSTRUCTION:\nrun the fixture",
        "SPEC WORK CONTEXT (bounded)\nUNMET AO-1: Prove the failing case\n\nOWNER TURN INSTRUCTION:\nrun the fixture",
      ])
      expect(revalidated).toEqual(["codex-local", "fable-local"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("dispatches through the shared engine: journal lifecycle, frozen envelope, exact usage, capability report", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-"))
    try {
      const harness = makeFixtureHarness(root)
      const thread = harness.store.newThread()
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      const result = await dispatcher.dispatchTurn(
        harness.lane,
        startRequest(thread.id),
        fakeSender(harness.forwarded),
      )
      expect(result.ok).toBe(true)

      // Turn journal: the durable record carries the fixture lane ref and the
      // full completed lifecycle — a lane the journal never knew by name.
      const key = { threadRef: thread.id, turnRef: "turn-fixture-1", lane: FIXTURE_LANE_REF }
      const record = harness.journal.get(key)
      expect(record?.lane).toBe(FIXTURE_LANE_REF)
      expect(record?.phase).toBe("completed")
      expect(record?.disposition).toBe("completed")
      expect(record?.assistantText).toBe("Hello from the fixture lane.")

      // Every forwarded payload decodes against the FROZEN envelope schema —
      // the SPI stream is the existing renderer vocabulary, not a third one.
      expect(harness.forwarded.length).toBeGreaterThan(0)
      for (const entry of harness.forwarded) {
        expect(entry.channel).toBe(FIXTURE_CHANNEL)
        expect(decodeFableLocalEventEnvelope(entry.payload)).not.toBeNull()
      }
      void FableLocalEventEnvelopeSchema

      // The persisted transcript carries the SAME shared notes the built-in
      // lanes persist: user note, tool trace (typed meta), lane-branded model
      // caption, and the assistant note with the lane's final metadata.
      const notes = harness.store.open(thread.id)?.notes ?? []
      const bodies = notes.map(note => `${note.role}:${note.text}`)
      expect(bodies).toContain("user:run the fixture")
      expect(bodies).toContain("system:Read · started · notes.md")
      expect(bodies).toContain("system:Read · ok · 12 lines")
      expect(bodies).toContain("system:Fixture · fixture-model-1")
      const traceNote = notes.find(note => note.key === "turn-fixture-1-tool-item-1")
      expect(traceNote?.meta?.trace?.toolName).toBe("Read")
      const assistant = notes.find(note => note.role === "assistant")
      expect(assistant?.text).toBe("Hello from the fixture lane.")
      expect(assistant?.meta).toMatchObject({
        lane: FIXTURE_LANE_REF,
        turnRef: "turn-fixture-1",
        model: "fixture-model-1",
        accountRef: "fixture-account-1",
        requestId: "fixture-session-1",
        totalTokens: 42,
      })

      // Exact usage attribution flows to the ledger under the lane's own
      // provider ref with the provider-reported split intact.
      expect(harness.ledgerRecords).toEqual([{
        provider: "fixture_provider",
        accountRef: "fixture-account-1",
        requestedModel: "fixture-model-1",
        kind: "turn",
        usage: {
          inputTokens: 20,
          cachedInputTokens: 5,
          outputTokens: 17,
          reasoningTokens: 5,
          totalTokens: 42,
        },
      }])

      // Live agent graph fold + workspace checkpoints ran for the lane.
      expect(harness.graphBegins).toEqual([
        { turnRef: "turn-fixture-1", threadRef: thread.id, lane: "fixture_lane" },
      ])
      expect(harness.graphEvents.length).toBe(9)
      expect(harness.checkpoints).toEqual(["turn_start", "turn_completed"])

      // Capability report: honest feature truth (input to L2).
      const report = harness.lane.capabilities()
      expect(report.laneRef).toBe(FIXTURE_LANE_REF)
      expect(report.recovery).toBe("interrupt_on_restart")
      expect(report.features.fullAuto).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("an unexpected provider throw emits a typed failure and cannot strand the accepted turn", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-throw-"))
    try {
      const harness = makeFixtureHarness(root)
      const thread = harness.store.newThread()
      const lane: ProviderLane<null> = {
        ...harness.lane,
        runTurn: async ({ emit }) => {
          emit({ kind: "turn_started" })
          emit({ kind: "text_delta", text: "Partial reply." })
          throw new Error("failed to list apps: 403 response body must stay private")
        },
      }
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      const result = await dispatcher.dispatchTurn(lane, startRequest(thread.id), fakeSender(harness.forwarded))

      expect(result).toEqual({
        ok: false,
        reason: "session_failed",
        error: "The fixture lane turn failed (session_failed · The provider lane stopped unexpectedly.).",
      })
      const key = { threadRef: thread.id, turnRef: "turn-fixture-1", lane: FIXTURE_LANE_REF }
      expect(harness.journal.get(key)).toMatchObject({
        phase: "failed",
        disposition: "failed",
        assistantText: "Partial reply.",
      })
      expect(harness.deps.localTurnFlushers.size).toBe(0)
      expect(harness.store.open(thread.id)?.notes.some(note =>
        note.role === "assistant" && note.text === "Partial reply."
      )).toBe(true)

      const finalEnvelope = decodeFableLocalEventEnvelope(harness.forwarded.at(-1)?.payload)
      expect(finalEnvelope?.event).toEqual({
        kind: "turn_failed",
        reason: "session_failed",
        detail: "The provider lane stopped unexpectedly.",
      })
      expect(harness.graphEvents.at(-1)).toMatchObject({
        threadRef: thread.id,
        envelope: { turnRef: "turn-fixture-1", event: finalEnvelope?.event },
      })
      expect(JSON.stringify(harness.forwarded)).not.toContain("403 response body")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("shared user-note/prompt helpers keep images-only turns honest", () => {
    expect(userNoteText("  ", [{}])).toBe("(1 image attached)")
    expect(userNoteText("hello", [])).toBe("hello")
    expect(turnPromptText(" ", [{}, {}])).toBe("Please look at the attached images.")
    expect(turnPromptText("do it")).toBe("do it")
  })

  test("renderer projection renders the fixture lane's captured stream through the shared path", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-render-"))
    try {
      const harness = makeFixtureHarness(root)
      const thread = harness.store.newThread()
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      await dispatcher.dispatchTurn(harness.lane, startRequest(thread.id), fakeSender(harness.forwarded))
      const captured = harness.forwarded.map(entry => decodeFableLocalEventEnvelope(entry.payload))
      expect(captured.every(envelope => envelope !== null)).toBe(true)
      const finalThread = harness.store.open(thread.id) as DesktopThread

      // Replay the fixture lane's EXACT captured envelopes through the
      // renderer's one shared local-lane projection (the same code path both
      // built-in lanes render through) — no fixture-specific renderer wiring.
      let listener: ((envelope: { turnRef: string; event: FableLocalEvent }) => void) | null = null
      const bridge: FableLocalRendererBridge = {
        availability: async () => ({ state: "available", accountRef: "fixture-account-1" }),
        start: async value => {
          const turnRef = (value as { turnRef: string }).turnRef
          for (const envelope of captured) {
            if (envelope !== null) listener?.({ turnRef, event: envelope.event })
          }
          return { ok: true, thread: finalThread }
        },
        interrupt: async () => true,
        onEvent: cb => {
          listener = cb as typeof listener
          return () => {}
        },
      }
      const host = makeLocalHarnessChatHost({
        base: {
          listThreads: async () => [finalThread],
          newThread: async () => finalThread,
          openThread: async () => finalThread,
          sendMessage: async () => ({ ok: false, error: "legacy path must not be used" }),
        },
        fable: bridge,
        fableAvailability: () => ({ state: "available", accountRef: "fixture-account-1" }),
        randomId: () => "fixture",
        scheduleProjection: flush => {
          let active = true
          queueMicrotask(() => {
            if (active) flush()
          })
          return () => {
            active = false
          }
        },
      })
      const updates: DesktopThread[] = []
      const result = await host.sendMessage({
        id: thread.id,
        message: "run the fixture",
        harness: "fable",
        onUpdate: projected => updates.push(projected),
      })
      await settle()
      expect(result.ok).toBe(true)
      const last = updates.at(-1)
      expect(last).toBeDefined()
      const bodies = (last?.notes ?? []).map(note => `${note.role}:${note.text}`)
      // The same transcript cards the built-in lanes render: streamed
      // assistant text, typed tool trace lines, reasoning treatment.
      expect(bodies).toContain("assistant:Hello from the fixture lane.")
      expect(bodies).toContain("system:Read · started · notes.md")
      expect(bodies).toContain("system:Read · ok · 12 lines")
      expect(bodies).toContain("system:Reasoning · planning the reply")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("interrupt settles the journal with the owner-interrupted disposition and the lane's failure copy", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-interrupt-"))
    try {
      const harness = makeFixtureHarness(root, { hangUntilInterrupt: true })
      const thread = harness.store.newThread()
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      const pending = dispatcher.dispatchTurn(
        harness.lane,
        startRequest(thread.id),
        fakeSender(harness.forwarded),
      )
      await settle()
      expect(harness.lane.interrupt("turn-fixture-1")).toBe(true)
      const result = await pending
      expect(result).toEqual({
        ok: false,
        reason: "interrupted",
        error: "The fixture lane turn failed (interrupted · turn interrupted).",
      })
      const record = harness.journal.get({
        threadRef: thread.id,
        turnRef: "turn-fixture-1",
        lane: FIXTURE_LANE_REF,
      })
      expect(record?.phase).toBe("interrupted")
      expect(record?.disposition).toBe("owner_interrupted")
      expect(harness.interruptRequested()).toBe(true)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a duplicate turn ref is refused typed before any provider dispatch", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-duplicate-"))
    try {
      const harness = makeFixtureHarness(root)
      const thread = harness.store.newThread()
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      const first = await dispatcher.dispatchTurn(harness.lane, startRequest(thread.id), null)
      expect(first.ok).toBe(true)
      const duplicate = await dispatcher.dispatchTurn(harness.lane, startRequest(thread.id), null)
      expect(duplicate).toEqual({ ok: false, error: "That turn is already accepted." })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("typed lane admission refusal never reaches the journal", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-admit-"))
    try {
      const harness = makeFixtureHarness(root)
      const thread = harness.store.newThread()
      const dispatcher = makeProviderLaneDispatcher(harness.deps)
      const refused = await dispatcher.dispatchTurn(harness.lane, {
        ...startRequest(thread.id),
        target: { provider: "codex", accountRef: "codex-1", model: "gpt-5.5" },
      }, null)
      expect(refused).toEqual({ ok: false, error: "That provider target is not available on the fixture lane." })
      expect(harness.journal.list()).toEqual([])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("restart recovery fails CLOSED for a nonterminal fixture-lane record (no fabricated resume)", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-lane-recovery-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const thread = store.newThread()
      const journal = openLocalTurnJournal(path.join(root, "turns.json"))
      const key = { threadRef: thread.id, turnRef: "turn-fixture-9", lane: FIXTURE_LANE_REF }
      journal.accept({
        ...key,
        userMessageKey: "turn-fixture-9-user",
        assistantMessageKey: "turn-fixture-9-assistant-0",
        accountRef: "fixture-account-1",
        model: "gpt-5.6-sol",
      })
      journal.recordDispatch(key, "fixture-account-1")
      // Even with a recorded provider session AND a codex-recoverable model,
      // an unknown lane must never enter the codex replay path.
      journal.recordProviderSession(key, {
        accountRef: "fixture-account-1",
        providerSessionRef: "fixture-session-9",
      })
      let resumes = 0
      const outcomes = await reconcileLocalTurns({
        journal,
        store,
        codex: {
          runTurn: async () => {
            resumes += 1
            throw new Error("a never-hand-wired lane must not replay through codex")
          },
        },
      })
      expect(resumes).toBe(0)
      expect(outcomes).toEqual([{ key: expect.objectContaining(key), state: "interrupted" }])
      expect(journal.get(key)?.disposition).toBe("interrupted_by_restart")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
