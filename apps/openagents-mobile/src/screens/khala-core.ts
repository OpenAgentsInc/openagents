import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  Composer,
  ComponentValueBinding,
  IntentRef,
  Stack,
  Spacer,
  StaticPayload,
  Text,
  Transcript,
  type TranscriptMessage,
  type View,
} from "@effect-native/core"

/**
 * The public Khala mode: one conversation over the public orchestration
 * endpoint. It deliberately has no Sarah relationship, FleetRun, account, or
 * backing-model claim. The server owns routing and returns an honest failure.
 */
export type KhalaRole = "user" | "assistant"

export interface KhalaEntry {
  readonly key: string
  readonly role: KhalaRole
  readonly text: string
  readonly status: "thinking" | "done" | "failed"
}

export interface KhalaState {
  readonly draft: string
  readonly entries: ReadonlyArray<KhalaEntry>
  readonly pending: boolean
  readonly turnCounter: number
}

export const initialKhalaState: KhalaState = {
  draft: "",
  entries: [],
  pending: false,
  turnCounter: 0,
}

export interface KhalaTurnClient {
  readonly sendTurn: (input: {
    readonly messages: ReadonlyArray<{ readonly role: KhalaRole; readonly content: string }>
  }) => Promise<{ readonly reply: string }>
}

export const KhalaDraftChanged = "KhalaDraftChanged"
export const KhalaTurnSubmitted = "KhalaTurnSubmitted"
export const KHALA_TURN_FAILED_TEXT =
  "Khala could not respond just now. Check your connection and send that again."

const boundedText = (value: string): string =>
  value.length > 4_000 ? `${value.slice(0, 4_000)}…` : value

const boundedEntries = (entries: ReadonlyArray<KhalaEntry>): ReadonlyArray<KhalaEntry> =>
  entries.length > 200 ? entries.slice(-200) : entries

const updateKhala = <State extends { readonly khala: KhalaState }>(
  state: SubscriptionRef.SubscriptionRef<State>,
  update: (khala: KhalaState) => KhalaState,
) =>
  SubscriptionRef.update(state, (current) => ({ ...current, khala: update(current.khala) }))

export const khalaHandlers = <State extends { readonly khala: KhalaState }>(
  state: SubscriptionRef.SubscriptionRef<State>,
  client: KhalaTurnClient | undefined,
) => ({
  [KhalaDraftChanged]: (text: string) =>
    updateKhala(state, (khala) => ({ ...khala, draft: boundedText(text) })),
  [KhalaTurnSubmitted]: (raw: string) =>
    Effect.gen(function* () {
      const message = raw.trim()
      if (message === "") return
      const before = yield* SubscriptionRef.get(state)
      if (before.khala.pending) return
      const turn = before.khala.turnCounter + 1
      const userKey = `khala-${turn}-user`
      const replyKey = `khala-${turn}-reply`
      const history = before.khala.entries
        .filter((entry) => entry.status === "done")
        .map((entry) => ({ role: entry.role, content: entry.text }))
      yield* updateKhala(state, (khala) => ({
        ...khala,
        draft: "",
        pending: true,
        turnCounter: turn,
        entries: boundedEntries([
          ...khala.entries,
          { key: userKey, role: "user", text: boundedText(message), status: "done" },
          { key: replyKey, role: "assistant", text: "", status: "thinking" },
        ]),
      }))
      const result =
        client === undefined
          ? null
          : yield* Effect.tryPromise({
              try: () => client.sendTurn({ messages: [...history, { role: "user", content: message }] }),
              catch: () => new Error("khala_turn_failed"),
            }).pipe(Effect.catch(() => Effect.succeed(null)))
      yield* updateKhala(state, (khala) => ({
        ...khala,
        pending: false,
        entries: khala.entries.map((entry) =>
          entry.key !== replyKey
            ? entry
            : result === null
              ? { ...entry, text: KHALA_TURN_FAILED_TEXT, status: "failed" as const }
              : { ...entry, text: boundedText(result.reply), status: "done" as const },
        ),
      }))
    }),
})

export const renderKhalaSurface = (state: KhalaState): View =>
  Stack(
    {
      key: "khala-surface",
      direction: "column",
      gap: "3",
      padding: "4",
      style: { width: "full", height: "full" },
    },
    [
      Spacer({ key: "khala-top-space", size: "16" }),
      Text({
        key: "khala-title",
        content: "Khala",
        variant: "title",
        color: "textPrimary",
      }),
      Text({
        key: "khala-subtitle",
        content: "One conversation, routed by the OpenAgents orchestrator.",
        variant: "body",
        color: "textMuted",
      }),
      Transcript({
        key: "khala-transcript",
        messages: state.entries.map((entry): TranscriptMessage => ({
          key: entry.key,
          role: entry.role,
          status: entry.status === "thinking" ? "thinking" : "done",
          body: [
            Text({
              key: `${entry.key}-text`,
              content: entry.status === "thinking" ? "Khala is thinking…" : entry.text,
              variant: "body",
              color: entry.status === "failed" ? "danger" : "textPrimary",
            }),
          ],
        })),
        pinToEnd: true,
        style: { width: "full", flex: 1 },
      }),
      Stack({ key: "khala-composer-row", direction: "row", gap: "2", style: { width: "full" } }, [
        Composer({
          key: "khala-composer",
          mode: "normal",
          doc: [{ kind: "text", text: state.draft }],
          placeholder: state.pending ? "Khala is replying…" : "Ask Khala",
          onChange: IntentRef(KhalaDraftChanged, ComponentValueBinding()),
          onSubmit: IntentRef(KhalaTurnSubmitted, ComponentValueBinding()),
          style: { surface: "glass", borderRadius: "lg", padding: "3", minHeight: 48, flex: 1 },
        }),
        Button({
          key: "khala-send",
          label: state.pending ? "…" : "Send",
          variant: "primary",
          onPress: IntentRef(KhalaTurnSubmitted, StaticPayload(state.draft)),
          style: { minHeight: 48 },
        }),
      ]),
      Spacer({ key: "khala-bottom-space", size: "8" }),
    ],
  )

export const khalaIntentDefinitions = [
  { name: KhalaDraftChanged, payload: Schema.String },
  { name: KhalaTurnSubmitted, payload: Schema.String },
] as const
