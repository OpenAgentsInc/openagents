/**
 * Fleet overview workspace (#8712): a read-only projection of the connected
 * provider accounts over the shared catalog. Pure Effect Native data — state,
 * typed intents, and a `state -> View` projection; the host bridge responses
 * are schema-decoded here (mirroring ../provider-accounts-contract.ts, same
 * both-sides pattern as ./settings.ts) and never trusted raw.
 *
 * Evidence discipline (Arbiter rule): a status dot renders lit ONLY when the
 * account readiness is "ready" AND the projection decoded successfully this
 * session. "credentials-missing" is unlit; "unknown" or a failed fetch is an
 * explicit "evidence unavailable" — never an optimistic lit dot. This view
 * mutates no accounts; management stays in Settings.
 */
import {
  Badge,
  Button,
  Icon,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Table,
  Text,
  defineIntent,
  type View,
} from "@effect-native/core"
import { Effect, Exit, Schema, SubscriptionRef } from "@effect-native/core/effect"

export type FleetAccountReadiness = "ready" | "credentials-missing" | "unknown"

export type FleetAccount = Readonly<{
  ref: string
  provider: string
  email: string | null
  readiness: FleetAccountReadiness
}>

export type FleetUsageEntry =
  | Readonly<{ state: "checking" }>
  | Readonly<{
      state: "checked"
      refreshedAt: string
      inputTokens: number | null
      outputTokens: number | null
      totalTokens: number | null
    }>
  | Readonly<{ state: "failed"; reason: string }>

export type FleetWorkspaceState = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable"
  /** ISO timestamp of the last successfully decoded accounts projection. */
  generatedAt: string | null
  accounts: ReadonlyArray<FleetAccount>
  usage: Readonly<Record<string, FleetUsageEntry>>
  reason: string | null
}>

export const emptyFleetWorkspaceState = (): FleetWorkspaceState => ({
  phase: "idle",
  generatedAt: null,
  accounts: [],
  usage: {},
  reason: null,
})

// ---------------------------------------------------------------------------
// Renderer-side bridge decoding (Effect Schema; mirrors the main-process
// contract in ../provider-accounts-contract.ts — the provider-accounts test
// asserts both sides agree).
// ---------------------------------------------------------------------------

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

const RendererFleetAccountsSchema = Schema.Union([
  Schema.Struct({
    ok: Schema.Literal(true),
    generatedAt: Schema.String,
    accounts: Schema.Array(Schema.Struct({
      ref: Schema.String,
      provider: Schema.String,
      email: Schema.NullOr(Schema.String),
      readiness: Schema.Literals(["ready", "credentials-missing", "unknown"]),
    })),
  }),
  Schema.Struct({ ok: Schema.Literal(false), reason: Schema.String }),
])

const RendererFleetUsageSchema = Schema.Union([
  Schema.Struct({
    ok: Schema.Literal(true),
    ref: Schema.String,
    refreshedAt: Schema.String,
    summary: Schema.Struct({
      inputTokens: Schema.NullOr(Schema.Number),
      outputTokens: Schema.NullOr(Schema.Number),
      totalTokens: Schema.NullOr(Schema.Number),
    }),
  }),
  Schema.Struct({ ok: Schema.Literal(false), ref: Schema.String, reason: Schema.String }),
])

export type FleetAccountsProjection =
  | Readonly<{ ok: true; generatedAt: string; accounts: ReadonlyArray<FleetAccount> }>
  | Readonly<{ ok: false; reason: string }>

export const decodeFleetAccountsProjection = (value: unknown): FleetAccountsProjection => {
  const decoded = Schema.decodeUnknownExit(RendererFleetAccountsSchema)(value)
  if (!Exit.isSuccess(decoded)) return { ok: false, reason: "invalid_bridge_payload" }
  if (!decoded.value.ok) return { ok: false, reason: decoded.value.reason.slice(0, 120) }
  return {
    ok: true,
    generatedAt: decoded.value.generatedAt.slice(0, 40),
    accounts: decoded.value.accounts
      .filter((account) => accountRefPattern.test(account.ref))
      .map((account) => ({
        ref: account.ref,
        provider: account.provider.slice(0, 40),
        email: account.email === null || account.email.length > 120 ? null : account.email,
        readiness: account.readiness,
      })),
  }
}

