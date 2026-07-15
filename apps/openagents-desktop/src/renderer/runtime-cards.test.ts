/**
 * Runtime-capability transcript cards (EP250 wave-2, #8712).
 *
 * The renderer half of the wave-1 substrate: plan/todo progress (J2/J4), the
 * steer/stop-a-running-child affordance (G4), and the queued-follow-up chip
 * (A3). Covers the pure glyph/model vocabulary (runtime-cards.ts), the shell
 * render trees (plan card + in-place update, child Interrupt dispatch +
 * child_steered outcome + message-action-absent, queue chip), and the
 * local-harness event projection that drives them.
 */
import { describe, expect, test } from "vite-plus/test"

import type { DesktopThread } from "../chat-contract.ts"
import type { FableLocalEventEnvelope } from "../fable-local-contract.ts"
import {
  childInterruptable,
  childSteerLine,
  childStatusChip,
  planProgressSummary,
  planStatusGlyph,
  runtimeCardFromNote,
} from "./runtime-cards.ts"
import {
  childCardMessage,
  planCardMessage,
  queueChipMessage,
  runtimeCardMessage,
  type DesktopNoteEntry,
} from "./shell.ts"
import { makeLocalHarnessChatHost, type FableLocalRendererBridge } from "./local-harness.ts"

// ---------------------------------------------------------------------------
// View-tree inspection helpers.
// ---------------------------------------------------------------------------
type AnyNode = Readonly<Record<string, unknown>> & { key?: string; _tag?: string }

const collect = (node: unknown, out: Array<AnyNode> = []): Array<AnyNode> => {
  if (Array.isArray(node)) {
    for (const item of node) collect(item, out)
    return out
  }
  if (typeof node !== "object" || node === null) return out
  const record = node as AnyNode
  if (typeof record._tag === "string") out.push(record)
  for (const value of Object.values(record)) collect(value, out)
  return out
}
const byKey = (root: unknown, key: string): AnyNode | undefined =>
  collect(root).find((node) => node.key === key)

const note = (input: Partial<DesktopNoteEntry> & Pick<DesktopNoteEntry, "key" | "runtime">): DesktopNoteEntry => ({
  role: "system",
  text: "",
  timestamp: "18:05",
  ...input,
} as DesktopNoteEntry)

// ---------------------------------------------------------------------------
// Pure glyph / model vocabulary.
// ---------------------------------------------------------------------------
describe("plan status glyphs (exact frozen enum)", () => {
  test("each status maps to its glyph, color token, and emphasis", () => {
    expect(planStatusGlyph("pending")).toMatchObject({ icon: "Circle", color: "textFaint", active: false })
    expect(planStatusGlyph("in_progress")).toMatchObject({ icon: "Play", color: "accent", active: true })
    expect(planStatusGlyph("completed")).toMatchObject({ icon: "Check", color: "success", active: false })
  })

  test("progress summary counts done and in-progress honestly", () => {
    expect(planProgressSummary([
      { step: "a", status: "completed" },
      { step: "b", status: "in_progress" },
      { step: "c", status: "pending" },
    ])).toBe("1 of 3 done · 1 in progress")
    expect(planProgressSummary([{ step: "a", status: "completed" }])).toBe("1 of 1 done")
  })
})

describe("child model helpers", () => {
  test("only a running child with no interrupt yet is interruptable", () => {
    const base = { kind: "child" as const, turnRef: "t", childRef: "c", title: "x", detail: "", steered: null }
    expect(childInterruptable({ ...base, status: "running" })).toBe(true)
    expect(childInterruptable({ ...base, status: "completed" })).toBe(false)
    expect(childInterruptable({ ...base, status: "failed" })).toBe(false)
    expect(childInterruptable({
      ...base,
      status: "running",
      steered: { action: "interrupt", outcome: "interrupted", detail: "" },
    })).toBe(false)
  })

  test("child status chip and steer line read honestly", () => {
    expect(childStatusChip("running")).toEqual({ label: "Running", tone: "neutral" })
    expect(childStatusChip("completed")).toEqual({ label: "Done", tone: "success" })
    expect(childStatusChip("failed")).toEqual({ label: "Failed", tone: "danger" })
    expect(childSteerLine(null)).toBeNull()
    expect(childSteerLine({ action: "interrupt", outcome: "interrupted", detail: "" })).toBe("Interrupt · interrupted")
    expect(childSteerLine({ action: "message", outcome: "unsupported", detail: "" })).toBe("Message · not supported")
  })
})

