import type { DesktopMessage } from "./chat-contract.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"

export type LocalRuntimePersistenceOperation =
  | Readonly<{ kind: "upsert"; note: DesktopMessage }>
  | Readonly<{ kind: "remove"; key: string }>
  | Readonly<{ kind: "none" }>

const childKey = (turnRef: string, childRef: string): string => `${turnRef}-child-${childRef}`

const childRuntime = (
  notes: ReadonlyArray<DesktopMessage>,
  turnRef: string,
  childRef: string,
) => {
  const runtime = notes.find(note => note.key === childKey(turnRef, childRef))?.runtime
  return runtime?.kind === "child" ? runtime : null
}

const note = (
  key: string,
  text: string,
  timestamp: string,
  fields: Pick<DesktopMessage, "question" | "runtime">,
): LocalRuntimePersistenceOperation => ({
  kind: "upsert",
  note: { key, role: "system", text, timestamp, ...fields },
})

/**
 * Convert one trusted runtime event into the exact durable note mutation that
 * survives renderer reload and app restart. Text/tool/model events remain on
 * their existing append/segment path; this projector owns interactive and
 * structured runtime cards only.
 */
export const localRuntimePersistenceOperation = (input: Readonly<{
  turnRef: string
  event: FableLocalEvent
  notes: ReadonlyArray<DesktopMessage>
  timestamp: string
}>): LocalRuntimePersistenceOperation => {
  const { turnRef, event, notes, timestamp } = input
  if (event.kind === "question_pending") {
    return note(
      `${turnRef}-question-${event.questionRef}`,
      event.questions[0]?.question ?? "Question",
      timestamp,
      {
        question: {
          turnRef,
          questionRef: event.questionRef,
          status: "pending",
          questions: event.questions,
        },
      },
    )
  }
  if (event.kind === "question_resolved") {
    const existing = notes.find(item => item.question?.questionRef === event.questionRef)
    return existing?.question === undefined
      ? { kind: "none" }
      : { kind: "upsert", note: { ...existing, question: { ...existing.question, status: event.outcome } } }
  }
  if (event.kind === "plan_updated") {
    return note(`${turnRef}-plan`, "Plan updated", timestamp, {
      runtime: { kind: "plan", entries: event.entries.map(entry => ({ ...entry })) },
    })
  }
  if (event.kind === "child_started") {
    return note(childKey(turnRef, event.childRef), `Delegate child started · ${event.summary}`, timestamp, {
      runtime: {
        kind: "child",
        turnRef,
        childRef: event.childRef,
        ...(event.parentChildRef === undefined ? {} : { parentChildRef: event.parentChildRef }),
        status: "running",
        title: event.summary,
        detail: "",
        transcript: [{ role: "user", text: event.prompt ?? event.summary }],
        steered: null,
      },
    })
  }
  if (event.kind === "child_activity" || event.kind === "child_completed" || event.kind === "child_failed") {
    const existing = childRuntime(notes, turnRef, event.childRef)
    const parent = event.parentChildRef ?? existing?.parentChildRef
    const detail = event.kind === "child_failed"
      ? (event.detail.trim() === "" ? event.reason : `${event.reason} · ${event.detail}`)
      : event.summary
    const transcript = [
      ...(existing?.transcript ?? []),
      event.kind === "child_completed"
        ? { role: "assistant" as const, text: event.response ?? event.summary }
        : { role: "system" as const, text: detail },
    ].slice(-128)
    const status = event.kind === "child_completed" ? "completed" as const
      : event.kind === "child_failed" ? "failed" as const
        : existing?.status ?? "running" as const
    const label = event.kind === "child_completed" ? "Delegate child completed"
      : event.kind === "child_failed" ? "Delegate child failed"
        : "Delegate child"
    return note(childKey(turnRef, event.childRef), `${label} · ${detail}`, timestamp, {
      runtime: {
        kind: "child",
        turnRef,
        childRef: event.childRef,
        ...(parent === undefined ? {} : { parentChildRef: parent }),
        status,
        title: existing?.title ?? (event.kind === "child_completed" ? event.summary : "Codex child agent"),
        detail,
        transcript,
        steered: existing?.steered ?? null,
      },
    })
  }
  if (event.kind === "child_steered") {
    const existing = childRuntime(notes, turnRef, event.childRef)
    return existing === null ? { kind: "none" } : note(
      childKey(turnRef, event.childRef),
      `Delegate child steered · ${event.action} · ${event.outcome}`,
      timestamp,
      { runtime: { ...existing, steered: { action: event.action, outcome: event.outcome, detail: event.detail } } },
    )
  }
  if (event.kind === "followup_queued") {
    return note(`${turnRef}-queue-${event.queueRef}`, `Follow-up queued (#${event.position})`, timestamp, {
      runtime: { kind: "queue", turnRef, queueRef: event.queueRef, position: event.position },
    })
  }
  if (event.kind === "followup_promoted") {
    return { kind: "remove", key: `${turnRef}-queue-${event.queueRef}` }
  }
  return { kind: "none" }
}