export const decodeFleetUsageEntry = (value: unknown, ref: string): FleetUsageEntry => {
  const decoded = Schema.decodeUnknownExit(RendererFleetUsageSchema)(value)
  if (!Exit.isSuccess(decoded) || decoded.value.ref !== ref) {
    return { state: "failed", reason: "invalid_bridge_payload" }
  }
  if (!decoded.value.ok) return { state: "failed", reason: decoded.value.reason.slice(0, 120) }
  return {
    state: "checked",
    refreshedAt: decoded.value.refreshedAt.slice(0, 40),
    inputTokens: decoded.value.summary.inputTokens,
    outputTokens: decoded.value.summary.outputTokens,
    totalTokens: decoded.value.summary.totalTokens,
  }
}

// ---------------------------------------------------------------------------
// Pure transitions
// ---------------------------------------------------------------------------

export const withFleetLoading = (fleet: FleetWorkspaceState): FleetWorkspaceState => ({
  ...fleet,
  phase: "loading",
  reason: null,
})

export const withFleetProjection = (
  fleet: FleetWorkspaceState,
  projection: FleetAccountsProjection,
): FleetWorkspaceState =>
  projection.ok
    ? {
        phase: "ready",
        generatedAt: projection.generatedAt,
        accounts: projection.accounts,
        // Usage evidence belongs to the projection it was checked against.
        usage: {},
        reason: null,
      }
    : { ...fleet, phase: "unavailable", reason: projection.reason }

export const withFleetUsageChecking = (
  fleet: FleetWorkspaceState,
  ref: string,
): FleetWorkspaceState => ({
  ...fleet,
  usage: { ...fleet.usage, [ref]: { state: "checking" } },
})

export const withFleetUsageEntry = (
  fleet: FleetWorkspaceState,
  ref: string,
  entry: FleetUsageEntry,
): FleetWorkspaceState => ({
  ...fleet,
  usage: { ...fleet.usage, [ref]: entry },
})

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

export const FleetRefreshRequested = defineIntent("FleetRefreshRequested", Schema.Null)
export const FleetUsageCheckRequested = defineIntent("FleetUsageCheckRequested", Schema.String)
export const FleetManageAccountsRequested = defineIntent(
  "FleetManageAccountsRequested",
  Schema.Null,
)

export const fleetWorkspaceIntents = [
  FleetRefreshRequested,
  FleetUsageCheckRequested,
  FleetManageAccountsRequested,
] as const

// ---------------------------------------------------------------------------
// Handlers (generic in the shell state shape, same pattern as ./settings.ts;
// shell.ts stays the single owner of its state type).
// ---------------------------------------------------------------------------

export type FleetAccountsBridge = Readonly<{
  list: () => Promise<unknown>
  usage: (ref: string) => Promise<unknown>
}>

export const unavailableFleetAccountsBridge: FleetAccountsBridge = {
  list: async () => ({ ok: false, reason: "pylon_runtime_unavailable" }),
  usage: async (ref) => ({ ok: false, ref, reason: "pylon_runtime_unavailable" }),
}

export type FleetCapableState = Readonly<{ fleet: FleetWorkspaceState }>

export const refreshFleetAccounts = <S extends FleetCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: FleetAccountsBridge,
) =>
  Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.fleet.phase === "loading") return
    yield* SubscriptionRef.update(state, (next) => ({
      ...next,
      fleet: withFleetLoading(next.fleet),
    }))
    const projection = decodeFleetAccountsProjection(
      yield* Effect.promise(() => bridge.list().catch(() => null)),
    )
    yield* SubscriptionRef.update(state, (next) => ({
      ...next,
      fleet: withFleetProjection(next.fleet, projection),
    }))
  })

