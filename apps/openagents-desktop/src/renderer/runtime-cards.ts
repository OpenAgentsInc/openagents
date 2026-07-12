/**
 * Runtime-capability transcript cards (EP250 wave-2, #8712).
 *
 * The wave-1 substrate (fable-local-contract.ts / fable-local-runtime.ts)
 * emits typed control events on the fable-local stream — `plan_updated`
 * (J2/J4 task progress), `child_started/activity/completed/failed` +
 * `child_steered` (G4 steer/stop a running delegate child), and
 * `followup_queued/promoted` (A3 queue-until-idle follow-ups). This module is
 * the PURE renderer half: local-harness.ts projects those events onto the
 * streaming transcript as system notes carrying a typed `runtime` payload
 * (the same note-payload precedent the interactive question cards use), and
 * this module turns each payload into a card MODEL plus its glyph/label
 * vocabulary. The View render lives in shell.ts (like the tool-card renders),
 * so this file stays free of styles and testable without a DOM.
 *
 * Presentation vocabulary (icons/labels) follows the tool-card family
 * (`./tool-cards.ts`) — the in-repo precedent for "custom components that
 * showed things properly, not JSON blobs".
 */
import type { IconName } from "@effect-native/core"
import type { ColorToken } from "@effect-native/tokens"

import type { DesktopRuntimeCard } from "../chat-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"

// ---------------------------------------------------------------------------
// Typed note payloads live in chat-contract.ts (schema-validated, like the
// question-card payload). This module is the pure MODEL/glyph half: the note
// carries one `DesktopRuntimeCard`, projected the same way tool/question cards
// are — one place they become cards is `projectToolCardEntries`.
// ---------------------------------------------------------------------------

export type RuntimeCardPayload = DesktopRuntimeCard
export type RuntimePlanCardPayload = Extract<DesktopRuntimeCard, { kind: "plan" }>
export type RuntimeChildCardPayload = Extract<DesktopRuntimeCard, { kind: "child" }>
export type RuntimeChildTranscript = NonNullable<RuntimeChildCardPayload["transcript"]>
export type RuntimeQueueChipPayload = Extract<DesktopRuntimeCard, { kind: "queue" }>
export type RuntimePlanEntry = RuntimePlanCardPayload["entries"][number]
export type RuntimePlanStatus = RuntimePlanEntry["status"]
export type RuntimeChildStatus = RuntimeChildCardPayload["status"]
export type RuntimeChildSteer = NonNullable<RuntimeChildCardPayload["steered"]>

/** The typed runtime payload on a note, or null for an ordinary note. */
export const runtimeCardFromNote = (note: DesktopNoteEntry): RuntimeCardPayload | null =>
  note.runtime ?? null

// ---------------------------------------------------------------------------
// Plan card model + glyph vocabulary.
// ---------------------------------------------------------------------------

export type PlanStatusGlyph = Readonly<{
  icon: IconName
  /** Color TOKEN (never a raw literal) — resolved by the View. */
  color: ColorToken
  /** Accessible/status label for the row. */
  label: string
  /** The in-progress row is subtly emphasized (its title carries weight). */
  active: boolean
}>

/**
 * The glyph + status color for one plan entry. `in_progress` is the emphasized
 * state (accent + active), `completed` reads as success, `pending` sits at the
 * faint level of the dim ladder.
 */
export const planStatusGlyph = (status: RuntimePlanStatus): PlanStatusGlyph => {
  switch (status) {
    case "completed":
      return { icon: "Check", color: "success", label: "Completed", active: false }
    case "in_progress":
      return { icon: "Play", color: "accent", label: "In progress", active: true }
    case "pending":
      return { icon: "Circle", color: "textFaint", label: "Pending", active: false }
  }
}

/** "2 of 5 done" (with a live-step suffix when one is in progress). */
export const planProgressSummary = (entries: ReadonlyArray<RuntimePlanEntry>): string => {
  const total = entries.length
  const done = entries.filter((entry) => entry.status === "completed").length
  const active = entries.filter((entry) => entry.status === "in_progress").length
  const base = `${done} of ${total} done`
  return active > 0 ? `${base} · ${active} in progress` : base
}

// ---------------------------------------------------------------------------
// Child card model.
// ---------------------------------------------------------------------------

export type ChildStatusChip = Readonly<{ label: string; tone: "neutral" | "success" | "danger" }>

export const childStatusChip = (status: RuntimeChildStatus): ChildStatusChip => {
  switch (status) {
    case "running":
      return { label: "Running", tone: "neutral" }
    case "completed":
      return { label: "Done", tone: "success" }
    case "failed":
      return { label: "Failed", tone: "danger" }
  }
}

/**
 * A running child (no terminal, no steer yet) can be interrupted. Once a
 * terminal status lands or an interrupt was already signaled, the control is
 * gone — a control that cannot act is never offered.
 */
export const childInterruptable = (child: RuntimeChildCardPayload): boolean =>
  child.status === "running" &&
  !(child.steered !== null && child.steered.action === "interrupt")

/** Compact human line for a steer outcome (G4), or null when none yet. */
export const childSteerLine = (steer: RuntimeChildSteer | null): string | null => {
  if (steer === null) return null
  const action = steer.action === "interrupt" ? "Interrupt" : "Message"
  const outcome = steer.outcome === "interrupted"
    ? "interrupted"
    : steer.outcome === "delivered"
      ? "delivered"
      : steer.outcome === "unsupported"
        ? "not supported"
        : "no matching child"
  return `${action} · ${outcome}`
}
