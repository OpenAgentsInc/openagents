/**
 * Local-mode harness routing (#8712): the renderer half of the
 * no-silent-substitution law. A "fable" send streams through the fableLocal
 * bridge; a "codex" send is an explicit typed refusal; NEITHER ever reaches
 * the base host's legacy gateway sendMessage. Only a laneless send does.
 */
import { describe, expect, test } from "vite-plus/test"

import type { DesktopThread } from "../chat-contract.ts"
import { makeComposerSubmitIntent } from "../composer-admission.ts"
import type { FableLocalEventEnvelope } from "../fable-local-contract.ts"
import type { ChatHost } from "./shell.ts"
import {
  codexLocalUnavailableMessage,
  makeLocalHarnessChatHost,
  type FableLocalRendererBridge,
} from "./local-harness.ts"

const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

const threadWithUserNote: DesktopThread = {
  id: "thread-1",
  title: "New chat",
  updatedAt: "2026-07-11T10:00:00.000Z",
  notes: [{ key: "user-1", role: "user", text: "hello fable", timestamp: "10:00" }],
}

const finalThread: DesktopThread = {
  ...threadWithUserNote,
  notes: [
    ...threadWithUserNote.notes,
    { key: "assistant-1", role: "assistant", text: "Hello world", timestamp: "10:00" },
  ],
}

type Harness = {
  host: ChatHost
  legacySends: Array<unknown>
  startCalls: Array<unknown>
  interruptCalls: Array<unknown>
  steerCalls: Array<unknown>
  currentSteerCalls: Array<unknown>
  queueCalls: Array<unknown>
  emit: (envelope: FableLocalEventEnvelope) => void
  resolveStart: (value: unknown) => void
  unsubscribed: () => boolean
}

const makeHarness = (input?: {
  fableAvailable?: boolean
  bridge?: boolean
  scheduleProjection?: (flush: () => void) => () => void
}): Harness => {
  const legacySends: Array<unknown> = []
  const startCalls: Array<unknown> = []
  const interruptCalls: Array<unknown> = []
  const steerCalls: Array<unknown> = []
  const currentSteerCalls: Array<unknown> = []
  const queueCalls: Array<unknown> = []
  let listener: ((envelope: FableLocalEventEnvelope) => void) | null = null
  let resolveStart: (value: unknown) => void = () => {}
  let starts = 0
  let unsubscribed = false
  const base: ChatHost = {
    listThreads: async () => [threadWithUserNote],
    newThread: async () => threadWithUserNote,
    openThread: async () => threadWithUserNote,
    sendMessage: async send => {
      legacySends.push(send)
      return { ok: true, thread: finalThread }
    },
  }
  const bridge: FableLocalRendererBridge = {
    availability: async () => ({ state: "available", accountRef: "claude-pylon-b" }),
    start: async value => {
      startCalls.push(value)
      starts += 1
      // A chained promoted-follow-up turn (A3) resolves immediately so a test
      // that promotes a queued message does not hang.
      if (starts > 1) return { ok: true, thread: finalThread }
      return new Promise(resolve => {
        resolveStart = resolve
      })
    },
    interrupt: async value => { interruptCalls.push(value); return true },
    steerChild: async value => {
      steerCalls.push(value)
      return { ok: true, outcome: "interrupted" }
    },
    steerCurrent: async value => {
      currentSteerCalls.push(value)
      return { ok: true, outcome: "delivered" }
    },
    queueFollowup: async value => {
      queueCalls.push(value)
      return { ok: true, queued: true, queueRef: "q1", position: 1 }
    },
    onEvent: cb => {
      listener = cb
      return () => {
        unsubscribed = true
      }
    },
  }
  const host = makeLocalHarnessChatHost({
    base,
    fable: input?.bridge === false ? null : bridge,
    fableAvailability: () =>
      input?.fableAvailable === false
        ? { state: "unavailable", reason: "no_claude_account" }
        : { state: "available", accountRef: "claude-pylon-b" },
    randomId: () => "fixed",
    now: () => new Date("2026-07-16T20:00:00.000Z"),
    scheduleProjection: input?.scheduleProjection ?? (flush => {
      let active = true
      queueMicrotask(() => { if (active) flush() })
      return () => { active = false }
    }),
  })
  return {
    host,
    legacySends,
    startCalls,
    interruptCalls,
    steerCalls,
    currentSteerCalls,
    queueCalls,
    emit: envelope => listener?.(envelope),
    resolveStart: value => resolveStart(value),
    unsubscribed: () => unsubscribed,
  }
}

