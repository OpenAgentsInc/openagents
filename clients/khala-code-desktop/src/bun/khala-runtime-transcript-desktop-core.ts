import type {
  KhalaRuntimeEvent,
  RuntimeEventEntity,
  RuntimeTurnEntity,
  RuntimeTurnStatus,
} from "@openagentsinc/khala-sync"

/**
 * Desktop's runtime_event/runtime_turn -> assistant-reply fold (#8425 render
 * gap closeout).
 *
 * Context: a turn dispatched from mobile (or any khala_sync_mobile_control /
 * runtime-intent-supervisor path) never produces a `chat_message` row for the
 * assistant's reply — only the human prompt lands as `chat_message`; the
 * reply streams as `runtime_event` (`text.delta` etc.) plus a `runtime_turn`
 * status row into `scope.thread.<threadId>` (see
 * `packages/khala-sync/src/runtime.ts`). Mobile already reduces this into a
 * rich transcript (`khala-runtime-transcript-core.ts` in khala-mobile);
 * desktop's chat surface had no equivalent read path at all
 * (`clients/khala-code-desktop/src/bun/khala-sync-service.ts` only ever read
 * `chat_message`).
 *
 * Desktop's message list model is simpler than mobile's part-by-part
 * transcript (`KhalaCodeDesktopMessage` is just `{ id, role, body }`), so
 * this module deliberately does NOT port the full `TranscriptPart` shape.
 * Instead it folds one `runtime_turn` + its `runtime_event` rows into ONE
 * synthesized assistant message per turn: the concatenated `text.delta` reply
 * text, with a short honest status suffix appended for any turn that hasn't
 * settled cleanly (queued/running/waiting_for_input/failed/interrupted) so
 * the existing message-list UI never renders a stale turn as if it were a
 * normal completed reply. This is an intentionally scoped-down port — see
 * `docs/khala-code/2026-07-04-mobile-tailnet-handshake.md` for the full
 * mobile-vs-desktop rendering architecture writeup.
 */

export type DesktopRuntimeTurnMessage = Readonly<{
  turnId: string
  /** ISO timestamp used to interleave this synthesized message with real
   * `chat_message` rows by chronological order (see
   * `mergeChatAndRuntimeMessages`). */
  sortKey: string
  role: "assistant"
  body: string
}>

export const sortRuntimeEventEntitiesBySequence = (
  entities: ReadonlyArray<RuntimeEventEntity>,
): ReadonlyArray<RuntimeEventEntity> => [...entities].sort((a, b) => a.sequence - b.sequence)

/** Concatenates every `text.delta` body in sequence order into one reply
 * string. Other event kinds (reasoning, tool calls, usage, turn-status) are
 * intentionally not surfaced in desktop's single-message-per-turn model —
 * mobile's richer part-by-part transcript remains the reference
 * implementation for a future desktop UI that wants that fidelity. */
export const foldRuntimeTurnReplyText = (
  events: ReadonlyArray<KhalaRuntimeEvent>,
): string => {
  let text = ""
  for (const event of events) {
    if (event.kind === "text.delta") text += event.text
  }
  return text
}

/** `null` means "no suffix" (a cleanly completed/closed turn speaks for
 * itself); every other status gets a short honest marker so a still-running,
 * queued, or non-clean turn is never mistaken for a finished reply. */
const RUNTIME_TURN_STATUS_SUFFIX: Readonly<Record<RuntimeTurnStatus, string | null>> = {
  queued: " (queued…)",
  running: " (running…)",
  waiting_for_input: " (waiting for input…)",
  completed: null,
  failed: " (failed)",
  interrupted: " (interrupted)",
  closed: null,
}

export const runtimeTurnMessageBody = (
  turnText: string,
  status: RuntimeTurnStatus,
): string => {
  const suffix = RUNTIME_TURN_STATUS_SUFFIX[status]
  if (suffix === null) return turnText
  return turnText.length === 0 ? suffix.trim() : `${turnText}${suffix}`
}

/** Turn ids are UUIDv7 (time-ordered) same as mobile's
 * `khala-runtime-compose-core.ts`, but `startedAt`/`createdAt` are the
 * server-truth ISO timestamps already used to sort `chat_message` rows
 * (`compareChatMessagesForTranscript`), so reusing them here keeps the two
 * entity kinds comparable on one timeline. */
export const runtimeTurnSortKey = (turn: RuntimeTurnEntity): string =>
  turn.startedAt ?? turn.createdAt

/** Folds `runtime_turn` + `runtime_event` rows for one thread scope into one
 * synthesized assistant message per turn. A turn is only dropped when it has
 * truly nothing to show: a settled turn (`completed`/`closed`) with an empty
 * folded reply (e.g. a turn that only ran tool calls, with no user-facing
 * text — desktop's single-message-per-turn model has no tool-call rendering
 * today). A queued/running/waiting_for_input/failed/interrupted turn always
 * keeps its status-suffix placeholder even with zero events, so an
 * in-flight or non-clean turn is never silently invisible. */
export const buildDesktopRuntimeTurnMessages = (
  turns: ReadonlyArray<RuntimeTurnEntity>,
  events: ReadonlyArray<RuntimeEventEntity>,
): ReadonlyArray<DesktopRuntimeTurnMessage> => {
  const eventsByTurn = new Map<string, Array<RuntimeEventEntity>>()
  for (const event of events) {
    const list = eventsByTurn.get(event.turnId)
    if (list === undefined) eventsByTurn.set(event.turnId, [event])
    else list.push(event)
  }

  const messages: Array<DesktopRuntimeTurnMessage> = []
  for (const turn of turns) {
    const turnEvents = sortRuntimeEventEntitiesBySequence(eventsByTurn.get(turn.turnId) ?? [])
    const text = foldRuntimeTurnReplyText(turnEvents.map(entity => entity.event))
    const body = runtimeTurnMessageBody(text, turn.status)
    if (body.length === 0) continue
    messages.push({
      body,
      role: "assistant",
      sortKey: runtimeTurnSortKey(turn),
      turnId: turn.turnId,
    })
  }

  return messages.sort((a, b) => a.sortKey.localeCompare(b.sortKey) || a.turnId.localeCompare(b.turnId))
}
