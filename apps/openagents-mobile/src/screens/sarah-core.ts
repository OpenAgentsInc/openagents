// effect via the @effect-native/core bridge — same instance the vendored
// packages pin (see home-core.ts for why).
import { Schema } from "@effect-native/core/effect"
import {
  Button,
  Card,
  Composer,
  ComponentValueBinding,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  Transcript,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"

/**
 * OpenAgents mobile GL-3 (#8649) — the PURE half of the Sarah conversation
 * surface: typed state, intent payload schemas, and the EN view projection
 * that renders INSIDE the GL-2 glass shell when the pill dropdown selects
 * "Sarah". This module imports only `@effect-native/core` (+ its effect
 * bridge) — never react/react-native/expo. The effectful client (prospect
 * session mint/persist, `/sarah/api/eve/turn`, bounded SSE with reconnect)
 * lives in `../sarah/sarah-client.ts`; the HOST wires it to the one Home
 * program through typed intents only.
 *
 * Grammar parity with the web surface (apps/sarah/src/ui/main.ts):
 * - text turns are request/response POSTs to `/sarah/api/eve/turn` with the
 *   transcript appended from the reply (the web `sendTextTurn` shape);
 * - the SSE bus (`/sarah/api/avatar/events?ref=`) carries typed
 *   transcript/card/session events. VERIFIED against production wiring: the
 *   owned TEXT turn path does not publish to that bus today — it feeds the
 *   avatar/brain tiers — so v1 binds the stream for liveness + typed cards
 *   and renders turn replies from the POST result, exactly like web.
 * - no parallel state models: one HomeState slice, typed intents only.
 */

export type SarahSessionPhase = "idle" | "minting" | "ready" | "unavailable"

/** Bounded SSE connection lifecycle (typed reconnect proof surface). */
export type SarahStreamPhase =
  | "idle"
  | "connecting"
  | "live"
  | "reconnecting"
  | "unavailable"

export type SarahRole = "user" | "assistant"

export type SarahEntryStatus = "thinking" | "done" | "failed"

export interface SarahEntry {
  readonly key: string
  readonly role: SarahRole
  readonly text: string
  readonly status: SarahEntryStatus
}

export interface SarahCardModel {
  readonly key: string
  readonly title: string
  readonly body: string
}

export interface SarahState {
  readonly phase: SarahSessionPhase
  readonly prospectRef: string | null
  readonly threadId: string | null
  /** True when the session ref was restored from disk (relationship survives
   * restarts — GL-3 acceptance). */
  readonly restored: boolean
  readonly stream: SarahStreamPhase
  readonly entries: ReadonlyArray<SarahEntry>
  readonly cards: ReadonlyArray<SarahCardModel>
  readonly draft: string
  readonly turnPending: boolean
  readonly turnCounter: number
  readonly eventCounter: number
  readonly lastFailure: string | null
}

export const initialSarahState: SarahState = {
  phase: "idle",
  prospectRef: null,
  threadId: null,
  restored: false,
  stream: "idle",
  entries: [],
  cards: [],
  draft: "",
  turnPending: false,
  turnCounter: 0,
  eventCounter: 0,
  lastFailure: null,
}

/** Bounded history: the transcript state never grows unbounded. */
export const MAX_SARAH_ENTRIES = 200
export const MAX_SARAH_CARDS = 20
/** Bounded text: any single entry/card body is clipped (no unbounded SSE or
 * reply payload enters state). */
export const MAX_SARAH_TEXT = 4000

export const clipSarahText = (text: string): string =>
  text.length > MAX_SARAH_TEXT ? `${text.slice(0, MAX_SARAH_TEXT)}…` : text

export const boundedEntries = (
  entries: ReadonlyArray<SarahEntry>,
): ReadonlyArray<SarahEntry> =>
  entries.length > MAX_SARAH_ENTRIES
    ? entries.slice(entries.length - MAX_SARAH_ENTRIES)
    : entries

// ---------------------------------------------------------------------------
// Intent payload schemas (registered in home-core's ONE registry)
// ---------------------------------------------------------------------------

export const SarahPersistedEntrySchema = Schema.Struct({
  key: Schema.NonEmptyString,
  role: Schema.Literals(["user", "assistant"]),
  text: Schema.String,
})

export const SarahSessionReadyPayload = Schema.Struct({
  prospectRef: Schema.NonEmptyString,
  threadId: Schema.NonEmptyString,
  restored: Schema.Boolean,
  entries: Schema.Array(SarahPersistedEntrySchema),
})

export const SarahSessionUnavailablePayload = Schema.Struct({
  reason: Schema.String,
})

export const SarahStreamStatusPayload = Schema.Struct({
  phase: Schema.Literals(["idle", "connecting", "live", "reconnecting", "unavailable"]),
})

/** Typed SSE event (avatar-event-bus contract, public-safe fields only). */
export const SarahEventPayload = Schema.Struct({
  type: Schema.String,
  role: Schema.String.pipe(Schema.optionalKey),
  text: Schema.String.pipe(Schema.optionalKey),
  title: Schema.String.pipe(Schema.optionalKey),
  body: Schema.String.pipe(Schema.optionalKey),
})

// ---------------------------------------------------------------------------
// Turn client seam (injected into the program; tests use a deterministic fake)
// ---------------------------------------------------------------------------

export interface SarahTurnResult {
  readonly ok: boolean
  readonly reply: string
  readonly modelPath: string | null
  readonly threadId: string | null
}

export interface SarahTurnClient {
  readonly sendTurn: (input: {
    readonly message: string
    readonly prospectRef: string | null
    readonly threadId: string | null
  }) => Promise<SarahTurnResult>
}

/** Honest degradation copy (typed failed entry — never a dead composer). */
export const SARAH_TURN_FAILED_TEXT =
  "I couldn't reach Sarah — check your connection and send that again."

export const SARAH_UNAVAILABLE_TITLE = "Sarah is unreachable"
export const SARAH_UNAVAILABLE_BODY =
  "The conversation service isn't reachable right now. Your message box stays open — sending will retry against the live service."

/** Adopt the prospect relationship from a turn result when the session was
 * bootstrapped by the turn itself (server mints on first contact and returns
 * threadId "prospect:<ref>"). */
export const prospectRefFromThreadId = (threadId: string | null): string | null =>
  threadId !== null && threadId.startsWith("prospect:") && threadId.length > "prospect:".length
    ? threadId.slice("prospect:".length)
    : null

/** Restored transcripts must never collide keys with new turns: the turn
 * counter resumes past the highest persisted `turn-<n>-…` key (a collision
 * would make a new reply overwrite a restored bubble). */
export const turnCounterFromEntries = (
  entries: ReadonlyArray<{ readonly key: string }>,
): number =>
  entries.reduce((max, entry) => {
    const match = /^turn-(\d+)-/.exec(entry.key)
    if (match === null) return max
    const value = Number.parseInt(match[1]!, 10)
    return Number.isFinite(value) && value > max ? value : max
  }, 0)

/** SSE transcript dedupe: the POST reply path already appended this text. */
export const isDuplicateTranscriptEvent = (
  entries: ReadonlyArray<SarahEntry>,
  role: SarahRole,
  text: string,
): boolean =>
  entries
    .slice(-8)
    .some((entry) => entry.role === role && entry.text === text)

/** Parse SSE frames out of an accumulating buffer; returns the remainder.
 * Comment frames (`: connected`, `: hb`) are liveness only. Pure (tested in
 * the bun sweep); the streaming loop that feeds it lives in
 * ../sarah/sarah-client. */
export const drainSseBuffer = (
  buffer: string,
  emit: (data: string) => void,
): string => {
  let rest = buffer
  for (;;) {
    const boundary = rest.indexOf("\n\n")
    if (boundary === -1) return rest
    const frame = rest.slice(0, boundary)
    rest = rest.slice(boundary + 2)
    for (const line of frame.split("\n")) {
      if (line.startsWith("data: ")) emit(line.slice("data: ".length))
      else if (line.startsWith("data:")) emit(line.slice("data:".length))
    }
  }
}

// ---------------------------------------------------------------------------
// View projection
// ---------------------------------------------------------------------------

const streamLabel: Record<SarahStreamPhase, string> = {
  idle: "connecting",
  connecting: "connecting",
  live: "live",
  reconnecting: "reconnecting",
  unavailable: "offline",
}

const entryBubble = (entry: SarahEntry): View =>
  Stack(
    {
      key: `${entry.key}-bubble`,
      direction: "column",
      padding: "3",
      style: {
        surface: "glass",
        borderRadius: "lg",
        marginBottom: "2",
        ...(entry.role === "user"
          ? { marginLeft: "8" }
          : { marginRight: "8" }),
      },
    },
    [
      Text({
        key: `${entry.key}-who`,
        content: entry.role === "user" ? "You" : "Sarah",
        variant: "caption",
        color: entry.role === "user" ? "textMuted" : "accent",
      }),
      Text({
        key: `${entry.key}-text`,
        content:
          entry.status === "thinking" ? "Sarah is thinking…" : entry.text,
        variant: "body",
        color: entry.status === "failed" ? "danger" : "textPrimary",
      }),
    ],
  )

const transcriptMessages = (
  entries: ReadonlyArray<SarahEntry>,
): ReadonlyArray<TranscriptMessage> =>
  entries.map((entry) => ({
    key: entry.key,
    role: entry.role,
    status: entry.status === "thinking" ? ("thinking" as const) : ("done" as const),
    body: [entryBubble(entry)],
  }))

const sarahCard = (card: SarahCardModel): View =>
  Card(
    {
      key: card.key,
      style: { surface: "glass", marginBottom: "2" },
    },
    [
      Text({ key: `${card.key}-title`, content: card.title, variant: "label", color: "accent" }),
      Text({ key: `${card.key}-body`, content: card.body, variant: "caption", color: "textPrimary" }),
    ],
  )

/**
 * The Sarah conversation column, rendered by home-core's content projection
 * when `surfaceMode === "sarah"`. Transparent root: the muted Sarah demo loop
 * (AMBIENT ONLY — presentation, never conversation evidence) plays beneath;
 * bubbles/cards/composer sit on honest glass surfaces.
 */
export const renderSarahSurface = (state: SarahState): View =>
  Stack(
    {
      key: "sarah-root",
      direction: "column",
      padding: "4",
      style: { width: "full", height: "full" },
    },
    [
      // Clear the floating glass chrome's top row (host overlay at ~52pt).
      Spacer({ key: "sarah-top-space", size: "16" }),
      Stack(
        { key: "sarah-status-row", direction: "row", gap: "2" },
        [
          Text({
            key: "sarah-status",
            content: `Sarah — ${streamLabel[state.stream]}${state.restored ? " · continued" : ""}`,
            variant: "caption",
            color: state.stream === "unavailable" ? "danger" : "textMuted",
          }),
        ],
      ),
      ...(state.phase === "unavailable"
        ? [
            Card(
              { key: "sarah-unavailable", style: { surface: "glass", marginTop: "2", marginBottom: "2" } },
              [
                Text({
                  key: "sarah-unavailable-title",
                  content: SARAH_UNAVAILABLE_TITLE,
                  variant: "label",
                  color: "danger",
                }),
                Text({
                  key: "sarah-unavailable-body",
                  content: SARAH_UNAVAILABLE_BODY,
                  variant: "caption",
                  color: "textPrimary",
                }),
              ],
            ),
          ]
        : []),
      ...state.cards.map(sarahCard),
      Transcript({
        key: "sarah-transcript",
        messages: transcriptMessages(state.entries),
        pinToEnd: true,
        style: { flex: 1, width: "full" },
      }),
      Stack(
        { key: "sarah-composer-row", direction: "row", gap: "2", style: { width: "full" } },
        [
          Composer({
            key: "sarah-composer",
            mode: "normal",
            doc: [{ kind: "text", text: state.draft }],
            placeholder: state.turnPending ? "Sarah is replying…" : "Message Sarah",
            onChange: IntentRef("SarahDraftChanged", ComponentValueBinding()),
            onSubmit: IntentRef("SarahTurnSubmitted", ComponentValueBinding()),
            style: {
              surface: "glass",
              borderRadius: "lg",
              padding: "3",
              minHeight: 48,
              flex: 1,
            },
          }),
          // RN multiline inputs do not fire onSubmitEditing on iOS, so the
          // typed Send affordance carries the CURRENT draft (the tree
          // re-renders on every SarahDraftChanged) through the same
          // SarahTurnSubmitted intent the keyboard path uses.
          Button({
            key: "sarah-send",
            label: state.turnPending ? "…" : "Send",
            variant: "primary",
            onPress: IntentRef("SarahTurnSubmitted", StaticPayload(state.draft)),
            style: { minHeight: 48 },
          }),
        ],
      ),
      Spacer({ key: "sarah-bottom-space", size: "8" }),
    ],
  )