describe("makeLocalHarnessChatHost", () => {
  test("Stop lowers the exact active local turn into a typed control outcome", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "hello fable",
      harness: "fable",
    })
    await settle()

    expect(await harness.host.interruptActiveControl!("wrong-thread")).toBeNull()
    expect(harness.interruptCalls).toEqual([])
    expect(await harness.host.interruptActiveControlIdentity!("thread-1")).toEqual({
      threadRef: "thread-1",
      intentRef: "intent.desktop.interrupt.turn.fable.fixed",
      idempotencyKey: "intent.desktop.interrupt.turn.fable.fixed",
    })
    expect(await harness.host.interruptActiveControl!("thread-1")).toMatchObject({
      schema: "openagents.runtime_control_outcome.v1",
      intentRef: "intent.desktop.interrupt.turn.fable.fixed",
      admission: { status: "accepted" },
      delivery: { status: "applied" },
      terminal: { status: "pending" },
    })
    expect(harness.interruptCalls).toEqual([{ turnRef: "turn.fable.fixed" }])

    harness.resolveStart({ ok: false, reason: "interrupted", error: "Turn interrupted." })
    await pending
    expect(await harness.host.interruptActiveControl!()).toBeNull()
  })

  test("fable send streams: progressive text, tool trace lines, finalized thread", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "hello fable",
      harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    expect(harness.startCalls).toEqual([
      { turnRef: "turn.fable.fixed", threadRef: "thread-1", message: "hello fable" },
    ])

    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "turn_started", thread: threadWithUserNote },
    })
    await settle()
    // Effective-model visibility: the SDK-reported model renders as a caption
    // trace line ("Claude · claude-fable-5") — model identity never comes from
    // the lane brand alone.
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "model_effective", model: "claude-fable-5" },
    })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "Hello " } })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "world" } })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "tool_use", toolName: "Read", summary: "notes.md" },
    })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "tool_result", toolName: "Read", ok: true, summary: "" },
    })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "Done." } })
    // A foreign turn's events never touch this stream.
    harness.emit({ turnRef: "turn.fable.other", event: { kind: "text_delta", text: "IGNORED" } })
    await settle()

    const last = updates.at(-1)!
    const bodies = last.notes.map(note => `${note.role}:${note.text}`)
    // Arrival order is exact: text before the tool stays before it, and text
    // after the tool starts a new assistant segment after the tool pair.
    expect(bodies).toEqual([
      "user:hello fable",
      "system:Claude · claude-fable-5",
      "assistant:Hello world",
      "system:Read · started · notes.md",
      "system:Read · ok",
      "assistant:Done.",
    ])
    // Progressive: an earlier snapshot carried the partial assistant text.
    expect(updates.some(update =>
      update.notes.some(note => note.role === "assistant" && note.text === "Hello "))).toBe(true)
    // Streaming metadata (#8712 message inspector): the growing assistant
    // note already carries lane/turn/effective-model facts.
    const streamedAssistants = last.notes.filter(note => note.role === "assistant")
    expect(streamedAssistants).toHaveLength(2)
    expect(streamedAssistants.every(note => note.meta?.lane === "fable-local" &&
      note.meta.turnRef === "turn.fable.fixed" && note.meta.model === "claude-fable-5")).toBe(true)
    expect(updates.every(update => update.notes.every(note => !note.text.includes("IGNORED")))).toBe(true)
    // Typed trace facts (EP250 tool cards): trace notes carry the same
    // bounded payload as their text line, so the shell builds typed cards
    // without re-parsing display strings.
    const traceNotes = last.notes.filter(note => note.meta?.trace !== undefined)
    expect(traceNotes.map(note => note.meta!.trace)).toEqual([
      { toolName: "Read", phase: "started", summary: "notes.md" },
      { toolName: "Read", phase: "ok", summary: "" },
    ])

    harness.resolveStart({ ok: true, thread: finalThread })
    const result = await pending
    expect(result).toEqual({ ok: true, thread: finalThread })
    expect(harness.unsubscribed()).toBe(true)
    // LAW: the legacy gateway path was never touched by a harness send.
    expect(harness.legacySends).toEqual([])
  })

  test("an exact provider target crosses the renderer bridge unchanged", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "hello fable",
      harness: "fable",
      target: { provider: "claude_agent", accountRef: "claude-pylon-b", model: "claude-fable-5" },
      skill: { pluginRef: "plugin.local.0123456789abcdef01234567", name: "review" },
      permissionMode: "plan_only",
    })
    await settle()
    expect(harness.startCalls[0]).toEqual({
      turnRef: "turn.fable.fixed",
      threadRef: "thread-1",
      message: "hello fable",
      target: { provider: "claude_agent", accountRef: "claude-pylon-b", model: "claude-fable-5" },
      skill: { pluginRef: "plugin.local.0123456789abcdef01234567", name: "review" },
      permissionMode: "plan_only",
    })
    harness.resolveStart({ ok: true, thread: finalThread })
    expect((await pending).ok).toBe(true)
  })

  test("item-keyed command progress updates one running note in place", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1", message: "run checks", harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "turn_started", thread: threadWithUserNote } })
    const started = {
      kind: "command", source: "codex", command: "pnpm test", cwd: "/safe/repo",
      status: "in_progress",
    } as const
    harness.emit({ turnRef: "turn.fable.fixed", event: {
      kind: "tool_use", toolName: "Bash", itemRef: "cmd-1", summary: "pnpm test", item: started,
    } })
    harness.emit({ turnRef: "turn.fable.fixed", event: {
      kind: "tool_progress", toolName: "Bash", itemRef: "cmd-1", summary: "7 output characters",
      item: { ...started, outputTail: "running" },
    } })
    await settle()
    const traces = updates.at(-1)!.notes.filter(note => note.meta?.trace !== undefined)
    expect(traces).toHaveLength(1)
    expect(traces[0]).toMatchObject({
      key: "turn.fable.fixed-tool-cmd-1",
      meta: { trace: { itemRef: "cmd-1", phase: "progress", item: { outputTail: "running" } } },
    })
    harness.resolveStart({ ok: true, thread: finalThread })
    expect((await pending).ok).toBe(true)
  })

  test("10,000 synchronous provider deltas publish once per cadence with exact text", async () => {
    const scheduled: Array<() => void> = []
    const harness = makeHarness({ scheduleProjection: flush => {
      let active = true
      scheduled.push(() => { if (active) flush() })
      return () => { active = false }
    } })
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({ id: "thread-1", message: "stress", harness: "fable", onUpdate: thread => updates.push(thread) })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "turn_started", thread: threadWithUserNote } })
    for (let index = 0; index < 10_000; index++) {
      harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "x" } })
    }
    expect(scheduled).toHaveLength(1)
    expect(updates).toHaveLength(0)
    scheduled[0]!()
    expect(updates).toHaveLength(1)
    expect(updates[0]!.notes.at(-1)?.text).toBe("x".repeat(10_000))
    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("question_pending projects an interactive question note; question_resolved updates it in place (EP250)", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "ask me",
      harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "turn_started", thread: threadWithUserNote },
    })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: {
        kind: "question_pending",
        questionRef: "question.1",
        questions: [{
          question: "Which path should we take?",
          header: "Fixture",
          multiSelect: false,
          options: [{ label: "Streamed", description: "Keep streaming" }, { label: "Static" }],
        }],
      },
    })
    await settle()
    const withQuestion = updates.at(-1)!
    const questionNote = withQuestion.notes.find(note => note.question !== undefined)
    expect(questionNote).toBeDefined()
    expect(questionNote!.key).toBe("turn.fable.fixed-question-question.1")
    expect(questionNote!.question).toMatchObject({
      turnRef: "turn.fable.fixed",
      questionRef: "question.1",
      status: "pending",
    })
    expect(questionNote!.question!.questions[0]).toMatchObject({
      question: "Which path should we take?",
      header: "Fixture",
      multiSelect: false,
    })
    // The runtime-authoritative outcome updates the SAME note in place.
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "question_resolved", questionRef: "question.1", outcome: "timeout" },
    })
    await settle()
    const resolved = updates.at(-1)!
    const resolvedNotes = resolved.notes.filter(note => note.question !== undefined)
    expect(resolvedNotes).toHaveLength(1)
    expect(resolvedNotes[0]!.question!.status).toBe("timeout")
    // A foreign question ref never mutates this turn's notes.
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "question_resolved", questionRef: "question.unknown", outcome: "denied" },
    })
    await settle()
    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
    expect(harness.legacySends).toEqual([])
  })

  test("codex send in local mode is an explicit refusal — never the legacy gateway", async () => {
    const harness = makeHarness()
    const result = await harness.host.sendMessage({ id: "thread-1", message: "hi", harness: "codex" })
    expect(result).toEqual({ ok: false, error: codexLocalUnavailableMessage, failureKind: "signed_out" })
    expect(harness.legacySends).toEqual([])
    expect(harness.startCalls).toEqual([])
  })

  test("fable send with no available account is a typed refusal — never the legacy gateway", async () => {
    const harness = makeHarness({ fableAvailable: false })
    const result = await harness.host.sendMessage({ id: "thread-1", message: "hi", harness: "fable" })
    expect(result.ok).toBe(false)
    expect(result.error).toContain("no linked Claude account home found")
    expect(result.error).toContain("No message was routed to any other lane.")
    expect(harness.legacySends).toEqual([])
    expect(harness.startCalls).toEqual([])
  })

  test("fable send with no bridge is a typed refusal — never the legacy gateway", async () => {
    const harness = makeHarness({ bridge: false })
    const result = await harness.host.sendMessage({ id: "thread-1", message: "hi", harness: "fable" })
    expect(result.ok).toBe(false)
    expect(harness.legacySends).toEqual([])
  })

  test("only a laneless send reaches the base host's legacy fallback", async () => {
    const harness = makeHarness()
    const result = await harness.host.sendMessage({ id: "thread-1", message: "hi" })
    expect(result.ok).toBe(true)
    expect(harness.legacySends.length).toBe(1)
  })

  test("an invalid start response maps to a typed error", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "hi", harness: "fable" })
    await settle()
    harness.resolveStart("garbage")
    const result = await pending
    expect(result).toEqual({ ok: false, error: "The local Claude lane returned an invalid response.", failureKind: "failed" })
  })

  test("an interrupted provider result remains interrupted at the renderer boundary", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "hi", harness: "fable" })
    await settle()
    harness.resolveStart({ ok: false, reason: "interrupted", error: "Turn interrupted." })
    expect(await pending).toEqual({
      ok: false,
      reason: "interrupted",
      error: "Turn interrupted.",
      failureKind: "interrupted",
    })
  })

  test("steerChild routes an interrupt to the active lane by exact ref (EP250 wave-2 G4)", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "go", harness: "fable" })
    await settle()
    const outcome = await harness.host.steerChild!({ turnRef: "turn.fable.fixed", childRef: "c1" })
    expect(outcome).toEqual({ ok: true, outcome: "interrupted" })
    // Only `interrupt` is ever offered (message is capability-unsupported).
    expect(harness.steerCalls).toEqual([{ turnRef: "turn.fable.fixed", childRef: "c1", action: "interrupt" }])
    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("steerChild with no active turn is a typed not_found no-op", async () => {
    const harness = makeHarness()
    expect(await harness.host.steerChild!({ turnRef: "x", childRef: "c" })).toEqual({ ok: false, outcome: "not_found" })
    expect(harness.steerCalls).toEqual([])
  })

  test("delegated child cards retain the exact prompt and answer as a selectable transcript", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "delegate",
      harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "turn_started", thread: threadWithUserNote },
    })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: {
        kind: "child_started",
        childRef: "child.codex.turn.fable.fixed.1",
        summary: "Review the patch",
        prompt: "Review the patch\n\nContext:\nFocus on the failing test.",
      },
    })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: {
        kind: "child_completed",
        childRef: "child.codex.turn.fable.fixed.1",
        accountRef: "codex-current",
        summary: "The stale fixture is the cause.",
        response: "The stale fixture is the cause. Update it and rerun the focused suite.",
        usage: null,
        durationMs: 42,
      },
    })
    await settle()

    const child = updates.at(-1)!.notes.find(note => note.runtime?.kind === "child")?.runtime
    expect(child?.kind).toBe("child")
    if (child?.kind !== "child") throw new Error("child runtime card missing")
    expect(child.transcript).toEqual([
      { role: "user", text: "Review the patch\n\nContext:\nFocus on the failing test." },
      { role: "assistant", text: "The stale fixture is the cause. Update it and rerun the focused suite." },
    ])

    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("queueFollowup routes to the active lane's queue channel (EP250 wave-2 A3)", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "go", harness: "fable" })
    await settle()
    const outcome = await harness.host.queueFollowup!({ threadRef: "thread-1", message: "and then this" })
    expect(outcome).toMatchObject({ ok: true, queued: true })
    expect(harness.queueCalls).toEqual([{ threadRef: "thread-1", message: "and then this" }])
    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("Queue and Steer return typed acknowledgements for only the exact active target", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "go", harness: "fable" })
    await settle()

    const queue = makeComposerSubmitIntent({
      admission: { state: "active_steerable", activeTurnId: "provider-turn-1", reason: null, queuedCount: 0 },
      mode: "queue",
      threadRef: "thread-1",
      message: "next",
      intentRef: "intent.queue.1",
      clientUserMessageId: "message.queue.1",
      createdAt: "2026-07-16T20:00:00.000Z",
    })
    if (queue?.kind !== "queue_next") throw new Error("queue intent missing")
    expect(await harness.host.queueFollowupControl!(queue)).toMatchObject({
      schema: "openagents.runtime_control_outcome.v1",
      intentRef: "intent.queue.1",
      admission: { status: "accepted" },
      delivery: { status: "queued", queueRef: "q1" },
      terminal: { status: "pending" },
    })

    const steer = makeComposerSubmitIntent({
      admission: { state: "active_steerable", activeTurnId: "provider-turn-1", reason: null, queuedCount: 1 },
      mode: "steer",
      threadRef: "thread-1",
      message: "change course",
      intentRef: "intent.steer.1",
      clientUserMessageId: "message.steer.1",
      createdAt: "2026-07-16T20:00:00.000Z",
    })
    if (steer?.kind !== "steer_current") throw new Error("steer intent missing")
    expect(await harness.host.steerCurrentControl!(steer)).toMatchObject({
      schema: "openagents.runtime_control_outcome.v1",
      intentRef: "intent.steer.1",
      admission: { status: "accepted" },
      delivery: { status: "applied" },
      terminal: { status: "pending" },
    })

    expect(await harness.host.queueFollowupControl!({ ...queue, threadRef: "wrong-thread" })).toMatchObject({
      admission: { status: "rejected", reasonRef: "reason.target_mismatch" },
      delivery: { status: "failed", reasonRef: "reason.target_mismatch" },
    })
    expect(harness.queueCalls).toHaveLength(1)
    expect(harness.currentSteerCalls).toHaveLength(1)

    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("a promoted follow-up is chained as the next turn (A3 queue-until-idle)", async () => {
    const harness = makeHarness()
    const pending = harness.host.sendMessage({ id: "thread-1", message: "first", harness: "fable" })
    await settle()
    expect(harness.startCalls).toHaveLength(1)
    // The runtime promotes a queued message on the ending turn's stream…
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "followup_promoted", queueRef: "q1", message: "second" } })
    // …and the turn finalizes; the host must start the promoted message next.
    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
    expect(harness.startCalls).toHaveLength(2)
    expect(harness.startCalls[1]).toMatchObject({ threadRef: "thread-1", message: "second" })
  })

  test("meter_updated projects a live thread.meter snapshot, NOT a timeline note (T11 #8868)", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "hello fable",
      harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "turn_started", thread: threadWithUserNote } })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "One continuous " } })
    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "meter_updated", inputTokens: 100, outputTokens: 20, totalTokens: 120 },
    })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "sentence." } })
    await settle()
    const afterFirst = updates.at(-1)!
    expect(afterFirst.meter).toEqual({ inputTokens: 100, outputTokens: 20, totalTokens: 120 })
    // A meter update is not a chat message or semantic boundary: deltas on
    // either side remain one assistant row, with no phantom vertical gap.
    expect(afterFirst.notes).toEqual([
      ...threadWithUserNote.notes,
      expect.objectContaining({ role: "assistant", text: "One continuous sentence." }),
    ])

    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "meter_updated", rateLimits: [{ label: "primary", usedPercent: 12 }] },
    })
    await settle()
    // Sparse rate-limit update: primary added, prior token fields carried forward.
    expect(updates.at(-1)!.meter).toEqual({
      inputTokens: 100,
      outputTokens: 20,
      totalTokens: 120,
      rateLimits: [{ label: "primary", usedPercent: 12 }],
    })

    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "meter_updated", rateLimits: [{ label: "secondary", usedPercent: 4 }] },
    })
    await settle()
    // A secondary-only rolling update must NOT erase the already-known primary.
    expect(updates.at(-1)!.meter!.rateLimits).toEqual([
      { label: "primary", usedPercent: 12 },
      { label: "secondary", usedPercent: 4 },
    ])

    harness.emit({
      turnRef: "turn.fable.fixed",
      event: { kind: "meter_updated", rateLimits: [{ label: "primary", usedPercent: 30 }] },
    })
    await settle()
    // A repeated label REPLACES that window (not merged field-by-field).
    expect(updates.at(-1)!.meter!.rateLimits).toEqual([
      { label: "primary", usedPercent: 30 },
      { label: "secondary", usedPercent: 4 },
    ])

    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })

  test("keyed card updates do not split one assistant paragraph into phantom rows", async () => {
    const harness = makeHarness()
    const updates: DesktopThread[] = []
    const pending = harness.host.sendMessage({
      id: "thread-1",
      message: "keep the prose together",
      harness: "fable",
      onUpdate: thread => updates.push(thread),
    })
    await settle()
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "turn_started", thread: threadWithUserNote } })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "Before the command." } })
    harness.emit({ turnRef: "turn.fable.fixed", event: {
      kind: "tool_use", toolName: "Bash", itemRef: "cmd-1", summary: "pnpm test",
    } })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "I will not touch " } })
    harness.emit({ turnRef: "turn.fable.fixed", event: {
      kind: "tool_progress", toolName: "Bash", itemRef: "cmd-1", summary: "still running",
      item: { kind: "command", source: "codex", command: "pnpm test", cwd: "/repo", status: "in_progress", outputTail: "still running" },
    } })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "or hide " } })
    harness.emit({ turnRef: "turn.fable.fixed", event: {
      kind: "tool_result", toolName: "Bash", itemRef: "cmd-1", ok: true, summary: "passed",
    } })
    harness.emit({ turnRef: "turn.fable.fixed", event: { kind: "text_delta", text: "that work." } })
    await settle()

    expect(updates.at(-1)!.notes.filter(note => note.role === "assistant").map(note => note.text)).toEqual([
      "Before the command.",
      "I will not touch or hide that work.",
    ])

    harness.resolveStart({ ok: true, thread: finalThread })
    await pending
  })
})
