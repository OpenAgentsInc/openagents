// Import effect through the @effect-native/core/effect bridge so this module
// uses the SAME effect instance the vendored @effect-native/* packages pin,
// not the app's catalog effect. Mixing the two effect copies makes
// SubscriptionRef/Effect/Schema types fail to unify across the adapter
// boundary. (Same bridge the about-effect-native proof uses.)
import { Effect, Schema, type Stream, SubscriptionRef } from "@effect-native/core/effect"
import {
  Button,
  Card,
  defineIntent,
  type IntentHandlers,
  IntentRef,
  type IntentReporter,
  List,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core"

import type { KhalaMobileCreditsTransaction } from "../sync/khala-mobile-credits-api"
import { signedAmountLabel, transactionKindLabel } from "../sync/khala-mobile-credits-format-core"

/**
 * MB-EN (#8597) — the PURE half of the Credits History screen, re-authored with
 * the Effect Native component set (the first data-driven Khala-mobile screen
 * converted off the ported Ignite RN primitives). It imports only
 * `@effect-native/core` + `effect` + the pure credits formatters, never
 * `react`/`react-native`, so it renders deterministically through the RN
 * renderer in tests with no native host.
 *
 * The screen's DATA and NAV stay in the `.tsx` shell as services (auth, the
 * `khala-mobile-credits-api` client, React Navigation). The shell maps its load
 * state into `CreditsHistoryViewModel` and pushes it into this program's state
 * ref; the two typed intents (Back / Load more) flow out through the adapter to
 * imperative callbacks the shell owns. This is a presentation migration, not a
 * data-layer change: the endpoint contract and the honest
 * unavailable/error/empty degradation are unchanged.
 */

/** Presentation view-model the `.tsx` shell derives from its load state and
 * pushes into the program. Mirrors the shell's `LoadState`, minus imperative
 * concerns — the shell keeps owning fetch/pagination/nav. */
export type CreditsHistoryViewModel =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "error" }>
  | Readonly<{
      status: "ready"
      hasMore: boolean
      transactions: ReadonlyArray<KhalaMobileCreditsTransaction>
    }>

export const initialCreditsHistoryViewModel: CreditsHistoryViewModel = { status: "loading" }

/** Typed intents. They carry no payload; each maps to an imperative callback the
 * `.tsx` shell owns (navigation.goBack / loadPage), dispatched through the RN
 * adapter's `report` seam rather than an inline closure in the tree. */
export const CreditsHistoryBack = defineIntent(
  "CreditsHistoryBack",
  Schema.Struct({}),
)
export const CreditsHistoryLoadMore = defineIntent(
  "CreditsHistoryLoadMore",
  Schema.Struct({}),
)

export const creditsHistoryIntentDefinitions = [
  CreditsHistoryBack,
  CreditsHistoryLoadMore,
] as const

const formatOccurredAt = (iso: string): string => {
  const parsed = new Date(iso)
  return Number.isNaN(parsed.getTime()) ? iso : parsed.toLocaleString()
}

const headerView = (): View =>
  Stack(
    {
      key: "credits-history-header",
      direction: "row",
      align: "center",
      gap: "2",
      padding: "4",
      style: { width: "full", backgroundColor: "background" },
    },
    [
      Button({
        key: "credits-history-back",
        label: "Back",
        variant: "ghost",
        onPress: IntentRef("CreditsHistoryBack", StaticPayload({})),
      }),
      Text({
        key: "credits-history-title",
        content: "Credit history",
        variant: "heading",
        color: "textPrimary",
      }),
    ],
  )

const centeredMessage = (
  key: string,
  heading: string,
  body?: string,
): View =>
  Stack(
    {
      key,
      direction: "column",
      gap: "2",
      padding: "5",
      align: "center",
      justify: "center",
      style: { width: "full", flex: 1 },
    },
    [
      Text({
        key: `${key}-heading`,
        content: heading,
        variant: "title",
        color: "textPrimary",
      }),
      ...(body === undefined
        ? []
        : [
            Text({
              key: `${key}-body`,
              content: body,
              variant: "body",
              color: "textMuted",
            }),
          ]),
    ],
  )

const transactionRow = (transaction: KhalaMobileCreditsTransaction): View => {
  const meta = signedAmountLabel(transaction.kind, transaction.amountUsdCents)
  const title =
    transaction.description.trim().length > 0
      ? transaction.description
      : transactionKindLabel(transaction.kind)
  return Card(
    {
      key: `credits-history-row-${transaction.id}`,
      padding: "3",
      radius: "md",
      style: {
        width: "full",
        backgroundColor: "surface",
        borderColor: "border",
        borderWidth: 1,
      },
    },
    [
      Stack(
        {
          key: `credits-history-row-${transaction.id}-line`,
          direction: "row",
          align: "center",
          justify: "between",
          gap: "3",
          style: { width: "full" },
        },
        [
          Stack(
            {
              key: `credits-history-row-${transaction.id}-text`,
              direction: "column",
              gap: "0.5",
              style: { flex: 1 },
            },
            [
              Text({
                key: `credits-history-row-${transaction.id}-title`,
                content: title,
                variant: "body",
                color: "textPrimary",
                weight: "medium",
              }),
              Text({
                key: `credits-history-row-${transaction.id}-date`,
                content: formatOccurredAt(transaction.occurredAt),
                variant: "caption",
                color: "textMuted",
              }),
            ],
          ),
          Text({
            key: `credits-history-row-${transaction.id}-meta`,
            content: meta,
            variant: "label",
            color: transaction.kind === "charge" ? "danger" : "success",
          }),
        ],
      ),
    ],
  )
}

