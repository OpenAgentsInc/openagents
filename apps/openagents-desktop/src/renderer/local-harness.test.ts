/**
 * Local-mode harness routing (#8712): the renderer half of the
 * no-silent-substitution law. A "fable" send streams through the fableLocal
 * bridge; a "codex" send is an explicit typed refusal; NEITHER ever reaches
 * the base host's legacy gateway sendMessage. Only a laneless send does.
 */
import { describe, expect, test } from "bun:test"

import type { DesktopThread } from "../chat-contract.ts"
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
  steerCalls: Array<unknown>
  queueCalls: Array<unknown>
  emit: (envelope: FableLocalEventEnvelope) => void
  resolveStart: (value: unknown) => void
  unsubscribed: () => boolean
}

const makeHarness = (input?: { fableAvailable?: boolean; bridge?: boolean }): Harness => {
  const legacySends: Array<unknown> = []
  const startCalls: Array<unknown> = []
  const steerCalls: Array<unknown> = []
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
    interrupt: async () => true,
    steerChild: async value => {
      steerCalls.push(value)
      return { ok: true, outcome: "interrupted" }
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
  })
  return {
    host,
    legacySends,
    startCalls,
    steerCalls,
    queueCalls,
    emit: envelope => listener?.(envelope),
    resolveStart: value => resolveStart(value),
    unsubscribed: () => unsubscribed,
  }
}

describe("makeLocalHarnessChatHost", () => {
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
    // A foreign turn's events never touch this stream.
    harness.emit({ turnRef: "turn.fable.other", event: { kind: "text_delta", text: "IGNORED" } })
    await settle()

    const last = updates.at(-1)!
    const bodies = last.notes.map(note => `${note.role}:${note.text}`)
    // Trace lines precede the growing assistant bubble — the same order the
    // finalized persisted thread carries.
    expect(bodies).toEqual([
      "user:hello fable",
      "system:Claude · claude-fable-5",
      "system:Read · started · notes.md",
      "system:Read · ok",
      "assistant:Hello world",
    ])
    // Progressive: an earlier snapshot carried the partial assistant text.
    expect(updates.some(update =>
      update.notes.some(note => note.role === "assistant" && note.text === "Hello "))).toBe(true)
    // Streaming metadata (#8712 message inspector): the growing assistant
    // note already carries lane/turn/effective-model facts.
    const streamedAssistant = last.notes.find(note => note.role === "assistant")
    expect(streamedAssistant?.meta).toEqual({
      lane: "fable-local",
      turnRef: "turn.fable.fixed",
      model: "claude-fable-5",
    })
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
    expect(result).toEqual({ ok: false, error: codexLocalUnavailableMessage })
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
    expect(result).toEqual({ ok: false, error: "The local Claude lane returned an invalid response." })
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
})