export const makeFleetWorkspaceHandlers = <S extends FleetCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: FleetAccountsBridge = unavailableFleetAccountsBridge,
  manageAccounts?: () => Effect.Effect<void>,
) => ({
  FleetRefreshRequested: () => refreshFleetAccounts(state, bridge),
  FleetUsageCheckRequested: (ref: string) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      if (!current.fleet.accounts.some((account) => account.ref === ref)) return
      if (current.fleet.usage[ref]?.state === "checking") return
      yield* SubscriptionRef.update(state, (next) => ({
        ...next,
        fleet: withFleetUsageChecking(next.fleet, ref),
      }))
      const entry = decodeFleetUsageEntry(
        yield* Effect.promise(() => bridge.usage(ref).catch(() => null)),
        ref,
      )
      yield* SubscriptionRef.update(state, (next) => ({
        ...next,
        fleet: withFleetUsageEntry(next.fleet, ref, entry),
      }))
    }),
  FleetManageAccountsRequested: () => manageAccounts?.() ?? Effect.void,
})

// ---------------------------------------------------------------------------
// View — pure `state -> View` over the shared catalog.
// ---------------------------------------------------------------------------

const readinessTone = (readiness: FleetAccountReadiness): "success" | "warn" | "neutral" =>
  readiness === "ready" ? "success" : readiness === "credentials-missing" ? "warn" : "neutral"

/** Lit ONLY on decoded ready evidence; everything else is explicitly not lit. */
export const fleetDotEvidence = (
  fleet: FleetWorkspaceState,
  account: FleetAccount,
): "lit" | "unlit" | "evidence-unavailable" =>
  fleet.phase !== "ready" || account.readiness === "unknown"
    ? "evidence-unavailable"
    : account.readiness === "ready"
      ? "lit"
      : "unlit"

const fleetStatusDot = (fleet: FleetWorkspaceState, account: FleetAccount): View => {
  const evidence = fleetDotEvidence(fleet, account)
  return Stack(
    { key: `fleet-dot-${account.ref}`, direction: "row", gap: "1", align: "center" },
    [
      Icon({
        key: `fleet-dot-icon-${account.ref}`,
        name: "Circle",
        size: "sm",
        color: evidence === "lit" ? "success" : "textMuted",
        label: `${account.ref} ${evidence === "lit" ? "ready" : evidence === "unlit" ? "credentials missing" : "evidence unavailable"}`,
      }),
      Text({
        key: `fleet-dot-label-${account.ref}`,
        content: `${account.ref} · ${account.provider}`,
        variant: "caption",
        color: "textMuted",
      }),
      ...(evidence === "evidence-unavailable"
        ? [Text({
            key: `fleet-dot-evidence-${account.ref}`,
            content: "evidence unavailable",
            variant: "caption",
            color: "warning",
          })]
        : []),
    ],
  )
}

const usageCell = (fleet: FleetWorkspaceState, account: FleetAccount): View => {
  const entry = fleet.usage[account.ref]
  if (entry?.state === "checking") {
    return Text({
      key: `fleet-usage-${account.ref}`,
      content: "Checking…",
      variant: "caption",
      color: "textMuted",
    })
  }
  const checkButton = Button({
    key: `fleet-usage-check-${account.ref}`,
    label: "Check",
    variant: "ghost",
    onPress: IntentRef("FleetUsageCheckRequested", StaticPayload(account.ref)),
    a11y: { label: `Check usage for account ${account.ref}` },
  })
  if (entry === undefined) {
    return Stack(
      { key: `fleet-usage-${account.ref}`, direction: "row", gap: "1", align: "center" },
      [
        Text({ key: `fleet-usage-empty-${account.ref}`, content: "—", variant: "caption", color: "textMuted" }),
        checkButton,
      ],
    )
  }
  if (entry.state === "failed") {
    return Stack(
      { key: `fleet-usage-${account.ref}`, direction: "row", gap: "1", align: "center" },
      [
        Text({
          key: `fleet-usage-failed-${account.ref}`,
          content: `evidence unavailable · ${entry.reason}`,
          variant: "caption",
          color: "warning",
        }),
        checkButton,
      ],
    )
  }
  return Stack(
    { key: `fleet-usage-${account.ref}`, direction: "row", gap: "1", align: "center" },
    [
      Text({
        key: `fleet-usage-total-${account.ref}`,
        content: entry.totalTokens === null
          ? "no usage recorded"
          : `${entry.totalTokens.toLocaleString("en-US")} tokens`,
        variant: "caption",
        color: "textPrimary",
      }),
      checkButton,
    ],
  )
}

