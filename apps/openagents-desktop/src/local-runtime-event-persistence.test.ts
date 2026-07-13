import { describe, expect, test } from "bun:test"

import type { DesktopMessage } from "./chat-contract.ts"
import { localRuntimePersistenceOperation } from "./local-runtime-event-persistence.ts"

const timestamp = "10:00 AM"
const apply = (notes: DesktopMessage[], event: Parameters<typeof localRuntimePersistenceOperation>[0]["event"]): DesktopMessage[] => {
  const operation = localRuntimePersistenceOperation({ turnRef: "turn-1", event, notes, timestamp })
  if (operation.kind === "none") return notes
  if (operation.kind === "remove") return notes.filter(note => note.key !== operation.key)
  const index = notes.findIndex(note => note.key === operation.note.key)
  return index === -1
    ? [...notes, operation.note]
    : notes.map((note, noteIndex) => noteIndex === index ? operation.note : note)
}

describe("durable local runtime event projection", () => {
  test("retains nested child identity and independent transcript through completion", () => {
    let notes: DesktopMessage[] = []
    notes = apply(notes, {
      kind: "child_started",
      childRef: "child-2",
      parentChildRef: "child-1",
      accountRef: "account-1",
      summary: "Nested audit",
      prompt: "Inspect the nested boundary",
    })
    notes = apply(notes, {
      kind: "child_activity",
      childRef: "child-2",
      parentChildRef: "child-1",
      accountRef: "account-1",
      activity: "item",
      summary: "Reading invariant",
    })
    notes = apply(notes, {
      kind: "child_completed",
      childRef: "child-2",
      parentChildRef: "child-1",
      accountRef: "account-1",
      summary: "Audit complete",
      response: "The invariant holds.",
      usage: null,
      durationMs: 20,
    })

    expect(notes).toHaveLength(1)
    expect(notes[0]?.runtime).toEqual({
      kind: "child",
      turnRef: "turn-1",
      childRef: "child-2",
      parentChildRef: "child-1",
      status: "completed",
      title: "Nested audit",
      detail: "Audit complete",
      transcript: [
        { role: "user", text: "Inspect the nested boundary" },
        { role: "system", text: "Reading invariant" },
        { role: "assistant", text: "The invariant holds." },
      ],
      steered: null,
    })
  })

  test("persists question resolution, plan replacement, and queue removal", () => {
    let notes: DesktopMessage[] = []
    notes = apply(notes, {
      kind: "question_pending",
      questionRef: "q-1",
      questions: [{ question: "Proceed?", header: "Review", options: [{ label: "Yes" }], multiSelect: false }],
    })
    notes = apply(notes, { kind: "question_resolved", questionRef: "q-1", outcome: "answered" })
    notes = apply(notes, { kind: "plan_updated", entries: [{ step: "One", status: "pending" }] })
    notes = apply(notes, { kind: "plan_updated", entries: [{ step: "One", status: "completed" }] })
    notes = apply(notes, { kind: "followup_queued", queueRef: "queue-1", position: 1 })
    expect(notes.find(note => note.question)?.question?.status).toBe("answered")
    expect(notes.find(note => note.runtime?.kind === "plan")?.runtime).toEqual({
      kind: "plan",
      entries: [{ step: "One", status: "completed" }],
    })
    expect(notes.some(note => note.runtime?.kind === "queue")).toBe(true)
    notes = apply(notes, { kind: "followup_promoted", queueRef: "queue-1", message: "Next" })
    expect(notes.some(note => note.runtime?.kind === "queue")).toBe(false)
  })
})
