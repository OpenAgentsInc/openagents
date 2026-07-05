import type { KhalaRuntimeEvent, KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import { reduceRuntimeTranscript } from "./khala-runtime-transcript-core"

/**
 * Pure logic for issue #8407 — cross-agent delegation ("Ask Claude/Codex to
 * review this"): starting a NEW turn on the OTHER lane whose prompt carries a
 * bounded summary of a just-completed turn. Named `khala-cross-agent-handoff`
 * (not `delegation`) to avoid colliding with the pre-existing, unrelated
 * `src/security/delegation-prompt.ts` — that module validates a user-typed
 * prompt for the "Khala -> Pylon -> Codex" own-capacity coding-delegation
 * runbook (a different feature: delegating a coding task to the user's own
 * connected Pylon fleet). This module is about handing a chat TURN off
 * between the two in-thread lanes (Codex <-> Claude), a distinct concept.
 *
 * No native/RN imports, so this stays unit-testable exactly like its sibling
 * `khala-runtime-compose-core.ts` / `khala-runtime-transcript-core.ts`.
 */

/** The two lanes a user can pick from the composer (#8405) are the only two
 * this issue's handoff button ever targets — every other `KhalaRuntimeLane`
 * literal (ai_sdk_core, khala_sync_mobile_control, test_fixture, …) is an
 * internal routing lane a turn never visibly "belongs to" from a user's
 * perspective, so handoff is simply unavailable for it (`undefined`) rather
 * than inventing a target no one asked for. */
export const handoffTargetLane = (lane: KhalaRuntimeLane): KhalaRuntimeLane | undefined => {
  if (lane === "codex_app_server") return "claude_pylon"
  if (lane === "claude_pylon") return "codex_app_server"
  return undefined
}

const HANDOFF_LANE_LABEL: Record<"codex_app_server" | "claude_pylon", string> = {
  claude_pylon: "Claude",
  codex_app_server: "Codex"
}

/** Same friendly-name convention as `transcript-part-row.tsx`'s `laneLabel`
 * (kept as a small intentional duplicate rather than a cross-import from a
 * component file into this pure module — see that file's own comment on why
 * only the two user-pickable lanes get a friendly name). */
export const handoffLaneLabel = (lane: KhalaRuntimeLane): string =>
  lane === "codex_app_server" || lane === "claude_pylon" ? HANDOFF_LANE_LABEL[lane] : lane

const MAX_SUMMARY_CHARS = 6000
const TRUNCATION_SUFFIX = "\n\n… [summary truncated]"

/** Bounded, human-readable summary of one completed turn's parts — built by
 * re-running the turn's OWN (already turnId-filtered, sequence-sorted) event
 * list through `reduceRuntimeTranscript`, the exact same fold the live
 * transcript UI uses. A handoff summary is not a new way of reading events;
 * it's a different rendering of the same folded parts, so it can never drift
 * from what the reviewing side would have seen scrolling the original turn.
 *
 * Deliberately excludes raw tool input/output refs — `resultRef`/`errorRef`
 * on `tool.result`/`tool.error` events are private blob pointers, not inline
 * content, so only `toolName` + settled status (+ the already-public-safe
 * `messageSafe` on a failure) make it into the summary, matching exactly what
 * `TranscriptPartRow` itself renders for a tool part. This keeps the handoff
 * prompt a bounded SUMMARY of the turn, per #8407's explicit design
 * requirement, never a replay of the full raw event stream. */
export const summarizeTurnEventsForHandoff = (events: ReadonlyArray<KhalaRuntimeEvent>): string => {
  const parts = reduceRuntimeTranscript(events)
  const lines: Array<string> = []
  for (const part of parts) {
    if (part.kind === "text" && part.text.trim().length > 0) {
      lines.push(part.text.trim())
    } else if (part.kind === "reasoning" && part.text.trim().length > 0) {
      lines.push(`(reasoning) ${part.text.trim()}`)
    } else if (part.kind === "tool") {
      const detail =
        part.status === "failed" && part.errorMessageSafe !== undefined ? ` — ${part.errorMessageSafe}` : ""
      lines.push(`- tool: ${part.toolName} (${part.status})${detail}`)
    }
  }
  const joined = lines.length > 0 ? lines.join("\n\n") : "(no text response; turn completed with no readable output)"
  return joined.length > MAX_SUMMARY_CHARS
    ? joined.slice(0, MAX_SUMMARY_CHARS - TRUNCATION_SUFFIX.length) + TRUNCATION_SUFFIX
    : joined
}

/** Builds the handoff turn's user-facing prompt body — persisted through the
 * existing `chat.appendMessage` mutator under a fresh messageId, then
 * referenced by the new turn's `bodyRef` via `chatMessageBodyRef`, reusing
 * the exact same convention `runtime.startTurn` already establishes for the
 * composer's own turn-starting flow (see `khala-runtime-compose-core.ts`).
 * No new schema, no new mutator — a handoff turn looks like any other
 * turn.start to the dispatch consumer. */
export const buildHandoffPromptBody = (input: {
  sourceLane: KhalaRuntimeLane
  targetLane: KhalaRuntimeLane
  summary: string
}): string => {
  const sourceLabel = handoffLaneLabel(input.sourceLane)
  const targetLabel = handoffLaneLabel(input.targetLane)
  return (
    `${targetLabel}, please review the following turn ${sourceLabel} just completed in this thread ` +
    "and give your assessment (correctness, risks, anything you'd change):\n\n" +
    `---\n${input.summary}\n---`
  )
}