const readyBody = (
  transactions: ReadonlyArray<KhalaMobileCreditsTransaction>,
  hasMore: boolean,
): View => {
  if (transactions.length === 0) {
    return centeredMessage("credits-history-empty", "No transactions yet")
  }
  const rows = transactions.map(transactionRow)
  const footer = hasMore
    ? [
        Button({
          key: "credits-history-load-more",
          label: "Load more",
          variant: "secondary",
          onPress: IntentRef("CreditsHistoryLoadMore", StaticPayload({})),
        }),
      ]
    : []
  return Stack(
    {
      key: "credits-history-ready",
      direction: "column",
      gap: "2",
      padding: "4",
      style: { width: "full", flex: 1 },
    },
    [
      List(
        {
          key: "credits-history-list",
          // `renderList` always maps to FlatList (native windowing on device);
          // `virtualize` only toggles the getItemLayout fast-path, which needs a
          // fixed `estimatedItemSize`. Credit rows are variable height, so leave
          // it off — the list still virtualizes natively.
          virtualize: false,
          style: { width: "full", flex: 1 },
        },
        rows as ReadonlyArray<View & { readonly key: string }>,
      ),
      ...footer,
      Spacer({ key: "credits-history-footer-space", size: "4" }),
    ],
  )
}

/** The screen authored as ONE typed Effect Native view tree, styled with the
 * Protoss-blue theme tokens (resolved by the renderer against
 * `khalaEffectNativeTheme`). */
export const renderCreditsHistoryView = (state: CreditsHistoryViewModel): View => {
  const body: View =
    state.status === "loading"
      ? centeredMessage("credits-history-loading", "Loading history")
      : state.status === "unavailable"
        ? centeredMessage(
            "credits-history-unavailable",
            "History not yet available",
            "Transaction history isn't available yet — it's coming soon.",
          )
        : state.status === "error"
          ? centeredMessage(
              "credits-history-error",
              "History unavailable",
              "Could not load your credit history right now.",
            )
          : readyBody(state.transactions, state.hasMore)
  return Stack(
    {
      key: "credits-history-root",
      direction: "column",
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    [headerView(), body],
  )
}

/** Imperative callbacks the `.tsx` shell owns; the typed intents dispatch into
 * them through the adapter. Held in a stable object the shell mutates each
 * render so handlers always see the current closures. */
export interface CreditsHistoryCallbacks {
  onBack: () => void
  onLoadMore: () => void
}

export const makeCreditsHistoryHandlers = (
  callbacks: CreditsHistoryCallbacks,
): IntentHandlers<typeof creditsHistoryIntentDefinitions> => ({
  CreditsHistoryBack: () => Effect.sync(() => callbacks.onBack()),
  CreditsHistoryLoadMore: () => Effect.sync(() => callbacks.onLoadMore()),
})

export interface CreditsHistoryProgramHandle {
  readonly viewStream: Stream.Stream<View>
  readonly report: IntentReporter
  readonly setViewModel: (next: CreditsHistoryViewModel) => void
}

/** Builds the runnable program: a `SubscriptionRef` of the view-model, an intent
 * registry bound to the shell's callbacks, a closure-captured `IntentReporter`
 * (R = never), and a `setViewModel` push so the React shell can drive the EN
 * surface from its load state. Runs synchronously — `makeIntentRegistry` /
 * `SubscriptionRef.make` need no Scope. */
export const buildCreditsHistoryProgram = (
  callbacks: CreditsHistoryCallbacks,
): CreditsHistoryProgramHandle =>
  Effect.runSync(
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<CreditsHistoryViewModel>(
        initialCreditsHistoryViewModel,
      )
      const registry = yield* makeIntentRegistry(
        creditsHistoryIntentDefinitions,
        makeCreditsHistoryHandlers(callbacks),
      )
      const report: IntentReporter = (ref, runtimeValue) =>
        registry.dispatch(resolveIntentRef(ref, runtimeValue))
      const program = makeViewProgramFromState(state, renderCreditsHistoryView)
      const setViewModel = (next: CreditsHistoryViewModel): void =>
        Effect.runSync(SubscriptionRef.set(state, next))
      return { viewStream: program.viewStream, report, setViewModel }
    }),
  )