const fleetAccountsTable = (fleet: FleetWorkspaceState): View =>
  Table({
    key: "fleet-accounts-table",
    columns: [
      { id: "provider", header: "Provider" },
      { id: "account", header: "Account" },
      { id: "email", header: "Email" },
      { id: "readiness", header: "Readiness" },
      { id: "usage", header: "Usage" },
    ],
    rows: fleet.accounts.map((account) => ({
      id: account.ref,
      cells: [
        Text({ key: `fleet-provider-${account.ref}`, content: account.provider, variant: "caption", color: "textMuted" }),
        Text({ key: `fleet-ref-${account.ref}`, content: account.ref, variant: "body", color: "textPrimary" }),
        Text({ key: `fleet-email-${account.ref}`, content: account.email ?? "—", variant: "caption", color: "textMuted" }),
        Badge({
          key: `fleet-readiness-${account.ref}`,
          label: account.readiness,
          tone: readinessTone(account.readiness),
          a11y: { label: `Account ${account.ref} readiness: ${account.readiness}` },
        }),
        usageCell(fleet, account),
      ],
    })),
    style: { width: "full" },
  })

const fleetBody = (fleet: FleetWorkspaceState): ReadonlyArray<View> => {
  if (fleet.phase === "loading" || fleet.phase === "idle") {
    return [Text({
      key: "fleet-loading",
      content: "Loading provider accounts…",
      variant: "body",
      color: "textMuted",
    })]
  }
  if (fleet.phase === "unavailable") {
    return [
      Text({
        key: "fleet-unavailable",
        content: "Fleet accounts are unavailable. No account evidence was read.",
        variant: "body",
        color: "warning",
      }),
      ...(fleet.reason === null ? [] : [Text({
        key: "fleet-unavailable-reason",
        content: fleet.reason,
        variant: "caption",
        color: "textMuted",
      })]),
    ]
  }
  if (fleet.accounts.length === 0) {
    return [Text({
      key: "fleet-empty",
      content: "No provider accounts connected yet. Connect one in Settings.",
      variant: "body",
      color: "textMuted",
    })]
  }
  return [
    Stack(
      { key: "fleet-status-dots", direction: "row", gap: "3", align: "center", style: { width: "full" } },
      fleet.accounts.map((account) => fleetStatusDot(fleet, account)),
    ),
    fleetAccountsTable(fleet),
  ]
}

export const fleetWorkspaceView = (fleet: FleetWorkspaceState): View =>
  Stack(
    {
      key: "workspace-fleet-panel",
      direction: "column",
      gap: "3",
      style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 },
    },
    [
      Stack({ key: "fleet-heading", direction: "row", gap: "2", align: "center" }, [
        Text({ key: "fleet-title", content: "Fleet", variant: "heading", color: "textPrimary" }),
        Text({
          key: "fleet-as-of",
          content: fleet.generatedAt === null
            ? "no snapshot yet"
            : `as of ${fleet.generatedAt.slice(0, 16).replace("T", " ")}`,
          variant: "caption",
          color: "textMuted",
        }),
        Spacer({ key: "fleet-heading-fill", flex: true }),
        Button({
          key: "fleet-refresh",
          label: fleet.phase === "loading" ? "Refreshing…" : "Refresh",
          variant: "secondary",
          disabled: fleet.phase === "loading",
          onPress: IntentRef("FleetRefreshRequested"),
          a11y: { label: "Refresh the fleet accounts projection" },
        }),
      ]),
      ...fleetBody(fleet),
      Stack({ key: "fleet-footer", direction: "row", gap: "2", align: "center" }, [
        Button({
          key: "fleet-manage-accounts",
          label: "Manage accounts in Settings",
          variant: "ghost",
          onPress: IntentRef("DesktopSettingsToggled"),
          a11y: { label: "Manage provider accounts in Settings" },
        }),
        Button({
          key: "fleet-new-chat",
          label: "New chat",
          variant: "ghost",
          onPress: IntentRef("DesktopNewChat"),
          a11y: { label: "Start a new chat" },
        }),
      ]),
    ],
  )
