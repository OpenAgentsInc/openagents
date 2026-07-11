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
  emit: (envelope: FableLocalEventEnvelope) => void
  resolveStart: (value: unknown) => void
  unsubscribed: () => boolean
}

const makeHarness = (input?: { fableAvailable?: boolean; bridge?: boolean }): Harness => {
  const legacySends: Array<unknown> = []
  const startCalls: Array<unknown> = []
  let listener: ((envelope: FableLocalEventEnvelope) => void) | null = null
  let resolveStart: (value: unknown) => void = () => {}
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
      return new Promise(resolve => {
        resolveStart = resolve
      })
    },
    interrupt: async () => true,
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
    // trace line ("Fable · claude-fable-5") — model identity never comes from
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
      "system:Fable · claude-fable-5",
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

    harness.resolveStart({ ok: true, thread: finalThread })
    const result = await pending
    expect(result).toEqual({ ok: true, thread: finalThread })
    expect(harness.unsubscribed()).toBe(true)
    // LAW: the legacy gateway path was never touched by a harness send.
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
    expect(result).toEqual({ ok: false, error: "The local Fable lane returned an invalid response." })
  })
})