// ---------------------------------------------------------------------------
// Plan card render + in-place update.
// ---------------------------------------------------------------------------
describe("plan card render (J2/J4)", () => {
  test("renders one row per todo with the status glyph and progress; no SYSTEM label", () => {
    const runtime = {
      kind: "plan" as const,
      entries: [
        { step: "Read the audit", status: "completed" as const },
        { step: "Build the card", status: "in_progress" as const },
        { step: "Write tests", status: "pending" as const },
      ],
    }
    const message = planCardMessage(note({ key: "n1", runtime }), runtime)
    expect(message.role).toBe("tool") // never a SYSTEM note
    expect(byKey(message.body, "plan-progress-n1")).toMatchObject({ content: "1 of 3 done · 1 in progress" })
    // Status glyphs, in enum order.
    expect(byKey(message.body, "plan-step-icon-n1-0")).toMatchObject({ name: "Check", color: "success" })
    expect(byKey(message.body, "plan-step-icon-n1-1")).toMatchObject({ name: "Play", color: "accent" })
    expect(byKey(message.body, "plan-step-icon-n1-2")).toMatchObject({ name: "Circle", color: "textFaint" })
    // The in-progress row is emphasized (weight medium); others regular.
    expect(byKey(message.body, "plan-step-text-n1-1")).toMatchObject({ content: "Build the card", weight: "medium" })
    expect(byKey(message.body, "plan-step-text-n1-2")).toMatchObject({ weight: "regular" })
  })
})

// ---------------------------------------------------------------------------
// Child steer affordance (G4).
// ---------------------------------------------------------------------------
describe("child card render (G4)", () => {
  test("a running child offers Interrupt that dispatches the exact ref; NO message action", () => {
    const runtime = {
      kind: "child" as const,
      turnRef: "turn.fable.7",
      childRef: "child.9",
      status: "running" as const,
      title: "Summarize the task",
      detail: "reading files",
      steered: null,
    }
    const message = childCardMessage(note({ key: "c1", runtime }), runtime)
    const open = byKey(message.body, "child-open-c1") as {
      label?: string
      onPress?: { name?: string; payload?: unknown }
    }
    expect(open?.label).toBe("Summarize the task")
    expect(open?.onPress?.name).toBe("DesktopAgentAction")
    expect(JSON.stringify(open?.onPress)).toContain("agent.local.turn.fable.7.child.child.9")
    const interrupt = byKey(message.body, "child-interrupt-c1") as {
      label?: string
      onPress?: { name?: string; payload?: unknown }
    }
    expect(interrupt?.label).toBe("Interrupt")
    expect(interrupt?.onPress?.name).toBe("DesktopChildInterruptRequested")
    const payload = JSON.stringify(interrupt?.onPress)
    expect(payload).toContain("turn.fable.7")
    expect(payload).toContain("child.9")
    // Capability-truthful: messaging an in-flight child is NOT offered.
    const labels = collect(message.body).map((n) => n["label"]).filter((l) => typeof l === "string")
    expect(labels).not.toContain("Message")
    expect(byKey(message.body, "child-message-c1")).toBeUndefined()
  })

  test("a completed / interrupted child no longer offers Interrupt and shows the steer outcome", () => {
    const completed = { kind: "child" as const, turnRef: "t", childRef: "c", status: "completed" as const, title: "x", detail: "done", steered: null }
    expect(byKey(childCardMessage(note({ key: "c2", runtime: completed }), completed).body, "child-interrupt-c2")).toBeUndefined()

    const steered = {
      kind: "child" as const,
      turnRef: "t",
      childRef: "c",
      status: "running" as const,
      title: "x",
      detail: "",
      steered: { action: "interrupt" as const, outcome: "interrupted" as const, detail: "child interrupt requested" },
    }
    const message = childCardMessage(note({ key: "c3", runtime: steered }), steered)
    expect(byKey(message.body, "child-interrupt-c3")).toBeUndefined()
    expect(byKey(message.body, "child-steer-c3")).toMatchObject({ content: "Interrupt · interrupted" })
  })
})

// ---------------------------------------------------------------------------
// Queue chip (A3).
// ---------------------------------------------------------------------------
describe("queue chip render (A3)", () => {
  test("renders a Queued follow-up (#N) chip with honest delivery copy", () => {
    const runtime = { kind: "queue" as const, turnRef: "t", queueRef: "q1", position: 2 }
    const message = queueChipMessage(note({ key: "u1", runtime }), runtime)
    expect(byKey(message.body, "queue-badge-u1")).toMatchObject({ label: "Queued follow-up (#2)", tone: "info" })
    expect(byKey(message.body, "queue-note-u1")).toMatchObject({ content: "delivered when this turn completes" })
  })

  test("runtimeCardMessage dispatches on the note payload kind", () => {
    expect(runtimeCardMessage(note({ key: "p", runtime: { kind: "plan", entries: [] } })).key).toBe("p")
    expect(runtimeCardMessage(note({ key: "u", runtime: { kind: "queue", turnRef: "t", queueRef: "q", position: 1 } })).key).toBe("u")
  })
})

// ---------------------------------------------------------------------------
// Local-harness event projection: events -> notes carrying runtime payloads.
// ---------------------------------------------------------------------------
const thread: DesktopThread = {
  id: "thread-1",
  title: "New chat",
  updatedAt: "2026-07-11T10:00:00.000Z",
  notes: [{ key: "user-1", role: "user", text: "go", timestamp: "10:00" }],
}
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

type Harness = {
  updates: DesktopThread[]
  emit: (event: FableLocalEventEnvelope["event"]) => void
  finish: () => Promise<void>
}
const runHarness = async (): Promise<Harness> => {
  let listener: ((envelope: FableLocalEventEnvelope) => void) | null = null
  let resolveStart: (value: unknown) => void = () => {}
  let starts = 0
  const bridge: FableLocalRendererBridge = {
    availability: async () => ({ state: "available", accountRef: "claude-pylon-b" }),
    // The first turn is driven manually; a CHAINED promoted-follow-up turn (A3)
    // resolves immediately so the harness `finish()` never hangs.
    start: async () => {
      starts += 1
      return starts === 1
        ? new Promise((resolve) => { resolveStart = resolve })
        : { ok: true, thread }
    },
    interrupt: async () => true,
    onEvent: (cb) => { listener = cb; return () => {} },
  }
  const host = makeLocalHarnessChatHost({
    base: {
      listThreads: async () => [thread],
      newThread: async () => thread,
      openThread: async () => thread,
      sendMessage: async () => ({ ok: true, thread }),
    },
    fable: bridge,
    fableAvailability: () => ({ state: "available", accountRef: "claude-pylon-b" }),
    randomId: () => "fixed",
    scheduleProjection: flush => {
      let active = true
      queueMicrotask(() => { if (active) flush() })
      return () => { active = false }
    },
  })
  const updates: DesktopThread[] = []
  // A closure so `listener` keeps its declared type (a straight-line call would
  // have CFA narrow the `= null` init to never).
  const emit = (event: FableLocalEventEnvelope["event"]): void => {
    listener?.({ turnRef: "turn.fable.fixed", event })
  }
  const pending = host.sendMessage({ id: "thread-1", message: "go", harness: "fable", onUpdate: (t) => updates.push(t) })
  await settle()
  emit({ kind: "turn_started", thread })
  await settle()
  return {
    updates,
    emit,
    finish: async () => { resolveStart({ ok: true, thread }); await pending },
  }
}

describe("local-harness runtime projection", () => {
  test("plan_updated projects ONE plan card and updates it in place (latest wins)", async () => {
    const h = await runHarness()
    h.emit({ kind: "plan_updated", entries: [{ step: "a", status: "in_progress" }] })
    await settle()
    h.emit({ kind: "plan_updated", entries: [{ step: "a", status: "completed" }, { step: "b", status: "in_progress" }] })
    await settle()
    const last = h.updates.at(-1)!
    const planNotes = last.notes.filter((n) => n.runtime?.kind === "plan")
    expect(planNotes).toHaveLength(1) // in place, not appended
    expect(runtimeCardFromNote(planNotes[0]!)).toMatchObject({
      kind: "plan",
      entries: [{ step: "a", status: "completed" }, { step: "b", status: "in_progress" }],
    })
    await h.finish()
  })

  test("child lifecycle + steer merge onto one childRef card", async () => {
    const h = await runHarness()
    h.emit({ kind: "child_started", childRef: "c1", summary: "Summarize", accountRef: "codex" })
    await settle()
    h.emit({ kind: "child_steered", childRef: "c1", action: "interrupt", outcome: "interrupted", detail: "aborted" })
    await settle()
    const last = h.updates.at(-1)!
    const childNotes = last.notes.filter((n) => n.runtime?.kind === "child")
    expect(childNotes).toHaveLength(1)
    expect(runtimeCardFromNote(childNotes[0]!)).toMatchObject({
      kind: "child",
      childRef: "c1",
      title: "Summarize",
      steered: { action: "interrupt", outcome: "interrupted" },
    })
    await h.finish()
  })

  test("followup_queued renders a chip that is cleared on followup_promoted", async () => {
    const h = await runHarness()
    h.emit({ kind: "followup_queued", queueRef: "q1", position: 1 })
    await settle()
    expect(h.updates.at(-1)!.notes.filter((n) => n.runtime?.kind === "queue")).toHaveLength(1)
    h.emit({ kind: "followup_promoted", queueRef: "q1", message: "next thing" })
    await settle()
    expect(h.updates.at(-1)!.notes.filter((n) => n.runtime?.kind === "queue")).toHaveLength(0)
    await h.finish()
  })
})
